use std::collections::HashMap;
use std::sync::Arc;

use dsh_domain::EventLog;
use dsh_types::{Error, ErrorCode, SessionEvent, SessionId};
use rusqlite::params;

use super::{SqliteStore, next_event_id};

/// 基于 SQLite 的会话事件日志。
///
/// `seq` 在同一写事务内按会话取 `MAX(seq)+1`，连接层串行化保证进程内不重号，
/// 库内 `UNIQUE(session_id, seq)` 作为跨进程兜底。
#[derive(Debug)]
pub struct SqliteEventLog {
    store: Arc<SqliteStore>,
}

impl SqliteEventLog {
    /// 使用共享连接句柄构造。
    pub fn new(store: Arc<SqliteStore>) -> Self {
        Self { store }
    }
}

impl EventLog for SqliteEventLog {
    fn append(&self, event: SessionEvent) -> Result<SessionEvent, Error> {
        let payload = serde_json::to_string(&event.payload).map_err(|error| {
            Error::new(ErrorCode::Internal, format!("事件载荷序列化失败: {error}"))
        })?;
        let event_id = next_event_id();

        let seq = self.store.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let next: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM session_event WHERE session_id = ?1",
                params![event.session_id.as_str()],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO session_event (event_id, session_id, seq, type, timestamp, payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    event_id,
                    event.session_id.as_str(),
                    next,
                    event.event_type,
                    event.timestamp,
                    payload
                ],
            )?;
            transaction.commit()?;
            Ok(next)
        })?;

        Ok(SessionEvent {
            event_id,
            seq: seq as u64,
            ..event
        })
    }

    /// 批量追加：整批共用一次事务，`seq` 在同一会话内按入参顺序连续分配。
    fn append_many(&self, events: Vec<SessionEvent>) -> Result<Vec<SessionEvent>, Error> {
        if events.is_empty() {
            return Ok(Vec::new());
        }

        // 序列化放在事务外，事务内只做查询与写入
        let prepared = events
            .iter()
            .map(|event| {
                let payload = serde_json::to_string(&event.payload).map_err(|error| {
                    Error::new(ErrorCode::Internal, format!("事件载荷序列化失败: {error}"))
                })?;
                Ok((next_event_id(), payload))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let seqs = self.store.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut cursors: HashMap<String, i64> = HashMap::new();
            let mut seqs = Vec::with_capacity(prepared.len());
            {
                let mut statement = transaction.prepare(
                    "INSERT INTO session_event
                        (event_id, session_id, seq, type, timestamp, payload_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?;
                for (event, (event_id, payload)) in events.iter().zip(prepared.iter()) {
                    let key = event.session_id.as_str().to_owned();
                    let next = match cursors.get(&key) {
                        Some(previous) => previous + 1,
                        None => {
                            let max: i64 = transaction.query_row(
                                "SELECT COALESCE(MAX(seq), 0) FROM session_event
                                 WHERE session_id = ?1",
                                params![event.session_id.as_str()],
                                |row| row.get(0),
                            )?;
                            max + 1
                        }
                    };
                    cursors.insert(key, next);
                    statement.execute(params![
                        event_id,
                        event.session_id.as_str(),
                        next,
                        event.event_type,
                        event.timestamp,
                        payload
                    ])?;
                    seqs.push(next);
                }
            }
            transaction.commit()?;
            Ok(seqs)
        })?;

        Ok(events
            .into_iter()
            .zip(prepared)
            .zip(seqs)
            .map(|((event, (event_id, _)), seq)| SessionEvent {
                event_id,
                seq: seq as u64,
                ..event
            })
            .collect())
    }

    fn list_after(
        &self,
        session_id: &SessionId,
        after_seq: u64,
        limit: usize,
    ) -> Result<Vec<SessionEvent>, Error> {
        let rows = self.store.with_connection(|connection| {
            connection
                .prepare(
                    "SELECT event_id, seq, type, timestamp, payload_json
                     FROM session_event
                     WHERE session_id = ?1 AND seq > ?2
                     ORDER BY seq ASC
                     LIMIT ?3",
                )?
                .query_map(
                    params![session_id.as_str(), after_seq as i64, limit as i64],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                        ))
                    },
                )?
                .collect::<rusqlite::Result<Vec<_>>>()
        })?;

        rows.into_iter()
            .map(|(event_id, seq, event_type, timestamp, payload)| {
                let payload = serde_json::from_str(&payload).map_err(|error| {
                    Error::new(
                        ErrorCode::StorageUnavailable,
                        format!("事件载荷解析失败: {error}"),
                    )
                })?;
                Ok(SessionEvent {
                    event_id,
                    session_id: session_id.clone(),
                    seq: seq as u64,
                    event_type,
                    timestamp,
                    payload,
                })
            })
            .collect()
    }

    fn last_seq(&self, session_id: &SessionId) -> Result<u64, Error> {
        let seq: i64 = self.store.with_connection(|connection| {
            connection.query_row(
                "SELECT COALESCE(MAX(seq), 0) FROM session_event WHERE session_id = ?1",
                params![session_id.as_str()],
                |row| row.get(0),
            )
        })?;
        Ok(seq as u64)
    }
}
