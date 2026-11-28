# DeepSeek Harness Rust v0.2.0 需求规格

> 版本：`v0.2.0` | 基线：v0.1.0 | 主线：**可运行 HTTP + ReAct 最小闭环** | 契约版本：`1.0.0-draft.1`
>
> **版本定位**：v0.2.0 是 Rust 版**首个可运行版本**。在 0.1.0 骨架之上接入 HTTP/SSE 服务与 ReAct 循环，让“一次请求 → 模型推理 → 工具调用 → 结果回填 → 终止”真实跑通，并严格按 `contracts/v1` 输出。
>
> **选型声明**：Web/异步采用 **Axum 0.8 + Tokio 1**；序列化采用 **serde/serde_json**；模型默认 **FakeModel**，真实 DeepSeek 适配器作为可选 feature（`deepseek`）。
>
> **依赖声明**：本版新增第三方依赖（清单与理由见 `design.md` §4.1）；**不新增 workspace 成员**，能力落在既有 crate 与 `dsh-runtime`。
>
> **数据声明**：本版**不新增数据库**。会话、事件日志与运行状态均为进程内存储，持久化留待 0.2.1。
>
> **契约声明**：本版按 `contracts/v1/http` 与 `contracts/v1/sse` 实现；契约版本保持 `1.0.0-draft.1`，录制校验通过后于 0.2.x 收口时评估升 `1.0.0`。

---

# **1. 组件定位**

## **1.1 核心职责**

1. **HTTP 服务**：提供 `/health`、`POST /api/agent/message`、`POST /api/agent/stream`、`POST /api/agent/cancel` 四个端点。
2. **ReAct 最小循环**：在步数上限内完成“模型推理 → 工具调用 → 工具结果回填 → 再推理 → 终止”。
3. **模型适配**：默认 FakeModel（可脚本化），可选 DeepSeek API 适配器（feature 门控）。
4. **工具执行**：内置 `echo`、`clock`、`read_file` 三个工具，带超时与路径沙箱。
5. **事件流**：按 `sse/catalog.md` 输出 `chunk`/`tool_call`/`done`/`error`，保证 seq 递增、单终态与心跳语义。
6. **取消**：支持按 `runId` 取消运行，取消后以 `done(cancelled)` 结束。
7. **契约对齐**：响应与事件结构对齐 `contracts/v1`，并提供 fixture 回放验证。

## **1.2 核心输入**

1. **Agent 消息请求**：`{agentId, sessionId?, content[], model?, workspaceId?, metadata?}`。
2. **流式请求**：与阻塞式请求同构（`/api/agent/stream`）。
3. **取消请求**：`{agentId, runId?, reason?}`。
4. **模型配置**：`DSH_MODEL`（`fake`/`deepseek`）、`DSH_MAX_STEPS`、`DSH_MODEL_BASE_URL`、`DEEPSEEK_API_KEY`（仅 deepseek）。
5. **工具配置**：`DSH_WORKSPACE_ROOT`、`DSH_READ_FILE_MAX_BYTES`、`DSH_TOOL_TIMEOUT_MS`。
6. **服务配置**：`DSH_BIND_ADDR`。

## **1.3 核心输出**

1. **阻塞响应**：`{agentId, sessionId, runId, message, toolCalls?, usage?, finishReason}`。
2. **SSE 流**：`chunk`/`tool_call`/`done`/`error` 事件序列，附 `:keepalive`。
3. **取消响应**：取消受理结果（含 `runId` 与状态）。
4. **错误体**：`{code, message, retryable?, traceId?, details?}`。
5. **运行记录**：进程内 `Run` 状态（`pending`/`running`/`completed`/`failed`/`cancelled`）。
6. **事件日志**：可导出 NDJSON，用于 fixture 回放。

## **1.4 职责边界**

本组件**不负责**：

