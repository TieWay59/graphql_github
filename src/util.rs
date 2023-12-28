use anyhow::{Context, Result};
use std::fs;
use std::{io::Write, path::Path};

pub enum TaskType {
    Discussion,
    Issue,
    PullRequest,
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            TaskType::Discussion => write!(f, "discussion"),
            TaskType::Issue => write!(f, "issue"),
            TaskType::PullRequest => write!(f, "pull_request"),
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
            id.as_ref().unwrap_or(&"first_cursor".to_string())
        ));

    if !full_path.exists() {
        fs::create_dir_all(full_path.parent().unwrap())
            .context(format!("{full_path:?} 路径创建出现问题"))?;
    }

    fs::File::create(&full_path)?.write_all(parsed_json.as_bytes())?;

    log::info!("成功导出文件 {full_path:?}");

    Ok(())
}
