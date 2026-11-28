# A2A 引用

不在本目录复制 A2A 规范。只记录 dsh 侧接入点与版本。

## 版本

- 桌面端 README 声明使用 A2A 0.2.1。
- 具体版本以 Java 基线实际响应和 Agent Card 为准，录制后回填。

## 端点

- DSH Agent Card：`/.well-known/dsh-agent-card`
- A2A Agent Card：`/.well-known/agent-card.json`
- JSON-RPC：`POST /a2a`

## 约束

- 消息与任务状态必须保留原始 JSON，未知字段不得丢弃。
- 远端 Agent 视为不可信：设置连接、读取、空闲超时。
- 认证、TLS 和重试策略在部署层配置，不写入 fixture。
