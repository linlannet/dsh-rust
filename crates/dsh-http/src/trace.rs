//! 请求追踪 ID。

use std::sync::atomic::{AtomicU64, Ordering};

/// 生成单进程唯一的 trace ID。
pub fn new_trace_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("trace-{millis}-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}
