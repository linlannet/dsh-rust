/// 工具输入 Schema 的原始 JSON 文本。
///
/// 骨架阶段不引入 `serde_json`，接入适配器后替换为结构化类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSchema(pub String);

/// Agent 可调用工具的跨进程描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDescriptor {
    /// 工具名，允许 `plugin__`、`mcp__` 前缀。
    pub name: String,
    /// 面向模型的说明。
    pub description: String,
    /// 参数 Schema。
    pub input_schema: JsonSchema,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_descriptor_fields() {
        let descriptor = ToolDescriptor {
            name: "word_count".to_owned(),
            description: "统计词频".to_owned(),
            input_schema: JsonSchema(r#"{"type":"object"}"#.to_owned()),
        };
        assert_eq!(descriptor.name, "word_count");
        assert_eq!(descriptor.input_schema.0, r#"{"type":"object"}"#);
    }
}
