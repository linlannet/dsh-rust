# SSE 事件目录

## 传输格式

线上为 `text/event-stream`：

```text
event: chunk
data: {"messageId":"msg-1","delta":"hello"}

:keepalive
```

fixture 存储为 NDJSON，一行一个业务事件，已剥离 `event:`、`data:` 包装与心跳：

```json
{"seq":1,"event":"chunk","data":{"messageId":"msg-1","delta":"hello"}}
```

## 事件

| 事件 | 终态 | 关键字段 | 说明 |
| --- | --- | --- | --- |
| `chunk` | 否 | `delta` | 模型文本增量，可合并展示 |
| `tool_call` | 否 | `toolCallId`、`name`、`status` | `started`/`completed`/`failed` 三段式 |
| `done` | 是 | `finishReason` | 正常结束或被取消 |
| `error` | 是 | `code`、`message` | 终止错误，`retryable` 提示可否重试 |

## 顺序与终态

1. `seq` 从 1 开始严格递增，不得跳号或复用。
2. 每个流恰好一个终态事件，且必须最后出现。
3. 取消返回 `done` + `finishReason: cancelled`，或 `error`，不得两者同时出现。
4. 心跳以 `:keepalive` 注释发送，不占 `seq`，不进入 fixture。
5. 未知事件必须被消费者忽略并记录告警，不得中断流或崩溃。
6. 单 run 输出超过 `DSH_STREAM_BUDGET_BYTES`（默认 8 MiB）时，丢弃当前事件并以唯一终态 `error`（`code: stream_budget_exceeded`）收尾，此后不再有事件；`seq` 仍连续。

## 一致性要求

- `tool_call` 的 `toolCallId` 在同一流内唯一。
- `completed` 必须带 `result`；`failed` 必须带 `error`。
- `done` 的 `usage` 若存在，必须为非负整数。
