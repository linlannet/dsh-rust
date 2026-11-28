//! 工具注册表。

use std::collections::BTreeMap;
use std::sync::Arc;

use dsh_domain::Tool;
use dsh_types::{Error, ErrorCode, ToolDescriptor};

/// 按名称注册的工具集合，目录按名称稳定排序。
#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 注册工具；同名覆盖。
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.descriptor().name;
        self.tools.insert(name, tool);
    }

    /// 按名称查找。
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// 返回稳定排序的工具目录。
    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.tools.values().map(|tool| tool.descriptor()).collect()
    }

    /// 工具数量。
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 执行工具；未知工具返回 `invalid_request`。
    pub fn call(
        &self,
        name: &str,
        arguments: dsh_types::JsonValue,
    ) -> Result<dsh_types::JsonValue, Error> {
        let tool = self.get(name).ok_or_else(|| {
            Error::new(ErrorCode::InvalidRequest, format!("unknown tool: {name}"))
        })?;
        tool.call(arguments)
    }
}
