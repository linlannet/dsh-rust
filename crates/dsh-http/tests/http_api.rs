use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use dsh_case::{AgentService, RunRegistry, RunnerConfig, ToolRegistry};
use dsh_domain::{EventLog, Tool};
use dsh_http::{AppState, build_router};
use dsh_infra::{
    ClockTool, EchoTool, FakeModel, FakeScript, InMemorySessionEventLog, InMemorySessionRepository,
    ReadFileSandbox, ReadFileTool,
};
use dsh_types::{Error, ErrorCode, SessionEvent, SessionId};
use serde_json::Value;
use tower::ServiceExt;

fn test_app(script: FakeScript) -> Router {
    test_app_with_events(script, Arc::new(InMemorySessionEventLog::default()))
}

fn test_app_with_events(script: FakeScript, events: Arc<dyn EventLog>) -> Router {
    let mut registry = ToolRegistry::default();
    registry.register(Arc::new(EchoTool) as Arc<dyn Tool>);
    registry.register(Arc::new(ClockTool) as Arc<dyn Tool>);
    let sandbox = Arc::new(
        ReadFileSandbox::new(std::env::current_dir().expect("cwd"), 4096).expect("sandbox"),
    );
    registry.register(Arc::new(ReadFileTool::new(sandbox)) as Arc<dyn Tool>);
    let service = AgentService::new(
        Arc::new(FakeModel::new(script)),
        Arc::new(registry),
        Arc::new(InMemorySessionRepository::default()),
        events,
        Arc::new(RunRegistry::default()),
        RunnerConfig::default(),
    );
    build_router(AppState::new(Arc::new(service)))
}

async fn send_json(app: Router, uri: &str, body: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .expect("request");
    let response = app.oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get_json(app: Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request");
    let response = app.oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn health_returns_version_and_contract() {
    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .expect("request");
    let response = test_app(FakeScript::Echo)
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], "0.2.1");
    assert_eq!(json["contractVersion"], "1.0.0-draft.1");
}

#[tokio::test]
async fn blocking_message_echoes_text() {
    let (status, json) = send_json(
        test_app(FakeScript::Echo),
        "/api/agent/message",
        r#"{"agentId":"agent-1","content":[{"type":"text","text":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["finishReason"], "stop");
    assert_eq!(json["message"]["role"], "assistant");
    assert_eq!(json["message"]["content"][0]["text"], "echo: hi");
    assert!(json["sessionId"].is_string());
    assert!(json["runId"].is_string());
}

#[tokio::test]
async fn blocking_message_runs_tool() {
    let (status, json) = send_json(
        test_app(FakeScript::Tool),
        "/api/agent/message",
        r#"{"agentId":"agent-1","content":[{"type":"text","text":"time"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["finishReason"], "stop");
    assert_eq!(json["toolCalls"][0]["name"], "clock");
    assert_eq!(json["toolCalls"][0]["status"], "completed");
}

#[tokio::test]
async fn invalid_json_returns_invalid_request() {
    let (status, json) = send_json(test_app(FakeScript::Echo), "/api/agent/message", "{").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "invalid_request");
}

#[tokio::test]
async fn missing_agent_id_returns_invalid_request() {
    let (status, json) = send_json(
        test_app(FakeScript::Echo),
        "/api/agent/message",
        r#"{"content":[{"type":"text","text":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["code"], "invalid_request");
}

#[tokio::test]
async fn cancel_unknown_run_returns_not_found() {
    let (status, json) = send_json(
        test_app(FakeScript::Echo),
        "/api/agent/cancel",
        r#"{"agentId":"agent-1","runId":"run-404"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["code"], "not_found");
}

#[tokio::test]
async fn stream_emits_chunk_and_single_done() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/stream")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"agentId":"agent-1","content":[{"type":"text","text":"hi"}]}"#,
        ))
        .expect("request");
    let response = test_app(FakeScript::Echo)
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let bytes = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("stream timeout")
    .expect("body");
    let text = String::from_utf8(bytes.to_vec()).expect("utf8");
    assert!(text.contains("event: chunk"), "{text}");
    assert_eq!(text.matches("event: done").count(), 1, "{text}");
    assert!(text.contains("finishReason"), "{text}");
}

#[tokio::test]
async fn stream_tool_call_is_three_phase() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/stream")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"agentId":"agent-1","content":[{"type":"text","text":"time"}]}"#,
        ))
        .expect("request");
    let response = test_app(FakeScript::Tool)
        .oneshot(request)
        .await
        .expect("response");
    let bytes = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("stream timeout")
    .expect("body");
    let text = String::from_utf8(bytes.to_vec()).expect("utf8");
    assert_eq!(text.matches("event: tool_call").count(), 2, "{text}");
    assert!(text.contains("\"status\":\"started\""), "{text}");
    assert!(text.contains("\"status\":\"completed\""), "{text}");
    assert_eq!(text.matches("event: done").count(), 1, "{text}");
}

