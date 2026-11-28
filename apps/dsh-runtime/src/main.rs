//! dsh-runtime：HTTP + ReAct 最小闭环入口。

mod config;

use std::sync::Arc;

use config::{Config, ModelChoice};
use dsh_case::{AgentService, RunRegistry, RunnerConfig, ToolRegistry};
use dsh_domain::{ModelPort, Tool};
use dsh_http::AppState;
use dsh_infra::{
    ClockTool, FakeModel, FakeScript, InMemorySessionRepository, ReadFileSandbox, ReadFileTool,
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

    let sessions = Arc::new(InMemorySessionRepository::default());
    let runs = Arc::new(RunRegistry::default());
    let service = AgentService::new(
        model,
        tools.clone(),
        sessions,
        runs,
        RunnerConfig {
            max_steps: config.max_steps,
            tool_timeout: config.tool_timeout,
            model_timeout: config.model_timeout,
        },
    );
    let state = AppState::new(Arc::new(service));

    tracing::info!(
        bind = %config.bind_addr,
        model = config.model_name(),
        tools = tools.len(),
        contract = CONTRACT_VERSION,
        version = env!("CARGO_PKG_VERSION"),
        "dsh-runtime starting"
    );

    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutdown signal received");
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
