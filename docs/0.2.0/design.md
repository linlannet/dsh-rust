# DeepSeek Harness Rust v0.2.0 实现方案文档

> 版本：`v0.2.0` | 基线：v0.1.0 | 架构：既有六层 + Axum 接入层 | 契约版本：`1.0.0-draft.1`
>
> 本文把 `docs/0.2.0/spec.md` 落到实现：新增依赖、分层落位、关键接口、ReAct 状态机、SSE 编码、取消与沙箱、测试与实施阶段。

---

# 一、需求与存量功能关系分析

## 1.1 0.1.0 存量与 0.2.0 增量

| 0.1.0 存量 | 0.2.0 用法 | 变更类型 |
| --- | --- | --- |
| `dsh-types`（Id/Error/ContentBlock/ToolDescriptor） | 增加 serde 序列化；映射契约 camelCase | 扩展 |
| `dsh-domain`（Session/SessionRepository） | 新增模型端口、工具 trait、运行事件与运行状态 | 扩展 |
| `dsh-case`（RunRequest/RunAgent） | 新增 ReAct 循环、工具编排、运行注册与取消 | 扩展 |
| `dsh-http`（路由/SSE 常量） | 接入 Axum 路由、提取器、SSE 编码、错误映射 | 实现 |
| `dsh-infra`（内存会话仓库） | 新增 FakeModel、内置工具、运行/事件内存存储；可选 DeepSeek | 扩展 |
| `apps/dsh-runtime`（自检） | 改为启动 HTTP 服务，读取环境配置 | 重写入口 |
| `contracts/v1` | 按其实现；补 fixtures 与 openapi | 补齐 |

## 1.2 存量约束

1. **`dsh-types` 零依赖**：0.2.0 需要 serde，意味 `dsh-types` 将新增 `serde` 依赖；这是本版最关键的结构变更（见 §4.1 依赖决策）。
2. **`SessionRepository` 同步**：ReAct 在异步上下文中调用同步端口，须确认内存实现无阻塞；`read_file` 用 `spawn_blocking`。
3. **workspace lints**：`unsafe_code=forbid`、`dbg_macro=deny` 不变。
4. **契约 draft**：DTO 字段名以 schema 为准；`ContentBlock` 需 `#[serde(tag = "type")]` 映射。

---

# 二、总体设计

## 2.1 分层落位（不新增 workspace 成员）

| 层 | 新增内容 | 说明 |
| --- | --- | --- |
| `dsh-types` | `RunEvent` DTO? 否；`ErrorCode` 扩展；serde derive | 事件类型放 domain，避免 types 承载业务 |
| `dsh-domain` | `ModelPort`、`ModelRequest`/`ModelStep`、`Tool` trait、`ToolOutput`、`RunEvent`、`RunStatus`、`RunRecord` | 纯领域，无 axum/tokio 依赖（可引入 `BoxFuture` 手写） |
| `dsh-case` | `AgentRunner`（ReAct）、`ToolRegistry`、`RunRegistry`、`CancelSignal`、`EventSink` | 编排；依赖 domain |
| `dsh-http` | Axum `Router`、DTO（serde）、SSE 编码、`HttpError` 映射、`traceId` | 唯一引入 axum 的 crate |
| `dsh-infra` | `FakeModel`、内置工具（echo/clock/read_file）、`InMemoryRunStore`、`RunEventLog`、`ReadFileSandbox`；feature `deepseek` 下 `DeepSeekModel` | 适配器 |
| `apps/dsh-runtime` | 配置读取、依赖装配、`axum::serve`、优雅关闭 | 入口 |

依赖方向保持：`http → case → domain → types`，`infra → domain`，`runtime → http/case/infra`。

## 2.2 关键接口

### 2.2.1 模型端口

```rust
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct ModelRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDescriptor>,
    pub max_tokens: Option<u32>,
}

pub enum ModelStep {
    Text { delta: String },
    ToolCalls { calls: Vec<ToolCallRequest>, text: Option<String> },
}

pub trait ModelPort: Send + Sync {
    fn step(&self, request: ModelRequest) -> BoxFuture<'_, Result<ModelStep, Error>>;
    fn name(&self) -> &'static str;
}
```

