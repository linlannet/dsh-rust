//! SSE 事件编码。

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::response::sse::{Event, KeepAlive, Sse};
use dsh_case::EventSink;
use dsh_domain::RunEvent;
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;

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

/// 构造 SSE 响应。
pub fn sse_response(
    rx: UnboundedReceiver<RunEvent>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    Sse::new(RunEventStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    )
}

/// 事件流：分配 seq，编码为 SSE 事件。
pub struct RunEventStream {
    rx: UnboundedReceiver<RunEvent>,
    seq: u64,
}

impl RunEventStream {
    fn new(rx: UnboundedReceiver<RunEvent>) -> Self {
        Self { rx, seq: 0 }
    }
}

impl futures_util::Stream for RunEventStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.rx.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(event)) => {
                self.seq += 1;
                let seq = self.seq;
                let encoded = encode(&event);
                Poll::Ready(Some(Ok(Event::default()
                    .event(encoded.0)
                    .id(seq.to_string())
                    .data(encoded.1))))
            }
        }
    }
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
