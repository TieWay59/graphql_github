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

    // TODO 关于目标仓库列表
    // https://open-leaderboard.x-lab.info/
    // 我可以先提取一个列表 txt，然后采集任务就从 txt 里面获取列表。
    // NixOS/nixpkgs
    let repo_owner = "NixOS";
    let repo_name = "nixpkgs";
    let mut query_cursor: Option<String> = None;

    // TODO 这里这个循环要进一步修改。
    for i in 0..2 {
        let response_data =
            query::single_query(repo_owner, repo_name, query_cursor.take(), &client).ok();

        let is_empty_page = response_data
            .as_ref()
            .and_then(|response_data| response_data.repository.as_ref())
            .and_then(|repo| repo.discussions.nodes.as_ref())
            .map_or(true, |nodes| nodes.is_empty());

        if is_empty_page {
            log::info!("{repo_owner}/{repo_name} is_empty_page: true");
            break;
        }

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

        let has_next_page = response_data
            .as_ref()
            .and_then(|response_data| response_data.repository.as_ref())
            .map_or(false, |repo| repo.discussions.page_info.has_next_page);

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
