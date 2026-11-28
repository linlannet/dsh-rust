//! 用例编排。
//!
//! ReAct 最小循环：模型推理 → 工具执行 → 结果回填 → 再推理 → 终止。

pub mod event_sink;
pub mod run_registry;
pub mod runner;
pub mod tool_registry;

pub use event_sink::{CollectSink, EventSink};
pub use run_registry::{RunRegistration, RunRegistry, new_run_id};
pub use runner::{AgentService, RunOutcome, RunRequest, RunnerConfig, ToolCallRecord};
pub use tool_registry::ToolRegistry;
