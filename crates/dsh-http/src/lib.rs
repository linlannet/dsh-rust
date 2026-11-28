//! HTTP 与 SSE 接入层。

pub mod dto;
pub mod error;
pub mod router;
pub mod sse;
pub mod trace;

/// 对外路由常量。
pub mod routes {
    /// 健康检查。
    pub const HEALTH: &str = "/health";
    /// 阻塞式 Agent 消息。
    pub const AGENT_MESSAGE: &str = "/api/agent/message";
    /// SSE 流式 Agent 消息。
    pub const AGENT_STREAM: &str = "/api/agent/stream";
    /// 取消运行。
    pub const AGENT_CANCEL: &str = "/api/agent/cancel";
    /// 会话事件回放。
    pub const SESSIONS_EVENTS: &str = "/api/sessions/{session_id}/events";
    /// 安装插件。
    pub const PLUGIN_INSTALL: &str = "/api/harness/plugins/install";
    /// 激活插件。
    pub const PLUGIN_ACTIVATE: &str = "/api/harness/plugins/activate";
    /// 查询插件状态。
    pub const PLUGIN_STATUS: &str = "/api/harness/plugins/status";
}

/// SSE 心跳注释行，不计入业务事件。
pub const SSE_KEEPALIVE: &str = ":keepalive";

pub use error::HttpError;
pub use router::{AppState, build_router, serve};
pub use sse::{RunEventStream, channel, sse_response, sse_response_with_budget};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_are_absolute() {
        assert!(routes::HEALTH.starts_with('/'));
        assert!(routes::AGENT_STREAM.starts_with("/api/"));
        assert!(routes::SESSIONS_EVENTS.starts_with("/api/sessions/"));
        assert!(routes::SESSIONS_EVENTS.ends_with("/events"));
        assert_eq!(SSE_KEEPALIVE, ":keepalive");
    }
}
