use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use dsh_case::{AgentService, RunRegistry, RunnerConfig, ToolRegistry};
use dsh_domain::Tool;
use dsh_http::{AppState, build_router};
use dsh_infra::{
    ClockTool, EchoTool, FakeModel, FakeScript, InMemorySessionRepository, ReadFileSandbox,
    ReadFileTool,
};
use serde_json::Value;
use tower::ServiceExt;

fn test_app(script: FakeScript) -> Router {
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
    assert_eq!(json["version"], "0.2.0");
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
