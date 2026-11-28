# DeepSeek Harness Rust v0.1.0 需求规格

> 版本：`v0.1.0` | 主线：骨架初始化（契约先行 + 分层工作区 + 最小闭环） | 参考源：`deepseek-harness` | Java 对照：`dsh-java`
>
> **版本定位**：v0.1.0 是 Rust 版的**首个骨架版本**。目标不是功能完整，而是把分层、契约、错误模型、质量门禁固定下来，为后续 HTTP、ReAct、插件宿主与持久化迭代提供稳定基座。
>
> **范围声明**：本版本只交付库 crate、常量、内存适配与可执行入口；不启动 HTTP 服务、不调用模型、不实现工具执行与审批。
>
> **契约声明**：`contracts/VERSION` 独立于产品版本，当前 `1.0.0-draft.1`；`contracts/v1` 为草案，允许随录制结果调整，但每次调整必须同步 `contracts/CHANGELOG.md` 与 `dsh-types`。
>
> **依赖声明**：v0.1.0 **不新增任何第三方依赖**，仅使用标准库与 Cargo workspace 机制；Node.js 仅用于契约校验脚本。

---

# **1. 组件定位**

## **1.1 核心职责**

本组件是 DeepSeek Harness 的 Rust 实现骨架，负责：

1. **契约类型**：以 `contracts/v1` 为唯一真源，在 `dsh-types` 中提供对应的值对象、标识符与错误模型。
2. **领域与端口**：在 `dsh-domain` 中定义会话聚合与 `SessionRepository` 端口，保持与传输、存储无关。
3. **用例编排**：在 `dsh-case` 中完成运行请求校验与会话确保（不存在则创建并保存）。
4. **接入常量**：在 `dsh-http` 中固定 HTTP 路由与 SSE 事件常量，服务实现留待后续版本。
5. **适配实现**：在 `dsh-infra` 中提供内存会话仓库，作为最小可运行实现。
6. **可执行入口**：`apps/dsh-runtime` 装配依赖并输出自检信息，验证链路可用。

## **1.2 核心输入**

1. **运行请求**：`RunRequest { agent_id, session_id?, text }`，来自后续 HTTP 层或测试。
2. **会话标识**：符合契约规则的受限标识符（1–128 字节，字母/数字/`._:-`）。
3. **契约 schema**：`contracts/v1/**/*.schema.json` 与 `contracts/openapi.yaml`。
4. **构建指令**：`cargo build` / `cargo test` / `cargo clippy` / `cargo fmt`。
5. **契约校验指令**：`node contracts/tools/validate.mjs`。

## **1.3 核心输出**

1. **类型化值对象**：`ContentBlock`、`ToolDescriptor`、`JsonSchema`、`AgentId`/`SessionId`/`RunId`/`ToolCallId`。
2. **结构化错误**：`Error { code, message, retryable }`，`code` 取稳定枚举 `ErrorCode`。
3. **会话聚合**：`Session { id, title? }`。
4. **用例结果**：校验通过返回 `Ok(())`；空文本返回 `invalid_request`。
5. **路由常量**：`/health`、`/api/agent/message`、`/api/agent/stream`、`/api/agent/cancel` 与插件管理路由。
6. **自检输出**：产品版本、契约版本、路由常量、会话仓库空态。
7. **质量结果**：fmt/clippy/test/contract 四条门禁的通过状态。

## **1.4 职责边界**

本组件**不负责**：

1. **HTTP 服务与 SSE 传输实现**：v0.1.0 只固定路径与事件常量，不绑定 Web 框架。
2. **模型调用与 ReAct 状态机**：无 LLM 适配器、无工具执行、无多步循环。
3. **插件宿主**：不加载插件、不解析 manifest、不做工具注册与回收。
4. **持久化**：只有进程内 `HashMap` 会话仓库，重启即丢；数据库与事件日志为后续版本。
5. **鉴权与审批**：无认证、无审批流、无权限矩阵。
6. **前端与桌面端**：属姊妹仓库范围。
7. **契约冻结**：v1 仍为草案，未承诺字段稳定。