1. **插件宿主与工具动态加载**：工具为内置静态注册，插件留待 0.2.1。
2. **持久化**：会话、运行、事件日志均为内存实现，重启即丢。
3. **审批与权限矩阵**：不实现人工审批与细粒度授权。
4. **鉴权**：默认仅监听回环地址，不提供认证（生产接入留待后续版本）。
5. **多租户与计费**：不做租户隔离、配额与计费。
6. **模型流式 token 的完整透传**：FakeModel 按脚本产出；DeepSeek 适配器按最小字段消费。
7. **契约冻结**：v1 仍为草案，字段调整需同步 `contracts/CHANGELOG.md`。

---

# **2. 领域术语**

**运行（Run）**
:: 一次 Agent 执行，拥有 `runId` 与状态机；同一会话可有多次运行。

**ReAct 循环**
:: “推理 → 行动 → 观察”的迭代；本版步数上限默认 8。

**步骤（step）**
:: 一次模型调用及其后续工具执行；达到上限以 `length` 终止。

**模型端口（ModelPort）**
:: 领域端口，输入消息历史与工具目录，输出文本增量或工具调用。

**FakeModel**
:: 默认模型实现，按脚本返回文本或触发指定工具，无需网络与密钥。

**工具（Tool）**
:: 具备名称、描述、输入 Schema 的可执行能力；本版内置 `echo`/`clock`/`read_file`。

**工具调用三段式**
:: `tool_call` 事件的 `started` → `completed`/`failed`，同一流内 `toolCallId` 唯一。

**终态事件**
:: `done` 或 `error`，每个流恰好一个且最后出现。

**取消（cancel）**
:: 按 `runId` 终止运行；以 `done` + `finishReason: cancelled` 收尾。

**工作区根（workspace root）**
:: `read_file` 的允许访问根目录，默认进程当前目录；路径解析后必须位于该根内。

**契约回放**
:: 用 `contracts/fixtures` 的 NDJSON 与响应样例驱动运行时，并逐事件比对。

---

# **3. 角色与边界**

## **3.1 核心角色**

1. **调用方**：桌面端、移动端、脚本或第三方，通过 HTTP/SSE 调用 Agent。
2. **运维者**：设置监听地址、模型与工具配置，观察日志与健康检查。
3. **开发者**：新增工具/模型适配器，扩展 ReAct 能力。
4. **契约维护者**：维护 fixtures 与 schema，保证 Rust 输出与契约一致。

## **3.2 外部系统**

1. **模型服务**：DeepSeek API（可选 feature）；默认不依赖。
2. **文件系统**：`read_file` 读取工作区内文件。
3. **契约目录**：`contracts/v1` 与 `fixtures`。
4. **CI**：fmt/clippy/test/contract 四条门禁。

## **3.3 交互上下文**

```text
调用方 ──POST /api/agent/message──► dsh-http ──► dsh-case（ReAct）
   │                                    │              │
   │◄──── JSON 响应（message/toolCalls）─┘              ├─► ModelPort（Fake/DeepSeek）
   │                                                   ├─► ToolRegistry → echo/clock/read_file
   └──POST /api/agent/stream──► SSE ◄── RunEvent ──────┘
                                  ▲
调用方 ──POST /api/agent/cancel────┘（按 runId）
```

---

# **4. DFX 约束**

## **4.1 性能**

1. **启动即用**：`cargo run -p dsh-runtime` 后无需额外依赖即可响应请求。
2. **流式优先**：`/api/agent/stream` 的首个事件应在模型首步完成后立即发送，不等待整个运行结束。
3. **工具超时**：单次工具执行默认 5 秒超时，超时以 `failed` + `timeout` 结束该次调用。
4. **步数上限**：默认 8 步，防止无限循环与资源耗尽。

## **4.2 可靠性**

1. **单终态**：每个 SSE 流恰好一个 `done` 或 `error`，且最后出现。
2. **取消幂等**：重复取消同一 `runId` 返回一致结果，不产生第二个终态。
3. **会话一致性**：同一 `sessionId` 并发运行须串行化或拒绝（`conflict`），不得交错写入。
4. **错误可诊断**：模型/工具/参数失败必须返回稳定 `code` 与可读 `message`。
5. **超时与断连**：客户端断开后运行须被取消并释放工具资源。

