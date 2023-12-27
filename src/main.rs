mod log;
mod query;

use reqwest::{blocking, header};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct Config {
    token: String,
    user_agent: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    log::set_logger(&log::MY_LOGGER).expect("logger init failed");
    log::set_max_level(log::LevelFilter::Info);

    log::info!("begin");

    // 读取配置构建 reqwest client
    let Config { token, user_agent } = serde_yaml::from_reader(std::fs::File::open("config.yml")?)?;
    let auth = format!("bearer {token}");

    let client = blocking::Client::builder()
        .default_headers(header::HeaderMap::from_iter([
            (header::AUTHORIZATION, auth.parse()?),
            (header::USER_AGENT, user_agent.parse()?),
        ]))
        // https_only，似乎不选择协议的话，客户端还是会按默认 http。（不明）
        .https_only(true)
        .build()?;

    let parsed_json = query::operate_query(client)?;

    dump_output(&parsed_json, "get_repository_discussions.json")?;

    log::info!("end");

    Ok(())
}

fn dump_output(parsed_json: &str, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::{fs, io::Write, path::Path};

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