---

# **2. 领域术语**

**契约（contract）**
:: `contracts/` 下跨进程、跨仓库、可回放的协议描述；只描述边界，不描述 Rust crate 内部类型。

**契约版本（contract version）**
:: `contracts/VERSION` 中的字符串，当前 `1.0.0-draft.1`，与产品版本独立演进；破坏性变更须新开大版本目录。

**草稿（draft）**
:: 契约状态之一：结构已设计，字段仍可能随基线录制调整。

**夹具（fixture）**
:: `contracts/fixtures/` 下的录制样例，用于录制-回放-差分；当前为手写样例，尚未从 Java 基线录制。

**受限标识符（Id）**
:: 1–128 字节，仅允许 ASCII 字母、数字与 `._:-`；空串、超长、非法字符均拒绝。

**会话聚合（Session）**
:: 本版本仅有 `id` 与可选 `title` 的骨架聚合，不含事件流与状态机。

**会话仓储端口（SessionRepository）**
:: 领域端口，提供 `load` 与 `save`；同步签名，便于先用内存实现跑通用例。

**运行请求（RunRequest）**
:: `agent_id` + 可选 `session_id` + `text`；`text` 去空白后为空即拒绝。

**骨架版本（skeleton release）**
:: 只保证可编译、可测试、可运行与契约一致，不承诺功能完整。

---

# **3. 角色与边界**

## **3.1 核心角色**

1. **Rust 开发者**：在 workspace 内扩展 crate、补充端口实现与用例。
2. **契约维护者**：维护 `contracts/v1` schema、fixture 与策略，保证 Rust 类型与契约同步。
3. **AI 编码助手**：依据 `AGENTS.md`、`llms.txt` 与四件套在既定分层内扩展代码。
4. **后续运行时调用方**：HTTP 层、桌面端与第三方，通过契约消费行为（后续版本）。

## **3.2 外部系统**

1. **Java 基线（dsh-java）**：契约录制来源与行为对照。
2. **参考源（deepseek-harness）**：能力面与演进方向。
3. **CI（GitHub Actions）**：执行 fmt/clippy/test 与契约校验。
4. **Node.js**：契约校验脚本运行时。

## **3.3 交互上下文**

```text
                 ┌─────────────────────────────┐
                 │  apps/dsh-runtime（自检）     │
                 └──────────────┬──────────────┘
                                │
   ┌─────────────┐      ┌───────▼────────┐      ┌──────────────┐
   │ dsh-http    │      │ dsh-case       │      │ dsh-infra    │
   │ 路由/事件常量│      │ 校验/会话编排   │      │ 内存会话仓储  │
   └─────────────┘      └───────┬────────┘      └──────┬───────┘
                                │ 依赖                  │ 实现
                        ┌───────▼────────┐      ┌───────▼───────┐
                        │ dsh-domain     │◄─────│ SessionRepository│
                        └───────┬────────┘      └───────────────┘
                                │
                        ┌───────▼────────┐
                        │ dsh-types      │◄──── contracts/v1
                        └────────────────┘
```

---

# **4. DFX 约束**

## **4.1 性能**

1. **零额外 IO**：类型、校验与内存仓储不得引入文件、网络或数据库 IO。
2. **依赖最小**：`dsh-types` 与 `dsh-http` 保持零第三方依赖；`dsh-domain` 只依赖 `dsh-types`。
3. **构建可控**：不引入异步运行时与 Web 框架，避免骨架阶段编译膨胀。

## **4.2 可靠性**

1. **标识符校验**：空串、超长（>128 字节）、非法字符必须返回对应 `IdError`。
2. **请求校验**：空白文本必须返回 `invalid_request`，不得进入会话创建。
3. **仓储一致性**：`save` 覆盖同 ID 记录；`load` 不存在返回 `Ok(None)`。
4. **锁中毒处理**：内存仓储锁中毒必须返回 `internal`，不得 panic。