## **4.3 安全性**

1. **回环默认**：默认监听 `127.0.0.1:8090`，不默认暴露公网。
2. **路径沙箱**：`read_file` 解析后必须位于工作区根内，拒绝 `..` 与符号链接逃逸。
3. **大小上限**：`read_file` 默认最多读取 256 KiB，超限返回 `invalid_request`（`details.limit`）。
4. **仅文本**：非 UTF-8 内容拒绝，不做二进制透传。
5. **密钥不落日志**：`DEEPSEEK_API_KEY` 不得出现在日志、错误体或事件中。
6. **unsafe 禁用**：沿用 workspace `unsafe_code = "forbid"`。

## **4.4 可维护性**

1. **分层不越界**：HTTP 只做协议转换；ReAct 与工具编排在 `dsh-case`；模型/工具端口在 `dsh-domain`；适配器在 `dsh-infra`。
2. **契约先行**：事件与响应结构以 schema 为准，类型变更先改契约。
3. **错误统一**：沿用 `dsh-types::Error`/`ErrorCode`，不新增 crate 专属错误枚举。
4. **可测试**：FakeModel + 内置工具保证 CI 无网络、无外部状态。

## **4.5 兼容性**

1. **契约版本**：保持 `1.0.0-draft.1`；字段新增遵循兼容新增。
2. **API 枚举开放**：`finishReason` 等枚举消费者须有 default 分支（与契约一致）。
3. **MSRV**：Rust 1.85、edition 2024；新增依赖不得抬高 MSRV 超过 1.85。
4. **平台**：Linux/macOS 优先；`read_file` 路径语义按 Unix 与 Windows 分别处理绝对路径与分隔符。

---

# **5. 核心能力**

## **5.1 HTTP 服务**

### **5.1.1 Business Rules**

1. **健康检查**：`GET /health` 返回 `200` 与 `{status:"ok", version, contractVersion}`。
2. **阻塞消息**：`POST /api/agent/message` 接收 `AgentMessageRequest`，返回 `AgentMessageResponse`。
3. **流式消息**：`POST /api/agent/stream` 接收同构请求，返回 `text/event-stream`。
4. **取消**：`POST /api/agent/cancel` 接收 `AgentCancelRequest`，返回 `200`/`202` 与运行状态。
5. **错误映射**：`invalid_request→400`、`not_found→404`、`conflict→409`、`timeout→504`、`unavailable→503`、`invalid_response→502`、`internal→500`；错误体统一为 `common/error`。
6. **请求校验**：`agentId` 必填且符合标识符规则；`content` 至少 1 项且仅文本；未知字段允许（`additionalProperties`）。
7. **内容类型**：JSON 请求与响应使用 `application/json`；SSE 使用 `text/event-stream`。

### **5.1.2 Acceptance Criteria**

1. [`GET /health`] → [`200` 且字段齐全]
2. [请求缺 `agentId` 或空 `content`] → [`400` + `invalid_request`]
3. [非法 JSON] → [`400` + `invalid_request`，消息指出位置]
4. [取消不存在的 `runId`] → [`404` + `not_found`]

## **5.2 SSE 流式输出**

### **5.2.1 Business Rules**

1. **事件集**：只允许 `chunk`、`tool_call`、`done`、`error`。
2. **seq**：从 1 开始严格递增，不跳号、不复用；心跳不占 seq。
3. **心跳**：空闲时按固定间隔发送 `:keepalive`，不计入业务事件与 fixture。
4. **终态**：每个流恰好一个终态，且最后出现；取消时 `done` + `finishReason: cancelled`。
5. **工具事件完整性**：`completed` 必须带 `result`；`failed` 必须带 `error`。
6. **未知事件**：服务端不产生未知事件；客户端忽略未知事件不得中断流（契约要求，服务端在 fixtures 中体现）。
7. **背压与断连**：客户端断开时停止推送并触发取消。

