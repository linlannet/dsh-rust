use dsh_types::SessionId;

/// 会话聚合的骨架形态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// 会话标识。
    pub id: SessionId,
    /// 可选标题。
    pub title: Option<String>,
}

impl Session {
    /// 创建无标题会话。
    pub fn new(id: SessionId) -> Self {
        Self { id, title: None }
    }
}
