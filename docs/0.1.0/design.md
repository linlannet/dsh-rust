# DeepSeek Harness Rust v0.1.0 实现方案文档

> 版本：`v0.1.0` | 基线：无（首版） | 架构：Cargo workspace + 六边形分层（types / domain / case / http / infra / app）
>
> 本文把 `docs/0.1.0/spec.md` 的“做什么”落到“怎么做”：分层依赖、核心类型、错误模型、契约对齐流程、选型边界与验证方式。

---

# 一、需求与存量功能关系分析

## 1.1 存量骨架与需求映射

仓库已具备可编译的 workspace 骨架，本版本的需求主要是**确认并固化**这些存量结构，而不是新增能力：

| 需求 | 存量实现 | 结论 |
| --- | --- | --- |
| §5.1 workspace 与分层 | `Cargo.toml` + 6 个成员 | 保留，补齐元数据与 lint 约束 |
| §5.2 契约类型 | `dsh-types`（content/error/ids/tool） | 保留，补测与文档 |
| §5.3 会话领域与端口 | `dsh-domain`（session/ports） | 保留 |
| §5.4 用例编排 | `dsh-case`（RunRequest/RunAgent） | 保留 |
| §5.5 HTTP/SSE 常量 | `dsh-http`（routes/SSE_KEEPALIVE） | 保留，不接服务 |
| §5.6 内存仓储 | `dsh-infra`（InMemorySessionRepository） | 保留 |
| §5.7 runtime 入口 | `apps/dsh-runtime` | 保留 |
| §5.8 契约真源 | `contracts/`（v1/fixtures/tools/policy） | 保留，版本独立 |
| §5.9 质量门禁 | `.github/workflows/ci.yml` | 保留，与本地命令对齐 |
| §5.10 AI 可发现性 | 仅 README 两行 | **新增** AGENTS.md / llms.txt / 中英 README / CHANGELOG |

## 1.2 存量约束分析

1. **edition 2024 + MSRV 1.85**：决定不能引入要求更高 MSRV 的依赖；CI 使用 stable，本地需 1.85+。
2. **workspace lints**：`unsafe_code = forbid`、`dbg_macro = deny`，意味着后续引入原生绑定必须通过 safe 封装或调整 lint（需评审）。
3. **`dsh-types` 无依赖**：`JsonSchema` 用原始字符串而非 `serde_json`，接入适配器时才替换为结构化类型。
4. **`SessionRepository` 同步签名**：先用同步跑通用例；异步存储接入时在 infra 边界桥接。
5. **契约 v1 为 draft**：Rust 类型可随 schema 调整，但必须保持 `CONTRACT_VERSION` 与 `contracts/VERSION` 一致。

---

# 二、增量设计方案

## 2.1 实现模型

### 2.1.1 分层与依赖方向

```text
        ┌───────────────────────────────┐
        │ apps/dsh-runtime              │  装配 + 自检
        └──────┬──────────┬─────────────┘
               │          │
     ┌─────────▼──┐  ┌────▼─────────┐
     │ dsh-case   │  │ dsh-http     │  常量（零依赖）
     └────┬───────┘  └──────────────┘
          │
     ┌────▼────────┐        ┌──────────────┐
     │ dsh-domain  │◄───────│ dsh-infra    │  实现 domain 端口
     └────┬────────┘        └──────────────┘
          │
     ┌────▼────────┐        ┌──────────────┐
     │ dsh-types   │◄───────│ contracts/v1 │  唯一真源
     └─────────────┘        └──────────────┘
```

约束：

| 规则 | 说明 |
| --- | --- |
| 单向依赖 | `case → domain → types`；`infra → domain`；`http` 无内部依赖 |
| 无环 | 不在 `types` 引入 workspace 内其它 crate |
| 无框架 | `types`/`domain` 不引入 Web、异步、数据库、模型 SDK |
| 常量优先 | `http` 只固定路径与事件，不提前绑定框架 |

### 2.1.2 运行时装配

```rust
let use_case = RunAgent::new(InMemorySessionRepository::default());
request.validate()?;
// 自检：版本、契约版本、路由、会话空态
```

装配只发生在 `apps/dsh-runtime`；库 crate 不提供全局单例。

## 2.2 接口设计

### 2.2.1 类型接口（dsh-types）

| 类型 | 形态 | 职责 |
| --- | --- | --- |
| `ContentBlock` | enum（`Text { text }`） | 消息内容块；v1 草案仅文本 |
| `ToolDescriptor` | struct | 工具跨进程描述（name/description/input_schema） |
| `JsonSchema` | newtype(String) | 原始 JSON Schema 文本 |
| `Id` | newtype(String) | 受限标识符，`parse` 校验 |
| `AgentId`/`SessionId`/`RunId`/`ToolCallId` | 类型别名 | 语义化 ID |
| `Error` | struct | `code` + `message` + `retryable` |
| `ErrorCode` | enum（7 值） | 稳定机器码 |

