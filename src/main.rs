mod config;
mod graphql_client_ext;
mod log;
mod query;
mod util;

use anyhow::{Ok, Result};
use reqwest::{blocking, header};

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
    for i in 0..2 {
        let response_data =
            query::single_query(repo_owner, repo_name, query_cursor.take(), &client).ok();

        let has_next_page = response_data
            .as_ref()
            .and_then(|response_data| response_data.repository.as_ref())
            .map_or(false, |repo| repo.discussions.page_info.has_next_page);

        let parsed_json = serde_json::to_string(&response_data)?;

        log::info!("step {i:03} response_data length: {}", parsed_json.len());

        util::dump_output(
            &parsed_json,
            repo_owner,
            repo_name,
            util::TaskType::Discussion,
            &query_cursor,
            i,
        )?;

        if !has_next_page {
            log::info!("{repo_owner}/{repo_name} has_next_page: false");
            break;
        }

        query_cursor = response_data
            .as_ref()
            .and_then(|response_data| response_data.repository.as_ref())
            .and_then(|repo| repo.discussions.page_info.end_cursor.clone());
    }

    log::info!("end");

    Ok(())
}
