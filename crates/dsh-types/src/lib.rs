//! 跨进程契约对应的 Rust 侧基础类型。
//!
//! 字段定义以 `contracts/v1` 为准。这里只放稳定、可复用的值对象，
//! 不引入 Web 框架、数据库驱动或模型 SDK。

pub mod content;
pub mod error;
pub mod ids;
pub mod session_event;
pub mod time;
pub mod tool;

pub use content::ContentBlock;
pub use error::{Error, ErrorCode};
pub use ids::{AgentId, Id, IdError, RunId, SessionId, ToolCallId};
pub use session_event::{SessionEvent, event_type};
pub use time::{format_rfc3339, now_rfc3339};
pub use tool::{JsonSchema, JsonValue, ToolCallRequest, ToolDescriptor};

/// 当前契约草案版本，需与 `contracts/VERSION` 保持一致。
pub const CONTRACT_VERSION: &str = "1.0.0-draft.1";
