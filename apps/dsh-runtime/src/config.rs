//! 环境变量配置。

use std::path::PathBuf;
use std::time::Duration;

/// 模型选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelChoice {
    /// 假模型。
    Fake,
    /// DeepSeek。
    DeepSeek,
}

/// 存储选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreKind {
    /// SQLite 文件库（默认）。
    Sqlite,
    /// 进程内存储，作为持久化回滚通道。
    Memory,
}

/// 运行时配置。
#[derive(Debug, Clone)]
pub struct Config {
    /// 监听地址。
    pub bind_addr: String,
    /// 模型选择。
    pub model: ModelChoice,
    /// Fake 脚本名。
    pub fake_script: String,
    /// 最大步数。
    pub max_steps: usize,
    /// 工具超时。
    pub tool_timeout: Duration,
    /// 模型超时。
    pub model_timeout: Duration,
    /// 工作区根。
    pub workspace_root: PathBuf,
    /// 单文件读取上限。
    pub read_file_max_bytes: usize,
    /// 存储选择。
    pub store_kind: StoreKind,
    /// 数据库文件路径。
    pub db_path: PathBuf,
    /// 单 run SSE 字节预算。
    pub stream_budget_bytes: usize,
    /// DeepSeek 密钥。
    #[cfg_attr(not(feature = "deepseek"), allow(dead_code))]
    pub deepseek_api_key: Option<String>,
    /// DeepSeek 基址。
    #[cfg_attr(not(feature = "deepseek"), allow(dead_code))]
    pub model_base_url: String,
}

impl Config {
    /// 从环境变量读取配置。
    pub fn from_env() -> Result<Self, String> {
        let model = match env_or("DSH_MODEL", "fake").as_str() {
            "fake" => ModelChoice::Fake,
            "deepseek" => ModelChoice::DeepSeek,
            other => {
                return Err(format!(
                    "unsupported DSH_MODEL: {other} (expected fake or deepseek)"
                ));
            }
        };
        let max_steps = parse_usize("DSH_MAX_STEPS", 8)?;
        if max_steps == 0 {
            return Err("DSH_MAX_STEPS must be greater than zero".to_owned());
        }
        let read_file_max_bytes = parse_usize("DSH_READ_FILE_MAX_BYTES", 262_144)?;
        if read_file_max_bytes == 0 {
            return Err("DSH_READ_FILE_MAX_BYTES must be greater than zero".to_owned());
        }
        let store_kind = match env_or("DSH_STORE", "sqlite").as_str() {
            "sqlite" => StoreKind::Sqlite,
            "memory" => StoreKind::Memory,
            other => {
                return Err(format!(
                    "unsupported DSH_STORE: {other} (expected sqlite or memory)"
                ));
            }
        };
        let db_path = PathBuf::from(env_or("DSH_DB_PATH", "./data/dsh-rust.db"));
        if db_path.as_os_str().is_empty() {
            return Err("DSH_DB_PATH must not be empty".to_owned());
        }
        let stream_budget_bytes = parse_usize(
            "DSH_STREAM_BUDGET_BYTES",
            dsh_http::sse::DEFAULT_STREAM_BUDGET_BYTES,
        )?;
        if stream_budget_bytes == 0 {
            return Err("DSH_STREAM_BUDGET_BYTES must be greater than zero".to_owned());
        }
        Ok(Self {
            bind_addr: env_or("DSH_BIND_ADDR", "127.0.0.1:8090"),
            model,
            fake_script: env_or("DSH_FAKE_SCRIPT", "echo"),
            max_steps,
            tool_timeout: Duration::from_millis(parse_u64("DSH_TOOL_TIMEOUT_MS", 5_000)?),
            model_timeout: Duration::from_millis(parse_u64("DSH_MODEL_TIMEOUT_MS", 60_000)?),
            workspace_root: std::env::var("DSH_WORKSPACE_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            read_file_max_bytes,
            store_kind,
            db_path,
            stream_budget_bytes,
            deepseek_api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
            model_base_url: env_or("DSH_MODEL_BASE_URL", "https://api.deepseek.com"),
        })
    }

    /// 模型名称，用于启动日志。
    pub fn model_name(&self) -> &'static str {
        match self.model {
            ModelChoice::Fake => "fake",
            ModelChoice::DeepSeek => "deepseek",
        }
    }

    /// 存储名称，用于启动日志。
    pub fn store_name(&self) -> &'static str {
        match self.store_kind {
            StoreKind::Sqlite => "sqlite",
            StoreKind::Memory => "memory",
        }
    }

    /// 启动日志中的数据库标识；内存模式不落盘。
    pub fn db_label(&self) -> String {
        match self.store_kind {
            StoreKind::Sqlite => self.db_path.display().to_string(),
            StoreKind::Memory => "<memory>".to_owned(),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse_usize(key: &str, default: usize) -> Result<usize, String> {
    match std::env::var(key) {
        Ok(value) => value
            .parse::<usize>()
            .map_err(|_| format!("{key} must be a positive integer")),
        Err(_) => Ok(default),
    }
}

fn parse_u64(key: &str, default: u64) -> Result<u64, String> {
    match std::env::var(key) {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| format!("{key} must be a positive integer")),
        Err(_) => Ok(default),
    }
}
