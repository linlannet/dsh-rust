//! 运行事件输出端口与内存收集实现。

use std::sync::Mutex;

use dsh_domain::RunEvent;

/// 事件消费者。
pub trait EventSink: Send + Sync {
    /// 发送一个事件。
    fn emit(&self, event: RunEvent);
}

/// 收集到内存的事件槽，主要用于阻塞响应与测试。
#[derive(Debug, Default)]
pub struct CollectSink {
    events: Mutex<Vec<RunEvent>>,
}

impl CollectSink {
    /// 返回已收集事件。
    pub fn events(&self) -> Vec<RunEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }
}

impl EventSink for CollectSink {
    fn emit(&self, event: RunEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}
