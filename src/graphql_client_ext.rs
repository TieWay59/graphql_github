use graphql_client::GraphQLQuery;
use log::warn;
use std::io::Write;
use std::{thread, time::Duration};

pub trait Window {
    fn get_window(&self) -> i64;
    fn set_window(&mut self, window: i64);
}

/// 重新定义 graphql_client::reqwest::post_graphql_blocking
/// 主要增加了一个观察者闭包函数，观察内部的 header。
pub fn post_graphql_blocking<Q: GraphQLQuery, U: reqwest::IntoUrl + Clone>(
    client: &reqwest::blocking::Client,
    url: U,
    variables: Q::Variables,
    // 目前只是一个粗略的实现，由于源库年久失修，这个
    mut f: impl FnMut(&reqwest::header::HeaderMap) -> anyhow::Result<()>,
) -> Result<graphql_client::Response<Q::ResponseData>, reqwest::Error>
where
    Q::Variables: Window,
{
    let mut body = Q::build_query(variables);

    let mut reqwest_response = client.post(url.clone()).json(&body).send();

    for retry_step in 0..=6 {
        // https://docs.github.com/en/graphql/overview/rate-limits-and-node-limits-for-the-graphql-api#exceeding-the-rate-limit
        // 主要速率限制（Primary Rate Limit）：
        //
        // 如果你超过了主要速率限制，响应状态仍然为 200，但会收到一个错误消息，
        // 并且 x-ratelimit-remaining 标头的值将为 0。在 x-ratelimit-reset 标头
        // 指定的时间之前，不应重试请求。
        //
        // 次要速率限制（Secondary Rate Limit）：
        //
        // 如果你超过了次要速率限制，响应状态将为 200 或 403，同时会收到一个指示
        // 你触发了次要速率限制的错误消息。如果响应头中包含了 retry-after，则应
        // 该在指定的秒数之后重试请求。
        //
        // 如果 x-ratelimit-remaining 标头的值为 0，则应在 x-ratelimit-reset 标
        // 头指定的 UTC epoch 秒数之后重试请求。否则，如果没有明确的重试时间，等
        // 待至少一分钟再进行重试。如果由于次要速率限制导致请求继续失败，等待重
        // 试的时间应按指数增加，最终在一定数量的重试后抛出错误。
        //
        if let Ok(r) = &reqwest_response {
            use reqwest::StatusCode as Code;
            match r.status() {
                Code::OK => {
                    // 如果是 200，但是 x-ratelimit-remaining 为 0，那么就需要等待 x-ratelimit-reset 了。
                    if r.headers()
                        .get("x-ratelimit-remaining")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<i32>().ok())
                        .unwrap_or(0)
                        > 0
                    {
                        break;
                    }
                }
                Code::BAD_GATEWAY | Code::GATEWAY_TIMEOUT => {
                    // https://github.com/orgs/community/discussions/24631#discussioncomment-3244785
                    // 但在实际情况中，502 504 的情况一般是数据规模太大导致。
                    // 如果是 502 504，那么就需要把会窗口大小改小。
                    //  TODO 这里也意味着每一页的大小是不固定的。
                    let size = body.variables.get_window();
                    let new_size = (size * 2 / 3).max(1);
                    // 此处每次缩小到原来的 2/3
                    log::info!("收到 502 or 504 响应码，尝试缩小本次窗口大小到 {new_size}。");
                    body.variables.set_window(new_size);
                }
                code => {
                    warn!("收到未处理过的意外响应码：{code}");
                }
            }
        }

        // github 给的时间戳是加州西 7 区的时间，所以需要转换一下。
        let time_zone = chrono::FixedOffset::west_opt(7 * 3600).unwrap();

        // 第一重限制可能提供的时间根据实际情况，第一层的 5000 次限制每个小时都
        // 是跑不满的，因为每个请求拉满 100 的 nodes 实际给 github 的计算时间还
        // 是偏多了一点。所以门槛都在计算复杂度上。
        let x_ratelimit_reset: u64 = reqwest_response
            .as_ref()
            .ok()
            .and_then(|r| r.headers().get("x-ratelimit-reset"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(chrono::Utc::now().with_timezone(&time_zone).timestamp() as u64);

        // 第二重限制可能提供的时间
        let retry_after: u64 = reqwest_response
            .as_ref()
            .ok()
            .and_then(|r| r.headers().get("retry-after"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let retry_secs = retry_after
            .max(x_ratelimit_reset - chrono::Utc::now().timestamp() as u64)
            // 假设基础的重试时间是 30 秒
            // 累计前面的 30 + 60 + 120 + 240 + 480 + 960 + 1920 = 3810 秒约等于等待一小时。
            // 简单算就是 3840（30 << 7）秒 - 30 秒
            // take max time on retry.
            .max(crate::BASE_RETRY_SECS << retry_step);

        log::info!("服务器请求被阻止，尝试 {retry_secs}s 后重试任务。");

        // dump the response body before retries to  logs/<datetime>_fail.json
        // 此处会移动消耗掉 reqwest_response
        dump_fail_request(reqwest_response);

        thread::sleep(Duration::from_secs(retry_secs));

        reqwest_response = client.post(url.clone()).json(&body).send();
    }

    // 如果是代理或者网络中断的情况，说实话我也没办法。
    let response = reqwest_response.expect("重试间隔 1h 之后还是失败，需要进一步寻找原因。");

    // take response headers out
    let _ = f(response.headers());

    response.json()
}

fn dump_fail_request(reqwest_response: Result<reqwest::blocking::Response, reqwest::Error>) {
    match reqwest_response {
        Ok(r) => {
            log::error!(
                "本次失败响应状态码：{code:?}，响应头：{head:#?}",
                code = r.status(),
                head = r.headers()
            );

            let body = r.text().unwrap_or("respnse.text() failed".to_owned());

            if body.starts_with("<!DOCTYPE html>") {
                let log_dir = std::path::Path::new("log");
                if !log_dir.exists() {
                    std::fs::create_dir(log_dir).unwrap();
                }

                let filename = log_dir.join(format!(
                    "{}_fail.html",
                    chrono::Utc::now().format("%Y-%m-%d_%H_%M_%S")
                ));

                let mut file = std::fs::File::create(&filename).unwrap();

                file.write_all(body.as_bytes()).unwrap();

                log::error!(
                    "本次失败响应体的内容为： {p}",
                    p = filename.as_path().to_string_lossy()
                );
            } else {
                log::error!("本次失败响应体的内容为： {body}");
            }
        }
        Err(e) => {
            log::error!("reqwest_response is Err: {e:#?}");
        }
    };
}

#[test]
fn test_dump_file_create() {
    let log_dir = std::path::Path::new("log");
    if !log_dir.exists() {
        std::fs::create_dir(log_dir).unwrap();
    }

    let filename = log_dir.join(format!(
        "{}_fail.html",
        chrono::Utc::now().format("%Y-%m-%d_%H%M%S")
    ));

    std::fs::File::create(filename).unwrap();
}

#[test]
fn test_time_stamp() {
    use chrono::{DateTime, TimeZone, Utc};

    // 注意系统给的 reset 是一个 time stamp
    let epoch_seconds = 1703858652;

    let datetime: DateTime<Utc> = Utc
        .timestamp_opt(epoch_seconds as i64, 0)
        .single()
        .expect("Invalid timestamp");

    let now_epoch_seconds = chrono::Utc::now().timestamp();

    println!("UTC: {}", datetime.to_rfc3339());

    let nowtime: DateTime<Utc> = Utc
        .timestamp_opt(now_epoch_seconds, 0)
        .single()
        .expect("Invalid timestamp");

    println!("UTC: {}", nowtime.to_rfc3339());
}

#[test]
fn text_time_zone() {
    let fo = chrono::FixedOffset::west_opt(7 * 3600).unwrap();
    dbg!(chrono::Utc::now().with_timezone(&fo).to_rfc3339());
}