### **5.2.2 Acceptance Criteria**

1. [正常流] → [`chunk*` → `done(stop)`，seq 连续]
2. [触发工具] → [`tool_call(started)` → `tool_call(completed)` → `chunk*` → `done(stop)`]
3. [取消] → [不再产生 `chunk`，以 `done(cancelled)` 结束]
4. [模型错误] → [`error(code,message)` 为唯一终态，无 `done`]

## **5.3 ReAct 最小循环**

### **5.3.1 Business Rules**

1. **循环结构**：构造消息历史 → 调用模型 → 若返回工具调用则执行并回填 → 重复，直至终止。
2. **步数上限**：默认 8（`DSH_MAX_STEPS`）；达到上限以 `finishReason: length` 结束。
3. **终止原因**：`stop`（模型结束）、`tool_calls`（仅当响应要求继续执行工具且流结束于此，用于阻塞响应）、`length`（步数上限）、`cancelled`、`error`。
4. **工具目录注入**：每步向模型提供当前工具目录（名称、描述、输入 Schema）。
5. **工具结果回填**：工具输出作为 `tool` 角色消息进入下一轮。
6. **重复调用保护**：同一轮内同名工具+同参数重复调用超过 2 次视为异常，返回 `conflict`。
7. **空响应**：模型返回空文本按 `stop` 处理并记录告警。

### **5.3.2 Acceptance Criteria**

1. [FakeModel 回显] → [响应 `message.content` 含用户文本，`finishReason=stop`]
2. [触发 clock] → [响应 `toolCalls` 含 `clock` 且 `status=completed`]
3. [超过步数上限] → [`finishReason=length`，运行状态 `completed`]
4. [模型不可用] → [`finishReason=error` 或 `error` 终态，`code=unavailable`]

## **5.4 模型适配**

### **5.4.1 Business Rules**

1. **统一端口**：`ModelPort` 接受消息历史与工具目录，返回文本增量或工具调用。
2. **FakeModel 默认**：`DSH_MODEL=fake`（默认）；按 `DSH_FAKE_SCRIPT` 选择 `echo`（默认）或 `tool`（触发一次 `clock`）。
3. **DeepSeek 可选**：`DSH_MODEL=deepseek` 且启用 `deepseek` feature 时生效；未启用则启动失败并给出可读指引。
4. **配置缺失**：`deepseek` 模式缺少 `DEEPSEEK_API_KEY` 时启动失败（`unavailable`），不降级为 Fake。
5. **超时与重试**：模型请求默认 60 秒超时；网络错误映射 `unavailable`，超时映射 `timeout`。
6. **流式消费**：DeepSeek 适配器按最小字段消费增量文本与工具调用，不承诺完整透传。

### **5.4.2 Acceptance Criteria**

1. [未启用 feature 却配置 deepseek] → [启动失败 + 指引]
2. [deepseek 缺 key] → [启动失败 + `unavailable`]
3. [Fake 默认] → [无网络即可完成一次运行]
4. [模型超时] → [运行以 `error(timeout)` 结束]

## **5.5 工具注册与执行**

### **5.5.1 Business Rules**

1. **工具目录**：`echo`、`clock`、`read_file`，描述与输入 Schema 对齐 `tool.schema.json`。
2. **echo**：输入 `{text}`，输出 `{text}` 原样回显。
3. **clock**：无输入，输出 `{now}`（RFC3339，UTC）。
4. **read_file**：输入 `{path}`，输出 `{path, content, bytes}`；路径相对工作区根。
5. **沙箱**：解析后路径必须位于 `DSH_WORKSPACE_ROOT` 内（默认进程当前目录）；拒绝 `..` 与符号链接逃逸。
6. **大小上限**：默认 256 KiB（`DSH_READ_FILE_MAX_BYTES`），超限 `invalid_request` 并在 `details.limit` 说明。
7. **超时**：单次工具默认 5 秒（`DSH_TOOL_TIMEOUT_MS`），超时 `timeout`。
8. **执行失败**：工具错误以 `tool_call(failed)` + `error` 表示，运行可继续由模型决定是否重试。

