# MCP 引用

不在本目录复制 MCP 规范。只记录与 dsh 集成相关的版本和边界。

## 版本

- Java 侧 Node sidecar / MCP 桥接握手使用 `protocolVersion: 2024-11-05`。
- Rust 侧必须使用同一版本或双方协商后的兼容版本；不支持时结构化报错。

## 传输

- stdio：换行分隔 JSON-RPC 2.0。
- HTTP/SSE：JSON-RPC over HTTP，响应可从 SSE 流返回。

## 方法

- `initialize`
- `notifications/initialized`
- `tools/list`
- `tools/call`

## 约束

- MCP Server 视为外部不可信进程：设置超时、输出上限、进程回收。
- 工具名在宿主侧统一映射为 `mcp__<server>__<tool>`，映射规则属于契约。
- 上游规范：<https://modelcontextprotocol.io/specification>