/// 先跑一轮对话，返回 `(app, sessionId)`。
async fn app_with_history() -> (Router, String) {
    let app = test_app(FakeScript::Echo);
    let (status, json) = send_json(
        app.clone(),
        "/api/agent/message",
        r#"{"agentId":"agent-1","content":[{"type":"text","text":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let session_id = json["sessionId"].as_str().expect("session id").to_owned();
    (app, session_id)
}

#[tokio::test]
async fn session_events_replays_recorded_run() {
    let (app, session_id) = app_with_history().await;

    let (status, json) = get_json(app.clone(), &format!("/api/sessions/{session_id}/events")).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["sessionId"], session_id.as_str());

    let events = json["events"].as_array().expect("events");
    let types: Vec<&str> = events
        .iter()
        .map(|event| event["type"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        types,
        vec![
            "session.created",
            "message.appended",
            "run.started",
            "message.appended",
            "run.completed",
        ]
    );
    assert_eq!(json["lastSeq"], 5, "lastSeq 为本次最后一条事件 seq: {json}");

    let (status, page) = get_json(
        app,
        &format!("/api/sessions/{session_id}/events?afterSeq=2&limit=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let seqs: Vec<u64> = page["events"]
        .as_array()
        .expect("events")
        .iter()
        .map(|event| event["seq"].as_u64().expect("seq"))
        .collect();
    assert_eq!(seqs, vec![3, 4], "afterSeq 与 limit 生效");
    assert_eq!(page["lastSeq"], 4);
}

#[tokio::test]
async fn session_events_rejects_bad_paging() {
    let (app, session_id) = app_with_history().await;
    for query in ["limit=0", "limit=1001", "afterSeq=-1"] {
        let (status, json) = get_json(
            app.clone(),
            &format!("/api/sessions/{session_id}/events?{query}"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {json}");
        assert_eq!(json["code"], "invalid_request", "{query}: {json}");
    }
}

#[tokio::test]
async fn session_events_missing_session_returns_not_found() {
    let (status, json) = get_json(
        test_app(FakeScript::Echo),
        "/api/sessions/sess-absent/events",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
    assert_eq!(json["code"], "not_found");
}

#[tokio::test]
async fn session_events_pages_without_gaps_or_duplicates() {
    let (app, session_id) = app_with_history().await;

    let mut after = 0;
    let mut seqs: Vec<u64> = Vec::new();
    let mut last_seq = 0;
    for _ in 0..3 {
        let (status, page) = get_json(
            app.clone(),
            &format!("/api/sessions/{session_id}/events?afterSeq={after}&limit=2"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        let batch: Vec<u64> = page["events"]
            .as_array()
            .expect("events")
            .iter()
            .map(|event| event["seq"].as_u64().expect("seq"))
            .collect();
        assert!(batch.len() <= 2, "limit 生效");
        if let Some(last) = batch.last() {
            after = *last;
        }
        last_seq = page["lastSeq"].as_u64().expect("lastSeq");
        seqs.extend(batch);
    }

    assert_eq!(seqs, vec![1, 2, 3, 4, 5], "3 次翻页无重复无遗漏");
    assert_eq!(last_seq, 5);
}

#[tokio::test]
async fn session_events_reports_storage_unavailable() {
    /// 写入正常、读取失败的日志，模拟运行期存储故障。
    struct ReadFailingLog {
        inner: InMemorySessionEventLog,
    }

    impl EventLog for ReadFailingLog {
        fn append(&self, event: SessionEvent) -> Result<SessionEvent, Error> {
            self.inner.append(event)
        }

        fn append_many(&self, events: Vec<SessionEvent>) -> Result<Vec<SessionEvent>, Error> {
            self.inner.append_many(events)
        }

        fn list_after(
            &self,
            _session_id: &SessionId,
            _after_seq: u64,
            _limit: usize,
        ) -> Result<Vec<SessionEvent>, Error> {
            Err(Error::new(
                ErrorCode::StorageUnavailable,
                "event log read failed",
            ))
        }

        fn last_seq(&self, session_id: &SessionId) -> Result<u64, Error> {
            self.inner.last_seq(session_id)
        }
    }

    let app = test_app_with_events(
        FakeScript::Echo,
        Arc::new(ReadFailingLog {
            inner: InMemorySessionEventLog::default(),
        }),
    );
    let (status, json) = send_json(
        app.clone(),
        "/api/agent/message",
        r#"{"agentId":"agent-1","content":[{"type":"text","text":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let session_id = json["sessionId"].as_str().expect("session id");

    let (status, json) = get_json(app, &format!("/api/sessions/{session_id}/events")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{json}");
    assert_eq!(json["code"], "storage_unavailable", "不得返回空数组");
}
