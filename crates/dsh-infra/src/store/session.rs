use std::sync::Arc;

use dsh_domain::{ChatMessage, Role, Session, SessionRepository};
use dsh_types::{ContentBlock, Error, ErrorCode, RunId, SessionId, now_rfc3339};
use rusqlite::{OptionalExtension, params};

use super::SqliteStore;

/// 基于 SQLite 的会话仓库。
///
/// 保存语义为「整会话覆盖」：会话行 upsert，消息行先删后插，保证与内存实现一致。
#[derive(Debug)]
pub struct SqliteSessionRepository {
    store: Arc<SqliteStore>,
}

impl SqliteSessionRepository {
    /// 使用共享连接句柄构造。
    pub fn new(store: Arc<SqliteStore>) -> Self {
        Self { store }
    }
}

impl SessionRepository for SqliteSessionRepository {
    fn load(&self, id: &SessionId) -> Result<Option<Session>, Error> {
        let raw = self.store.with_connection(|connection| {
            let head = connection
                .prepare("SELECT title, active_run_id FROM session WHERE id = ?1")?
                .query_row([id.as_str()], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                })
                .optional()?;
            let Some((title, active_run_id)) = head else {
                return Ok(None);
            };

            let messages = connection
                .prepare(
                    "SELECT role, content FROM session_message WHERE session_id = ?1 ORDER BY ordinal",
                )?
                .query_map([id.as_str()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            Ok(Some((title, active_run_id, messages)))
        })?;

        let Some((title, active_run_id, raw_messages)) = raw else {
            return Ok(None);
        };

        let messages = raw_messages
            .iter()
            .map(|(role, content)| decode_message(role, content))
            .collect::<Result<Vec<_>, _>>()?;

        let active_run = match active_run_id {
            Some(value) => Some(RunId::parse(value).map_err(|error| {
                Error::new(
                    ErrorCode::StorageUnavailable,
                    format!("会话 active_run_id 非法: {error}"),
                )
            })?),
            None => None,
        };

        Ok(Some(Session {
            id: id.clone(),
            title,
            messages,
            active_run,
        }))
    }

    fn save(&self, session: &Session) -> Result<(), Error> {
        let encoded = session
            .messages
            .iter()
            .map(|message| {
                let content = serde_json::to_string(&message.content).map_err(|error| {
                    Error::new(ErrorCode::Internal, format!("消息内容序列化失败: {error}"))
                })?;
                Ok((encode_role(message.role), content))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let timestamp = now_rfc3339();
        let active_run = session.active_run.as_ref().map(|id| id.as_str().to_owned());
        let session_id = session.id.as_str().to_owned();

        self.store.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO session (id, title, active_run_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                     title = excluded.title,
                     active_run_id = excluded.active_run_id,
                     updated_at = excluded.updated_at",
                params![session_id, session.title, active_run, timestamp],
            )?;
            transaction.execute(
                "DELETE FROM session_message WHERE session_id = ?1",
                params![session_id],
            )?;
            {
                let mut statement = transaction.prepare(
                    "INSERT INTO session_message
                        (session_id, ordinal, message_id, role, content, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?;
                for (index, (role, content)) in encoded.iter().enumerate() {
                    let ordinal = (index + 1) as i64;
                    statement.execute(params![
                        session_id,
                        ordinal,
                        format!("{session_id}-msg-{ordinal}"),
                        role,
                        content,
                        timestamp
                    ])?;
                }
            }
            transaction.commit()
        })
    }
}

fn encode_role(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn decode_message(role: &str, content: &str) -> Result<ChatMessage, Error> {
    let role = match role {
        "system" => Role::System,
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        unknown => {
            return Err(Error::new(
                ErrorCode::StorageUnavailable,
                format!("未知消息角色: {unknown}"),
            ));
        }
    };
    let content: Vec<ContentBlock> = serde_json::from_str(content).map_err(|error| {
        Error::new(
            ErrorCode::StorageUnavailable,
            format!("消息内容解析失败: {error}"),
        )
    })?;
    Ok(ChatMessage { role, content })
}
