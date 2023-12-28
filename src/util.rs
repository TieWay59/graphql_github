use anyhow::{Context, Result};
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

    log::info!("成功导出文件 {full_path:?}");

    Ok(())
}

/// 参考 https://docs.github.com/en/graphql/overview/rate-limits-and-node-limits-for-the-graphql-api
#[derive(Debug, Default)]
pub struct RateLimit {
    pub limit: i32,
    pub remaining: i32,
    pub used: i32,
    pub reset: i32,
}

impl RateLimit {
    fn new(limit: i32, remaining: i32, used: i32, reset: i32) -> Self {
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
        let extract = |hm: &HeaderMap, key: &str| -> Result<i32> {
            hm[key]
                .to_str()?
                .parse()
                .context(format!("headers {key} 数值解析失败"))
        };

        Ok(Self::new(
            extract(headers, "x-ratelimit-limit")?,
            extract(headers, "x-ratelimit-remaining")?,
            extract(headers, "x-ratelimit-used")?,
            extract(headers, "x-ratelimit-reset")?,
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
    log::info!("limit: ({used}/{limit}) remaining: {remaining} reset: {reset}");

    if remaining < 2 {
        log::warn!("remaining < 2, 开始休眠 {remaining}s");
        thread::sleep(Duration::from_secs(remaining as u64));
    } else {
        // github 限制 gql 查询次数 5000/h
        // 算一下 5000 / 60 / 60 = 1.38/s
        // 1 / 1.38 = 0.72s 可以发一个请求，我感觉可以设置 sleep 650-750 ms。
        // 考虑上中间的延迟，基本上会不太可能超过限制。
        thread::sleep(Duration::from_millis(rand::random::<u64>() % 100 + 650));
    }
}
