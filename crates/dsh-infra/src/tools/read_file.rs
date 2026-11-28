//! 只读文件工具。

use std::sync::Arc;

use dsh_domain::Tool;
use dsh_types::{Error, JsonSchema, JsonValue, ToolDescriptor};

use crate::sandbox::ReadFileSandbox;

/// 读取工作区内的 UTF-8 文本文件。
#[derive(Debug, Clone)]
pub struct ReadFileTool {
    sandbox: Arc<ReadFileSandbox>,
}

impl ReadFileTool {
    /// 注入沙箱。
    pub fn new(sandbox: Arc<ReadFileSandbox>) -> Self {
        Self { sandbox }
    }
}

impl Tool for ReadFileTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor::new(
            "read_file",
            format!(
                "Read a UTF-8 text file inside the workspace (limit {} bytes).",
                self.sandbox.max_bytes()
            ),
            JsonSchema::from_object(serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string", "description": "Relative path inside the workspace"}},
                "required": ["path"],
                "additionalProperties": true
            }))
            .expect("valid object schema"),
        )
    }

    fn call(&self, arguments: JsonValue) -> Result<JsonValue, Error> {
        let path = arguments
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| {
                Error::new(
                    dsh_types::ErrorCode::InvalidRequest,
                    "read_file requires string field path",
                )
            })?;
        let (relative, content, bytes) = self.sandbox.read(path)?;
        Ok(serde_json::json!({
            "path": relative,
            "content": content,
            "bytes": bytes
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn reads_workspace_file() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "dsh-read-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("dir");
        fs::write(root.join("a.txt"), "abc").expect("write");
        let sandbox = Arc::new(ReadFileSandbox::new(&root, 1024).expect("sandbox"));
        let tool = ReadFileTool::new(sandbox);
        let result = tool
            .call(serde_json::json!({"path": "a.txt"}))
            .expect("call");
        assert_eq!(result["content"], "abc");
        assert_eq!(result["bytes"], 3);
        let _ = fs::remove_dir_all(root);
    }
}
