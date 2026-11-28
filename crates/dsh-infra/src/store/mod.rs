//! SQLite 持久化适配器。
//!
//! 建表语句来自 `contracts/v1/persistence/migrations`，不在代码内联 DDL。
//! 所有访问经 [`SqliteStore`] 串行化，规避 SQLite 单写者限制。

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use dsh_types::{Error, ErrorCode};
use rusqlite::Connection;

mod event_log;
mod migration;
mod session;

pub use event_log::SqliteEventLog;
pub use session::SqliteSessionRepository;

/// 事件标识去重计数器，避免同纳秒内重复。
static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// SQLite 连接持有者，由仓储与事件日志共享。
#[derive(Debug)]
pub struct SqliteStore {
    connection: Mutex<Connection>,
}

impl SqliteStore {
    /// 打开（不存在则创建）数据库文件并执行迁移。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    Error::new(
                        ErrorCode::StorageUnavailable,
                        format!("创建数据库目录失败: {error}"),
                    )
                })?;
            }
        }
        let connection = Connection::open(path).map_err(map_sqlite_error)?;
        Self::initialize(connection)
    }

    /// 打开内存数据库，仅用于测试。
    pub fn open_in_memory() -> Result<Self, Error> {
        let connection = Connection::open_in_memory().map_err(map_sqlite_error)?;
        Self::initialize(connection)
    }

    fn initialize(connection: Connection) -> Result<Self, Error> {
        connection
            .busy_timeout(std::time::Duration::from_millis(5_000))
            .map_err(map_sqlite_error)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(map_sqlite_error)?;
        // 内存库不支持 WAL，此处失败可忽略
        let _ = connection.pragma_update(None, "journal_mode", "WAL");

        let mut connection = connection;
        migration::apply(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// 在串行化连接上执行闭包。
    pub(crate) fn with_connection<T>(
        &self,
        action: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, Error> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| Error::new(ErrorCode::Internal, "数据库连接锁已中毒"))?;
        action(&connection).map_err(map_sqlite_error)
    }
}

pub(crate) fn map_sqlite_error(error: rusqlite::Error) -> Error {
    Error::new(
        ErrorCode::StorageUnavailable,
        format!("存储访问失败: {error}"),
    )
}

