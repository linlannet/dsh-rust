use dsh_types::{Error, SessionId};

use crate::Session;

/// 会话持久化端口。
///
/// 当前使用同步签名，便于先跑通用例与内存实现。接入异步存储时，
/// 在 `dsh-infra` 边界使用阻塞桥接，或统一改为异步 trait。
pub trait SessionRepository {
    /// 读取会话；不存在返回 `Ok(None)`。
    fn load(&self, id: &SessionId) -> Result<Option<Session>, Error>;

    /// 保存会话，已存在则覆盖。
    fn save(&self, session: &Session) -> Result<(), Error>;
}
