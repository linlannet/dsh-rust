# 版本

> 各版本详情见 `docs/<version>/release-v<version>-development-notes.md`；以下按 crate 与工程项记录变更。

## 版本号：0.2.1
- 更新日期：2026-11-28
- 详情：docs/0.2.1/release-v0.2.1-development-notes.md
- 契约版本：1.0.0-draft.1（独立于产品版本）

- **新增**：
  - **持久化**：新增 `EventLog` 端口（`append`/`append_many`/`list_after`/`last_seq`）与 SQLite 实现；会话投影、消息、事件与运行四表落 `DSH_DB_PATH`（默认 `./data/dsh-rust.db`）
  - **事件回放**：`GET /api/sessions/{sessionId}/events`，支持 `afterSeq`/`limit`（默认 200、上限 1000），会话不存在返回 `not_found`
  - **重启续跑**：进程重启后同一 `sessionId` 复用历史消息，事件 `seq` 跨重启接续
  - **SSE 输出预算**：`DSH_STREAM_BUDGET_BYTES`（默认 8 MiB），超限以唯一终态 `error(stream_budget_exceeded)` 收尾
  - **存储切换**：`DSH_STORE=sqlite|memory`，内存实现保留为回滚通道
  - **契约**：新增 persistence schema/migration、回放端点 schema、错误码 `$defs/errorCode` 与 2 组 fixtures
- **更新**：
  - workspace 版本 `0.2.0` → `0.2.1`
  - `dsh-case`：`AgentService` 注入 `EventLog` 并落 6 类事件；仓储与事件调用统一 `spawn_blocking`；`RunRegistry.finished` 改有序淘汰（保留最近 1024 条）
  - `dsh-http`：`AppState` 增加 `stream_budget_bytes`；新增 `SESSIONS_EVENTS` 路由、DTO 与错误映射
  - `dsh-infra`：新增 `SqliteStore`/`SqliteSessionRepository`/`SqliteEventLog`/`InMemorySessionEventLog`
  - `dsh-types`：新增 `SessionEvent`、`event_type` 常量、`now_rfc3339`、`ErrorCode::StorageUnavailable`/`StreamBudgetExceeded`
  - `dsh-runtime`：SQLite 装配、启动迁移、关闭前有限等待在途 run
- **质量**：
  - `cargo test --workspace --all-features` 87 passed
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过
  - 契约校验 31 schema / 68 refs / 10 fixtures / 0 warnings
  - 新增内存 vs SQLite 事件序列差分测试与重启续跑集成测试
- **破坏性变更**：
  - `AgentService::new` 增加 `EventLog` 参数（`dsh-case`）
  - `AppState` 增加 `stream_budget_bytes` 字段（`dsh-http`）
  - 新增依赖 `rusqlite 0.40`（仅 `dsh-infra`）
- **已知限制**：DeepSeek 未真实联调；工具超时后 `spawn_blocking` 任务不可取消；单 run 事件批量提交使中间事件延迟可回放；事件无留存治理；插件宿主与审批未接入

## 版本号：0.2.0
- 更新日期：2026-11-27
- 详情：docs/0.2.0/release-v0.2.0-development-notes.md
- 契约版本：1.0.0-draft.1（独立于产品版本）

- **新增**：
  - **HTTP/SSE 服务**：`GET /health`、`POST /api/agent/message`、`POST /api/agent/stream`、`POST /api/agent/cancel`（Axum 0.8 + Tokio 1）
  - **ReAct 最小闭环**：`AgentService` 按“模型 → 工具 → 观察 → 再推理”循环，默认 8 步上限；终止原因 `stop`/`length`/`cancelled`/`error`
  - **模型双轨**：`FakeModel`（`echo`/`tool` 脚本，默认）与 `DeepSeekModel`（feature `deepseek`，Chat Completions 最小字段）
  - **工具**：`echo`、`clock`（RFC3339 UTC）、`read_file`（canonicalize 沙箱 + 256 KiB + UTF-8）
  - **SSE**：`chunk`/`tool_call`/`done`/`error`，seq 递增、单终态、keepalive 不占 seq
  - **取消**：按 `runId`/`agentId` 取消，完成后重复取消返回终态
  - **契约**：新增 4 组运行时导出 fixtures；`generate-openapi.mjs` 生成 `v1/openapi.yaml` 并纳入 CI
  - **文档**：`docs/0.2.0` 四件套、README 中英更新