/// 生成事件标识。
pub(crate) fn next_event_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let counter = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("evt-{nanos}-{counter}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use dsh_domain::{ChatMessage, EventLog, Role, Session, SessionRepository};
    use dsh_types::{SessionEvent, SessionId, event_type};
    use rusqlite::params;

    use super::*;

    /// 临时数据库文件，测试结束自动清理（含 WAL 附属文件）。
    struct TempDb {
        path: PathBuf,
    }

    impl TempDb {
        fn new(name: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!("dsh-infra-{name}-{}.db", next_event_id()));
            Self { path }
        }

        fn store(&self) -> Arc<SqliteStore> {
            Arc::new(SqliteStore::open(&self.path).expect("open store"))
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
            }
        }
    }

    fn session_id(value: &str) -> SessionId {
        SessionId::parse(value).expect("valid identifier")
    }

    fn pending(kind: &str) -> SessionEvent {
        SessionEvent::pending(session_id("sess-1"), kind, "2026-11-28T00:00:00Z")
    }

    #[test]
    fn migration_is_idempotent() {
        let db = TempDb::new("migration");
        {
            let _ = db.store();
        }
        // 第二次打开：迁移重复执行不报错，且不丢数据
        let store = db.store();
        let session = Session::new(session_id("sess-1"));
        SqliteSessionRepository::new(store.clone())
            .save(&session)
            .expect("save");

        drop(store);
        let reopened = SqliteSessionRepository::new(db.store());
        assert!(
            reopened
                .load(&session_id("sess-1"))
                .expect("load")
                .is_some()
        );
    }

    #[test]
    fn round_trips_session_across_reopen() {
        let db = TempDb::new("session-round-trip");
        let id = session_id("sess-1");
        {
            let repository = SqliteSessionRepository::new(db.store());
            let mut session = Session::new(id.clone());
            session.title = Some("演示".to_owned());
            session.messages = vec![
                ChatMessage::text(Role::User, "统计 rust go rust"),
                ChatMessage::text(Role::Assistant, "rust: 2，go: 1"),
            ];
            repository.save(&session).expect("save");
        }

        let repository = SqliteSessionRepository::new(db.store());
        let loaded = repository.load(&id).expect("load").expect("present");
        assert_eq!(loaded.title.as_deref(), Some("演示"));
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].role, Role::User);
        assert_eq!(loaded.messages[0].as_text(), "统计 rust go rust");
        assert_eq!(loaded.messages[1].role, Role::Assistant);
    }

    #[test]
    fn load_missing_session_returns_none() {
        let db = TempDb::new("session-missing");
        let repository = SqliteSessionRepository::new(db.store());
        assert!(
            repository
                .load(&session_id("sess-absent"))
                .expect("load")
                .is_none()
        );
    }

    #[test]
    fn save_replaces_previous_messages() {
        let db = TempDb::new("session-replace");
        let repository = SqliteSessionRepository::new(db.store());
        let id = session_id("sess-1");

        let mut session = Session::new(id.clone());
        session.messages = vec![
            ChatMessage::text(Role::User, "first"),
            ChatMessage::text(Role::Assistant, "answer"),
        ];
        repository.save(&session).expect("first save");

        session.messages = vec![ChatMessage::text(Role::User, "only")];
        repository.save(&session).expect("second save");

        let loaded = repository.load(&id).expect("load").expect("present");
        assert_eq!(loaded.messages.len(), 1, "旧消息不应残留");
        assert_eq!(loaded.messages[0].as_text(), "only");
    }

    #[test]
    fn event_log_keeps_sequence_across_reopen() {
        let db = TempDb::new("event-seq");
        {
            let log = SqliteEventLog::new(db.store());
            for _ in 0..3 {
                log.append(pending(event_type::RUN_STARTED))
                    .expect("append");
            }
            assert_eq!(log.last_seq(&session_id("sess-1")).expect("last_seq"), 3);
        }

        let log = SqliteEventLog::new(db.store());
        let stored = log
            .append(pending(event_type::RUN_COMPLETED))
            .expect("append");
        assert_eq!(stored.seq, 4, "重启后 seq 接续历史最大值");
    }

    #[test]
    fn list_after_pages_and_isolates_sessions() {
        let db = TempDb::new("event-page");
        let log = SqliteEventLog::new(db.store());
        let other = SessionId::parse("sess-2").expect("valid identifier");

        for _ in 0..3 {
            log.append(pending(event_type::TOOL_REQUESTED))
                .expect("append");
        }
        log.append(SessionEvent::pending(
            other.clone(),
            event_type::TOOL_REQUESTED,
            "2026-11-28T00:00:00Z",
        ))
        .expect("append other");

        let first = log.list_after(&session_id("sess-1"), 0, 2).expect("page 1");
        let second = log.list_after(&session_id("sess-1"), 2, 2).expect("page 2");
        let isolated = log.list_after(&other, 0, 10).expect("other");

        assert_eq!(
            first.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            second.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(isolated.len(), 1, "不同会话互不干扰");
        assert_eq!(isolated[0].seq, 1);
    }

    #[test]
    fn append_assigns_authoritative_identity() {
        let db = TempDb::new("event-identity");
        let log = SqliteEventLog::new(db.store());
        let mut event = pending(event_type::SESSION_CREATED);
        event.seq = 42;
        event.event_id = "caller-supplied".to_owned();

        let stored = log.append(event).expect("append");
        assert_eq!(stored.seq, 1, "seq 以日志分配为准");
        assert_ne!(stored.event_id, "caller-supplied");
        assert!(stored.event_id.starts_with("evt-"));
    }

    #[test]
    fn rejects_duplicate_sequence_for_same_session() {
        let db = TempDb::new("event-duplicate");
        let store = db.store();
        let log = SqliteEventLog::new(store.clone());
        log.append(pending(event_type::RUN_STARTED))
            .expect("append");

        let result = store.with_connection(|connection| {
            connection.execute(
                "INSERT INTO session_event (event_id, session_id, seq, type, timestamp, payload_json)
                 VALUES ('evt-dup', 'sess-1', 1, 'run.started', '2026-11-28T00:00:00Z', '{}')",
                [],
            )
        });
        assert!(result.is_err(), "UNIQUE(session_id, seq) 必须拦截重复序号");
    }

    #[test]
    fn batch_append_allocates_contiguous_sequences() {
        let db = TempDb::new("event-batch");
        let log = SqliteEventLog::new(db.store());

        let batch = log
            .append_many(vec![
                pending(event_type::SESSION_CREATED),
                pending(event_type::MESSAGE_APPENDED),
                pending(event_type::RUN_STARTED),
            ])
            .expect("append_many");

        assert_eq!(
            batch.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "单事务内按入参顺序连续分配"
        );
        assert_eq!(
            batch
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec![
                event_type::SESSION_CREATED,
                event_type::MESSAGE_APPENDED,
                event_type::RUN_STARTED
            ]
        );
        assert_eq!(log.last_seq(&session_id("sess-1")).expect("last_seq"), 3);

        let next = log
            .append_many(vec![pending(event_type::RUN_COMPLETED)])
            .expect("append_many again");
        assert_eq!(next[0].seq, 4, "批量与单条共用同一 seq 序列");
    }

    #[test]
    fn concurrent_appends_keep_sequence_unique() {
        let db = TempDb::new("event-concurrency");
        let log = Arc::new(SqliteEventLog::new(db.store()));

        let mut handles = Vec::new();
        for _ in 0..4 {
            let log = Arc::clone(&log);
            handles.push(std::thread::spawn(move || {
                for _ in 0..10 {
                    log.append(pending(event_type::RUN_STARTED))
                        .expect("append");
                }
            }));
        }
        for handle in handles {
            handle.join().expect("join");
        }

        let events = log
            .list_after(&session_id("sess-1"), 0, 100)
            .expect("list_after");
        assert_eq!(events.len(), 40, "并发写入不丢事件");
        assert_eq!(
            events.iter().map(|event| event.seq).collect::<Vec<_>>(),
            (1..=40).collect::<Vec<_>>(),
            "并发写入不重号、无空洞"
        );
    }

    #[test]
    fn duplicate_message_ordinal_rolls_back_transaction() {
        let db = TempDb::new("message-ordinal");
        let store = db.store();
        let id = session_id("sess-1");

        let mut session = Session::new(id.clone());
        session.messages.push(ChatMessage::text(Role::User, "a"));
        session.messages.push(ChatMessage::text(Role::User, "b"));
        SqliteSessionRepository::new(Arc::clone(&store))
            .save(&session)
            .expect("save");

        let result = store.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            // ordinal 1 已存在，唯一约束应让整个事务失败
            transaction.execute(
                "INSERT INTO session_message (session_id, ordinal, role, content, created_at)
                 VALUES (?1, 1, 'user', 'dup', '2026-11-28T00:00:00Z')",
                params![id.as_str()],
            )?;
            transaction.execute(
                "INSERT INTO session_message (session_id, ordinal, role, content, created_at)
                 VALUES (?1, 2, 'user', 'new', '2026-11-28T00:00:00Z')",
                params![id.as_str()],
            )?;
            transaction.commit()?;
            Ok(())
        });
        assert!(result.is_err(), "重复 ordinal 必须被唯一约束拦截");

        let reloaded = SqliteSessionRepository::new(store)
            .load(&id)
            .expect("load")
            .expect("present");
        assert_eq!(reloaded.messages.len(), 2, "失败事务不得留下脏数据");
    }
}
