use std::fs::File;
use std::io::Write;
use commander::Commander;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    server: String,
    port: u32,
    //version 可以不存在，如果不存在，则使用 project_version
    version: String,
    dir: String,
    project: String,
    profile: String,
    auth: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = Commander::new()
        .version(&env!("CARGO_PKG_VERSION").to_string())
        .usage("init")
        .usage_desc("Micro Release Management System.")
        .option("-i, --init", "init a config file for pull releases", None)
        .option_str("-c, --cfg <config_file>", "config file path", Some("./mrms-pull.json".to_string()))
        .parse_env_or_exit();
    let config_path = command.get_str("cfg").unwrap();
    // 如果有 init 参数，则执行 init_pull_file 函数
    if let Some(_i) = command.get("init") {
        init_pull_file(config_path);
        Ok(())
    } else {
        read_releases_config(config_path).await;
        Ok(())
    }
}

/**
 * 初始化配置文件,将默认配置写入到配置文件中
 */
fn init_pull_file(config_path: String) {
    let default_config: Config = Config {
        server: "127.0.0.1".to_string(),
        port: 11111,
        version: "".to_string(),
        dir: ".".to_string(),
        project: "project_name".to_string(),
        profile: "__default__".to_string(),
        auth: "your_auth_code".to_string(),
    };

    println!("init pull file,{}", config_path);
    let config = serde_json::to_string_pretty(&default_config).unwrap();
    println!("{}", config);
    std::fs::write(config_path, config).unwrap();
}

/**
 * 从配置文件中读取配置，然后拉取发布包
 */
async fn read_releases_config(config_path: String) {
    //读取配置文件
    if let Ok(config_str) = std::fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<Config>(&config_str) {
            pull_releases(config).await.unwrap();
        } else {
            println!("config file format error,{}", config_path);
        }
    } else {
        println!("config file not found,{}", &config_path);
    }
}

// 拉取版本
async fn pull_releases(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("pull releases  project:{}, version:{}, profile:{}", config.project, config.server, config.profile);
    let url = format!("http://{}:{}", config.server, config.port);
    let resp = reqwest::Client::new().get(url)
        .header("action", "pull-start")
        .header("project", &config.project)
        .header("version", &config.version)
        .header("profile", &config.profile)
        .header("auth", &config.auth)
        .send()
        .await?
        .text()
        .await?;
    // resp 是服务器返回的文件列表的数组
    let file_arr = serde_json::from_str::<Vec<String>>(&resp).unwrap();
    for file in file_arr {
        download_file(config.clone(), file).await?;
    }
    Ok(())
}

async fn download_file(config: Config, location: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("downloading file:{}", location);
    let file_path = format!("{}/{}", config.dir, location.replace("\\", "/"));
    let dir = file_path.rsplitn(2, "/").last().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    let url = format!("http://{}:{}", config.server, config.port);
    let mut response = reqwest::Client::new().get(&url)
        .header("action", "pull-file")
        .header("project", config.project)
        .header("version", config.version)
        .header("profile", config.profile)
        .header("auth", config.auth)
        .header("location", urlencoding::encode(&location))
        .send()
        .await?;

    let total_size = response.headers().get(reqwest::header::CONTENT_LENGTH)
        .and_then(|ct_len| ct_len.to_str().ok().and_then(|ct_len| ct_len.parse().ok()))
        .unwrap_or(0);

    let pb = ProgressBar::new(total_size);

    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    let mut dest = File::create(&file_path)?;
    while let Some(chunk) = response.chunk().await? {
        pb.inc(chunk.len() as u64);
        dest.write_all(&chunk)?;
    }
    dest.flush()?;
    println!();
    Ok(())
}
