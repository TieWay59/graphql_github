use anyhow::{Context, Ok, Result};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct Config {
    pub token: String,
    pub user_agent: String,
}

pub(crate) fn load() -> Result<Config> {
    Ok(())
        .and_then(|_| std::fs::File::open("config.yml").context("config.yml 没有找到"))
        .and_then(|f| serde_yaml::from_reader(f).context("config.yml 解析错误"))
}