### **5.5.2 Acceptance Criteria**

1. [`echo` 调用] → [`completed` 且 `result.text` 等于输入]
2. [`read_file` 读取工作区内文件] → [`completed` 且 `bytes` 与内容长度一致]
3. [`read_file` 传 `../etc/passwd`] → [`failed` + `invalid_request`]
4. [超过大小上限] → [`failed` + `invalid_request` + `details.limit`]
5. [工具超时] → [`failed` + `timeout`]

## **5.6 取消**

### **5.6.1 Business Rules**

1. **取消键**：优先按 `runId`；缺省时按 `agentId` 取消该 Agent 当前运行。
2. **传播**：取消信号传递到模型调用与工具执行，尽快中止。
3. **收尾**：SSE 以 `done(cancelled)` 结束；阻塞请求返回 `finishReason=cancelled`。
4. **幂等**：重复取消返回同样状态，不重复产生终态。
5. **竞态**：运行已完成时取消返回 `conflict` 或当前终态（实现取一致策略并在设计中记录）。

### **5.6.2 Acceptance Criteria**

1. [运行中取消] → [`done(cancelled)` 且无后续 `chunk`]
2. [重复取消] → [状态一致，无第二终态]
3. [取消已完成运行] → [返回当前终态信息]

## **5.7 会话与事件日志（内存）**

### **5.7.1 Business Rules**

1. **会话**：沿用 0.1.0 `Session`；新增 `runId` 关联的运行记录。
2. **运行状态**：`pending`/`running`/`completed`/`failed`/`cancelled`，与 `enums.runStatus` 一致。
3. **消息历史**：同会话消息按序保存于内存，供下一轮模型使用。
4. **事件日志**：每次运行记录有序事件（seq、event、data），可导出 NDJSON。
5. **并发**：同一会话同时只允许一个运行，第二个请求返回 `conflict`。
6. **非持久化**：重启即丢，不提供恢复。

### **5.7.2 Acceptance Criteria**

1. [同会话连续两次请求] → [第二次基于第一次历史]
2. [同会话并发请求] → [一个成功，一个 `409 conflict`]
3. [导出事件] → [NDJSON 每行一个业务事件，心跳不出现]

## **5.8 契约对齐与回放**

### **5.8.1 Business Rules**

1. **结构对齐**：响应/事件字段名与 `contracts/v1` 一致（camelCase）。
2. **fixtures 补齐**：新增阻塞、流式、工具调用、取消四组样例；样例可由运行时导出并归一化。
3. **回放测试**：以 fixtures 驱动运行时，逐事件比对（seq、event、关键字段）。
4. **归一化**：只允许时间、随机 ID、耗时、绝对路径归一化；事件数量与终态不得归一化。
5. **openapi**：路由稳定后由路由表生成 `contracts/v1/openapi.yaml`，纳入 CI 再生成校验。
6. **契约版本**：保持 `1.0.0-draft.1`；录制校验全部通过后评估升 `1.0.0`。

### **5.8.2 Acceptance Criteria**

1. [`node contracts/tools/validate.mjs`] → [0 警告]
2. [回放新增 4 组 fixtures（总计 8 组）] → [全部通过]
3. [openapi 再生成] → [与提交版本一致，无差异]

## **5.9 配置与可观测性**

### **5.9.1 Business Rules**

1. **环境变量**：`DSH_BIND_ADDR`、`DSH_MODEL`、`DSH_FAKE_SCRIPT`、`DSH_MAX_STEPS`、`DSH_TOOL_TIMEOUT_MS`、`DSH_MODEL_TIMEOUT_MS`、`DSH_WORKSPACE_ROOT`、`DSH_READ_FILE_MAX_BYTES`、`DEEPSEEK_API_KEY`、`DSH_MODEL_BASE_URL`。
2. **默认值**：回环地址 `127.0.0.1:8090`、fake 模型、8 步、5 秒工具超时、256 KiB。
3. **启动日志**：打印监听地址、模型类型、工具数量与契约版本；不打印密钥。
4. **请求追踪**：每个请求生成 `traceId`，错误体与日志携带；不写入响应正文之外的位置。
5. **健康检查**：`/health` 反映进程存活与版本，不探测外部依赖。