- 用 `BoxFuture` 而非 `async_trait`，避免新增过程宏依赖，并支持 `Arc<dyn ModelPort>`。
- `ModelStep::ToolCalls` 携带可选文本，便于先输出 chunk 再执行工具。

### 2.2.2 工具接口

```rust
pub struct ToolCallRequest { pub tool_call_id: String, pub name: String, pub arguments: serde_json::Value }

pub enum ToolOutput { Json(serde_json::Value), Text(String) }

pub trait Tool: Send + Sync {
    fn descriptor(&self) -> ToolDescriptor;
    fn call(&self, arguments: serde_json::Value) -> Result<ToolOutput, Error>;
}
```

- 工具保持同步（read_file 为阻塞 IO），由 case 在 `spawn_blocking` + `timeout` 中执行。
- 新增工具不改用例代码（`ToolRegistry` 注册）。

### 2.2.3 运行事件与终结

```rust
pub enum RunEvent {
    Chunk { message_id: String, delta: String },
    ToolCall { tool_call_id: String, name: String, status: ToolCallStatus,
               arguments: Option<Value>, result: Option<Value>, error: Option<Error> },
    Done { finish_reason: FinishReason, usage: Option<Usage> },
    Error { code: String, message: String, retryable: bool, trace_id: Option<String> },
}

pub enum FinishReason { Stop, ToolCalls, Length, Cancelled, Error }
```

- `seq` 由 `EventSink` 分配，从 1 递增；心跳不经过 sink。
- 终态由 `RunOutcome` 统一收口，保证单终态。

### 2.2.4 取消与运行注册

```rust
pub struct CancelSender(tokio::sync::watch::Sender<bool>);
pub struct CancelSignal(tokio::sync::watch::Receiver<bool>);

pub struct RunRegistry { /* Mutex<HashMap<RunId, RunHandle>> + 会话占用集合 */ }
```

- 取消端点按 `runId`/`agentId` 找到 handle，`send(true)`。
- ReAct 每步与工具执行前后检查 `CancelSignal`。
- 会话占用用 RAII guard，运行结束自动释放。

## 2.3 ReAct 状态机

```text
            ┌─────────────┐
            │   Start     │
            └──────┬──────┘
                   ▼
            ┌─────────────┐   取消/超限   ┌──────────┐
            │ Model Step  ├──────────────►│  Finish  │
            └──────┬──────┘               └──────────┘
         文本 │         │ 工具调用
              ▼         ▼
        ┌─────────┐  ┌──────────────┐
        │  Chunk  │  │ Tool Execute │
        └────┬────┘  └──────┬───────┘
             │              ▼
             │        ┌──────────────┐
             │        │ Tool Result  │
             │        └──────┬───────┘
             └───────────────┴──► 下一轮（step+1）
```

伪代码：

```rust
let mut messages = history;
for step in 1..=max_steps {
    check_cancel()?;
    emit tool catalog? // 随请求传给模型，不单独发事件
    let out = timeout(model_timeout, model.step(req)).await??;
    match out {
        Text { delta } => { sink.chunk(delta); messages.push(assistant(delta)); }
        ToolCalls { calls, text } => {
            if let Some(t) = text { sink.chunk(t); }
            for call in calls {
                sink.tool_started(&call);
                let result = spawn_blocking(|| tool.call(args)).timeout(tool_timeout).await;
                match result {
                    Ok(out) => { sink.tool_completed(&call, out); messages.push(tool_msg(out)); }
                    Err(err)  => { sink.tool_failed(&call, err); messages.push(tool_error(err)); }
                }
            }
            continue;
        }
    }
    return Done(Stop);
}
Done(Length)
```

## 2.4 HTTP 与 SSE 设计

### 2.4.1 路由

| 方法 | 路径 | 请求 | 响应 |
| --- | --- | --- | --- |
| GET | `/health` | 无 | `{status, version, contractVersion}` |
| POST | `/api/agent/message` | `AgentMessageRequest` | `AgentMessageResponse` |
| POST | `/api/agent/stream` | `AgentStreamRequest` | `text/event-stream` |
| POST | `/api/agent/cancel` | `AgentCancelRequest` | 运行状态 |

