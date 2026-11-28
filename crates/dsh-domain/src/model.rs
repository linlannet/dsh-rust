//! 模型端口与请求/响应值对象。

use std::future::Future;
use std::pin::Pin;

use dsh_types::{ContentBlock, Error, ToolCallRequest, ToolDescriptor};

/// 便于 `Arc<dyn ModelPort>` 使用的装箱 Future。
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 对话角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// 系统提示。
    System,
    /// 用户消息。
    User,
    /// 助手消息。
    Assistant,
    /// 工具结果。
    Tool,
}

/// 一条对话消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// 角色。
    pub role: Role,
    /// 内容块。
    pub content: Vec<ContentBlock>,
}

impl ChatMessage {
    /// 构造文本消息。
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentBlock::text(text)],
        }
    }

    /// 拼接为纯文本，供模型适配器使用。
    pub fn as_text(&self) -> String {
        self.content
            .iter()
            .map(ContentBlock::as_text)
            .collect::<Vec<_>>()
            .join("")
    }
}

/// 一次模型调用请求。
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRequest {
    /// 消息历史。
    pub messages: Vec<ChatMessage>,
    /// 可用工具目录。
    pub tools: Vec<ToolDescriptor>,
}

/// 模型返回：纯文本或工具调用。
#[derive(Debug, Clone, PartialEq)]
pub enum ModelStep {
    /// 文本增量，运行可继续。
    Text {
        /// 文本内容。
        delta: String,
    },
    /// 请求执行工具。
    ToolCalls {
        /// 工具调用列表。
        calls: Vec<ToolCallRequest>,
        /// 可选的伴随文本。
        text: Option<String>,
    },
}

/// 模型端口。
pub trait ModelPort: Send + Sync {
    /// 执行一次模型推理。
    fn step<'a>(&'a self, request: ModelRequest) -> BoxFuture<'a, Result<ModelStep, Error>>;

    /// 模型名称，用于启动日志与诊断。
    fn name(&self) -> &'static str;
}
