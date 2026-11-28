//! 基础设施适配器。
//!
//! 当前提供内存会话仓库、运行事件日志、FakeModel、内置工具与文件沙箱；
//! DeepSeek 适配器由 feature `deepseek` 开启。

pub mod memory;
pub mod model;
pub mod sandbox;
pub mod tools;

pub use memory::{InMemoryRunEventLog, InMemorySessionRepository};
pub use model::{FakeModel, FakeScript};
pub use sandbox::ReadFileSandbox;
pub use tools::{ClockTool, EchoTool, ReadFileTool};

#[cfg(feature = "deepseek")]
pub use model::DeepSeekModel;
