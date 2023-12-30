mod config;
mod graphql_client_ext;
mod log;
mod query;
mod util;

use ::log::info;
use anyhow::{Context, Ok, Result};
use reqwest::{blocking, header};
use std::fs::File;
use std::io::{self, BufRead, Cursor};
use util::TaskType;

// 每个仓库每个类型的数据采集步数的上限
const STEP_THRESHHOLD: i32 = 3;

// 重试间隔时间，单位秒
const BASE_RETRY_SECS: u64 = 60;

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
            if line.starts_with('#') {
                return None;
            }
            let mut it = line.split('/');
            let repo_owner = it.next()?.to_string();
            let repo_name = it.next()?.to_string();
            Some((repo_owner, repo_name))
        })
        // 采集任务主体：遍历仓库列表，采集每个仓库的讨论区。
        .try_for_each(|(repo_owner, repo_name)| {
            log::info!("crawling {}/{}", repo_owner, repo_name);
            // 遍历三种类型的任务

            for task_type in [
                TaskType::Discussions,
                TaskType::PRCommits,
                TaskType::ClosedIssues,
            ] {
                //  检查对应的文件是否存在
                let (last_step, last_cursor) = read_state(&repo_owner, &repo_name, task_type)
                    .unwrap_or(/* 不管如何报错都当空的 */ (None, None));
                info!(
                    "读取到状态 last_step: {:?}, last_cursor: {:?}",
                    last_step, last_cursor
                );

                crawling(
                    &repo_owner,
                    &repo_name,
                    &client,
                    task_type,
                    last_step,
                    last_cursor,
                )?;
            }
            Ok(())
        })?;

    log::info!("end");

    Ok(())
}

fn read_state(
    repo_owner: &str,
    repo_name: &str,
    task_type: TaskType,
) -> Result<(Option<i32>, Option<String>)> {
    // TODO 虽然这样的设计也算是可以解决问题，但为了更长远的考虑，最好每个仓库建
    // 立一个 metadata 专门保留所有的状态数据

    use std::path::Path;

    let task_path = Path::new("output")
        .join(format!("{}_{}", repo_owner, repo_name))
        .join(task_type.to_string());

    //  这里直接用 last 检查最后一个文件，原因是编号体系保证顺序最后一个是最新的。
    //  从文件名中提取出 step & cursor
    let last = std::fs::read_dir(task_path)
        .context("文件夹不存在")?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .last()
        .context("未能找到已有历史")?;

    let (last_step, last_cursor) = last
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".json"))
        .map(|name| name.split('_'))
        .and_then(|mut splits| {
            let this_step = splits.next();
            let this_cursor = splits.next();
            if let (Some(this_step), Some(this_cursor)) = (this_step, this_cursor) {
                let step = this_step.parse::<i32>().ok();
                if step == Some(0) {
                    // 首个 step 的 cursor 是 first_cursor
                    return None;
                }
                return step.zip(Some(this_cursor.to_string()));
            }
            None
        })
        .context("最后一个文件解析失败（原因可能复杂）")?;

    Ok((Some(last_step), Some(last_cursor)))
}

fn crawling(
    repo_owner: &str,
    repo_name: &str,
    client: &blocking::Client,
    task_type: TaskType,
    last_step: Option<i32>,
    last_cursor: Option<String>,
) -> Result<()> {
    let mut cursor: Option<String> = last_cursor;

    // 上一次爬虫最后一个请求要重新求，因为新的数据会增长到后面，每一批 100 个节点不一定都在
    let begining_step = last_step.unwrap_or(0);

    for i in begining_step..=STEP_THRESHHOLD {
        // 静态分发调用函数。
        let query::QueryResult {
            is_empty_page,
            has_next_page,
            rate_limit,
            query_cursor,
            response_data: query_response_data,
        } = match task_type {
            TaskType::Discussions => {
                query::single_discussion_query(repo_owner, repo_name, &cursor, client)?
            }
            TaskType::PRCommits => {
                query::single_pr_commits_query(repo_owner, repo_name, &cursor, client)?
            }
            TaskType::ClosedIssues => {
                query::single_issues_query(repo_owner, repo_name, &cursor, client)?
            }
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
                Discussions(response_data) => serde_json::to_string(&response_data)?,
                PRCommits(response_data) => serde_json::to_string(&response_data)?,
                ClosedIssues(response_data) => serde_json::to_string(&response_data)?,
            }
        };

        log::info!(
            "[{task_type}] [{repo_owner}] [{repo_name}] step {i:03} parsed_json length: {}",
            parsed_json.len()
        );

        // 写入文件还是用的老 cursor，拿这个 Option string 没办法。
        util::dump_output(&parsed_json, repo_owner, repo_name, task_type, &cursor, i)?;

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

#[test]
fn test_read_dir() -> Result<()> {
    use std::fs;
    use std::path::Path;

    let task_path = Path::new("output")
        .join(format!("{}_{}", "AleoHQ", "leo"))
        .join(TaskType::ClosedIssues.to_string());

    // 也就是说 fs read_dir 有能力拿到文件列表最后一个文件。
    // 但是前提条件是文件按照字典序排列吧。
    // TODO 目前文件命名上，每个文件的数字保留位数只有 3，一旦文件数量超过四位数就会有问题。
    // 目前最大的规模来自 nixos，1456 份让他也采集不完。
    dbg!(fs::read_dir(task_path)?.last());

    //  检查对应的文件是否存在
    let (last_step, _) = read_state("AleoHQ", "leo", TaskType::ClosedIssues)?;

    assert!(last_step.is_some());

    Ok(())
}
