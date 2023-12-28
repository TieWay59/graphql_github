mod config;
mod graphql_client_ext;
mod log;
mod query;
mod util;

use anyhow::{Context, Ok, Result};
use reqwest::{blocking, header};
use std::borrow::Borrow;
use std::fs::File;
use std::io::{self, BufRead};
use util::TaskType;

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

    let file = File::open("repolist.txt").context("没有找到 repolist.txt")?;

    io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| {
            let mut it = line.split('/');
            let repo_owner = it.next()?.to_string();
            let repo_name = it.next()?.to_string();
            Some((repo_owner, repo_name))
        })
        // 采集任务主体：遍历仓库列表，采集每个仓库的讨论区。
        .try_for_each(|(repo_owner, repo_name)| {
            log::info!("crawling {}/{}", repo_owner, repo_name);
            crawling(&repo_owner, &repo_name, &client, TaskType::PRCommits)?;
            crawling(&repo_owner, &repo_name, &client, TaskType::Discussion)?;
            Ok(())
        })?;

    log::info!("end");

    Ok(())
}

fn crawling(
    repo_owner: &str,
    repo_name: &str,
    client: &blocking::Client,
    task_type: TaskType,
) -> Result<()> {
    let mut cursor: Option<String> = None;

    // TODO 测试阶段，先都只采集小的。
    for i in 0..2 {
        // 静态分发调用函数。
        let query::QueryResult {
            is_empty_page,
            has_next_page,
            rate_limit,
            query_cursor,
            response_data: query_response_data,
        } = match task_type {
            TaskType::Discussion => {
                query::single_discussion_query(repo_owner, repo_name, cursor.take(), client)?
            }
            TaskType::PRCommits => {
                query::single_pr_commits_query(repo_owner, repo_name, cursor.take(), client)?
            }
            _ => unreachable!(),
        };

        // 如果是空页，就不用再继续了。
        if is_empty_page {
            log::info!("{repo_owner}/{repo_name} is_empty_page: true");
            break;
        }

        let parsed_json = {
            // 静态分发序列化函数
            // 表面上看起来都一样，实际上每个 data 类型都不同。
            use query::QueryResponseData::*;
            match query_response_data {
                Discussion(response_data) => serde_json::to_string(&response_data)?,
                PRCommits(response_data) => serde_json::to_string(&response_data)?,
            }
        };

        log::info!("step {i:03} response_data length: {}", parsed_json.len());

        // 写入文件
        util::dump_output(
            &parsed_json,
            repo_owner,
            repo_name,
            util::TaskType::Discussion,
            &cursor,
            i,
        )?;

        // 检查 rate limit 是否超速
        util::check_limit_and_block(rate_limit);

        // 如果没有下一页，就不用再继续了。
        if !has_next_page {
            log::info!("{repo_owner}/{repo_name} has_next_page: false");
            break;
        }

        // 如果有下一页，就继续。
        cursor = query_cursor;
    }

    Ok(())
}
