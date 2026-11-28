//! SSE 事件编码。

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::response::sse::{Event, KeepAlive, Sse};
use dsh_case::EventSink;
use dsh_domain::RunEvent;
use dsh_types::ErrorCode;
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;

/// 默认单 run 事件字节预算（8 MiB）。
pub const DEFAULT_STREAM_BUDGET_BYTES: usize = 8 * 1024 * 1024;

/// 预算环境变量名。
pub const STREAM_BUDGET_ENV: &str = "DSH_STREAM_BUDGET_BYTES";

/// 从环境变量读取字节预算；缺失或非法时回退默认值。
pub fn stream_budget_from_env() -> usize {
    std::env::var(STREAM_BUDGET_ENV)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_STREAM_BUDGET_BYTES)
}

/// 基于无界通道的事件消费者。
pub struct ChannelSink {
    tx: tokio::sync::mpsc::UnboundedSender<RunEvent>,
}

impl ChannelSink {
    /// 由发送端构造。
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<RunEvent>) -> Self {
        Self { tx }
    }
}

impl EventSink for ChannelSink {
    fn emit(&self, event: RunEvent) {
        let _ = self.tx.send(event);
    }
}

/// 创建 SSE 响应与事件接收端。
pub fn channel() -> (ChannelSink, UnboundedReceiver<RunEvent>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (ChannelSink::new(tx), rx)
}

/// 构造 SSE 响应，字节预算取自 `DSH_STREAM_BUDGET_BYTES`。
pub fn sse_response(
    rx: UnboundedReceiver<RunEvent>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    sse_response_with_budget(rx, stream_budget_from_env())
}

/// 构造 SSE 响应并显式指定字节预算。
pub fn sse_response_with_budget(
    rx: UnboundedReceiver<RunEvent>,
    budget: usize,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    Sse::new(RunEventStream::new(rx, budget)).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    )
}

/// 事件流：分配 seq，编码为 SSE 事件，并守住单 run 字节预算。
///
/// 预算超限时丢弃当前事件、补发唯一终态 `error(stream_budget_exceeded)` 后结束流；
/// 心跳不计入预算与 `seq`。
pub struct RunEventStream {
    rx: UnboundedReceiver<RunEvent>,
    seq: u64,
    budget: usize,
    emitted_bytes: usize,
    finished: bool,
}

impl RunEventStream {
    fn new(rx: UnboundedReceiver<RunEvent>, budget: usize) -> Self {
        Self {
            rx,
            seq: 0,
            budget,
            emitted_bytes: 0,
            finished: false,
        }
    }

    fn next_event(&mut self, name: &'static str, data: String) -> Event {
        self.seq += 1;
        Event::default()
            .event(name)
            .id(self.seq.to_string())
            .data(data)
    }
}

impl futures_util::Stream for RunEventStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        match self.rx.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(event)) => {
                let (name, data) = encode(&event);
                let size = name.len() + data.len();
                if self.emitted_bytes + size > self.budget {
                    self.finished = true;
                    let data = budget_exceeded_data(self.budget);
                    return Poll::Ready(Some(Ok(self.next_event("error", data))));
                }
                self.emitted_bytes += size;
                Poll::Ready(Some(Ok(self.next_event(name, data))))
            }
        }
    }
}

/// 预算超限终态载荷。
fn budget_exceeded_data(budget: usize) -> String {
    json!({
        "code": ErrorCode::StreamBudgetExceeded.as_str(),
        "message": format!("stream output limited to {budget} bytes"),
    })
    .to_string()
}

