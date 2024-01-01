use anyhow::{Context, Result};
use chrono::TimeZone;
use rand::Rng;
use reqwest::header::HeaderMap;
use std::time::Duration;
use std::{fs, thread};
use std::{io::Write, path::Path};

#[derive(Debug, Clone, Copy)]
pub enum TaskType {
    Discussions,
    ClosedIssues,
    PRCommits,
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            TaskType::Discussions => write!(f, "discussion"),
            TaskType::ClosedIssues => write!(f, "issue"),
            TaskType::PRCommits => write!(f, "pull_request"),
        }
    }
}

pub fn dump_output(
    parsed_json: &str,
    owner: &str,
    repo: &str,
    task_type: TaskType,
    id: &Option<String>,
    window_number: i32,
) -> Result<()> {
    let full_path = Path::new("output")
        .join(format!("{}_{}", owner, repo))
        .join(task_type.to_string())
        .join(format!(
            "{window_number:03}_{}.json",
            id.clone().unwrap_or("first_cursor".to_string())
        ));

    if !full_path.exists() {
        fs::create_dir_all(full_path.parent().unwrap())
            .context(format!("{full_path:?} 路径创建出现问题"))?;
    }

    fs::File::create(&full_path)?.write_all(parsed_json.as_bytes())?;

    log::info!("成功导出文件： {fp}", fp = full_path.to_string_lossy());

    Ok(())
}

/// 参考 https://docs.github.com/en/graphql/overview/rate-limits-and-node-limits-for-the-graphql-api
/// If you exceed your primary rate limit, the response status will still be 200, but you will receive
/// an error message, and the value of the x-ratelimit-remaining header will be 0. You should not retry
///  your request until after the time specified by the x-ratelimit-reset header.
#[derive(Debug)]
pub struct RateLimit {
    pub limit: i64,
    pub remaining: i64,
    pub used: i64,
    pub reset: i64,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            limit: 5000,
            remaining: 0,
            used: 5000,
            reset: chrono::Utc::now().timestamp() + 3600,
        }
    }
}

impl RateLimit {
    fn new(limit: i64, remaining: i64, used: i64, reset: i64) -> Self {
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
        let extract = |hm: &HeaderMap, key: &str| -> Result<i64> {
            hm.get(key)
                .context(format!("headers {key} 不存在"))?
                .to_str()?
                .parse()
                .context(format!("headers {key} 数值解析失败"))
        };

        Ok(Self::new(
            extract(headers, "x-ratelimit-limit").unwrap_or(5000),
            // 理论上 remaining 不应该会没有。
            extract(headers, "x-ratelimit-remaining").unwrap_or(0),
            extract(headers, "x-ratelimit-used").unwrap_or(5000),
            extract(headers, "x-ratelimit-reset").unwrap_or(chrono::Utc::now().timestamp() + 3600),
        ))
    }
}

pub fn check_limit_and_block(
    RateLimit {
        limit,
        remaining,
        used,
        reset,
    }: RateLimit,
) {
    // github 给的时间戳是加州西 7 区的时间，所以需要转换一下。
    let time_zone = chrono::FixedOffset::west_opt(7 * 3600).unwrap();

    // 限制说明：
    //  No more than 60 seconds of this CPU time may be for the GraphQL API.
    //      You can roughly estimate the CPU time by measuring the total response time for your API requests.
    log::info!(
        "limit: ({used}/{limit}) remaining: {remaining} reset(github 本部时间):  {reset_time}",
        reset_time = time_zone
            .from_utc_datetime(&chrono::NaiveDateTime::from_timestamp_opt(reset, 0).unwrap())
            .to_rfc3339(),
    );

    // TODO 我感觉请求间隔时间放在这个函数其实已经不合适了，需要调整到 graphql_client_ext 文件中去更好。
    if remaining < 3 {
        // 实际上这种情况非常难触及到，因为每次请求的查询量很大，门槛不在 5000 次的限制。
        // 先设置睡眠五分钟，一般会在请求函数中等待更久。
        thread::sleep(Duration::from_secs(300));
    } else {
        let sleep_millis = if used % 3 == 0 {
            // 每 3 次，就设置一个稍微长的休眠。
            rand::thread_rng().gen_range(3_000..=9_000)
        } else {
            rand::thread_rng().gen_range(1_000..=3_000)
        };

        log::info!("开始休眠随机间隔 {sleep_millis}ms");
        thread::sleep(Duration::from_millis(sleep_millis));
    }
}
