//! 基础设施适配器。
//!
//! 提供内存与 SQLite 两种会话仓库、会话事件日志、运行事件日志、
//! FakeModel、内置工具与文件沙箱；DeepSeek 适配器由 feature `deepseek` 开启。

pub mod memory;
pub mod model;
pub mod sandbox;
pub mod store;
pub mod tools;

pub use memory::{InMemoryRunEventLog, InMemorySessionEventLog, InMemorySessionRepository};
pub use model::{FakeModel, FakeScript};
pub use sandbox::ReadFileSandbox;
pub use store::{SqliteEventLog, SqliteSessionRepository, SqliteStore};
pub use tools::{ClockTool, EchoTool, ReadFileTool};

#[cfg(feature = "deepseek")]
pub use model::DeepSeekModel;
