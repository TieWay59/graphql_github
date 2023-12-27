use anyhow::Result;
use graphql_client::reqwest::post_graphql_blocking;
use graphql_client::GraphQLQuery;
use reqwest::blocking;

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

    let response_data = post_graphql_blocking::<GetRepositoryDiscussions, _>(
        &client,
        "https://api.github.com/graphql",
        variables,
    )
    .expect("failed to execute query")
    .data
    .expect("missing response data");

    Ok(serde_json::to_string(&response_data)?)
}
