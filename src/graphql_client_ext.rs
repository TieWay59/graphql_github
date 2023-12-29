use graphql_client::GraphQLQuery;
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
        if match &reqwest_response {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        } {
            break;
        }

        // 累计前面的 30 + 60 + 120 + 240 + 480 + 960 + 1920 = 3810 秒约等于等待一小时。
        // 简单算就是 3840（30 << 7）秒 - 30 秒
        let retry_secs = 1 << retry_step;
        log::info!("服务器请求被阻止，尝试 {retry_secs}s 后重试任务。");
        thread::sleep(Duration::from_secs(retry_secs));

        reqwest_response = client.post(url.clone()).json(&body).send();
    }

    // 如果是代理或者网络中断的情况，说实话我也没办法。
    let response = reqwest_response.expect("重试间隔 1h 之后还是失败，需要进一步寻找原因。");

    // take response headers out
    let _ = f(response.headers());

    response.json()
}
