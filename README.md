# DeepSeek Harness Rust (dsh-rust)

> **TL;DR** · Rust 版 DeepSeek Harness 骨架：契约先行的分层工作区（types → domain ← case → http/infra），事件会话端口与内存实现，可运行的 runtime 入口。
> **Keywords**: Agent Harness, ReAct, AI Agent, MCP, Plugin, Rust, Cargo, LLM
> **参考源**: [deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) · **Java 对照**: [dsh-java](https://github.com/linlannet/dsh-java)
> **AI 入口**: [llms.txt](./llms.txt) · [AGENTS.md](./AGENTS.md)

[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](#5-技术栈)
[![Edition](https://img.shields.io/badge/edition-2024-blue.svg)](#5-技术栈)

---

**中文** | [English](./README.en.md)

基于 DeepSeek Harness 的 **Rust 版本实现**，目标是**一切皆插件**的 Rust 复刻，并持续迭代跟踪参考源与 Java 版的动态演进。

当前版本 `0.1.0` 是**骨架版本**：先把分层、契约与最小闭环固定下来，暂不启动 HTTP 服务、不接入模型、不实现插件运行时。骨架阶段的取舍是“契约先行、依赖最小”，避免在选型未定时过早绑定 Web 框架或数据库驱动。

- **端口**：暂无（HTTP 服务在后续版本接入）
- **契约版本**：`1.0.0-draft.1`（与产品版本独立，见 `contracts/VERSION`）
- **License**：见仓库根目录

---

## 版本历史

全部版本变更与修订统一记录在 [CHANGELOG](./CHANGELOG.md)。本 README 只保留稳定的架构设计与使用说明，不随版本更新变化明细。

---

## 目录

- [1. 项目介绍](#1-项目介绍)
- [2. 快速开始](#2-快速开始)
- [3. 架构与分层](#3-架构与分层)
- [4. 契约（contracts）](#4-契约contracts)
- [5. 技术栈](#5-技术栈)
- [6. 质量门禁](#6-质量门禁)
- [7. 文档](#7-文档)
- [8. 路线图](#8-路线图)

---

## 1. 项目介绍

dsh-rust 用 Rust 复刻 DeepSeek Harness 的核心形态，当前版本交付的是**可编译、可测试、可运行的骨架**：

- **契约先行**：`contracts/v1` 是跨进程、跨仓库协议的唯一真源；Rust 类型与 schema 一一对应。
- **分层工作区**：`dsh-types`（契约类型）→ `dsh-domain`（模型与端口）← `dsh-case`（用例）→ `dsh-http` / `dsh-infra`（接入与适配）。
- **最小闭环**：会话创建/读取/保存 + 请求校验 + 路由常量 + 内存仓储 + runtime 入口。
- **质量门禁**：`cargo fmt`、`cargo clippy -D warnings`、`cargo test`、契约 schema 校验四条门禁与 CI 一致。

本版本**不包含**：HTTP 服务、模型调用、ReAct 状态机、插件宿主、数据库持久化、审批与工具执行。这些能力在 `docs/0.1.0/spec.md` 的边界章节明确列出，按版本迭代补齐。

---

## 2. 快速开始

### 2.1 环境要求

- Rust `1.85+`（edition 2024）
- Node.js `20+`（仅用于 `contracts/tools/validate.mjs`）
- Cargo（随 Rust 工具链安装）

### 2.2 构建与运行

```bash
cargo build --workspace
cargo run -p dsh-runtime
```

`dsh-runtime` 当前输出产品版本、契约版本与已固定的路由常量，用于验证依赖装配与请求校验链路：

```text
dsh-runtime 0.1.0 contract 1.0.0-draft.1
routes health=/health message=/api/agent/message stream=/api/agent/stream
session store empty: true
```

### 2.3 测试与校验

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node contracts/tools/validate.mjs
```

---

## 3. 架构与分层

```text
apps/dsh-runtime        可执行入口（装配与自检）
        │
        ├── dsh-case       用例编排、请求校验、会话创建
        │        │
        │        └── dsh-domain   会话聚合 + 端口（SessionRepository）
        │                    │
        │                    └── dsh-types   契约值对象、错误模型、ID
        ├── dsh-http       HTTP/SSE 路由常量与事件常量
        └── dsh-infra      适配器（当前仅内存会话仓库）
```

依赖方向固定为 `case → domain → types`，`http`/`infra` 依赖 `domain` 与 `types` 实现外侧能力；`dsh-types` 不依赖任何 crate，作为最小基座。

| crate | 职责 | 当前状态 |
| --- | --- | --- |
| `dsh-types` | 契约对应的基础类型、错误码、标识符 | 已实现 |
| `dsh-domain` | 会话聚合与端口定义 | 已实现 |
| `dsh-case` | 运行请求校验、会话确保 | 已实现 |
| `dsh-http` | 路由与 SSE 常量 | 常量已固定，服务未接入 |
| `dsh-infra` | 内存会话仓库 | 已实现 |
| `apps/dsh-runtime` | 可执行入口与自检输出 | 已实现 |

---

## 4. 契约（contracts）

`contracts/` 与工作区版本独立演进：

- `contracts/VERSION`：契约版本，当前 `1.0.0-draft.1`
- `contracts/v1/`：按大版本隔离的 schema（common/http/sse/plugin-host/plugin-manifest/persistence/config/external）
- `contracts/fixtures/`：录制与回放样例（当前为手写样例）
- `contracts/tools/`：校验、从 Java 基线录制、Rust 回放与差分脚本
- `contracts/policy/`：兼容、版本与废弃规则

契约变更流程：先改 schema 与 fixture，再同步 `dsh-types`，最后更新 `contracts/CHANGELOG.md`。

---

## 5. 技术栈

| 项 | 版本/选型 |
| --- | --- |
| 语言 | Rust 1.85+，edition 2024 |
| 构建 | Cargo workspace（`crates/*`、`apps/*`） |
| Lint | workspace lints：`unsafe_code = forbid`、`dbg_macro = deny` |
| 契约校验 | Node.js + `contracts/tools/validate.mjs` |
| Web/异步/存储 | **暂未选型**（骨架阶段不绑定 Axum/Tokio/SQLx） |

---

## 6. 质量门禁

| 门禁 | 命令 | 当前状态 |
| --- | --- | --- |
| 格式 | `cargo fmt --all --check` | 通过 |
| 静态检查 | `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| 单元测试 | `cargo test --workspace` | 11 个测试通过 |
| 契约校验 | `node contracts/tools/validate.mjs` | 29 schema / 46 refs / 4 fixtures，0 警告 |

CI 见 `.github/workflows/ci.yml`，与上表命令一致。

---

## 7. 文档

- `docs/0.1.0/spec.md`：需求规格（做什么、验收标准）
- `docs/0.1.0/design.md`：实现方案（分层、模型、选型与风险）
- `docs/0.1.0/tasks.md`：编码任务清单（完成状态）
- `docs/0.1.0/release-v0.1.0-development-notes.md`：开发说明
- `contracts/README.md`：契约目录与录制流程
- `AGENTS.md` / `llms.txt`：AI 编码助手入口

---

## 8. 路线图

| 阶段 | 目标 | 说明 |
| --- | --- | --- |
| 0.1.0 | 骨架与契约基线 | 本版本：分层、类型、用例、常量、内存仓储、CI |
| 0.2.0 | 可运行 HTTP + ReAct 最小闭环 | 确定 Web/异步选型，接入 SSE、工具注册与假模型 |
| 0.3.x | 插件宿主与持久化 | 插件 manifest/host、会话事件持久化与回放 |
| 后续 | 对齐参考源能力面 | 按参考源与 dsh-java 的升级项逐版对齐 |