- **更新**：
  - workspace 版本 `0.1.0` → `0.2.0`
  - `dsh-types` 引入 serde/serde_json：`ContentBlock`/`ToolDescriptor`/`Error`/`Id` 支持契约序列化；`JsonSchema` 由字符串改为 JSON 对象；新增 `JsonValue`、`ToolCallRequest`、`ErrorCode::InvalidResponse`
  - `dsh-domain` 新增 `ModelPort`/`Tool`/`RunEvent`/`RunStatus`/`FinishReason`；`Session` 增加消息历史与活动运行
  - `dsh-http` 由“仅常量”升级为完整接入层（DTO、路由、错误映射、SSE、traceId、serve）
  - CI 增加 `--all-features` 检查与 openapi 再生成校验
- **质量**：
  - `cargo test --workspace --all-features` 51 passed
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过
  - 契约校验 29 schema / 52 refs / 8 fixtures / 0 warnings
- **破坏性变更**：
  - `dsh-types`/`dsh-http` 不再零依赖
  - `JsonSchema` 类型变更（字符串 → JSON 值）
  - `Session` 新增字段（构造 `Session::new` 不变）
- **已知限制**：DeepSeek 未真实联调；事件日志未暴露 HTTP；会话/运行为内存态；无鉴权与插件宿主

## 版本号：0.1.0
- 更新日期：2026-11-27
- 详情：docs/0.1.0/release-v0.1.0-development-notes.md
- 契约版本：1.0.0-draft.1（独立于产品版本）

- **新增**：
  - **Cargo workspace 骨架**：成员 `crates/{dsh-types,dsh-domain,dsh-case,dsh-http,dsh-infra}` 与 `apps/dsh-runtime`；统一 `[workspace.package]`（edition 2024 / MSRV 1.85 / MIT）与 `[workspace.dependencies]`
  - **dsh-types**：`ContentBlock`（文本块）、`ToolDescriptor` + `JsonSchema`、受限标识符 `Id` 与 `AgentId`/`SessionId`/`RunId`/`ToolCallId`、统一错误模型 `Error`/`ErrorCode`（7 个稳定码）、`CONTRACT_VERSION = "1.0.0-draft.1"`
  - **dsh-domain**：`Session` 聚合骨架、`SessionRepository` 端口（`load`/`save`，同步签名）
  - **dsh-case**：`RunRequest` 校验（空白文本 → `invalid_request`）、`RunAgent::ensure_session`（读-建-存）
  - **dsh-http**：7 条路由常量（`/health`、`/api/agent/{message,stream,cancel}`、`/api/harness/plugins/{install,activate,status}`）与 `SSE_KEEPALIVE`
  - **dsh-infra**：`InMemorySessionRepository`（进程内、覆盖写、锁中毒映射 `internal`）
  - **apps/dsh-runtime**：装配与自检入口，输出版本、契约版本、核心路由与会话空态
  - **contracts**：`v1` 草案（29 schema / 46 refs）、4 组手写 fixtures、`tools`（validate/record/replay/diff）、`policy`（兼容/版本/废弃）
  - **工程文档**：`README.md` + `README.en.md`、`AGENTS.md`、`llms.txt`、`docs/0.1.0/` 四件套（spec/design/tasks/release notes）
- **质量**：
  - workspace lints：`unsafe_code = "forbid"`、`dbg_macro = "deny"`
  - CI 两个 job（rust: fmt/clippy/test；contracts: validate），与本地命令一致
  - 单元测试 11 个（types 7、case 2、http 1、infra 1），四条门禁全部通过
- **更新**：
  - README 由两行简介扩展为完整的项目说明（定位、快速开始、分层、契约、门禁、路线图），并新增英文副本
- **已知限制**：
  - 无 HTTP 服务、无模型调用、无 ReAct、无插件宿主、无持久化、无鉴权与审批
  - 契约 `v1` 为草案，尚未从 Java 基线录制；`SessionRepository` 为同步端口
- **依赖**：无新增第三方依赖；无新增数据库表