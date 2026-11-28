use std::collections::HashMap;
use std::sync::Mutex;

use dsh_domain::{Session, SessionRepository};
use dsh_types::{Error, ErrorCode, SessionId};

/// 进程内会话仓库，仅用于骨架与测试，不提供持久化。
#[derive(Debug, Default)]
pub struct InMemorySessionRepository {
    sessions: Mutex<HashMap<SessionId, Session>>,
}

impl InMemorySessionRepository {
    /// 当前是否没有任何会话。
    pub fn is_empty(&self) -> bool {
        self.sessions
            .lock()
            .is_ok_and(|sessions| sessions.is_empty())
    }
}

impl SessionRepository for InMemorySessionRepository {
    fn load(&self, id: &SessionId) -> Result<Option<Session>, Error> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "session store lock poisoned"))?;
        Ok(sessions.get(id).cloned())
    }

    fn save(&self, session: &Session) -> Result<(), Error> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "session store lock poisoned"))?;
        sessions.insert(session.id.clone(), session.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_session() {
        let repository = InMemorySessionRepository::default();
        let id = SessionId::parse("sess-1").expect("valid identifier");
        assert!(repository.load(&id).expect("load").is_none());

        let mut session = Session::new(id.clone());
        session.title = Some("demo".to_owned());
        repository.save(&session).expect("save");

        assert_eq!(repository.load(&id).expect("load"), Some(session));
        assert!(!repository.is_empty());
    }
}

/// 进程内运行事件日志，可按运行导出 NDJSON。
#[derive(Debug, Default)]
pub struct InMemoryRunEventLog {
    events: Mutex<HashMap<dsh_types::RunId, Vec<dsh_domain::RunEvent>>>,
}

impl InMemoryRunEventLog {
    /// 追加事件。
    pub fn append(&self, run_id: &dsh_types::RunId, event: dsh_domain::RunEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.entry(run_id.clone()).or_default().push(event);
        }
    }

    /// 返回某运行的事件。
    pub fn events(&self, run_id: &dsh_types::RunId) -> Vec<dsh_domain::RunEvent> {
        self.events
            .lock()
            .ok()
            .and_then(|events| events.get(run_id).cloned())
            .unwrap_or_default()
    }

    /// 导出事件名序列，供契约回放比对（不含 seq）。
    pub fn event_names(&self, run_id: &dsh_types::RunId) -> Vec<&'static str> {
        self.events
            .lock()
            .ok()
            .and_then(|events| {
                events.get(run_id).map(|events| {
                    events
                        .iter()
                        .map(|event| match event {
                            dsh_domain::RunEvent::Chunk { .. } => "chunk",
                            dsh_domain::RunEvent::ToolCall { .. } => "tool_call",
                            dsh_domain::RunEvent::Done { .. } => "done",
                            dsh_domain::RunEvent::Error { .. } => "error",
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }
}
