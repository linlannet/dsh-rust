//! 运行事件与状态。

use dsh_types::{Error, JsonValue};

/// 工具调用状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    /// 已开始。
    Started,
    /// 已完成。
    Completed,
    /// 已失败。
    Failed,
}

impl ToolCallStatus {
    /// 稳定字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// 终止原因，对齐契约 `finishReason`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    /// 模型正常结束。
    Stop,
    /// 以工具调用结束（阻塞响应语义）。
    ToolCalls,
    /// 达到步数上限。
    Length,
    /// 被取消。
    Cancelled,
    /// 运行失败。
    Error,
}

impl FinishReason {
    /// 稳定字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::ToolCalls => "tool_calls",
            Self::Length => "length",
            Self::Cancelled => "cancelled",
            Self::Error => "error",
        }
    }
}

/// 运行状态，对齐契约 `runStatus`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// 待执行。
    Pending,
    /// 执行中。
    Running,
    /// 已完成。
    Completed,
    /// 已失败。
    Failed,
    /// 已取消。
    Cancelled,
}

impl RunStatus {
    /// 稳定字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Token 用量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// 输入 token。
    pub input_tokens: u64,
    /// 输出 token。
    pub output_tokens: u64,
}

/// 运行事件。
#[derive(Debug, Clone, PartialEq)]
pub enum RunEvent {
    /// 文本增量。
    Chunk {
        /// 消息 ID。
        message_id: String,
        /// 增量文本。
        delta: String,
    },
    /// 工具调用状态变更。
    ToolCall {
        /// 工具调用 ID。
        tool_call_id: String,
        /// 工具名。
        name: String,
        /// 状态。
        status: ToolCallStatus,
        /// 参数（started）。
        arguments: Option<JsonValue>,
        /// 结果（completed）。
        result: Option<JsonValue>,
        /// 错误（failed）。
        error: Option<Error>,
    },
    /// 正常终止。
    Done {
        /// 终止原因。
        finish_reason: FinishReason,
        /// 用量。
        usage: Option<Usage>,
    },
    /// 错误终止。
    Error {
        /// 错误。
        error: Error,
        /// 追踪 ID。
        trace_id: Option<String>,
    },
}
