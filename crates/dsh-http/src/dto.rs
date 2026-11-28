//! HTTP 请求与响应 DTO，字段名对齐 `contracts/v1`。

use dsh_case::{RunOutcome, ToolCallRecord};
use dsh_domain::RunStatus;
use dsh_types::{AgentId, ContentBlock, Error, Id, JsonValue, RunId, SessionEvent, SessionId};
use serde::{Deserialize, Serialize};

/// `POST /api/agent/message` 与 `/api/agent/stream` 请求体。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageRequest {
    /// 目标 Agent。
    pub agent_id: AgentId,
    /// 续用会话。
    #[serde(default)]
    pub session_id: Option<SessionId>,
    /// 内容块。
    pub content: Vec<ContentBlock>,
    /// 模型选择（骨架阶段忽略）。
    #[serde(default)]
    pub model: Option<JsonValue>,
    /// 工作区。
    #[serde(default)]
    pub workspace_id: Option<Id>,
    /// 附加元数据。
    #[serde(default)]
    pub metadata: Option<JsonValue>,
}

/// 阻塞响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageResponse {
    /// Agent。
    pub agent_id: AgentId,
    /// 会话。
    pub session_id: SessionId,
    /// 运行。
    pub run_id: RunId,
    /// 助手消息。
    pub message: MessageDto,
    /// 工具调用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDto>>,
    /// 用量。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageDto>,
    /// 终止原因。
    pub finish_reason: &'static str,
}

/// 助手消息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    /// 角色固定 assistant。
    pub role: &'static str,
    /// 内容块。
    pub content: Vec<ContentBlock>,
}

/// 工具调用。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallDto {
    /// 调用 ID。
    pub tool_call_id: String,
    /// 工具名。
    pub name: String,
    /// 状态。
    pub status: &'static str,
    /// 参数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<JsonValue>,
    /// 结果。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<JsonValue>,
    /// 错误。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

/// 用量。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageDto {
    /// 输入 token。
    pub input_tokens: u64,
    /// 输出 token。
    pub output_tokens: u64,
}

/// 取消请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCancelRequest {
    /// Agent。
    pub agent_id: AgentId,
    /// 运行。
    #[serde(default)]
    pub run_id: Option<RunId>,
    /// 原因。
    #[serde(default)]
    pub reason: Option<String>,
}

/// 取消响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCancelResponse {
    /// Agent。
    pub agent_id: AgentId,
    /// 运行。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    /// 当前状态。
    pub status: &'static str,
}

/// 健康检查。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    /// 固定 ok。
    pub status: &'static str,
    /// 产品版本。
    pub version: &'static str,
    /// 契约版本。
    pub contract_version: &'static str,
}

/// `GET /api/sessions/{sessionId}/events` 查询参数。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEventsQuery {
    /// 只返回 seq 大于该值的事件，默认 0。
    #[serde(default)]
    pub after_seq: Option<i64>,
    /// 单次返回上限，默认 200、上限 1000。
    #[serde(default)]
    pub limit: Option<i64>,
}

/// 会话事件分页响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEventsResponse {
    /// 会话。
    pub session_id: SessionId,
    /// 按 seq 升序的事件。
    pub events: Vec<SessionEvent>,
    /// 本次返回的最后一条事件 seq；无事件时为 0。
    pub last_seq: u64,
}

/// 统一错误体。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// 稳定错误码。
    pub code: &'static str,
    /// 可读消息。
    pub message: String,
    /// 是否可重试。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    /// 追踪 ID。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl AgentMessageResponse {
    /// 由运行结果构造响应。
    pub fn from_outcome(agent_id: &AgentId, outcome: RunOutcome) -> Self {
        let tool_calls = if outcome.tool_calls.is_empty() {
            None
        } else {
            Some(
                outcome
                    .tool_calls
                    .iter()
                    .map(ToolCallDto::from_record)
                    .collect(),
            )
        };
        Self {
            agent_id: agent_id.clone(),
            session_id: outcome.session_id,
            run_id: outcome.run_id,
            message: MessageDto {
                role: "assistant",
                content: vec![ContentBlock::text(outcome.text)],
            },
            tool_calls,
            usage: outcome.usage.map(|usage| UsageDto {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            }),
            finish_reason: outcome.finish_reason.as_str(),
        }
    }
}

impl ToolCallDto {
    fn from_record(record: &ToolCallRecord) -> Self {
        Self {
            tool_call_id: record.tool_call_id.clone(),
            name: record.name.clone(),
            status: record.status.as_str(),
            arguments: record.arguments.clone(),
            result: record.result.clone(),
            error: record.error.clone(),
        }
    }
}

impl AgentCancelResponse {
    /// 构造取消响应。
    pub fn new(agent_id: &AgentId, run_id: Option<RunId>, status: RunStatus) -> Self {
        Self {
            agent_id: agent_id.clone(),
            run_id,
            status: status.as_str(),
        }
    }
}
