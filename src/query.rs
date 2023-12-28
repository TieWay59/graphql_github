use std::{f32::consts::E, str::FromStr};

use anyhow::{Context, Ok, Result};
use graphql_client::GraphQLQuery;
use reqwest::{
    blocking,
    header::{self, HeaderMap},
};

use crate::graphql_client_ext;

// TODO: 暂时不知道为什么，但是 https://github.com/graphql-rust/graphql-client/blob/main/examples/github/examples/github.rs 案例中这样写。
#[allow(clippy::upper_case_acronyms)]
type DateTime = String;

#[allow(clippy::upper_case_acronyms)]
type URI = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/schema.docs.graphql",
    query_path = "get_repository_discussions.graphql",
    response_derives = "Debug, Serialize, Deserialize"
)]
// 一个 get_repository_discussions 命名的模块会包含进来。
pub struct GetRepositoryDiscussions;

struct RateLimit {
    limit: i32,
    remaining: i32,
    used: i32,
    reset: i32,
}

impl RateLimit {
    fn new(limit: i32, remaining: i32, used: i32, reset: i32) -> Self {
        Self {
            limit,
            remaining,
            used,
            reset,
        }
    }
}

impl TryFrom<&HeaderMap> for RateLimit {
    type Error = anyhow::Error;

    fn try_from(headers: &HeaderMap) -> anyhow::Result<Self> {
        let extract = |hm: &HeaderMap, key: &str| -> Result<i32> {
            Ok(hm[key]
                .to_str()?
                .parse()
                .context(format!("headers {key} 数值解析失败"))?)
        };

        Ok(Self::new(
            extract(headers, "x-ratelimit-limit")?,
            extract(headers, "x-ratelimit-remaining")?,
            extract(headers, "x-ratelimit-used")?,
            extract(headers, "x-ratelimit-reset")?,
        ))
    }
}

pub fn operate_query(client: blocking::Client) -> Result<String> {
    // TODO 经过分析我发觉，Variables 是每个任务都不一样的，在 post_graphql_blocking 的时候其实隐含了类型信息。所以不可以抽象成一组高度类似的函数。
    // TODO 下一步是实现循环请求，要考虑 github graphql 的请求上限，需要做一点计算。

    let variables = get_repository_discussions::Variables {
        repo_owner: "mermaid-js".to_string(),
        repo_name: "mermaid".to_string(),
        // TODO 还不确定这个缺省该怎么处理
        query_cursor: Some("Y3Vyc29yOnYyOpK5MjAyMy0wNi0wM1QwMjowOToyMSswODowMM4AUD8C".into()),
        query_window: Some(3),
    };

    // 关注以下几个字段就可以计算出剩余流量：
    //
    // x-ratelimit-limit	    The maximum number of points that you can use per hour
    // x-ratelimit-remaining	The number of points remaining in the current rate limit window
    // x-ratelimit-used	        The number of points you have used in the current rate limit window
    // x-ratelimit-reset	    The time at which the current rate limit window resets, in UTC epoch seconds
    // x-ratelimit-resource	    The rate limit resource that the request counted against. For GraphQL requests, this will always be graphql.
    //
    //  ref: https://docs.github.com/en/graphql/overview/rate-limits-and-node-limits-for-the-graphql-api
    let mut headers = header::HeaderMap::new();

    let response = graphql_client_ext::post_graphql_blocking::<GetRepositoryDiscussions, _>(
        &client,
        "https://api.github.com/graphql",
        variables,
        |h| {
            headers = h.clone();
        },
    )
    .expect("failed to execute query");

    let RateLimit {
        limit,
        remaining,
        used,
        reset,
    } = (&headers).try_into()?;

    log::info!("limit: {limit}");
    log::info!("remaining: {remaining}");
    log::info!("used: {used}");
    log::info!("reset: {reset}");

    let response_data: get_repository_discussions::ResponseData =
        response.data.expect("missing response data");

    Ok(serde_json::to_string(&response_data)?)
}
