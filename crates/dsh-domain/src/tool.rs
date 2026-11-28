//! 工具端口。

use dsh_types::{Error, JsonValue, ToolDescriptor};

/// Agent 可调用工具。
pub trait Tool: Send + Sync {
    /// 跨进程描述。
    fn descriptor(&self) -> ToolDescriptor;

    /// 执行工具；参数与结果均为 JSON。
    fn call(&self, arguments: JsonValue) -> Result<JsonValue, Error>;
}
