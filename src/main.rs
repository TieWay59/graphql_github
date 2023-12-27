use graphql_client::GraphQLQuery;
use reqwest::{blocking, header, header::HeaderMap};
use std::io::Write;

use crate::get_repository_discussions::Variables;
use graphql_client::reqwest::post_graphql_blocking;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Config {
    token: String,
    user_agent: String,
}

// TODO: 暂时不知道为什么，但是 https://github.com/graphql-rust/graphql-client/blob/main/examples/github/examples/github.rs 案例中这样写。
#[allow(clippy::upper_case_acronyms)]
type DateTime = String;

#[allow(clippy::upper_case_acronyms)]
type URI = String;

// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schemas/schema.docs.graphql",
    query_path = "get_repository_discussions.graphql",
    response_derives = "Debug, Serialize, Deserialize"
)]
pub struct GetRepositoryDiscussions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");

    // 读取配置
    let Config { token, user_agent } = serde_yaml::from_reader(std::fs::File::open("config.yml")?)?;

    let auth = format!("bearer {token}");

    // TODO: 这里可以 collect
    let headers: HeaderMap = HeaderMap::from_iter([
        (header::AUTHORIZATION, auth.parse()?),
        (header::USER_AGENT, user_agent.parse()?),
    ]);

    let client = blocking::Client::builder()
        .default_headers(headers)
        // 这个设置很重要，似乎不选择协议的话，客户端还是会按默认 http。
        .https_only(true)
        .build()?;

    let variables = Variables {
        repo_owner: "mermaid-js".to_string(),
        repo_name: "mermaid".to_string(),
        // TODO 还不确定这个缺省该怎么处理
        query_cursor: Some("Y3Vyc29yOnYyOpK5MjAyMy0wNi0wM1QwMjowOToyMSswODowMM4AUD8C".into()),
        query_window: Some(3),
    };

    let response_body = post_graphql_blocking::<GetRepositoryDiscussions, _>(
        &client,
        "https://api.github.com/graphql",
        variables,
    )?;

    let response_data: get_repository_discussions::ResponseData =
        response_body.data.expect("missing response data");

    let parsed_json = serde_json::to_string(&response_data)?;

    // 把 parsed_json 这个文件导出到 output.json 当中
    let mut file = std::fs::File::create("output2.json")?;
    file.write_all(parsed_json.to_string().as_bytes())?;

    println!("finished");

    Ok(())
}