### 2.4.2 错误映射

| ErrorCode | HTTP | 说明 |
| --- | --- | --- |
| `InvalidRequest` | 400 | 参数/JSON/沙箱/大小上限 |
| `NotFound` | 404 | runId/会话不存在 |
| `Conflict` | 409 | 会话并发、重复调用保护、已完成取消 |
| `Timeout` | 504 | 模型/工具超时 |
| `Cancelled` | 200（响应体标注） | 取消结果 |
| `Unavailable` | 503 | 模型不可用 |
| `InvalidResponse` | 502 | 上游响应非法 |
| `Internal` | 500 | 兜底 |

错误体统一 `{code, message, retryable?, traceId?, details?}`。

### 2.4.3 SSE 编码

- 使用 `axum::response::sse::Sse` + `KeepAlive`；
- 事件名与 `sseEventName` 一致；
- `data` 为 JSON（camelCase），`seq` 放在事件 data 内还是 SSE `id`？**契约 fixture 有 `seq` 在 envelope**，线上包装由适配层剥离。实现取 `id: <seq>` 便于客户端去重，同时 fixture 导出时把 `seq` 写回 envelope。
- 断连通过 `Sse` 的 stream drop 触发取消。

## 2.5 工具沙箱设计

```text
DSH_WORKSPACE_ROOT (canonicalize)
        │
        ├─ 拼接输入 path（相对）
        ├─ canonicalize（解析 .. 与符号链接）
        ├─ 必须 starts_with(root)
        ├─ 读取上限 256 KiB（先 metadata 再 read，防超大文件）
        └─ UTF-8 校验 → ToolOutput
```

拒绝情形统一 `InvalidRequest`，`details` 带 `reason`（`outside_root`/`too_large`/`not_utf8`/`not_found`）。

## 2.6 配置

| 环境变量 | 默认 | 说明 |
| --- | --- | --- |
| `DSH_BIND_ADDR` | `127.0.0.1:8090` | 监听地址 |
| `DSH_MODEL` | `fake` | `fake`/`deepseek` |
| `DSH_FAKE_SCRIPT` | `echo` | `echo`/`tool` |
| `DSH_MAX_STEPS` | `8` | ReAct 步数上限 |
| `DSH_TOOL_TIMEOUT_MS` | `5000` | 单次工具超时 |
| `DSH_WORKSPACE_ROOT` | 当前目录 | read_file 根 |
| `DSH_READ_FILE_MAX_BYTES` | `262144` | 读取上限 |
| `DSH_MODEL_TIMEOUT_MS` | `60000` | 模型超时 |
| `DSH_MODEL_BASE_URL` | `https://api.deepseek.com` | 仅 deepseek |
| `DEEPSEEK_API_KEY` | 无 | 仅 deepseek，缺失启动失败 |

## 2.7 契约映射

- `ContentBlock`：`#[serde(tag = "type", rename_all = "lowercase")]` → `{"type":"text","text":"..."}`。
- `ToolDescriptor`：serde rename 为 `inputSchema`/`outputSchema`。
- `Error`：新增 serde 序列化（code 为字符串，`retryable` 可选）。
- DTO 只出现在 `dsh-http` 或 `dsh-types`，领域结构不直接暴露协议字段。

---

# 三、模块级详细设计

## 3.1 dsh-types

- 新增 `serde` / `serde_json` 依赖与 derive；
- `ErrorCode` 增加 `as_str` 映射保持，新增 `Timeout`/`Unavailable` 使用路径；
- `Error` 增加 `to_body()`（HTTP 错误体）或保留在 http 层转换（推荐后者，types 不感知 HTTP）。

## 3.2 dsh-domain

- `model.rs`：`ModelPort`、`ModelRequest`、`ModelStep`、`ChatMessage`。
- `tool.rs`：`Tool`、`ToolOutput`、`ToolCallRequest`（`dsh-types` 已有 `ToolDescriptor`）。
- `run.rs`：`RunEvent`、`FinishReason`、`RunStatus`、`RunRecord`、`Usage`。
- Session 扩展：`messages: Vec<ChatMessage>`（内存）、`active_run: Option<RunId>`。

