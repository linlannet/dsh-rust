//! 重启续跑：同一 SQLite 库上重建服务后，会话历史与事件 seq 都能接续。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use dsh_case::{AgentService, CollectSink, RunRegistry, RunRequest, RunnerConfig, ToolRegistry};
use dsh_domain::{BoxFuture, EventLog, ModelPort, ModelRequest, ModelStep, SessionRepository};
use dsh_infra::{SqliteEventLog, SqliteSessionRepository, SqliteStore};
use dsh_types::{AgentId, ContentBlock, Error, SessionId};

/// 返回历史消息条数的模型，用于验证续跑时模型确实看到了首轮内容。
struct HistoryModel {
    seen: Mutex<Vec<usize>>,
}

impl HistoryModel {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            seen: Mutex::new(Vec::new()),
        })
    }
}

impl ModelPort for HistoryModel {
    fn step<'a>(&'a self, request: ModelRequest) -> BoxFuture<'a, Result<ModelStep, Error>> {
        let count = request.messages.len();
        self.seen.lock().expect("lock").push(count);
        Box::pin(async move {
            Ok(ModelStep::Text {
                delta: format!("messages={count}"),
            })
        })
    }

    fn name(&self) -> &'static str {
        "history"
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
        path.push(format!("dsh-restart-{name}-{nanos}.db"));
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

fn build_service(store: &Arc<SqliteStore>, model: Arc<dyn ModelPort>) -> Arc<AgentService> {
    Arc::new(AgentService::new(
        model,
        Arc::new(ToolRegistry::default()),
        Arc::new(SqliteSessionRepository::new(Arc::clone(store))),
        Arc::new(SqliteEventLog::new(Arc::clone(store))),
        Arc::new(RunRegistry::default()),
        RunnerConfig::default(),
    ))
}

#[tokio::test]
async fn restart_reuses_history_and_event_sequence() {
    let db = TempDb::new("continuation");
    let agent = AgentId::parse("agent-1").expect("valid");
    let session_id: SessionId;
    let first_text: String;
    let first_last_seq: u64;

    // 第一轮：写入会话投影与事件后关闭连接，模拟进程退出
    {
        let store = Arc::new(SqliteStore::open(&db.path).expect("open"));
        let model = HistoryModel::new();
        let service = build_service(&store, model.clone());

        let outcome = service
            .run(
                RunRequest::new(agent.clone(), "one"),
                &CollectSink::default(),
                None,
            )
            .await
            .expect("first run");
        assert_eq!(outcome.text, "messages=1");
        assert_eq!(*model.seen.lock().expect("lock"), vec![1]);

        session_id = outcome.session_id.clone();
        first_text = outcome.text.clone();
        first_last_seq = SqliteEventLog::new(Arc::clone(&store))
            .last_seq(&session_id)
            .expect("last seq");
        assert_eq!(first_last_seq, 5, "新会话首轮应落 5 条事件");
    }

    // 重启：新连接、新服务实例，复用同一 sessionId
    let store = Arc::new(SqliteStore::open(&db.path).expect("reopen"));
    let model = HistoryModel::new();
    let service = build_service(&store, model.clone());

    let outcome = service
        .run(
            RunRequest::with_content(
                agent.clone(),
                Some(session_id.clone()),
                vec![ContentBlock::text("two")],
            ),
            &CollectSink::default(),
            None,
        )
        .await
        .expect("second run");

    assert_eq!(
        outcome.text, "messages=3",
        "重启后续跑应带上首轮 user + assistant 消息"
    );
    assert_eq!(*model.seen.lock().expect("lock"), vec![3]);

    let session = SqliteSessionRepository::new(Arc::clone(&store))
        .load(&session_id)
        .expect("load")
        .expect("present");
    assert_eq!(session.messages.len(), 4);
    assert_eq!(session.messages[0].as_text(), "one");
    assert_eq!(session.messages[1].as_text(), first_text);
    assert_eq!(session.messages[2].as_text(), "two");

    // 事件 seq 跨重启接续：第二轮从 6 开始，共 4 条（无 session.created）
    let log = SqliteEventLog::new(Arc::clone(&store));
    assert_eq!(
        log.last_seq(&session_id).expect("last seq"),
        first_last_seq + 4
    );
    let appended = log
        .list_after(&session_id, first_last_seq, 100)
        .expect("list after");
    let appended_types: Vec<&str> = appended
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    assert_eq!(
        appended_types,
        vec![
            "message.appended",
            "run.started",
            "message.appended",
            "run.completed"
        ]
    );
    assert_eq!(appended[0].seq, first_last_seq + 1, "seq 顺序接续不跳号");
}
