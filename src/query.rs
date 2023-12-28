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

pub fn single_query(
    repo_owner: &str,
    repo_name: &str,
    query_cursor: Option<String>,
    client: &blocking::Client,
) -> Result<get_repository_discussions::ResponseData> {
    let variables = get_repository_discussions::Variables {
        repo_owner: repo_owner.into(),
        repo_name: repo_name.into(),
        // 此处输入 None 可以获得第一页的内容，随后不断接收 cursor 来访问下一页。
        query_cursor,
        // 虽然 last 或者 first 只能填写 1-100，但是一次请求的 node 上限是 500,000。
        // 所以设计上一次只请求一个请求 100 个其实有点小。
        query_window: Some(100),
    };

    let mut headers = header::HeaderMap::new();

    let response = graphql_client_ext::post_graphql_blocking::<GetRepositoryDiscussions, _>(
        client,
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

    response.data.context("missing response data")
}
