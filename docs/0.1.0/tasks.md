# DeepSeek Harness Rust v0.1.0 编码任务清单

> 版本：`v0.1.0` | 基线：无（首版） | 架构：Cargo workspace（Rust 2024 / MSRV 1.85）
>
> 本清单把 `docs/0.1.0/spec.md` 的 §5.1–§5.10 与 `design.md` 的分层落位转化为可执行、可验收的条目。
>
> **完成状态图例**：`[x]` 已完成 ｜ `[ ]` 待办/后续版本
>
> **任务统计**：11 个任务组，49 条条目；其中 45 条已完成，4 条为后续版本预留。

---

## 1. Workspace 基线

> 依赖：无（基座）｜ 覆盖：spec §5.1 ｜ 状态：已完成

- [x] 建立 Cargo workspace，成员为 `crates/*` 与 `apps/*`，`resolver = "2"`；验收：`cargo metadata` 可列出 6 个成员 —— `Cargo.toml`
- [x] `[workspace.package]` 统一 `version = "0.1.0"`、`edition = "2024"`、`license = "MIT"`、`rust-version = "1.85"`；验收：成员使用 `version.workspace = true` 等继承
- [x] `[workspace.dependencies]` 声明 5 个内部 crate 路径依赖；验收：成员以 `.workspace = true` 引用
- [x] workspace lints：`unsafe_code = "forbid"`、`dbg_macro = "deny"`；验收：`cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 所有库 crate `publish = false`；验收：`cargo metadata` 中 `publish` 为空 —— 各 `Cargo.toml`

---

## 2. dsh-types（契约类型）

> 依赖：无 ｜ 覆盖：spec §5.2 ｜ 状态：已完成

- [x] `ContentBlock` 文本块 + `text()`/`as_text()`；验收：构造与读取单测 —— `crates/dsh-types/src/content.rs`
- [x] `Id::parse` 校验空/超长/非法字符，`IdError` 三类原因；验收：4 个单测 —— `crates/dsh-types/src/ids.rs`
- [x] 类型别名 `AgentId`/`SessionId`/`RunId`/`ToolCallId`；验收：与契约 identifiers 对应
- [x] `ErrorCode` 7 个稳定码与 `as_str()` 小写下划线；验收：码值与 `contracts/v1/common/error.schema.json` 一致 —— `crates/dsh-types/src/error.rs`
- [x] `Error` 支持 `retryable()` 构建器与 `Display` `<code>: <message>`；验收：单测断言 `timeout: timed out`
- [x] `ToolDescriptor` + `JsonSchema`（原始 JSON 文本）；验收：字段保持单测 —— `crates/dsh-types/src/tool.rs`
- [x] `CONTRACT_VERSION = "1.0.0-draft.1"`；验收：与 `contracts/VERSION` 一致 —— `crates/dsh-types/src/lib.rs`

---

## 3. dsh-domain（领域与端口）

> 依赖：dsh-types ｜ 覆盖：spec §5.3 ｜ 状态：已完成

- [x] `Session { id, title? }` 与 `Session::new`；验收：构造结果 —— `crates/dsh-domain/src/session.rs`
- [x] `SessionRepository` 端口（`load`/`save`，同步签名，统一 `Error`）；验收：实现方可替换 —— `crates/dsh-domain/src/ports.rs`
- [x] 领域层不依赖 Web/DB/模型 SDK；验收：`Cargo.toml` 仅依赖 `dsh-types`

---

## 4. dsh-case（用例编排）

> 依赖：dsh-domain、dsh-types ｜ 覆盖：spec §5.4 ｜ 状态：已完成

- [x] `RunRequest::new` 与默认不续用会话；验收：构造字段 —— `crates/dsh-case/src/lib.rs`
- [x] `validate()` 空白文本返回 `invalid_request`；验收：2 个单测（拒绝空白/接受非空白）
- [x] `RunAgent<R>::ensure_session` 先读后建再存；验收：不存在时创建并保存
- [x] `RunAgent::repository()` 暴露只读引用；验收：runtime 可查询空态

---

## 5. dsh-http（接入常量）

> 依赖：无 ｜ 覆盖：spec §5.5 ｜ 状态：已完成

- [x] 7 条路由常量（health/agent×3/plugin×3）；验收：均以 `/` 开头 —— `crates/dsh-http/src/lib.rs`
- [x] `SSE_KEEPALIVE = ":keepalive"`；验收：常量断言
- [x] 不引入 Web/异步框架；验收：`Cargo.toml` 无依赖

---

## 6. dsh-infra（基础设施）

> 依赖：dsh-domain、dsh-types ｜ 覆盖：spec §5.6 ｜ 状态：已完成

- [x] `InMemorySessionRepository`（`Mutex<HashMap>`）；验收：往返单测 —— `crates/dsh-infra/src/memory.rs`
- [x] 锁中毒映射 `internal`，不 panic；验收：错误映射实现
- [x] `is_empty()` 空态查询；验收：往返后为 false

---

## 7. apps/dsh-runtime（可执行入口）

> 依赖：case/http/infra/types ｜ 覆盖：spec §5.7 ｜ 状态：已完成

- [x] 装配内存仓储、用例与请求校验；验收：`cargo run -p dsh-runtime` 正常退出 —— `apps/dsh-runtime/src/main.rs`
- [x] 输出产品版本、契约版本、三条核心路由与会话空态；验收：输出含 `dsh-runtime 0.1.0 contract 1.0.0-draft.1`
- [x] 不监听端口、不阻塞；验收：进程立即退出

---

## 8. contracts（契约真源）

> 依赖：无 ｜ 覆盖：spec §5.8 ｜ 状态：已完成（draft）

- [x] `VERSION = 1.0.0-draft.1` 独立于产品版本；验收：与 `CONTRACT_VERSION` 一致 —— `contracts/VERSION`
- [x] `v1` 目录：common/http/sse/plugin-host/plugin-manifest/persistence/config/external + `openapi.yaml`；验收：29 个 schema 可校验
- [x] `policy` 兼容/版本/废弃规则；验收：文档齐全 —— `contracts/policy/`
- [x] `tools` validate/record/replay/diff 四个脚本；验收：`node contracts/tools/validate.mjs` 输出 0 警告
- [x] `fixtures` 4 组手写样例；验收：校验器识别 4 个 fixture
- [x] `CHANGELOG` 记录 draft.1 与初始化；验收：条目存在 —— `contracts/CHANGELOG.md`

---

## 9. 质量门禁与 CI

> 依赖：1–8 ｜ 覆盖：spec §5.9 ｜ 状态：已完成

- [x] `cargo fmt --all --check` 通过；验收：本地与 CI 一致
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 通过；验收：无警告
- [x] `cargo test --workspace` 通过；验收：11 个测试通过
- [x] `node contracts/tools/validate.mjs` 通过；验收：29 schema / 46 refs / 4 fixtures / 0 warnings
- [x] CI 两个 job（rust、contracts）与本地命令一致；验收：`.github/workflows/ci.yml`

---

## 10. 工程初始化文档

> 依赖：1–9 ｜ 覆盖：spec §5.10 ｜ 状态：本次交付

- [x] `README.md`（中文为主）与 `README.en.md`（英文副本）；验收：TL;DR、关键词、快速开始、分层、门禁、文档、路线图齐全
- [x] `AGENTS.md`：构建/测试/lint 命令、布局、约定、禁止事项；验收：命令可直接执行
- [x] `llms.txt`：能力、技术栈、命令、crate 地图、文档入口；验收：版本与 README 一致
- [x] `CHANGELOG.md`：0.1.0 初始化条目；验收：分“新增/质量/限制”记录
- [x] `docs/0.1.0/` 四件套：spec/design/tasks/release notes；验收：相互引用一致
- [x] 版本核对：Cargo `0.1.0`、contracts `1.0.0-draft.1`；验收：文档与元数据一致

---

## 11. 后续版本任务（本版本不做）

> 状态：预留，见 `spec.md` §8

- [ ] 0.2.0：Web/异步选型 + HTTP/SSE 服务接入
- [ ] 0.2.0：ReAct 最小循环 + 工具注册 + 假模型
- [ ] 0.2.1：插件宿主与 manifest（对 `contracts/v1/plugin-host`）
- [ ] 0.2.1：会话事件日志与持久化（SQLite/PostgreSQL）

---

## 附：验收速查

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node contracts/tools/validate.mjs
cargo run -p dsh-runtime
```