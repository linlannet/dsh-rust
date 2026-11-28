//! 同步存储端口的异步桥接。
//!
//! `dsh-domain` 的 [`SessionRepository`](dsh_domain::SessionRepository) 与
//! [`EventLog`](dsh_domain::EventLog) 保持同步签名，避免领域层依赖 tokio；
//! 异步化统一在用例层用 `spawn_blocking` 完成，与 `execute_tool` 的处理方式一致。

use dsh_types::{Error, ErrorCode};

/// 在阻塞线程池上执行一次同步存储调用。
///
/// 不使用 `block_in_place`：后者在单线程运行时下会 panic，且阻塞调度器线程。
pub(crate) async fn block_on_store<T, F>(action: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    match tokio::task::spawn_blocking(action).await {
        Ok(result) => result,
        Err(error) => Err(Error::new(
            ErrorCode::Internal,
            format!("存储任务执行失败: {error}"),
        )),
    }
}
