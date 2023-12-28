mod config;
mod graphql_client_ext;
mod log;
mod query;

use anyhow::{Ok, Result};
use reqwest::{blocking, header};
use std::fs;

fn main() -> Result<()> {
    log::set_logger(&log::MY_LOGGER).expect("logger init failed");
    log::set_max_level(log::LevelFilter::Info);

    log::info!("begin");

    // 读取配置构建 reqwest client
    let config::Config { token, user_agent } = config::load()?;

    let client = blocking::Client::builder()
        .default_headers(header::HeaderMap::from_iter([
            (header::AUTHORIZATION, format!("bearer {token}").parse()?),
            (header::USER_AGENT, user_agent.parse()?),
        ]))
        // https_only，似乎不选择协议的话，客户端还是会按默认 http。（不明）
        .https_only(true)
        .build()?;

    log::info!("client built");

    let repo_owner = "mermaid-js";
    let repo_name = "mermaid";
    let mut query_cursor: Option<String> = None;

    // TODO 这里这个循环要进一步修改。
    for i in 0..1 {
        let response_data =
            query::single_query(repo_owner, repo_name, query_cursor.take(), &client)?;
        
        let parsed_json = serde_json::to_string(&response_data)?;

        log::info!("response_data length: {}", parsed_json.len());

        dump_output(&parsed_json, "get_repository_discussions.json")?;

        // TODO UPD  query_cursor = Some(todo!());
    }

    log::info!("end");

    Ok(())
}

fn dump_output(parsed_json: &str, filename: &str) -> Result<()> {
    use std::{io::Write, path::Path};

    let output_dir = Path::new("output");

    if fs::read_dir(output_dir).is_err() {
        fs::create_dir(output_dir)?;
    };

    fs::File::create(output_dir.join(Path::new(filename)))
        .expect("文件打开失败")
        .write_all(parsed_json.to_string().as_bytes())
        .expect("文件写入失败");

    Ok(())
}
