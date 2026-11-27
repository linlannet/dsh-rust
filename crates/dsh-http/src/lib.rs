//! HTTP 与 SSE 接入层。
//!
//! 骨架阶段只固定路径与事件常量。Axum、Tokio 等具体选型留待依赖审查与基准，
//! 不在此处提前绑定。

/// 对外路由。
pub mod routes {
    /// 健康检查。
    pub const HEALTH: &str = "/health";
    /// 阻塞式 Agent 消息。
    pub const AGENT_MESSAGE: &str = "/api/agent/message";
    /// SSE 流式 Agent 消息。
    pub const AGENT_STREAM: &str = "/api/agent/stream";
    /// 取消运行。
    pub const AGENT_CANCEL: &str = "/api/agent/cancel";
    /// 安装插件。
    pub const PLUGIN_INSTALL: &str = "/api/harness/plugins/install";
    /// 激活插件。
    pub const PLUGIN_ACTIVATE: &str = "/api/harness/plugins/activate";
    /// 查询插件状态。
    pub const PLUGIN_STATUS: &str = "/api/harness/plugins/status";
}

/// SSE 心跳注释行，不计入业务事件。
pub const SSE_KEEPALIVE: &str = ":keepalive";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_are_absolute() {
        assert!(routes::HEALTH.starts_with('/'));
        assert!(routes::AGENT_STREAM.starts_with("/api/"));
        assert_eq!(SSE_KEEPALIVE, ":keepalive");
    }
}
