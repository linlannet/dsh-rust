//! 领域模型与端口。
//!
//! 只依赖 `dsh-types`，不依赖 Web 框架、数据库驱动或具体模型 SDK。

pub mod model;
pub mod ports;
pub mod run;
pub mod session;
pub mod tool;

pub use model::{BoxFuture, ChatMessage, ModelPort, ModelRequest, ModelStep, Role};
pub use ports::{EventLog, SessionRepository};
pub use run::{FinishReason, RunEvent, RunStatus, ToolCallStatus, Usage};
pub use session::Session;
pub use tool::Tool;
