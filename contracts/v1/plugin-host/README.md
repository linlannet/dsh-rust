# 插件宿主协议

Rust Runtime 与独立 JVM 插件宿主之间的进程协议。传输默认 stdio，换行分隔 JSON-RPC 2.0；远程场景另行设计 TLS 与鉴权。

## 目的

- 复用现有 `JavaHarnessPlugin`、`PluginContext`、`URLClassLoader`，不要求插件改代码。
- 覆盖完整生命周期：安装、激活、停用、状态、热重载、关闭。
- 覆盖 `PluginContext` 的全部注册面：工具、资源释放、事件、Hook、配置、系统提示词。

## 握手

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"dsh-plugin-host/1","clientInfo":{"name":"dsh-rust","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"dsh-plugin-host/1","capabilities":["tools","hooks","events","prompts"],"hostVersion":"0.1.0"}}
```

随后发送 `notifications/initialized`，与 MCP 风格保持一致。

## 方法

| 方法 | 用途 |
| --- | --- |
| `plugin.install` | 校验并登记 JAR / 插件源 |
| `plugin.activate` | 加载类、实例化、执行 `onStart` |
| `plugin.deactivate` | 执行 `onStop`、注销注册项、关闭 ClassLoader |
| `plugin.status` | 返回已加载插件与版本 |
| `tools/list` | 返回工具描述 |
| `tools/call` | 调用工具 |
| `hooks/invoke` | 触发 PRE/POST Hook，返回允许/阻断 |
| `events/subscribe` | 订阅插件事件 |
| `events/publish` | 发布插件事件 |
| `prompts/register` | 注册系统提示词段落 |

## 错误

- JSON-RPC 错误对象使用 `code`、`message`、`data`。
- 业务失败区分“协议错误”和“工具执行失败”：工具失败返回 `isError: true` 的 `result`，不升级为 JSON-RPC error。
- 超时、取消、宿主崩溃必须有明确错误码，不得挂起请求。

## 边界

- ClassLoader 不是安全沙箱。多个不可信插件应使用独立宿主进程或容器。
- 插件宿主崩溃不得带崩 Rust 主进程；主进程应能诊断并重启或明确失败。
- 协议版本不支持时拒绝启动，不做静默降级。
