use std::{thread, time::Duration};

use anyhow::{Context, Ok, Result};
use graphql_client::GraphQLQuery;
use reqwest::{
    blocking,
    header::{self, HeaderMap},
};

use crate::{graphql_client_ext, query};

// TODO: 暂时不知道为什么，但是 https://github.com/graphql-rust/graphql-client/blob/main/examples/github/examples/github.rs 案例中这样写。
#[allow(clippy::upper_case_acronyms)]
type DateTime = String;

#[allow(clippy::upper_case_acronyms)]
type URI = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/schema.docs.graphql",
    query_path = "get_repository_discussions.graphql",
    response_derives = "Debug, Serialize, Deserialize, Clone"
)]
// 一个 get_repository_discussions 命名的模块会包含进来。
pub struct GetRepositoryDiscussions;

/// 参考 https://docs.github.com/en/graphql/overview/rate-limits-and-node-limits-for-the-graphql-api
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

pub enum QueryResponseData {
    Discussion(get_repository_discussions::ResponseData),
}

pub struct QueryResult {
    pub is_empty_page: bool,
    pub has_next_page: bool,
    pub query_cursor: Option<String>,
    pub response_data: QueryResponseData,
}

pub fn single_discussion_query(
    repo_owner: &str,
    repo_name: &str,
    query_cursor: Option<String>,
    client: &blocking::Client,
) -> Result<QueryResult> {
    // discussion 的查询变量
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

    // discussion 的特化查询
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

    log::info!("limit: ({used}/{limit}) remaining: {remaining} reset: {reset}");

    if remaining < 2 {
        log::warn!("remaining < 2, 开始休眠 {remaining}s");
        thread::sleep(Duration::from_secs(remaining as u64));
    } else {
        // github 限制 gql 查询次数 5000/h
        // 算一下 5000 / 60 / 60 = 1.38/s
        // 1 / 1.38 = 0.72s 可以发一个请求，我感觉可以设置 sleep 650-750 ms。
        // 考虑上中间的延迟，基本上会不太可能超过限制。
        thread::sleep(Duration::from_millis(rand::random::<u64>() % 100 + 650));
    }

    let response_data = response.data.context("missing response data")?;

    let is_empty_page = response_data
        .repository
        .as_ref()
        .and_then(|repo| repo.discussions.nodes.as_ref())
        .map_or(true, |nodes| nodes.is_empty());

    let has_next_page = response_data
        .repository
        .as_ref()
        .map_or(false, |repo| repo.discussions.page_info.has_next_page);

    let query_cursor = if has_next_page {
        // TODO 其实我有个怀疑，end cursor 真的能用作下一次查询的起点么？会不会有问题？
        response_data
            .repository
            .as_ref()
            .and_then(|repo| repo.discussions.page_info.end_cursor.clone())
    } else {
        None
    };

    Ok(QueryResult {
        is_empty_page,
        has_next_page,
        query_cursor,
        response_data: QueryResponseData::Discussion(response_data),
    })
}
