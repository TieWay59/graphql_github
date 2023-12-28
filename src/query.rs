use anyhow::Context; // 在这个文件里不要引入 anyhow 的 Result
use graphql_client::GraphQLQuery;
use reqwest::blocking;

use crate::graphql_client_ext;

use crate::util;

pub enum QueryResponseData {
    Discussion(get_repository_discussions::ResponseData),
    PRCommits(get_pr_commits::ResponseData),
    ClosedIssues(get_closed_issues::ResponseData),
}

pub struct QueryResult {
    pub is_empty_page: bool,
    pub has_next_page: bool,
    pub rate_limit: util::RateLimit,
    pub query_cursor: Option<String>,
    pub response_data: QueryResponseData,
}

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

pub fn single_discussion_query(
    repo_owner: &str,
    repo_name: &str,
    query_cursor: &Option<String>,
    client: &blocking::Client,
) -> anyhow::Result<QueryResult> {
    // discussion 的查询变量
    let variables = get_repository_discussions::Variables {
        repo_owner: repo_owner.into(),
        repo_name: repo_name.into(),
        // 此处输入 None 可以获得第一页的内容，随后不断接收 cursor 来访问下一页。
        query_cursor: query_cursor.clone(),
        // 虽然 last 或者 first 只能填写 1-100，但是一次请求的 node 上限是 500,000。
        // 所以设计上一次只请求一个请求 100 个其实有点小。
        query_window: Some(100),
    };

    let mut rate_limit = util::RateLimit::default();

    // discussion 的特化查询
    let response = graphql_client_ext::post_graphql_blocking::<GetRepositoryDiscussions, _>(
        client,
        "https://api.github.com/graphql",
        variables,
        |h| {
            // TODO 仔细一想其实这里也可以检查 其他 head 情况。
            rate_limit = h.try_into()?;
            Ok(())
        },
    )
    .expect("failed to execute query");

    let response_data = response.data.context("missing response data")?;

    let repository = response_data.repository.as_ref();

    // 这里有实质上的
    let is_empty_page = repository
        .and_then(|repo| repo.discussions.nodes.as_ref())
        .map_or(true, |nodes| nodes.is_empty());

    let has_next_page = repository.map_or(false, |repo| repo.discussions.page_info.has_next_page);

    let query_cursor = if has_next_page {
        // TODO 其实我有个怀疑，end cursor 真的能用作下一次查询的起点么？会不会有问题？
        repository.and_then(|repo| repo.discussions.page_info.end_cursor.clone())
    } else {
        None
    };

    Ok(QueryResult {
        is_empty_page,
        has_next_page,
        query_cursor,
        rate_limit,
        response_data: QueryResponseData::Discussion(response_data),
    })
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/schema.docs.graphql",
    query_path = "get_pr_commits.graphql",
    response_derives = "Debug, Serialize, Deserialize, Clone"
)]
pub struct GetPRCommits;

pub fn single_pr_commits_query(
    repo_owner: &str,
    repo_name: &str,
    query_cursor: &Option<String>,
    client: &blocking::Client,
) -> anyhow::Result<QueryResult> {
    let variables = get_pr_commits::Variables {
        repo_owner: repo_owner.into(),
        repo_name: repo_name.into(),
        query_cursor: query_cursor.clone(),
        query_window: Some(100),
    };

    let mut rate_limit = util::RateLimit::default();

    // discussion 的特化查询
    let response = graphql_client_ext::post_graphql_blocking::<GetPRCommits, _>(
        client,
        "https://api.github.com/graphql",
        variables,
        |h| {
            // TODO 仔细一想其实这里也可以检查 其他 head 情况。
            rate_limit = h.try_into()?;
            Ok(())
        },
    )
    .expect("failed to execute query");

    let response_data = response.data.context("missing response data")?;

    let repository = response_data.repository.as_ref();

    let is_empty_page = repository
        .and_then(|repo| repo.pull_requests.nodes.as_ref())
        .map_or(true, |nodes| nodes.is_empty());

    let has_next_page = repository.map_or(false, |repo| repo.pull_requests.page_info.has_next_page);

    let query_cursor = if has_next_page {
        // TODO 其实我有个怀疑，end cursor 真的能用作下一次查询的起点么？会不会有问题？
        repository.and_then(|repo| repo.pull_requests.page_info.end_cursor.clone())
    } else {
        None
    };

    Ok(QueryResult {
        is_empty_page,
        has_next_page,
        query_cursor,
        rate_limit,
        response_data: QueryResponseData::PRCommits(response_data),
    })
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/schema.docs.graphql",
    query_path = "get_closed_issues.graphql",
    response_derives = "Debug, Serialize, Deserialize, Clone"
)]
pub struct GetClosedIssues;

pub fn single_closed_issues_query(
    repo_owner: &str,
    repo_name: &str,
    query_cursor: &Option<String>,
    client: &blocking::Client,
) -> anyhow::Result<QueryResult> {
    let variables = get_closed_issues::Variables {
        repo_owner: repo_owner.into(),
        repo_name: repo_name.into(),
        query_cursor: query_cursor.clone(),
        query_window: Some(100),
    };

    let mut rate_limit = util::RateLimit::default();

    // discussion 的特化查询
    let response = graphql_client_ext::post_graphql_blocking::<GetClosedIssues, _>(
        client,
        "https://api.github.com/graphql",
        variables,
        |h| {
            // TODO 仔细一想其实这里也可以检查 其他 head 情况。
            rate_limit = h.try_into()?;
            Ok(())
        },
    )
    .expect("failed to execute query");

    let response_data = response.data.context("missing response data")?;

    let repository = response_data.repository.as_ref();

    let is_empty_page = repository
        .and_then(|repo| repo.issues.nodes.as_ref())
        .map_or(true, |nodes| nodes.is_empty());

    let has_next_page = repository.map_or(false, |repo| repo.issues.page_info.has_next_page);

    let query_cursor = if has_next_page {
        // TODO 其实我有个怀疑，end cursor 真的能用作下一次查询的起点么？会不会有问题？
        repository.and_then(|repo| repo.issues.page_info.end_cursor.clone())
    } else {
        None
    };

    Ok(QueryResult {
        is_empty_page,
        has_next_page,
        query_cursor,
        rate_limit,
        response_data: QueryResponseData::ClosedIssues(response_data),
    })
}
