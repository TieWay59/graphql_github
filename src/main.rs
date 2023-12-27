use reqwest::{
    blocking, header,
    header::{HeaderMap, HeaderValue},
};
use serde_json::json;

use std::io::Write;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Config {
    token: String,
    user_agent: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello, world!");

    // 读取配置
    let Config { token, user_agent } = serde_yaml::from_reader(std::fs::File::open("config.yml")?)?;

    let auth = format!("bearer {token}");

    let headers: HeaderMap = HeaderMap::from_iter([
        (header::AUTHORIZATION, auth.parse()?),
        (header::USER_AGENT, user_agent.parse()?),
    ]);

    let client = blocking::Client::builder()
        .default_headers(headers)
        // 这个设置很重要，似乎不选择协议的话，客户端还是会按默认 http。
        .https_only(true)
        .build()?;

    // TODO: 这里需要模板化
    let query_body = r#"
{
  repository(owner: "mermaid-js", name: "mermaid") {
    discussions(
      after: "Y3Vyc29yOnYyOpK5MjAyMy0wNi0wM1QwMjowOToyMSswODowMM4AUD8C"
      first: 3
      answered: true
      orderBy: { field: CREATED_AT, direction: DESC }
    ) {
      pageInfo { 
        endCursor
      }
      nodes {
        title
        body
        url
        answer {
          body
          publishedAt
          reactions {
            totalCount
          }
        }
      }
    }
  }
}"#;

    let payload = json!({
        "query": query_body
    });

    let resp_text = client
        .post("https://api.github.com/graphql")
        .bearer_auth(token)
        .json(&payload)
        .send()?
        .text()?;

    let parsed_json: serde_json::Value = serde_json::from_str(&resp_text)?;

    // println!("{:#?}", parsed_json);

    // 把 parsed_json 这个文件导出到 output.json 当中
    let mut file = std::fs::File::create("output.json")?;
    file.write_all(parsed_json.to_string().as_bytes())?;

    println!("finished");

    Ok(())
}