### 2.2.2 领域端口（dsh-domain）

```rust
pub trait SessionRepository {
    fn load(&self, id: &SessionId) -> Result<Option<Session>, Error>;
    fn save(&self, session: &Session) -> Result<(), Error>;
}
```

- 同步签名，错误统一 `dsh_types::Error`。
- 端口不感知存储介质；`dsh-infra` 提供实现。

### 2.2.3 用例接口（dsh-case）

| 接口 | 签名 | 说明 |
| --- | --- | --- |
| `RunRequest::new` | `(AgentId, impl Into<String>) -> RunRequest` | 默认不续用会话 |
| `RunRequest::validate` | `(&self) -> Result<(), Error>` | 空文本 → `invalid_request` |
| `RunAgent::new` | `(R) -> Self` | 注入仓储 |
| `RunAgent::repository` | `(&self) -> &R` | 只读引用，便于自检 |
| `RunAgent::ensure_session` | `(&self, &SessionId) -> Result<Session, Error>` | 读-建-存 |

### 2.2.4 接入常量（dsh-http）

| 常量 | 值 |
| --- | --- |
| `routes::HEALTH` | `/health` |
| `routes::AGENT_MESSAGE` | `/api/agent/message` |
| `routes::AGENT_STREAM` | `/api/agent/stream` |
| `routes::AGENT_CANCEL` | `/api/agent/cancel` |
| `routes::PLUGIN_INSTALL` | `/api/harness/plugins/install` |
| `routes::PLUGIN_ACTIVATE` | `/api/harness/plugins/activate` |
| `routes::PLUGIN_STATUS` | `/api/harness/plugins/status` |
| `SSE_KEEPALIVE` | `:keepalive` |

## 2.3 数据模型

v0.1.0 无数据库、无事件流，仅两个内存结构：

| 结构 | 字段 | 说明 |
| --- | --- | --- |
| `Session` | `id: SessionId`、`title: Option<String>` | 骨架聚合 |
| `InMemorySessionRepository` | `Mutex<HashMap<SessionId, Session>>` | 进程内，覆盖写 |

约束：

1. 同 ID `save` 覆盖。
2. 锁中毒映射为 `internal`，不 panic。
3. 进程退出即丢，不提供落盘与恢复。

## 2.4 契约对齐流程

```text
参考源 / Java 基线
   │ 录制（record-from-java.sh）
   ▼
contracts/fixtures（脱敏、归一化）
   │ validate.mjs
   ▼
contracts/v1/*.schema.json
   │ 手工/脚本同步
   ▼
crates/dsh-types（类型 + CONTRACT_VERSION）
   │
   ▼
contracts/CHANGELOG.md
```

规则：

1. schema 与 fixture 先变更，Rust 类型后变更。
2. 破坏性变更新开大版本目录，不原地修改 `v1`。
3. 归一化只允许时间戳、随机 ID、耗时、绝对路径。
4. 当前全部 schema 为 `draft`，未承诺稳定。

## 2.5 测试策略

| 层级 | 方式 | 覆盖 |
| --- | --- | --- |
| 单元测试 | crate 内 `#[cfg(test)]` | 标识符校验、错误构造、请求校验、仓储往返、路由常量 |
| 集成测试 | 暂缺 | HTTP/ReAct 接入后补充 |
| 契约校验 | `validate.mjs` | schema 引用、fixture 结构 |
| 门禁 | fmt/clippy/test/contract | CI 四条 |

基线：11 个单元测试（types 7、case 2、http 1、infra 1）。

---

# 三、模块级详细设计

## 3.1 dsh-types

- `content.rs`：`ContentBlock::text` / `as_text`；后续扩展图片、工具调用块时按契约新增变体。
- `ids.rs`：`Id::parse` 校验顺序为 空 → 长度 → 字符；`IdError` 区分三类原因。
- `error.rs`：`ErrorCode::as_str` 为稳定码；`Error` 提供 `retryable` 构建器；`Display` 为 `<code>: <message>`。
- `tool.rs`：`JsonSchema` 暂存原始文本；接入 `serde_json` 后改为结构化类型，需更新设计。

## 3.2 dsh-domain

- `session.rs`：仅 `id` 与可选 `title`；事件、状态、分支均未建模。
- `ports.rs`：`SessionRepository` 同步端口；异步化时评估“桥接”与“改异步 trait”两条路径，并在设计文档记录决策。

