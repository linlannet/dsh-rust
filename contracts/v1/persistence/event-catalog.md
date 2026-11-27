# 持久化事件目录

会话事件是对外可回放的行为契约，而不只是内部日志。审计、恢复和 Rust/Java 差分都依赖它。

## 事件外形

```json
{
  "eventId": "evt-1",
  "sessionId": "sess-1",
  "seq": 1,
  "type": "session.created",
  "timestamp": "2026-11-27T00:00:00Z",
  "payload": {}
}
```

## 事件类型

| 类型 | 触发点 | 关键 payload |
| --- | --- | --- |
| `session.created` | 创建会话 | `agentId`、`workspaceId` |
| `session.updated` | 标题或状态变化 | 变更字段 |
| `message.appended` | 追加消息 | `messageId`、`role`、`content` |
| `run.started` | 开始一次运行 | `runId` |
| `run.completed` | 运行正常结束 | `runId`、`finishReason` |
| `run.failed` | 运行失败或取消 | `runId`、`error` |
| `tool.requested` | 请求工具 | `toolCallId`、`name`、`arguments` |
| `tool.completed` | 工具返回 | `toolCallId`、`isError` |
| `approval.requested` | 发起审批 | `approvalId`、`toolName` |
| `approval.decided` | 审批结论 | `approvalId`、`decision` |
| `task.created` | 创建任务 | `taskId`、`type` |
| `task.updated` | 任务状态变化 | `taskId`、`status` |

## 规则

1. `eventId` 全局唯一；`seq` 在单个会话内从 1 严格递增。
2. 事件只追加，不原地修改；纠错通过新事件表达。
3. `payload` 不得包含 API Key、Cookie、完整模型请求头等敏感信息。
4. `timestamp` 为 UTC ISO-8601，归一化时可替换为固定值，但不得丢失相对顺序。
5. 未知 `type` 必须可被读取方跳过并保留原始 JSON，不得丢弃。

## 回放

- 回放输入使用 `fixtures/persistence/*/events.ndjson`。
- 断言重点是业务不变量：终态、审批结果、工具调用配对、消息顺序，而不是逐字节相等。