## **4.3 安全性**

1. **unsafe 禁用**：workspace 级 `unsafe_code = "forbid"`。
2. **调试宏禁用**：`dbg_macro = "deny"`。
3. **秘密不入库**：骨架阶段不接触密钥；后续接入模型与插件时，令牌不得进入日志或 DTO。
4. **标识符白名单**：只允许受限字符集，降低路径/协议注入风险。

## **4.4 可维护性**

1. **分层不越界**：`case → domain → types`；`http`/`infra` 实现外侧能力，不得反向依赖。
2. **错误模型统一**：所有库错误使用 `Error`/`ErrorCode`，不新增 crate 专属错误枚举。
3. **契约先行**：schema 与 fixture 先于 Rust 类型变更。
4. **文档四件套**：需求、设计、任务、开发说明同目录归档。

## **4.5 兼容性**

1. **契约版本独立**：产品 `0.1.0` 与契约 `1.0.0-draft.1` 不互相绑定。
2. **草案允许调整**：v1 草案字段可变，但必须记录到 `contracts/CHANGELOG.md`。
3. **MSRV**：最低 Rust 1.85，edition 2024。
4. **平台**：Linux/macOS 优先，CI 在 Ubuntu 验证。

---

# **5. 核心能力**

## **5.1 Workspace 与分层基座**

### **5.1.1 Business Rules**

1. **成员范围**：workspace 成员为 `crates/*` 与 `apps/*`，resolver 2。
2. **统一元数据**：版本、edition、license、rust-version 由 `[workspace.package]` 提供，成员继承。
3. **统一 lint**：成员使用 `[workspace.lints]`；不得在成员内放宽 `unsafe_code`。
4. **依赖集中**：workspace 内 crate 依赖通过 `[workspace.dependencies]` 声明。

### **5.1.2 Acceptance Criteria**

1. [新增 crate 未继承 workspace 元数据] → [`cargo metadata` 显示版本不一致，CI 失败]
2. [成员引入 unsafe 代码] → [`cargo clippy` 失败]

## **5.2 契约类型（dsh-types）**

### **5.2.1 Business Rules**

1. **内容块**：`ContentBlock` 当前仅支持 `Text`，提供 `text()` 与 `as_text()`。
2. **工具描述**：`ToolDescriptor { name, description, input_schema }`；`JsonSchema` 为原始 JSON 文本，骨架阶段不引入 `serde_json`。
3. **标识符**：`Id` 提供 `parse`、`as_str`、`Display`、`AsRef<str>`；类型别名 `AgentId`/`SessionId`/`RunId`/`ToolCallId`。
4. **错误模型**：`ErrorCode` 固定为 `InvalidRequest`/`NotFound`/`Conflict`/`Timeout`/`Cancelled`/`Internal`/`Unavailable`，字符串形式为小写下划线。
5. **可重试标记**：`Error::retryable()` 置位，`is_retryable()` 读取；`Display` 输出 `<code>: <message>`。
6. **契约版本常量**：`CONTRACT_VERSION` 必须与 `contracts/VERSION` 一致。

### **5.2.2 Acceptance Criteria**

1. [解析 `agent-1`] → [成功且 `as_str()` 原样返回]
2. [解析空串/129 字节/含空格] → [分别返回 `Empty`/`TooLong`/`InvalidChar(' ')`]
3. [构造可重试超时错误] → [`to_string()` 为 `timeout: timed out`]

## **5.3 会话领域与端口（dsh-domain）**

### **5.3.1 Business Rules**

1. **聚合骨架**：`Session { id, title? }`，`Session::new(id)` 创建无标题会话。
2. **端口契约**：`SessionRepository` 提供 `load(&SessionId) -> Result<Option<Session>, Error>` 与 `save(&Session) -> Result<(), Error>`。
3. **同步签名**：端口保持同步；接入异步存储时在基础设施边界桥接，或统一改为异步 trait。
4. **无外部依赖**：`dsh-domain` 只依赖 `dsh-types`。

