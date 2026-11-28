//! dsh-runtime：HTTP + ReAct 最小闭环入口。

mod config;

use std::sync::Arc;

use config::{Config, ModelChoice, StoreKind};
use dsh_case::{AgentService, RunRegistry, RunnerConfig, ToolRegistry};
use dsh_domain::{EventLog, ModelPort, SessionRepository, Tool};
use dsh_http::AppState;
use dsh_infra::{
    ClockTool, FakeModel, FakeScript, InMemorySessionEventLog, InMemorySessionRepository,
    ReadFileSandbox, ReadFileTool, SqliteEventLog, SqliteSessionRepository, SqliteStore,
};
use dsh_types::CONTRACT_VERSION;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env().map_err(|error| format!("invalid configuration: {error}"))?;

    let model: Arc<dyn ModelPort> = match config.model {
        ModelChoice::Fake => Arc::new(FakeModel::new(FakeScript::parse(&config.fake_script))),
        ModelChoice::DeepSeek => build_deepseek(&config)?,
    };

    let sandbox = Arc::new(ReadFileSandbox::new(
        config.workspace_root.clone(),
        config.read_file_max_bytes,
    )?);
    let mut registry = ToolRegistry::default();
    registry.register(Arc::new(dsh_infra::EchoTool) as Arc<dyn Tool>);
    registry.register(Arc::new(ClockTool) as Arc<dyn Tool>);
    registry.register(Arc::new(ReadFileTool::new(sandbox)) as Arc<dyn Tool>);
    let tools = Arc::new(registry);

    let (sessions, events): (Arc<dyn SessionRepository>, Arc<dyn EventLog>) =
        match config.store_kind {
            StoreKind::Sqlite => {
                // 打开即建目录并执行迁移，失败直接以非零码退出
                let store = Arc::new(SqliteStore::open(&config.db_path).map_err(|error| {
                    format!(
                        "failed to open database {}: {error}",
                        config.db_path.display()
                    )
                })?);
                (
                    Arc::new(SqliteSessionRepository::new(Arc::clone(&store))),
                    Arc::new(SqliteEventLog::new(store)),
                )
            }
            StoreKind::Memory => (
                Arc::new(InMemorySessionRepository::default()),
                Arc::new(InMemorySessionEventLog::default()),
            ),
        };

    let runs = Arc::new(RunRegistry::default());
    let service = Arc::new(AgentService::new(
        model,
        tools.clone(),
        sessions,
        events,
        runs,
        RunnerConfig {
            max_steps: config.max_steps,
            tool_timeout: config.tool_timeout,
            model_timeout: config.model_timeout,
        },
    ));
    let state = AppState::new(Arc::clone(&service)).with_stream_budget(config.stream_budget_bytes);

    tracing::info!(
        bind = %config.bind_addr,
        model = config.model_name(),
        store = config.store_name(),
        db = %config.db_label(),
        tools = tools.len(),
        stream_budget_bytes = config.stream_budget_bytes,
        contract = CONTRACT_VERSION,
        version = env!("CARGO_PKG_VERSION"),
        "dsh-runtime starting"
    );

    let shutdown_service = Arc::clone(&service);
    let shutdown = async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutdown signal received");
        // 有限等待在途 run 结束，保证会话投影与事件在退出前写完
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while !shutdown_service.runs().is_empty() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let pending = shutdown_service.runs().len();
        if pending == 0 {
            tracing::info!("shutdown drain complete");
        } else {
            tracing::warn!(pending, "shutdown drain timed out");
        }
    };
    dsh_http::serve(&config.bind_addr, state, shutdown)
        .await
        .map_err(|error| format!("failed to serve: {error}").into())
}

#[cfg(feature = "deepseek")]
fn build_deepseek(config: &Config) -> Result<Arc<dyn ModelPort>, Box<dyn std::error::Error>> {
    let api_key = config
        .deepseek_api_key
        .clone()
        .filter(|key| !key.trim().is_empty())
        .ok_or("DEEPSEEK_API_KEY is required when DSH_MODEL=deepseek")?;
    let model =
        dsh_infra::DeepSeekModel::new(api_key, config.model_base_url.clone(), "deepseek-chat")?;
    Ok(Arc::new(model))
}

#[cfg(not(feature = "deepseek"))]
fn build_deepseek(_config: &Config) -> Result<Arc<dyn ModelPort>, Box<dyn std::error::Error>> {
    Err("DSH_MODEL=deepseek requires building with --features deepseek".into())
}
