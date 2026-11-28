//! 可脚本化的假模型，用于离线运行与测试。

use dsh_domain::{BoxFuture, ModelPort, ModelRequest, ModelStep, Role};
use dsh_types::{Error, JsonValue, ToolCallRequest};

/// FakeModel 行为脚本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeScript {
    /// 回显最后一条用户/工具消息。
    Echo,
    /// 首步请求 `clock` 工具，次步总结工具结果。
    Tool,
}

impl FakeScript {
    /// 解析脚本名；未知值回落 `Echo`。
    pub fn parse(value: &str) -> Self {
        match value {
            "tool" => Self::Tool,
            _ => Self::Echo,
        }
    }
}

/// 假模型。
#[derive(Debug, Clone)]
pub struct FakeModel {
    script: FakeScript,
}

impl FakeModel {
    /// 按脚本构造。
    pub fn new(script: FakeScript) -> Self {
        Self { script }
    }

    /// 当前脚本。
    pub fn script(&self) -> FakeScript {
        self.script
    }
}

impl Default for FakeModel {
    fn default() -> Self {
        Self::new(FakeScript::Echo)
    }
}

impl ModelPort for FakeModel {
    fn step<'a>(&'a self, request: ModelRequest) -> BoxFuture<'a, Result<ModelStep, Error>> {
        Box::pin(async move {
            let last = request.messages.last();
            match self.script {
                FakeScript::Echo => {
                    let text = last.map(|message| message.as_text()).unwrap_or_default();
                    Ok(ModelStep::Text {
                        delta: format!("echo: {text}"),
                    })
                }
                FakeScript::Tool => {
                    let has_tool_result = request
                        .messages
                        .iter()
                        .any(|message| message.role == Role::Tool);
                    if has_tool_result {
                        let text = last.map(|message| message.as_text()).unwrap_or_default();
                        Ok(ModelStep::Text {
                            delta: format!("observed: {text}"),
                        })
                    } else {
                        Ok(ModelStep::ToolCalls {
                            calls: vec![ToolCallRequest {
                                tool_call_id: "call-clock-1".to_owned(),
                                name: "clock".to_owned(),
                                arguments: JsonValue::Object(serde_json::Map::new()),
                            }],
                            text: None,
                        })
                    }
                }
            }
        })
    }

    fn name(&self) -> &'static str {
        "fake"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsh_domain::ChatMessage;

    #[tokio::test]
    async fn echoes_last_message() {
        let model = FakeModel::default();
        let step = model
            .step(ModelRequest {
                messages: vec![ChatMessage::text(Role::User, "hello")],
                tools: Vec::new(),
            })
            .await
            .expect("step");
        assert_eq!(
            step,
            ModelStep::Text {
                delta: "echo: hello".to_owned()
            }
        );
    }

    #[tokio::test]
    async fn tool_script_calls_clock_once() {
        let model = FakeModel::new(FakeScript::Tool);
        let first = model
            .step(ModelRequest {
                messages: vec![ChatMessage::text(Role::User, "time")],
                tools: Vec::new(),
            })
            .await
            .expect("first");
        let ModelStep::ToolCalls { calls, .. } = first else {
            panic!("expected tool calls");
        };
        assert_eq!(calls[0].name, "clock");

        let second = model
            .step(ModelRequest {
                messages: vec![
                    ChatMessage::text(Role::User, "time"),
                    ChatMessage::text(Role::Tool, "{\"now\":\"...\"}"),
                ],
                tools: Vec::new(),
            })
            .await
            .expect("second");
        assert_eq!(
            second,
            ModelStep::Text {
                delta: "observed: {\"now\":\"...\"}".to_owned()
            }
        );
    }
}
