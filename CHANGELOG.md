# 版本

> 各版本详情见 `docs/<version>/release-v<version>-development-notes.md`；以下按 crate 与工程项记录变更。

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