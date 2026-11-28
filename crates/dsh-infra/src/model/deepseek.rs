//! DeepSeek Chat Completions 适配器（feature `deepseek`）。

use std::time::Duration;

use dsh_domain::{BoxFuture, ModelPort, ModelRequest, ModelStep, Role};
use dsh_types::{Error, ErrorCode, JsonValue, ToolCallRequest};
use serde::Deserialize;
use serde_json::json;

/// DeepSeek 模型适配器。
#[derive(Debug, Clone)]
pub struct DeepSeekModel {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl DeepSeekModel {
    /// 构造适配器。
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, Error> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(Error::new(
                ErrorCode::Unavailable,
                "DEEPSEEK_API_KEY is required for deepseek model",
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| {
                Error::new(
                    ErrorCode::Internal,
                    format!("failed to build http client: {error}"),
                )
            })?;
        Ok(Self {
            client,
            api_key,
            base_url: base_url.into(),
            model: model.into(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    id: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    name: String,
    #[serde(default)]
    arguments: String,
}

impl ModelPort for DeepSeekModel {
    fn step<'a>(&'a self, request: ModelRequest) -> BoxFuture<'a, Result<ModelStep, Error>> {
        Box::pin(async move {
            let messages: Vec<JsonValue> = request
                .messages
                .iter()
                .map(|message| {
                    json!({
                        "role": match message.role {
                            Role::System => "system",
                            Role::User => "user",
                            Role::Assistant => "assistant",
                            Role::Tool => "tool",
                        },
                        "content": message.as_text()
                    })
                })
                .collect();

            let tools: Vec<JsonValue> = request
                .tools
                .iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.input_schema.0
                        }
                    })
                })
                .collect();

            let mut body = json!({
                "model": self.model,
                "messages": messages,
                "stream": false
            });
            if !tools.is_empty() {
                body["tools"] = JsonValue::Array(tools);
            }

            let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
            let response = self
                .client
                .post(url)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await
                .map_err(map_reqwest)?;

            let status = response.status();
            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                let code = if status.is_server_error() || status.as_u16() == 429 {
                    ErrorCode::Unavailable
                } else {
                    ErrorCode::InvalidRequest
                };
                return Err(Error::new(
                    code,
                    format!("deepseek http {}: {}", status.as_u16(), text),
                ));
            }

            let payload: ChatResponse = response.json().await.map_err(|error| {
                Error::new(
                    ErrorCode::InvalidResponse,
                    format!("failed to decode deepseek response: {error}"),
                )
            })?;
            parse_message(payload)
        })
    }

    fn name(&self) -> &'static str {
        "deepseek"
    }
}

fn map_reqwest(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::new(ErrorCode::Timeout, "deepseek request timed out")
    } else {
        Error::new(
            ErrorCode::Unavailable,
            format!("deepseek request failed: {error}"),
        )
    }
}

fn parse_message(payload: ChatResponse) -> Result<ModelStep, Error> {
    let message = payload
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| Error::new(ErrorCode::InvalidResponse, "deepseek returned no choices"))?
        .message;

    if message.tool_calls.is_empty() {
        return Ok(ModelStep::Text {
            delta: message.content.unwrap_or_default(),
        });
    }

    let calls = message
        .tool_calls
        .into_iter()
        .map(|call| ToolCallRequest {
            tool_call_id: call.id,
            name: call.function.name,
            arguments: serde_json::from_str::<JsonValue>(&call.function.arguments)
                .unwrap_or(JsonValue::Object(serde_json::Map::new())),
        })
        .collect();

    Ok(ModelStep::ToolCalls {
        calls,
        text: message.content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(value: serde_json::Value) -> ChatResponse {
        serde_json::from_value(value).expect("valid payload")
    }

    #[test]
    fn rejects_empty_api_key() {
        let error = DeepSeekModel::new("", "https://api.deepseek.com", "deepseek-chat")
            .expect_err("missing key");
        assert_eq!(error.code(), ErrorCode::Unavailable);
    }

    #[test]
    fn parses_text_response() {
        let step = parse_message(payload(serde_json::json!({
            "choices": [{"message": {"content": "hello"}}]
        })))
        .expect("step");
        assert_eq!(
            step,
            ModelStep::Text {
                delta: "hello".to_owned()
            }
        );
    }

    #[test]
    fn parses_tool_call_response() {
        let step = parse_message(payload(serde_json::json!({
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call-1",
                        "function": {"name": "clock", "arguments": "{}"}
                    }]
                }
            }]
        })))
        .expect("step");
        let ModelStep::ToolCalls { calls, text } = step else {
            panic!("expected tool calls");
        };
        assert_eq!(calls[0].name, "clock");
        assert_eq!(calls[0].tool_call_id, "call-1");
        assert!(text.is_none());
    }

    #[test]
    fn rejects_empty_choices() {
        let error = parse_message(payload(serde_json::json!({"choices": []}))).expect_err("empty");
        assert_eq!(error.code(), ErrorCode::InvalidResponse);
    }
}
