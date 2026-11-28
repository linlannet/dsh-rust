//! 事件序列差分：内存实现与 SQLite 实现必须逐事件一致。
//!
//! 同一输入分别跑两种存储，归一化标识与时间戳后逐事件比对；
//! 这是「回放语义不依赖存储实现」的回归防线。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use dsh_case::{AgentService, CollectSink, RunRegistry, RunRequest, RunnerConfig, ToolRegistry};
use dsh_domain::{
    BoxFuture, EventLog, ModelPort, ModelRequest, ModelStep, SessionRepository, Tool,
};
use dsh_infra::{
    EchoTool, InMemorySessionEventLog, InMemorySessionRepository, SqliteEventLog,
    SqliteSessionRepository, SqliteStore,
};
use dsh_types::{AgentId, Error, JsonValue, SessionEvent, ToolCallRequest};

/// 第一步调用 `echo` 工具，第二步以文本收尾；两次运行的事件形状完全可复现。
struct ToolThenText {
    step: AtomicUsize,
}

impl ModelPort for ToolThenText {
    fn step<'a>(&'a self, _request: ModelRequest) -> BoxFuture<'a, Result<ModelStep, Error>> {
        let index = self.step.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            if index == 0 {
                Ok(ModelStep::ToolCalls {
                    calls: vec![ToolCallRequest {
                        tool_call_id: "call-1".to_owned(),
                        name: "echo".to_owned(),
                        arguments: serde_json::json!({"text": "ping"}),
                    }],
                    text: None,
                })
            } else {
                Ok(ModelStep::Text {
                    delta: "done".to_owned(),
                })
            }
        })
    }

    fn name(&self) -> &'static str {
        "tool-then-text"
    }
}

fn tools() -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::default();
    registry.register(Arc::new(EchoTool) as Arc<dyn Tool>);
    Arc::new(registry)
}

fn build(sessions: Arc<dyn SessionRepository>, events: Arc<dyn EventLog>) -> Arc<AgentService> {
    Arc::new(AgentService::new(
        Arc::new(ToolThenText {
            step: AtomicUsize::new(0),
        }),
        tools(),
        sessions,
        events,
        Arc::new(RunRegistry::default()),
        RunnerConfig::default(),
    ))
}

fn agent() -> AgentId {
    AgentId::parse("agent-1").expect("valid")
}

/// 把运行相关标识与时间戳替换为占位值，只保留结构与业务字段。
fn normalize(events: &[SessionEvent]) -> Vec<JsonValue> {
    events
        .iter()
        .map(|event| {
            let mut value = serde_json::to_value(event).expect("serialize");
            scrub(&mut value);
            value
        })
        .collect()
}

fn scrub(value: &mut JsonValue) {
    match value {
        JsonValue::Object(map) => {
            for (key, item) in map.iter_mut() {
                match key.as_str() {
                    "eventId" | "sessionId" | "runId" | "messageId" | "toolCallId"
                    | "timestamp" => {
                        *item = JsonValue::String("<id>".to_owned());
                    }
                    _ => scrub(item),
                }
            }
        }
        JsonValue::Array(items) => items.iter_mut().for_each(scrub),
        _ => {}
    }
}

/// 临时数据库文件，测试结束自动清理（含 WAL 附属文件）。
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let mut path = std::env::temp_dir();
        path.push(format!("dsh-parity-{name}-{nanos}.db"));
        Self { path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
        }
    }
}

#[tokio::test]
async fn sqlite_and_memory_produce_identical_event_streams() {
    // 内存实现
    let memory_log = Arc::new(InMemorySessionEventLog::default());
    let memory_service = build(
        Arc::new(InMemorySessionRepository::default()),
        Arc::clone(&memory_log) as Arc<dyn EventLog>,
    );
    let memory_outcome = memory_service
        .run(
            RunRequest::new(agent(), "ping"),
            &CollectSink::default(),
            None,
        )
        .await
        .expect("memory run");

    // SQLite 实现
    let db = TempDb::new("parity");
    let store = Arc::new(SqliteStore::open(&db.path).expect("open"));
    let sqlite_log = Arc::new(SqliteEventLog::new(Arc::clone(&store)));
    let sqlite_service = build(
        Arc::new(SqliteSessionRepository::new(Arc::clone(&store))),
        Arc::clone(&sqlite_log) as Arc<dyn EventLog>,
    );
    let sqlite_outcome = sqlite_service
        .run(
            RunRequest::new(agent(), "ping"),
            &CollectSink::default(),
            None,
        )
        .await
        .expect("sqlite run");

    // 运行结论一致
    assert_eq!(memory_outcome.text, sqlite_outcome.text);
    assert_eq!(memory_outcome.finish_reason, sqlite_outcome.finish_reason);
    assert_eq!(memory_outcome.tool_calls, sqlite_outcome.tool_calls);

    let memory_events = memory_log
        .list_after(&memory_outcome.session_id, 0, 1000)
        .expect("memory events");
    let sqlite_events = sqlite_log
        .list_after(&sqlite_outcome.session_id, 0, 1000)
        .expect("sqlite events");

    assert_eq!(
        memory_events.len(),
        sqlite_events.len(),
        "两种实现事件条数一致"
    );
    assert_eq!(
        memory_log
            .last_seq(&memory_outcome.session_id)
            .expect("memory last_seq"),
        sqlite_log
            .last_seq(&sqlite_outcome.session_id)
            .expect("sqlite last_seq"),
        "last_seq 一致"
    );
    assert_eq!(
        normalize(&memory_events),
        normalize(&sqlite_events),
        "归一化后逐事件一致"
    );

    // 关键业务字段抽查，避免归一化掩盖问题
    assert_eq!(memory_events[3].event_type, "tool.requested");
    assert_eq!(memory_events[3].payload["name"], "echo");
    assert_eq!(memory_events[4].payload["isError"], false);
    assert_eq!(memory_events[7].payload["finishReason"], "stop");
}
