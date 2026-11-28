use dsh_types::{Error, SessionEvent, SessionId};

use crate::Session;

/// 会话持久化端口。
///
/// 当前使用同步签名，便于先跑通用例与内存实现。接入异步存储时，
/// 在 `dsh-infra` 边界使用阻塞桥接，或统一改为异步 trait。
pub trait SessionRepository: Send + Sync {
    /// 读取会话；不存在返回 `Ok(None)`。
    fn load(&self, id: &SessionId) -> Result<Option<Session>, Error>;

    /// 保存会话，已存在则覆盖。
    fn save(&self, session: &Session) -> Result<(), Error>;
}

/// 会话事件日志端口（只追加）。
///
/// 与 [`SessionRepository`] 一致保持同步签名，异步化由 `dsh-case` 的阻塞桥接承担，
/// 避免领域层依赖 tokio。
pub trait EventLog: Send + Sync {
    /// 追加事件并分配 `eventId` 与 `seq`，返回分配后的完整事件。
    ///
    /// 入参中的 `eventId` 与 `seq` 会被忽略——以日志分配的结果为准。
    fn append(&self, event: SessionEvent) -> Result<SessionEvent, Error>;

    /// 批量追加事件，返回按 `seq` 分配的完整事件（顺序与入参一致）。
    ///
    /// 默认实现逐条调用 [`EventLog::append`]；持久化实现应覆盖为单事务写入，
    /// 以降低 ReAct 循环的写放大。
    fn append_many(&self, events: Vec<SessionEvent>) -> Result<Vec<SessionEvent>, Error> {
        events.into_iter().map(|event| self.append(event)).collect()
    }

    /// 按 `seq` 升序读取 `after_seq` 之后的至多 `limit` 条事件。
    fn list_after(
        &self,
        session_id: &SessionId,
        after_seq: u64,
        limit: usize,
    ) -> Result<Vec<SessionEvent>, Error>;

    /// 读取会话当前最大 `seq`；无事件时返回 0。
    fn last_seq(&self, session_id: &SessionId) -> Result<u64, Error>;
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use dsh_types::event_type;

    use super::*;

    /// 最小实现，用于确认端口可被实现且可作为 trait object 注入。
    #[derive(Default)]
    struct MemoryLog {
        events: Mutex<Vec<SessionEvent>>,
    }

    impl EventLog for MemoryLog {
        fn append(&self, event: SessionEvent) -> Result<SessionEvent, Error> {
            let mut events = self.events.lock().expect("lock");
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

        fn list_after(
            &self,
            session_id: &SessionId,
            after_seq: u64,
            limit: usize,
        ) -> Result<Vec<SessionEvent>, Error> {
            let events = self.events.lock().expect("lock");
            Ok(events
                .iter()
                .filter(|stored| stored.session_id == *session_id && stored.seq > after_seq)
                .take(limit)
                .cloned()
                .collect())
        }

        fn last_seq(&self, session_id: &SessionId) -> Result<u64, Error> {
            let events = self.events.lock().expect("lock");
            Ok(events
                .iter()
                .filter(|stored| stored.session_id == *session_id)
                .map(|stored| stored.seq)
                .max()
                .unwrap_or(0))
        }
    }

    fn session_id() -> SessionId {
        SessionId::parse("sess-1").expect("valid id")
    }

    fn pending(kind: &str) -> SessionEvent {
        SessionEvent::pending(session_id(), kind, "2026-11-28T00:00:00Z")
    }

    #[test]
    fn event_log_is_usable_as_trait_object() {
        let log: Box<dyn EventLog> = Box::new(MemoryLog::default());
        assert_eq!(log.last_seq(&session_id()).expect("last_seq"), 0);

        let stored = log
            .append(pending(event_type::MESSAGE_APPENDED))
            .expect("append");
        assert_eq!(stored.seq, 1, "seq 从 1 开始");
        assert_eq!(stored.event_id, "evt-1");
        assert_eq!(log.last_seq(&session_id()).expect("last_seq"), 1);
    }

    #[test]
    fn append_ignores_caller_supplied_identity() {
        let log = MemoryLog::default();
        let mut event = pending(event_type::RUN_STARTED);
        event.seq = 99;
        event.event_id = "caller-supplied".to_string();

        let stored = log.append(event).expect("append");
        assert_eq!(stored.seq, 1, "以日志分配为准");
        assert_eq!(stored.event_id, "evt-1");
    }

    #[test]
    fn list_after_pages_without_gaps() {
        let log = MemoryLog::default();
        for _ in 0..3 {
            log.append(pending(event_type::RUN_STARTED))
                .expect("append");
        }

        let first = log.list_after(&session_id(), 0, 2).expect("page 1");
        let second = log.list_after(&session_id(), 2, 2).expect("page 2");
        assert_eq!(
            first.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            second.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![3]
        );
    }
}