### **5.9.2 Acceptance Criteria**

1. [未设置任何环境变量] → [以默认配置启动并可用]
2. [启动日志] → [不含 `DEEPSEEK_API_KEY` 值]

## **5.10 测试与质量门禁**

### **5.10.1 Business Rules**

1. **单元测试**：ReAct 状态机、工具沙箱、请求校验、错误映射、SSE 排序。
2. **集成测试**：`/health`、阻塞消息、流式、取消；使用 FakeModel 与临时目录。
3. **契约回放**：新增 4 组 fixtures 全通过；既有 4 组随路由校准。
4. **门禁**：`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`、`node contracts/tools/validate.mjs` 全绿。
5. **覆盖基线**：核心用例（state machine/tool sandbox/cancel）必须有负面用例。

### **5.10.2 Acceptance Criteria**

1. [任一测试失败] → [CI 红灯]
2. [新增依赖未记录] → [评审阻断]

---

# **6. 数据约束**

1. **无数据库**：不新增表、不新增迁移。
2. **内存态**：会话、运行、事件日志、消息历史均为进程内。
3. **事件顺序**：`seq` 单流内严格递增，从 1 开始。
4. **ID**：`runId` 单进程唯一；格式沿用受限标识符，建议 `run-<时间戳>-<计数>`。
5. **契约版本**：`1.0.0-draft.1`，与产品 `0.2.0` 独立。
6. **文件读取**：仅 UTF-8 文本，默认上限 256 KiB，限定工作区根。

---

# **7. 验收标准（版本级）**

1. `cargo run -p dsh-runtime` 启动后 `/health` 返回 200。
2. 阻塞请求可完成一次 ReAct（含至少一次工具调用）并返回契约结构。
3. 流式请求事件序列符合 `sse/catalog.md`。
4. 取消请求可终止运行并以 `done(cancelled)` 收尾。
5. `read_file` 沙箱与大小上限的负面用例全部通过。
6. 新增 4 组 fixtures（总计 8 组）回放通过；`validate.mjs` 0 警告。
7. 四条门禁全绿；新增依赖清单与理由归档于 `design.md`。

---

# **8. 边界与后续版本**

| 能力 | v0.2.0 | 后续版本 |
| --- | --- | --- |
| 插件宿主/manifest | 无 | 0.2.1 |
| 持久化与回放存储 | 无（仅内存导出） | 0.2.1 |
| 审批与权限矩阵 | 无 | 0.2.1 |
| 鉴权/多租户 | 无 | 0.2.2+ |
| 完整模型能力（多模态、函数调用高级特性） | 最小字段 | 后续对齐 |
| 契约冻结 | draft.1 | 录制通过后升 1.0.0 |

---

## 附：v0.2.0 能力清单

| 序号 | 能力 | 归属 | 状态 |
| --- | --- | --- | --- |
| 1 | HTTP 四端点 | §5.1 | 规划 |
| 2 | SSE 事件流 | §5.2 | 规划 |
| 3 | ReAct 最小循环 | §5.3 | 规划 |
| 4 | FakeModel + DeepSeek 可选 | §5.4 | 规划 |
| 5 | 工具 echo/clock/read_file | §5.5 | 规划 |
| 6 | 取消与幂等 | §5.6 | 规划 |
| 7 | 会话与事件日志（内存） | §5.7 | 规划 |
| 8 | 契约 fixtures 与回放 | §5.8 | 规划 |
| 9 | 配置与可观测性 | §5.9 | 规划 |
| 10 | 测试与门禁 | §5.10 | 规划 |