//! 用例编排。
//!
//! 骨架阶段只定义运行请求、校验和会话创建；ReAct 状态机在后续迭代落地。

use dsh_domain::{Session, SessionRepository};
use dsh_types::{AgentId, Error, ErrorCode, SessionId};

/// 一次 Agent 运行请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    /// 目标 Agent。
    pub agent_id: AgentId,
    /// 续用已有会话。
    pub session_id: Option<SessionId>,
    /// 用户输入。
    pub text: String,
}

impl RunRequest {
    /// 构造请求。
    pub fn new(agent_id: AgentId, text: impl Into<String>) -> Self {
        Self {
            agent_id,
            session_id: None,
            text: text.into(),
        }
    }

    /// 校验请求是否可以进入运行。
    pub fn validate(&self) -> Result<(), Error> {
        if self.text.trim().is_empty() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "message text must not be empty",
            ));
        }
        Ok(())
    }
}

/// 运行 Agent 的用例入口。
#[derive(Debug)]
pub struct RunAgent<R> {
    repository: R,
}

impl<R> RunAgent<R> {
    /// 注入会话仓库。
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// 返回会话仓库引用。
    pub fn repository(&self) -> &R {
        &self.repository
    }
}

impl<R: SessionRepository> RunAgent<R> {
    /// 读取会话，不存在则创建并保存。
    pub fn ensure_session(&self, id: &SessionId) -> Result<Session, Error> {
        if let Some(session) = self.repository.load(id)? {
            return Ok(session);
        }
        let session = Session::new(id.clone());
        self.repository.save(&session)?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_text() {
        let request = RunRequest::new(AgentId::parse("agent-1").expect("valid"), "   ");
        let error = request.validate().expect_err("blank text must fail");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
    }

    #[test]
    fn accepts_non_blank_text() {
        let request = RunRequest::new(AgentId::parse("agent-1").expect("valid"), "ping");
        assert!(request.validate().is_ok());
    }
}
