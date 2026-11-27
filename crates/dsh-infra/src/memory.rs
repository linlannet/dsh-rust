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
