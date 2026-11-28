//! 契约回放：用 `contracts/fixtures` 驱动运行时并比对归一化结果。
//!
//! 覆盖 0.2.0 新增的 4 组样例（阻塞工具调用、流式文本、流式工具调用、取消）；
//! 既有手写样例（001）依赖 word_count 工具，不在本版回放范围。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use dsh_case::{AgentService, RunRegistry, RunnerConfig, ToolRegistry};
use dsh_domain::{BoxFuture, ModelPort, ModelRequest, ModelStep, Tool};
use dsh_http::{AppState, build_router};
use dsh_infra::{
    ClockTool, EchoTool, FakeModel, FakeScript, InMemorySessionRepository, ReadFileSandbox,
    ReadFileTool,
};
use serde_json::Value;
use tower::ServiceExt;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures")
}

fn app_with(model: Arc<dyn ModelPort>) -> Router {
    let mut registry = ToolRegistry::default();
    registry.register(Arc::new(EchoTool) as Arc<dyn Tool>);
    registry.register(Arc::new(ClockTool) as Arc<dyn Tool>);
    let sandbox = Arc::new(
        ReadFileSandbox::new(std::env::current_dir().expect("cwd"), 4096).expect("sandbox"),
    );
    registry.register(Arc::new(ReadFileTool::new(sandbox)) as Arc<dyn Tool>);
    let service = AgentService::new(
        model,
        Arc::new(registry),
        Arc::new(InMemorySessionRepository::default()),
        Arc::new(RunRegistry::default()),
        RunnerConfig::default(),
    );
    build_router(AppState::new(Arc::new(service)))
}

fn normalize(mut value: Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                if matches!(
                    key.as_str(),
                    "sessionId" | "runId" | "messageId" | "toolCallId"
                ) {
                    *item = Value::String("<id>".to_owned());
                } else {
                    *item = normalize(std::mem::take(item));
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                *item = normalize(std::mem::take(item));
            }
        }
        Value::String(text) => {
            *text = replace_timestamps(text);
        }
        _ => {}
    }
    value
}

fn replace_timestamps(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if index + 20 <= chars.len() && is_timestamp(&chars[index..index + 20]) {
            out.push_str("<now>");
            index += 20;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

fn is_timestamp(chars: &[char]) -> bool {
    let digit = |c: char| c.is_ascii_digit();
    digit(chars[0])
        && digit(chars[1])
        && digit(chars[2])
        && digit(chars[3])
        && chars[4] == '-'
        && digit(chars[5])
        && digit(chars[6])
        && chars[7] == '-'
        && digit(chars[8])
        && digit(chars[9])
        && chars[10] == 'T'
        && digit(chars[11])
        && digit(chars[12])
        && chars[13] == ':'
        && digit(chars[14])
        && digit(chars[15])
        && chars[16] == ':'
        && digit(chars[17])
        && digit(chars[18])
        && chars[19] == 'Z'
}

async fn post_json(app: Router, uri: &str, body: &str) -> (StatusCode, String) {
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
    (status, String::from_utf8(bytes.to_vec()).expect("utf8"))
}

fn read_json(path: &PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read fixture")).expect("json")
}

fn parse_sse(text: &str) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    for block in text.split("\n\n") {
        let mut name = None;
        let mut data = None;
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("event: ") {
                name = Some(rest.trim().to_owned());
            } else if let Some(rest) = line.strip_prefix("data: ") {
                data = serde_json::from_str::<Value>(rest).ok();
            }
        }
        if let (Some(name), Some(data)) = (name, data) {
            events.push((name, normalize(data)));
        }
    }
    events
}

fn fixture_events(dir: &str) -> Vec<(String, Value)> {
    std::fs::read_to_string(fixtures().join(dir).join("stream.ndjson"))
        .expect("read stream")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value: Value = serde_json::from_str(line).expect("envelope");
            (
                value["event"].as_str().expect("event").to_owned(),
                normalize(value["data"].clone()),
            )
        })
        .collect()
}

#[tokio::test]
async fn replays_blocking_tool_call_fixture() {
    let dir = fixtures().join("http/agent-message/002-tool-call");
    let request = read_json(&dir.join("request.json"));
    let expected = normalize(read_json(&dir.join("response.json")));
    let (status, body) = post_json(
        app_with(Arc::new(FakeModel::new(FakeScript::Tool))),
        "/api/agent/message",
        &request.to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let actual: Value = serde_json::from_str(&body).expect("json");
    assert_eq!(normalize(actual), expected);
}

#[tokio::test]
async fn replays_stream_text_fixture() {
    let (status, body) = post_json(
        app_with(Arc::new(FakeModel::new(FakeScript::Echo))),
        "/api/agent/stream",
        r#"{"agentId":"agent-1","content":[{"type":"text","text":"hi"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parse_sse(&body), fixture_events("sse/002-echo-text"));
}

#[tokio::test]
async fn replays_stream_tool_call_fixture() {
    let (status, body) = post_json(
        app_with(Arc::new(FakeModel::new(FakeScript::Tool))),
        "/api/agent/stream",
        r#"{"agentId":"agent-1","content":[{"type":"text","text":"time"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parse_sse(&body), fixture_events("sse/003-tool-call"));
}

#[tokio::test]
async fn replays_cancelled_fixture() {
    struct SlowModel;

    impl ModelPort for SlowModel {
        fn step<'a>(
            &'a self,
            _request: ModelRequest,
        ) -> BoxFuture<'a, Result<ModelStep, dsh_types::Error>> {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Ok(ModelStep::Text {
                    delta: "late".to_owned(),
                })
            })
        }

        fn name(&self) -> &'static str {
            "slow"
        }
    }

    let app = app_with(Arc::new(SlowModel));
    let stream_request = Request::builder()
        .method("POST")
        .uri("/api/agent/stream")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"agentId":"agent-1","content":[{"type":"text","text":"wait"}]}"#,
        ))
        .expect("request");
    let stream = tokio::spawn(app.clone().oneshot(stream_request));

    let mut cancelled = false;
    for _ in 0..100 {
        let (status, body) =
            post_json(app.clone(), "/api/agent/cancel", r#"{"agentId":"agent-1"}"#).await;
        if status == StatusCode::OK {
            assert!(body.contains("\"status\""), "{body}");
            cancelled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(cancelled, "run should be registered before cancel");

    let response = stream.await.expect("join").expect("stream response");
    let bytes = tokio::time::timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("stream timeout")
    .expect("body");
    let text = String::from_utf8(bytes.to_vec()).expect("utf8");
    assert_eq!(parse_sse(&text), fixture_events("sse/004-cancelled"));
}