## 3.3 dsh-case

- `RunRequest`：构造 + 校验，错误码单一（`invalid_request`）。
- `RunAgent<R>`：泛型注入仓储，避免用例依赖具体实现；`ensure_session` 为后续 ReAct 循环的会话入口。

## 3.4 dsh-http

- 仅常量与 1 个单测；不引入框架。
- 后续接入时，常量作为路由表输入，事件名与 `contracts/v1/sse` 对齐。

## 3.5 dsh-infra

- `InMemorySessionRepository`：`Mutex` 保护；`is_empty` 用于自检。
- 后续新增 SQLite/PostgreSQL 适配器时，实现同一端口，不修改用例。

## 3.6 apps/dsh-runtime

- 构造请求 → 校验 → 装配用例 → 打印自检信息。
- 错误通过 `?` 传播为 `Box<dyn Error>`；不启动服务、不阻塞。

## 3.7 contracts

- `VERSION` 独立；`v1` 目录包含 common/http/sse/plugin-host/plugin-manifest/persistence/config/external。
- `fixtures` 当前 4 组手写样例；`tools` 提供 validate/record/replay/diff。
- `policy` 定义兼容、版本、废弃规则。

---

# 四、技术选型与约束

## 4.1 已确定

| 项 | 选型 | 理由 |
| --- | --- | --- |
| 语言版本 | Rust 1.85+，edition 2024 | 使用最新 edition，MSRV 与工具链匹配 |
| 构建 | Cargo workspace | 成员统一元数据与 lint |
| 错误 | `dsh-types::Error` + `ErrorCode` | 跨 crate 统一，映射契约 `code` |
| 契约 | JSON Schema + fixtures | 与 Java/Python 侧共用 |
| 测试 | 内置 `#[test]` | 零依赖，骨架阶段足够 |

## 4.2 暂不选型

| 项 | 决策触发条件 |
| --- | --- |
| Web 框架（Axum/Actix） | 0.2.0 接入 HTTP 前，用基准与依赖审查决定 |
| 异步运行时（Tokio） | 与 Web 框架一并决定 |
| 序列化（serde/serde_json） | `dsh-types` 需要结构化 JSON 时 |
| 存储（SQLx/SQLite/PostgreSQL） | 0.2.1 持久化前 |
| 插件加载（动态库/WASM/进程） | 0.2.1 插件宿主前 |

**约束**：任何选型落地前，先在本文档新增决策记录（选型、备选、理由、影响），再改代码。

## 4.3 与参考源/Java 的差异

| 维度 | 参考源/Java | Rust 骨架口径 |
| --- | --- | --- |
| 协议 | 自有 wire 协议 | 先以 `contracts/v1` 固定边界，Rust 类型映射 |
| 插件 | Cordis/JVM SPI/MCP | 暂不实现；预留 `plugin-host` 契约 |
| 会话 | 事件溯源 + 持久化 | 先聚合骨架 + 端口，事件后续 |
| 错误 | `HarnessError` 多码 | 先 7 个基础码，按能力扩展 |

---

# 五、风险与回滚

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 契约 draft 频繁调整 | 中 | 变更严格走 schema→fixture→types→CHANGELOG；`CONTRACT_VERSION` 单点维护 |
| 过早绑定框架 | 中 | 骨架阶段禁止引入 Axum/Tokio/SQLx；选型须评审 |
| 同步端口限制并发 | 低 | 当前仅内存实现；异步化时记录桥接/改 trait 决策 |
| 文档与代码漂移 | 低 | 版本/命令/路由写入四件套与 llms.txt，CI + 评审校验 |
| `unsafe` 禁用限制原生能力 | 低 | 后续原生绑定需评审并记录 lint 调整 |

**回滚**：v0.1.0 全部为新增文档与既有骨架确认，无数据库、无迁移、无外部服务；回滚只需回退版本提交。

---

# 六、实施计划

| 阶段 | 内容 | 产出 |
| --- | --- | --- |
| P0 基线确认 | 验证 workspace、测试、fmt、clippy、contracts | 门禁基线记录 |
| P1 文档初始化 | 根目录 README/AGENTS/llms/CHANGELOG | AI 可发现性与使用说明 |
| P2 四件套 | `docs/0.1.0/` spec/design/tasks/release notes | 版本级需求与任务 |
| P3 收口 | 版本号核对、文档一致性检查 | 0.1.0 骨架发布口径 |

**验收口径**：四条门禁通过 + 四件套齐全 + 根目录规范文件齐全，即可标记 0.1.0 骨架完成。