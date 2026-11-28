use serde::{Deserialize, Serialize};

use crate::ids::SessionId;
use crate::tool::JsonValue;

/// 已登记的会话事件类型。
///
/// 契约允许未知类型存在（见 `contracts/v1/persistence/session-event.schema.json`：
/// “type 保持开放，未知类型必须保留”），因此这里使用常量而非枚举，
/// 反序列化时不会拒绝未登记的类型。
pub mod event_type {
    /// 创建会话。
    pub const SESSION_CREATED: &str = "session.created";
    /// 标题或状态变化。
    pub const SESSION_UPDATED: &str = "session.updated";
    /// 追加消息。
    pub const MESSAGE_APPENDED: &str = "message.appended";
    /// 开始一次运行。
    pub const RUN_STARTED: &str = "run.started";
    /// 运行正常结束。
    pub const RUN_COMPLETED: &str = "run.completed";
    /// 运行失败或取消。
    pub const RUN_FAILED: &str = "run.failed";
    /// 请求工具。
    pub const TOOL_REQUESTED: &str = "tool.requested";
    /// 工具返回。
    pub const TOOL_COMPLETED: &str = "tool.completed";
    /// 发起审批。
    pub const APPROVAL_REQUESTED: &str = "approval.requested";
    /// 审批结论。
    pub const APPROVAL_DECIDED: &str = "approval.decided";
    /// 创建任务。
    pub const TASK_CREATED: &str = "task.created";
    /// 任务状态变化。
    pub const TASK_UPDATED: &str = "task.updated";

    /// 全部已登记类型，顺序与 `contracts/v1/persistence/event-catalog.md` 一致。
    pub const ALL: [&str; 12] = [
        SESSION_CREATED,
        SESSION_UPDATED,
        MESSAGE_APPENDED,
        RUN_STARTED,
        RUN_COMPLETED,
        RUN_FAILED,
        TOOL_REQUESTED,
        TOOL_COMPLETED,
        APPROVAL_REQUESTED,
        APPROVAL_DECIDED,
        TASK_CREATED,
        TASK_UPDATED,
    ];
}

/// 会话事件溯源记录。
///
/// `eventId` 与 `seq` 由事件日志在追加时分配；通过 [`SessionEvent::pending`]
/// 构造的事件只承载业务字段，两个分配字段为占位值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    /// 事件标识。
    pub event_id: String,
    /// 所属会话。
    pub session_id: SessionId,
    /// 会话内严格递增序号，从 1 开始。
    pub seq: u64,
    /// 事件类型，取值见 [`event_type`]。
    #[serde(rename = "type")]
    pub event_type: String,
    /// UTC ISO-8601 时间戳。
    pub timestamp: String,
    /// 事件载荷，字段随类型而定。
    pub payload: JsonValue,
}

impl SessionEvent {
    /// 构造待追加事件：`eventId` 为空、`seq` 为 0，等待事件日志填充。
    pub fn pending(
        session_id: SessionId,
        event_type: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            event_id: String::new(),
            session_id,
            seq: 0,
            event_type: event_type.into(),
            timestamp: timestamp.into(),
            payload: JsonValue::Object(serde_json::Map::new()),
        }
    }

    /// 附加载荷。
    pub fn with_payload(mut self, payload: JsonValue) -> Self {
        self.payload = payload;
        self
    }

    /// 是否尚未分配 `eventId` 与 `seq`。
    pub fn is_pending(&self) -> bool {
        self.seq == 0 && self.event_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_types_match_catalog() {
        assert_eq!(
            event_type::ALL,
            [
                "session.created",
                "session.updated",
                "message.appended",
                "run.started",
                "run.completed",
                "run.failed",
                "tool.requested",
                "tool.completed",
                "approval.requested",
                "approval.decided",
                "task.created",
                "task.updated",
            ]
        );
    }

    #[test]
    fn serializes_contract_field_names() {
        let event = SessionEvent::pending(
            SessionId::parse("sess-1").expect("valid id"),
            event_type::RUN_STARTED,
            "2026-11-28T00:00:00Z",
        )
        .with_payload(serde_json::json!({"runId": "run-1"}));

        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "eventId": "",
                "sessionId": "sess-1",
                "seq": 0,
                "type": "run.started",
                "timestamp": "2026-11-28T00:00:00Z",
                "payload": {"runId": "run-1"}
            })
        );
    }

    #[test]
    fn keeps_unknown_event_type() {
        let raw = r#"{
            "eventId": "evt-9",
            "sessionId": "sess-1",
            "seq": 9,
            "type": "future.event",
            "timestamp": "2026-11-28T00:00:00Z",
            "payload": {}
        }"#;
        let event: SessionEvent = serde_json::from_str(raw).expect("deserialize");
        assert_eq!(event.event_type, "future.event");
        assert_eq!(event.seq, 9);
        assert!(!event.is_pending());
    }

    #[test]
    fn pending_event_has_no_assigned_fields() {
        let event = SessionEvent::pending(
            SessionId::parse("sess-1").expect("valid id"),
            event_type::SESSION_CREATED,
            "2026-11-28T00:00:00Z",
        );
        assert!(event.is_pending());
        assert_eq!(event.seq, 0);
        assert!(event.event_id.is_empty());
    }
}