### **5.3.2 Acceptance Criteria**

1. [业务规则引用 Web 框架类型] → [评审拒绝；依赖检查失败]

## **5.4 用例编排（dsh-case）**

### **5.4.1 Business Rules**

1. **请求构造**：`RunRequest::new(agent_id, text)` 默认不续用会话。
2. **校验**：`validate()` 对 `text.trim()` 为空返回 `invalid_request`。
3. **会话确保**：`RunAgent::ensure_session(id)` 先 `load`，存在直接返回；不存在则创建并 `save`。
4. **依赖注入**：`RunAgent::new(repository)` 注入仓储，`repository()` 暴露只读引用。
5. **不越界**：用例不得直接依赖具体适配器类型。

### **5.4.2 Acceptance Criteria**

1. [空白文本] → [`invalid_request`，不创建会话]
2. [不存在会话调用 `ensure_session`] → [创建并保存后返回]
3. [已存在会话调用 `ensure_session`] → [返回原对象，不重复保存]

## **5.5 HTTP/SSE 常量（dsh-http）**

### **5.5.1 Business Rules**

1. **路由固定**：`/health`、`/api/agent/message`、`/api/agent/stream`、`/api/agent/cancel`、`/api/harness/plugins/{install,activate,status}`。
2. **SSE 心跳**：`SSE_KEEPALIVE = ":keepalive"`，不计入业务事件。
3. **不绑定框架**：骨架阶段不引入 Axum/Tokio；常量可被任意框架消费。
4. **路径规范**：所有路由以 `/` 开头。

### **5.5.2 Acceptance Criteria**

1. [路由常量不以 `/` 开头] → [单测失败]
2. [引入 Web 框架依赖] → [评审拒绝，须先更新 design 选型章节]

## **5.6 基础设施（dsh-infra）**

### **5.6.1 Business Rules**

1. **内存实现**：`InMemorySessionRepository` 使用 `Mutex<HashMap<SessionId, Session>>`。
2. **覆盖语义**：同 ID `save` 覆盖旧值。
3. **空态查询**：`is_empty()` 返回当前是否无会话。
4. **非持久化**：进程退出即丢，不提供落盘。

### **5.6.2 Acceptance Criteria**

1. [保存后读取] → [返回等值会话]
2. [锁中毒] → [返回 `internal`，不 panic]

## **5.7 可执行入口（apps/dsh-runtime）**

### **5.7.1 Business Rules**

1. **启动自检**：构造请求并校验，装配内存仓储与用例。
2. **输出内容**：产品版本、契约版本、三条核心路由、会话仓库空态。
3. **失败传播**：校验失败以 `Box<dyn Error>` 返回非零退出。
4. **不启动服务**：不监听端口，不阻塞。

### **5.7.2 Acceptance Criteria**

1. [`cargo run -p dsh-runtime`] → [输出 `dsh-runtime 0.1.0 contract 1.0.0-draft.1`]
2. [契约版本与 `contracts/VERSION` 不一致] → [评审阻断]

## **5.8 契约单一真源（contracts）**

### **5.8.1 Business Rules**

1. **目录职责**：`v1` schema、`fixtures` 样例、`tools` 校验/录制/回放/差分、`policy` 兼容与废弃规则。
2. **状态标注**：schema 状态为 `draft`/`frozen`/`deprecated`；当前全部 `draft`。
3. **版本独立**：`contracts/VERSION` 不随产品版本变更。
4. **变更顺序**：schema → fixture → `dsh-types` → `contracts/CHANGELOG.md`。
5. **归一化边界**：只允许时间戳、随机 ID、耗时、绝对路径归一化；错误类型、事件数量、工具参数、审批结果、终态不得归一化。

### **5.8.2 Acceptance Criteria**

1. [`node contracts/tools/validate.mjs`] → [输出 `OK schemas=... refs=... fixtures=... warnings=0`]
2. [schema 引用缺失] → [校验失败，CI 阻断]