fn encode(event: &RunEvent) -> (&'static str, String) {
    match event {
        RunEvent::Chunk { message_id, delta } => (
            "chunk",
            json!({"messageId": message_id, "delta": delta}).to_string(),
        ),
        RunEvent::ToolCall {
            tool_call_id,
            name,
            status,
            arguments,
            result,
            error,
        } => {
            let mut data = json!({
                "toolCallId": tool_call_id,
                "name": name,
                "status": status.as_str()
            });
            if let Some(arguments) = arguments {
                data["arguments"] = arguments.clone();
            }
            if let Some(result) = result {
                data["result"] = result.clone();
            }
            if let Some(error) = error {
                data["error"] = serde_json::to_value(error).unwrap_or_default();
            }
            ("tool_call", data.to_string())
        }
        RunEvent::Done {
            finish_reason,
            usage,
        } => {
            let mut data = json!({"finishReason": finish_reason.as_str()});
            if let Some(usage) = usage {
                data["usage"] = json!({
                    "inputTokens": usage.input_tokens,
                    "outputTokens": usage.output_tokens
                });
            }
            ("done", data.to_string())
        }
        RunEvent::Error { error, trace_id } => {
            let mut data = serde_json::to_value(error).unwrap_or_default();
            if let Some(trace_id) = trace_id {
                data["traceId"] = json!(trace_id);
            }
            ("error", data.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::response::IntoResponse;
    use dsh_domain::RunEvent;
    use dsh_types::Error;

    use super::*;

    /// 解析 SSE 线上文本为 `(event, id, data)` 三元组。
    async fn collect(rx: UnboundedReceiver<RunEvent>, budget: usize) -> Vec<(String, u64, String)> {
        let response = sse_response_with_budget(rx, budget).into_response();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let text = String::from_utf8(bytes.to_vec()).expect("utf8");

        text.split("\n\n")
            .filter(|block| !block.trim().is_empty())
            .map(|block| {
                let mut name = String::new();
                let mut id = 0;
                let mut data = String::new();
                for line in block.lines() {
                    if let Some(value) = line.strip_prefix("event: ") {
                        name = value.to_owned();
                    } else if let Some(value) = line.strip_prefix("id: ") {
                        id = value.trim().parse().expect("numeric id");
                    } else if let Some(value) = line.strip_prefix("data: ") {
                        data = value.to_owned();
                    }
                }
                (name, id, data)
            })
            .collect()
    }

    #[tokio::test]
    async fn budget_exceeded_emits_single_terminal_error() {
        let (sink, rx) = channel();
        sink.emit(RunEvent::Chunk {
            message_id: "msg-1".to_owned(),
            delta: "0123456789".to_owned(),
        });
        sink.emit(RunEvent::Done {
            finish_reason: dsh_domain::FinishReason::Stop,
            usage: None,
        });
        drop(sink);

        // 预算 10 字节：任何业务事件都放不下，直接由终态错误收尾
        let events = collect(rx, 10).await;
        assert_eq!(events.len(), 1, "超限后不再转发任何业务事件");
        assert_eq!(events[0].0, "error");
        assert_eq!(events[0].1, 1, "seq 从 1 开始且无跳号");
        let payload: serde_json::Value = serde_json::from_str(&events[0].2).expect("json");
        assert_eq!(payload["code"], ErrorCode::StreamBudgetExceeded.as_str());
    }

    #[tokio::test]
    async fn within_budget_keeps_stream_and_sequence() {
        let (sink, rx) = channel();
        sink.emit(RunEvent::Chunk {
            message_id: "msg-1".to_owned(),
            delta: "hi".to_owned(),
        });
        sink.emit(RunEvent::Error {
            error: Error::new(ErrorCode::Unavailable, "upstream down"),
            trace_id: None,
        });
        drop(sink);

        let events = collect(rx, DEFAULT_STREAM_BUDGET_BYTES).await;
        assert_eq!(
            events
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            vec!["chunk", "error"]
        );
        assert_eq!(
            events.iter().map(|event| event.1).collect::<Vec<_>>(),
            vec![1, 2],
            "seq 连续"
        );
        let payload: serde_json::Value = serde_json::from_str(&events[1].2).expect("json");
        assert_eq!(payload["code"], "unavailable", "预算内原样透传");
    }
}