## 3.3 dsh-case

- `AgentRunner`：编排 ReAct；注入 `Arc<dyn ModelPort>`、`ToolRegistry`、`SessionRepository`、`EventSink`、`CancelSignal`。
- `ToolRegistry`：`HashMap<String, Arc<dyn Tool>>` + 目录排序（保证模型输入稳定）。
- `RunRegistry`：运行注册、取消广播、会话占用。
- `EventSink`：seq 分配与终态保证（`finish_once`）。

## 3.4 dsh-http

- `router.rs`：路由与状态（`AppState`）。
- `dto.rs`：请求/响应 DTO，serde 严格对应契约。
- `error.rs`：`HttpError` 与 `IntoResponse`。
- `sse.rs`：`RunEvent` → SSE；心跳；断连取消。
- `trace.rs`：`traceId` 生成（时间 + 计数，不引入 uuid）。

## 3.5 dsh-infra

- `model/fake.rs`：`FakeModel`，`echo` 与 `tool` 脚本。
- `model/deepseek.rs`（feature）：`reqwest` 客户端、超时、错误映射；只消费最小字段。
- `tools/echo.rs`、`tools/clock.rs`、`tools/read_file.rs`。
- `storage/memory.rs`：`InMemoryRunStore`、`RunEventLog`。
- `sandbox.rs`：`ReadFileSandbox`。

## 3.6 apps/dsh-runtime

- 读取环境变量 → 构造依赖 → 启动 `axum::serve`；
- `tokio::signal` 优雅关闭；
- 启动日志（地址/模型/工具数/契约版本）。

---

# 四、技术选型与约束

## 4.1 新增依赖清单

| 依赖 | 版本 | 作用域 | 理由 | 备选 |
| --- | --- | --- | --- | --- |
| `axum` | `0.8` | `dsh-http` | 路由、提取器、SSE 原生支持 | actix-web（生态差异）、hyper 裸用（工作量大） |
| `tokio` | `1`（rt-multi-thread/macros/time/sync/signal/net） | `dsh-http`、`dsh-runtime` | 异步运行时与 `spawn_blocking`/`timeout`/`watch` | async-std（生态弱） |
| `serde` | `1`（derive） | `dsh-types`、`dsh-http` | 契约序列化 | 手写序列化（易错） |
| `serde_json` | `1` | `dsh-types`、`dsh-case`、`dsh-infra` | 工具参数/结果与 JSON Schema | 无（标准库无 JSON） |
| `tracing` | `0.1` | 全层（可选） | 结构化日志与 traceId | `log`（能力弱） |
| `tracing-subscriber` | `0.3`（env-filter） | `dsh-runtime` | 日志初始化 | 无 |
| `reqwest` | `0.12`（rustls-tls，可选 feature `deepseek`） | `dsh-infra` | DeepSeek HTTP 客户端 | hyper 手写（成本高） |

**约束**：

1. 依赖必须低于或等于 MSRV 1.85；CI 增 `cargo check --all-features`。
2. 除上述清单外不新增依赖；如需新增，先在本文档登记。
3. `dsh-domain` 不依赖 axum/tokio（`BoxFuture` 使用 std 的 `Future`）。
4. `tokio` 仅出现在 `dsh-http`/`dsh-runtime`/可选适配器；`dsh-case` 用 `futures`? **不引入**，case 只返回 `BoxFuture` 并在 runtime 驱动。

> 决策记录：`dsh-domain` 只用 std 的 `Future`（`BoxFuture`），不依赖 tokio；`dsh-case` 需要超时与取消广播，允许依赖 `tokio` 的 `time`、`sync`（不启用 `rt`），由 `dsh-runtime` 提供运行器。

## 4.2 版本与 feature

- `deepseek` feature 默认关闭；`cargo test --all-features` 需编译通过（不联网）。
- DeepSeek 集成测试标记 `#[ignore]`，仅在配置密钥时手动执行。

## 4.3 与 0.1.0 约束的冲突与处理