## **5.9 质量门禁与 CI**

### **5.9.1 Business Rules**

1. **四条门禁**：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`node contracts/tools/validate.mjs`。
2. **CI 一致**：`.github/workflows/ci.yml` 必须与本地命令一致。
3. **测试基线**：v0.1.0 至少 11 个单元测试通过。

### **5.9.2 Acceptance Criteria**

1. [任一 format/clippy/test/contract 失败] → [CI 红灯]

## **5.10 AI 可发现性**

### **5.10.1 Business Rules**

1. **llms.txt**：面向 LLM 的项目摘要（能力、技术栈、命令、crate 地图、文档）。
2. **AGENTS.md**：构建/测试/lint 命令、布局、约定与禁止事项。
3. **中英双语**：`README.md` 与 `README.en.md` 并存，中文为主。
4. **内容一致**：版本号、命令、crate 状态须与实际一致。

### **5.10.2 Acceptance Criteria**

1. [文档命令无法执行] → [视为缺陷，随版本修复]

---

# **6. 数据约束**

1. **无数据库**：v0.1.0 不新增数据库、不新增迁移脚本。
2. **会话存储**：仅进程内内存，无持久化保证。
3. **标识符**：1–128 字节，字符集 `[A-Za-z0-9._:-]`。
4. **契约文件**：`contracts/v1` 发布后不原地修改；破坏性变更新开大版本目录。
5. **版本**：Cargo 产品版本 `0.1.0`；契约版本 `1.0.0-draft.1`；两者独立。

---

# **7. 验收标准（版本级）**

1. `cargo build --workspace` 通过。
2. `cargo fmt --all --check` 通过。
3. `cargo clippy --workspace --all-targets -- -D warnings` 通过。
4. `cargo test --workspace` 通过（11 个测试）。
5. `node contracts/tools/validate.mjs` 输出 0 警告。
6. `cargo run -p dsh-runtime` 输出产品版本、契约版本与路由常量。
7. 根目录具备 README（中英）、AGENTS.md、llms.txt、CHANGELOG.md。
8. `docs/0.1.0/` 具备 spec/design/tasks/release notes 四件套。

---

# **8. 边界与后续版本**

| 能力 | v0.1.0 | 后续版本 |
| --- | --- | --- |
| HTTP 服务与 SSE | 仅常量 | 0.2.0 选型后接入 |
| ReAct 循环 | 无 | 0.2.0 最小闭环 |
| 工具注册与执行 | 无 | 0.2.0 |
| 插件宿主/manifest | 无 | 0.2.1 |
| 会话事件日志与持久化 | 无 | 0.2.1 |
| 模型适配与凭据 | 无 | 0.2.0 |
| 审批与权限矩阵 | 无 | 0.2.1 |
| 契约冻结 | draft | 录制后逐项 frozen |

> **版本对齐口径**：Rust 产品版本独立于参考源 tag；后续按“主.次.修订”跟踪参考源与 dsh-java 的升级项，具体映射在每版 spec 中记录。

---

## 附：v0.1.0 能力清单

| 序号 | 能力 | 归属 | 状态 |
| --- | --- | --- | --- |
| 1 | Workspace 与统一 lint | §5.1 | 已实现 |
| 2 | 契约类型与错误模型 | §5.2 | 已实现 |
| 3 | 会话聚合与仓储端口 | §5.3 | 已实现 |
| 4 | 运行请求校验与会话确保 | §5.4 | 已实现 |
| 5 | HTTP/SSE 路由常量 | §5.5 | 已实现 |
| 6 | 内存会话仓储 | §5.6 | 已实现 |
| 7 | runtime 自检入口 | §5.7 | 已实现 |
| 8 | 契约 v1 草案与校验工具 | §5.8 | 已实现（draft） |
| 9 | 质量门禁与 CI | §5.9 | 已实现 |
| 10 | AI 可发现性文档 | §5.10 | 已实现 |