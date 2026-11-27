# DeepSeek Harness Rust v0.1.0 开发说明

> 版本：`v0.1.0` | 开发周期：2026-11-27 | 基线：无（首版） | 契约版本：`1.0.0-draft.1`

## 1. 版本概览

v0.1.0 是 Rust 版的**骨架初始化版本**，目标是先把“契约、分层、门禁、文档”四件基座固定下来：

1. **Cargo workspace 分层**：`dsh-types` → `dsh-domain` ← `dsh-case`，`dsh-http`/`dsh-infra` 作为外侧能力，`apps/dsh-runtime` 负责装配与自检。
2. **契约先行的类型层**：`dsh-types` 提供内容块、工具描述、受限 ID 与统一错误模型，`CONTRACT_VERSION` 与 `contracts/VERSION` 同源。
3. **最小闭环**：运行请求校验 → 会话确保 → 内存仓储 → runtime 自检输出。
4. **工程初始化文档**：中英 README、AGENTS.md、llms.txt、CHANGELOG 与 `docs/0.1.0/` 四件套。

本版本**不启动 HTTP 服务、不调用模型、不实现工具/插件/持久化**，对应能力在 `spec.md` §8 明确列为后续版本。

## 2. 版本亮点

### 2.1 契约先行的工作区

- `contracts/v1` 固定跨进程边界（common/http/sse/plugin-host/plugin-manifest/persistence/config/external）；
- Rust 类型与 schema 一一映射，`CONTRACT_VERSION` 单点维护；
- 契约版本（`1.0.0-draft.1`）与产品版本（`0.1.0`）独立演进。

### 2.2 分层与依赖约束

- 依赖方向固定为 `case → domain → types`，`infra` 实现 `domain` 端口；
- `dsh-types` 与 `dsh-http` 保持零第三方依赖；
- `unsafe_code = forbid`、`dbg_macro = deny` 由 workspace lints 统一守护。

### 2.3 统一错误模型

- `ErrorCode` 固定 7 个稳定码（`invalid_request`/`not_found`/`conflict`/`timeout`/`cancelled`/`internal`/`unavailable`）；
- `Error` 携带 `code`、可读消息与 `retryable` 标记；
- 后续子系统错误必须复用该模型，不新增 crate 专属错误枚举。

### 2.4 最小可运行闭环

- `RunRequest::validate` 拒绝空白文本；
- `RunAgent::ensure_session` 读-建-存；
- `InMemorySessionRepository` 提供进程内实现，锁中毒映射为 `internal`。

## 3. 架构与模块清单

```text
apps/dsh-runtime          可执行入口（装配 + 自检）
crates/dsh-case           用例：RunRequest / RunAgent
crates/dsh-domain         领域：Session / SessionRepository
crates/dsh-types          契约类型：ContentBlock / ToolDescriptor / Id / Error
crates/dsh-http           路由与 SSE 常量（不接服务）
crates/dsh-infra          适配器：InMemorySessionRepository
contracts/                v1 schema / fixtures / tools / policy
```

| 模块 | 主要文件 | 说明 |
| --- | --- | --- |
| dsh-types | `content.rs`、`ids.rs`、`error.rs`、`tool.rs` | 零依赖，契约映射 |
| dsh-domain | `session.rs`、`ports.rs` | 只依赖 dsh-types |
| dsh-case | `lib.rs` | 校验与会话编排 |
| dsh-http | `lib.rs` | 7 条路由 + SSE 心跳常量 |
| dsh-infra | `memory.rs` | 进程内会话仓库 |
| dsh-runtime | `main.rs` | 版本/路由/空态自检 |

## 4. 契约基线

| 项 | 值 |
| --- | --- |
| 契约版本 | `1.0.0-draft.1` |
| schema 状态 | 全部 `draft` |
| schema 数量 | 29 |
| 引用数量 | 46 |
| fixtures | 4 组（手写样例） |
| 校验结果 | `OK schemas=29 refs=46 fixtures=4 warnings=0` |

契约变更顺序：schema → fixture → `dsh-types` → `contracts/CHANGELOG.md`。

## 5. 验证结论

| 门禁 | 命令 | 结果 |
| --- | --- | --- |
| 构建 | `cargo build --workspace` | 通过 |
| 格式 | `cargo fmt --all --check` | 通过 |
| 静态检查 | `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| 单元测试 | `cargo test --workspace` | 11 个通过 |
| 契约校验 | `node contracts/tools/validate.mjs` | 0 警告 |
| 运行自检 | `cargo run -p dsh-runtime` | 输出正常 |

测试分布：`dsh-types` 7、`dsh-case` 2、`dsh-http` 1、`dsh-infra` 1。

## 6. 已知限制

1. **无 HTTP 服务**：只固定路由常量，未监听端口。
2. **无模型与 ReAct**：无 LLM 适配器、无工具执行、无多步循环。
3. **无插件宿主**：不加载插件、不解析 manifest；`plugin-host` 仅契约草案。
4. **无持久化**：会话仅在进程内存，重启即丢。
5. **无鉴权/审批**：未实现认证、权限矩阵与人工审批。
6. **契约未冻结**：`v1` 为草案，字段可能随录制调整。
7. **同步端口**：`SessionRepository` 为同步签名，异步存储需桥接或改造。

## 7. 依赖与体积

- **新增第三方依赖：无**（仅标准库）；
- **新增数据库表：无**；
- **新增 Maven/npm 依赖：无**；Node.js 仅用于契约校验脚本；
- 构建产物为 5 个库 crate + 1 个可执行 crate。

## 8. 后续建议

| 优先级 | 建议 | 说明 |
| --- | --- | --- |
| P0 | 确定 Web/异步选型 | 在 0.2.0 接入 HTTP/SSE 前完成基准与依赖审查 |
| P0 | 从 Java 基线录制 fixtures | 把 4 组手写样例替换为真实录制，提升契约可信度 |
| P1 | ReAct 最小闭环 | 假模型 + 工具注册 + 事件写入 |
| P1 | 会话事件与持久化 | 事件日志、回放、SQLite/PostgreSQL 适配 |
| P2 | 插件宿主 | 对 `contracts/v1/plugin-host` 落地 manifest 与加载 |
| P2 | 与 dsh-java 能力对齐 | 按版本跟踪参考源和 Java 侧升级项 |