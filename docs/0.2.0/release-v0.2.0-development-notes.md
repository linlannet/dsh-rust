# DeepSeek Harness Rust v0.2.0 开发说明

> 版本：`v0.2.0` | 状态：**已实现并验证** | 基线：v0.1.0 | 契约版本：`1.0.0-draft.1`
>
> 本文记录 0.2.0 的交付内容、验证结果、已知限制与后续建议。

## 1. 版本概览

v0.2.0 是 Rust 版**首个可运行版本**，在 0.1.0 骨架之上接入：

1. **HTTP 服务**：`GET /health`、`POST /api/agent/message`、`POST /api/agent/stream`、`POST /api/agent/cancel`；
2. **ReAct 最小闭环**：模型推理 → 工具调用 → 结果回填 → 再推理 → 终止（默认 8 步上限）；
3. **模型双轨**：默认 FakeModel（零网络），DeepSeek 适配器由 feature `deepseek` 开启；
4. **内置工具**：`echo`、`clock`、`read_file`（工作区沙箱 + 256 KiB 上限 + 5s 超时）；
5. **SSE 契约输出**：`chunk`/`tool_call`/`done`/`error`，seq 递增、单终态、keepalive 不占 seq；
6. **契约回放**：新增 4 组运行时导出 fixtures，openapi 由路由表生成并纳入 CI 校验。

**不在本版**：插件宿主、持久化、审批、鉴权、多租户。

## 2. 选型落地

| 项 | 实现 | 说明 |
| --- | --- | --- |
| Web | Axum 0.8 | 路由、JSON 提取、SSE 原生支持 |
| 异步 | Tokio 1 | `spawn_blocking`、`timeout`、`watch`、信号 |
| 序列化 | serde 1 + serde_json 1 | DTO 与契约 camelCase |
| 模型 | Fake 默认 / DeepSeek 可选 | `--features deepseek` + `DEEPSEEK_API_KEY` |
| 工具 | echo / clock / read_file | 静态注册，`BTreeMap` 稳定排序 |

## 3. 模块落位

| 层 | 增量 |
| --- | --- |
| `dsh-types` | serde 序列化、`JsonValue`、`ToolCallRequest`、`ErrorCode::InvalidResponse` |
| `dsh-domain` | `ModelPort`/`ModelRequest`/`ModelStep`、`Tool`、`RunEvent`/`RunStatus`/`FinishReason`、Session 消息历史 |
| `dsh-case` | `AgentService`（ReAct）、`ToolRegistry`、`RunRegistry`（取消幂等）、`EventSink` |
| `dsh-http` | DTO、路由、错误映射、SSE 编码、traceId、`serve` |
| `dsh-infra` | FakeModel、DeepSeek（feature）、三个工具、文件沙箱、运行事件日志 |
| `dsh-runtime` | 环境变量配置、装配、启动日志、优雅关闭 |

不新增 workspace 成员；依赖方向保持 `http → case → domain → types`，`infra → domain`。

## 4. 验证结论（2026-11-27）

| 门禁 | 命令 | 结果 |
| --- | --- | --- |
| 格式 | `cargo fmt --all --check` | 通过 |
| 静态检查 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过 |
| 编译（全 feature） | `cargo check --workspace --all-features` | 通过 |
| 单元/集成测试 | `cargo test --workspace --all-features` | **51 passed** |
| 契约校验 | `node contracts/tools/validate.mjs` | 29 schema / 52 refs / 8 fixtures / 0 warnings |
| openapi 一致性 | `node contracts/tools/generate-openapi.mjs --check` | 通过 |

测试分布：`dsh-types` 11、`dsh-case` 10、`dsh-http` 13（含 8 集成 + 4 回放）、`dsh-infra` 17。

端到端（本地 FakeModel + `DSH_FAKE_SCRIPT=tool`）：

- `GET /health` → `{"status":"ok","version":"0.2.0","contractVersion":"1.0.0-draft.1"}`
- 阻塞消息 → `toolCalls` 含 `clock`（`completed`）、`finishReason=stop`
- 流式 → `tool_call(started)` → `tool_call(completed)` → `chunk` → `done(stop)`，seq 1–4
- 取消未知运行 → `404 not_found` + `traceId`
- `kill -INT` → 输出 `shutdown signal received` 并优雅退出

## 5. 契约结论

- 新增 4 组 fixtures：`http/agent-message/002-tool-call`、`sse/002-echo-text`、`sse/003-tool-call`、`sse/004-cancelled`，状态 `runtime-export`。
- `contracts/v1/openapi.yaml` 由 `tools/generate-openapi.mjs` 生成，CI 校验再生成一致性。
- **契约版本保持 `1.0.0-draft.1`**：新 fixtures 为运行时导出，尚未与 Java 基线交叉录制；待 Java/Rust 双方回放一致后再评估升 `1.0.0`。

## 6. 依赖与体积

- **新增依赖**：axum 0.8、tokio 1、serde 1、serde_json 1、tracing 0.1、tracing-subscriber 0.3、futures-util 0.3；可选 reqwest 0.12（`deepseek` feature）。
- **新增数据库表**：无；**新增 workspace 成员**：无。
- **破坏性变化**：`dsh-types` 与 `dsh-http` 不再零依赖；`JsonSchema` 从字符串改为 JSON 对象；`Session` 新增消息历史字段。

## 7. 已知限制

1. **DeepSeek 未真实联调**：适配器按 Chat Completions 最小字段实现，覆盖解析与缺 key 单测；未对真实 API 做端到端验证（需密钥与网络）。
2. **事件日志未暴露端点**：`InMemoryRunEventLog` 提供追加/查询 API，但未提供 HTTP 导出端点。
3. **内存态**：会话、运行与事件重启即丢；持久化留待 0.2.1。
4. **无鉴权**：默认仅监听回环地址，不提供认证与多租户。
5. **工具静态注册**：不支持插件动态加载。
6. **DeepSeek 流式未透传**：当前以非流式请求 + 文本增量输出，未消费上游 token 流。

## 8. 后续建议

| 优先级 | 建议 |
| --- | --- |
| P0 | 0.2.1：插件宿主（`plugin-host` 契约）+ 会话事件持久化 |
| P1 | 用 Java 基线与 Rust 双向录制 fixtures，评估契约升 `1.0.0` |
| P1 | DeepSeek 真实联调与上游流式增量透传 |
| P2 | 鉴权、限流、多租户与可观测性完善 |
| P2 | 审批与权限矩阵（0.2.1） |