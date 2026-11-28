//! 回显工具。

use dsh_domain::Tool;
use dsh_types::{Error, ErrorCode, JsonSchema, JsonValue, ToolDescriptor};

/// 原样返回输入文本。
#[derive(Debug, Default)]
pub struct EchoTool;

impl Tool for EchoTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor::new(
            "echo",
            "Echo the provided text back to the caller.",
            JsonSchema::from_object(serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": true
            }))
            .expect("valid object schema"),
        )
    }

    fn call(&self, arguments: JsonValue) -> Result<JsonValue, Error> {
        let text = arguments
            .get("text")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                Error::new(ErrorCode::InvalidRequest, "echo requires string field text")
            })?;
        Ok(serde_json::json!({ "text": text }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echoes_text() {
        let tool = EchoTool;
        let result = tool.call(serde_json::json!({"text": "hi"})).expect("call");
        assert_eq!(result["text"], "hi");
    }

    #[test]
    fn rejects_missing_text() {
        let tool = EchoTool;
        let error = tool.call(serde_json::json!({})).expect_err("must fail");
        assert_eq!(error.code(), ErrorCode::InvalidRequest);
    }
}
