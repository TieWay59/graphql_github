use chrono::{DateTime, TimeZone, Utc};
use graphql_client::GraphQLQuery;
use reqwest::header::{self, HeaderValue};
use std::{thread, time::Duration};

/// 重新定义 graphql_client::reqwest::post_graphql_blocking
/// 主要增加了一个观察者闭包函数，观察内部的 header。
pub fn post_graphql_blocking<Q: GraphQLQuery, U: reqwest::IntoUrl + Clone>(
    client: &reqwest::blocking::Client,
    url: U,
    variables: Q::Variables,
    // 目前只是一个粗略的实现，由于源库年久失修，这个
    mut f: impl FnMut(&reqwest::header::HeaderMap) -> anyhow::Result<()>,
) -> Result<graphql_client::Response<Q::ResponseData>, reqwest::Error> {
    let body = Q::build_query(variables);

    let mut reqwest_response = client.post(url.clone()).json(&body).send();

    for retry_step in 0..=6 {
        // https://docs.github.com/en/graphql/overview/rate-limits-and-node-limits-for-the-graphql-api#exceeding-the-rate-limit
        //
        // If you exceed your primary rate limit, the response status will still
        // be 200, but you will receive an error message, and the value of the
        // x-ratelimit-remaining header will be 0. You should not retry your
        // request until after the time specified by the x-ratelimit-reset
        // header.（因为每秒间隔 1 次，基本上不太可能用完 5000 每小时的额度）
        //
        // If you exceed a secondary rate limit, the response status will be 200
        // or 403, and you will receive an error message that indicates that you
        // hit a secondary rate limit.
        //
        // If the *retry-after* response header is present, you should not retry
        // your request until after that many seconds has elapsed. If the
        // x-ratelimit-remaining header is 0, you should not retry your request
        // until after the time, in UTC epoch seconds, specified by the
        // x-ratelimit-reset header.
        //
        // Otherwise, wait for at least one minute before retrying. If your
        // request continues to fail due to a secondary rate limit, wait for an
        // exponentially increasing amount of time between retries, and throw an
        // error after a specific number of retries.
        if let Ok(r) = &reqwest_response {
            if r.status().is_success() {
                let headers = r.headers();
                if headers
                    .get("x-ratelimit-remaining")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0)
                    > 0
                {
                    // 存在 x-ratelimit-remaining 说明至少还是第二层限制没有超
                    // 但是如果是 0，那么就需要等待 x-ratelimit-reset 了。
                    break;
                }
            }
        }

        // 第一重限制可能提供的时间
        let x_ratelimit_reset: u64 = reqwest_response
            .as_ref()
            .ok()
            .and_then(|r| r.headers().get("x-ratelimit-reset"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(chrono::Utc::now().timestamp() as u64);

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
            // 累计前面的 30 + 60 + 120 + 240 + 480 + 960 + 1920 = 3810 秒约等于等待一小时。
            // 简单算就是 3840（30 << 7）秒 - 30 秒
            // take max time on retry.
            .max(1 << retry_step);

        log::info!("服务器请求被阻止，尝试 {retry_secs}s 后重试任务。");

        // dump the response body before retries to  logs/<datetime>_fail.json
        // TODO not tested need be careful.
        {
            let now = chrono::Local::now();
            let file_name = format!("logs/{}_fail.json", now.format("%Y-%m-%d_%H-%M-%S"));
            let mut file = std::fs::File::create(&file_name).unwrap();
            let _ = std::io::copy(&mut reqwest_response.as_mut().unwrap(), &mut file);
            log::info!("response body dumped to {}", file_name);
        }

        thread::sleep(Duration::from_secs(retry_secs));

        reqwest_response = client.post(url.clone()).json(&body).send();
    }

    // 如果是代理或者网络中断的情况，说实话我也没办法。
    let response = reqwest_response.expect("重试间隔 1h 之后还是失败，需要进一步寻找原因。");

    // take response headers out
    let _ = f(response.headers());

    response.json()
}

#[test]
fn test_time_stamp() {
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
