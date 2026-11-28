use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON 值别名，避免领域层直接依赖 serde_json。
pub type JsonValue = Value;

/// 一次工具调用请求。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRequest {
    /// 调用 ID，同一流内唯一。
    pub tool_call_id: String,
    /// 工具名。
    pub name: String,
    /// 参数 JSON。
    #[serde(default)]
    pub arguments: Value,
}

/// 工具输入 Schema 的 JSON 对象。
///
/// 骨架阶段只保证结构合法，不做 JSON Schema 校验。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonSchema(pub Value);

impl JsonSchema {
    /// 构造空对象 schema。
    pub fn empty_object() -> Self {
        Self(Value::Object(serde_json::Map::new()))
    }

    /// 从 JSON 对象构造；非对象输入返回 `None`。
    pub fn from_object(value: Value) -> Option<Self> {
        value.is_object().then_some(Self(value))
    }
}

/// Agent 可调用工具的跨进程描述。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    /// 工具名，允许 `plugin__`、`mcp__` 前缀。
    pub name: String,
    /// 面向模型的说明。
    pub description: String,
    /// 参数 Schema。
    pub input_schema: JsonSchema,
    /// 输出 Schema；骨架阶段可省略。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<JsonSchema>,
}

impl ToolDescriptor {
    /// 构造仅含输入 Schema 的工具描述。
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: JsonSchema,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            output_schema: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_descriptor_fields() {
        let descriptor = ToolDescriptor::new(
            "word_count",
            "统计词频",
            JsonSchema::from_object(serde_json::json!({"type": "object"})).expect("object"),
        );
        assert_eq!(descriptor.name, "word_count");
        assert_eq!(descriptor.input_schema.0["type"], "object");
    }

    #[test]
    fn serializes_contract_field_names() {
        let descriptor = ToolDescriptor::new(
            "word_count",
            "统计词频",
            JsonSchema::from_object(serde_json::json!({"type": "object"})).expect("object"),
        );
        let json = serde_json::to_value(&descriptor).expect("serialize");
        assert_eq!(json["inputSchema"]["type"], "object");
        assert!(json.get("outputSchema").is_none());
    }
}