| 冲突 | 处理 |
| --- | --- |
| `dsh-types` 零依赖 | 0.2.0 打破：新增 serde/serde_json，更新 README/AGENTS 描述 |
| `dsh-http` 零依赖 | 0.2.0 打破：新增 axum/tokio |
| “暂不绑定框架” | 本版正式选型 Axum + Tokio，记录于本节 |

---

# 五、测试策略

| 类型 | 范围 | 关键用例 |
| --- | --- | --- |
| 单元 | ReAct 状态机 | 单步停止、工具一次、工具失败、步数上限、取消 |
| 单元 | 沙箱 | 相对路径、`..` 逃逸、符号链接、超限、非 UTF-8 |
| 单元 | SSE | seq 递增、单终态、心跳不占 seq |
| 集成 | HTTP | health、阻塞消息、流式、取消、错误映射 |
| 契约 | 回放 | 新增 4 组 fixtures（总计 8 组）逐事件比对 |
| 并发 | 会话 | 同会话并发冲突、重复取消幂等 |

测试基线目标：0.1.0 的 11 个 + 新增 ≥ 25 个；`cargo test` 全绿。

---

# 六、风险与回滚

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 新增依赖抬高 MSRV | 中 | CI 固定 1.85 检查；依赖评估 |
| 首次框架选型不可逆 | 中 | 路由/DTO 与框架解耦；`dsh-http` 隔离 |
| `read_file` 路径逃逸 | 高 | canonicalize + starts_with + 单测负面用例 |
| SSE 终态重复/缺失 | 高 | `EventSink::finish_once` + 集成断言 |
| 取消竞态 | 中 | watch 广播 + 幂等测试 |
| 同会话并发写乱序 | 中 | 会话占用 guard + `conflict` |
| 契约字段漂移 | 中 | 回放测试 + schema 校验 |

**回滚**：0.2.0 不涉及数据库迁移；回滚到 0.1.0 只需回退版本与新增 crate 代码。默认监听回环地址，风险面可控。

---

# 七、实施计划

| 阶段 | 内容 | 出口 |
| --- | --- | --- |
| M1 装配与 HTTP | 依赖、DTO、路由、health、错误映射 | `/health` 与阻塞端点可跑（echo 模型） |
| M2 ReAct 与工具 | 状态机、FakeModel、echo/clock/read_file、沙箱 | 阻塞与流式可完成工具调用 |
| M3 SSE 与取消 | 事件编码、seq/终态、心跳、cancel、并发 | 流式与取消验收通过 |
| M4 契约与收口 | fixtures、回放、openapi、门禁、文档 | 四条门禁全绿，版本收口 |

**收口条件**：`spec.md` §7 全部满足，且新增依赖与契约变更均已归档。
## 4.4 实施回填（2026-11-27）

实现与本文规划的差异与补充，供后续维护对照：

| 项 | 规划 | 实施 |
| --- | --- | --- |
| 依赖 | 未列 `futures-util`、`tower`（dev） | `dsh-http` 使用 `futures-util` 实现 SSE `Stream`；测试用 `tower::ServiceExt` |
| `serde` 作用域 | `dsh-types`/`dsh-http` | 另用于 `dsh-infra`（DeepSeek 响应解析） |
| 领域 JSON | 未明确 | `dsh-types::JsonValue` 别名，避免 `dsh-domain` 直接依赖 serde_json |
| 工具目录 | 规划未定数据结构 | `BTreeMap`，保证目录与执行顺序稳定 |
| 取消幂等 | 仅要求“重复取消返回一致结果” | `RunRegistry` 以 `finished` + `last_agent_run` 保留最近终态（上限 1024 条） |
| SSE seq | 规划仅要求递增 | 线上以 SSE `id: <seq>` 携带，fixture 以 `{seq,event,data}` 封装 |
| 事件日志 | 可导出 NDJSON | 提供 `InMemoryRunEventLog` API，未暴露 HTTP 端点（列入已知限制） |
| DeepSeek | 最小字段消费 | 非流式请求 + 文本增量输出；未消费上游 token 流（列入已知限制） |
