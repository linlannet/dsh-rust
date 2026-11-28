use std::collections::HashMap;
use std::sync::Mutex;

use dsh_domain::{EventLog, Session, SessionRepository};
use dsh_types::{Error, ErrorCode, SessionEvent, SessionId};

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

/// 进程内会话事件日志，用于测试与 `DSH_STORE=memory` 回滚通道。
///
/// 与 [`InMemoryRunEventLog`] 区分：后者是单次运行的 SSE 事件广播，
/// 本类型对应领域端口 [`EventLog`]，即跨会话可回放的溯源日志。
#[derive(Debug, Default)]
pub struct InMemorySessionEventLog {
    events: Mutex<Vec<SessionEvent>>,
}

impl InMemorySessionEventLog {
    /// 当前事件总数。
    pub fn len(&self) -> usize {
        self.events.lock().map(|events| events.len()).unwrap_or(0)
    }

    /// 是否没有任何事件。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl EventLog for InMemorySessionEventLog {
    fn append(&self, event: SessionEvent) -> Result<SessionEvent, Error> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "event log lock poisoned"))?;
        let seq = events
            .iter()
            .filter(|stored| stored.session_id == event.session_id)
            .map(|stored| stored.seq)
            .max()
            .unwrap_or(0)
            + 1;
        let stored = SessionEvent {
            event_id: format!("evt-{seq}"),
            seq,
            ..event
        };
        events.push(stored.clone());
        Ok(stored)
    }

    fn append_many(&self, events: Vec<SessionEvent>) -> Result<Vec<SessionEvent>, Error> {
        let mut stored_events = self
            .events
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "event log lock poisoned"))?;
        let mut stored = Vec::with_capacity(events.len());
        for event in events {
            let seq = stored_events
                .iter()
                .chain(stored.iter())
                .filter(|existing| existing.session_id == event.session_id)
                .map(|existing| existing.seq)
                .max()
                .unwrap_or(0)
                + 1;
            let assigned = SessionEvent {
                event_id: format!("evt-{seq}"),
                seq,
                ..event
            };
            stored.push(assigned);
        }
        stored_events.extend(stored.iter().cloned());
        Ok(stored)
    }

    fn list_after(
        &self,
        session_id: &SessionId,
        after_seq: u64,
        limit: usize,
    ) -> Result<Vec<SessionEvent>, Error> {
        let events = self
            .events
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "event log lock poisoned"))?;
        Ok(events
            .iter()
            .filter(|stored| stored.session_id == *session_id && stored.seq > after_seq)
            .take(limit)
            .cloned()
            .collect())
    }

    fn last_seq(&self, session_id: &SessionId) -> Result<u64, Error> {
        let events = self
            .events
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "event log lock poisoned"))?;
        Ok(events
            .iter()
            .filter(|stored| stored.session_id == *session_id)
            .map(|stored| stored.seq)
            .max()
            .unwrap_or(0))
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
