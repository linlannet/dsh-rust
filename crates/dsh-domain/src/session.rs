use dsh_types::{RunId, SessionId};

use crate::model::ChatMessage;

/// 会话聚合的骨架形态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// 会话标识。
    pub id: SessionId,
    /// 可选标题。
    pub title: Option<String>,
    /// 消息历史（内存）。
    pub messages: Vec<ChatMessage>,
    /// 当前运行。
    pub active_run: Option<RunId>,
}

impl Session {
    /// 创建无标题会话。
    pub fn new(id: SessionId) -> Self {
        Self {
            id,
            title: None,
            messages: Vec::new(),
            active_run: None,
        }
    }
}
