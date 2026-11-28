# DeepSeek Harness Rust (dsh-rust)

> **TL;DR** · Rust 版 DeepSeek Harness：分层工作区 + 可运行 HTTP/SSE 服务 + ReAct 最小闭环（模型 → 工具 → 结果回填 → 终止），契约先行的 Agent Harness。
> **Keywords**: Agent Harness, ReAct, AI Agent, MCP, Plugin, Rust, Cargo, Axum, SSE, LLM
> **参考源**: [deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) · **Java 对照**: [dsh-java](https://github.com/linlannet/dsh-java)
> **AI 入口**: [llms.txt](./llms.txt) · [AGENTS.md](./AGENTS.md)

[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](#6-技术栈)
[![Edition](https://img.shields.io/badge/edition-2024-blue.svg)](#6-技术栈)
[![Version](https://img.shields.io/badge/version-0.2.0-blue.svg)](#9-路线图)

---

**中文** | [English](./README.en.md)

基于 DeepSeek Harness 的 **Rust 版本实现**，目标是**一切皆插件**的 Rust 复刻，并持续迭代跟踪参考源与 Java 版的动态演进。

当前版本 `0.2.0` 是**首个可运行版本**：HTTP/SSE 服务可用，ReAct 最小闭环可完成一次真实工具调用，严格按 `contracts/v1` 输出；默认 FakeModel 无需网络即可运行，DeepSeek 适配器由 feature 开启。

- **端口**：`127.0.0.1:8090`（默认，可通过 `DSH_BIND_ADDR` 修改）
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
- [4. HTTP 与 SSE API](#4-http-与-sse-api)
- [5. 契约（contracts）](#5-契约contracts)
- [6. 技术栈](#6-技术栈)
- [7. 质量门禁](#7-质量门禁)
- [8. 文档](#8-文档)
- [9. 路线图](#9-路线图)

---

## 1. 项目介绍

dsh-rust 用 Rust 复刻 DeepSeek Harness 的核心形态。0.2.0 交付：

- **可运行 HTTP 服务**：健康检查、阻塞消息、SSE 流式、取消四个端点。
- **ReAct 最小闭环**：模型推理 → 工具调用 → 工具结果回填 → 再推理 → 终止；默认步数上限 8。
- **模型双轨**：FakeModel（默认，可脚本化）与 DeepSeek Chat Completions 适配器（feature `deepseek`）。
- **内置工具**：`echo`、`clock`、`read_file`（工作区沙箱、UTF-8、256 KiB 上限、5s 超时）。
- **契约对齐**：SSE 事件 `chunk`/`tool_call`/`done`/`error`；seq 递增、单终态、keepalive 不占 seq。
- **契约回放**：4 组运行时导出 fixtures + 回放测试；openapi 由路由表生成并在 CI 校验。

本版本**不包含**：插件宿主、持久化、鉴权、审批、多租户。对应能力见 `docs/0.2.0/spec.md` §8。

---

## 2. 快速开始

### 2.1 环境要求

- Rust `1.85+`（edition 2024）
- Node.js `20+`（仅用于契约校验与 openapi 生成）
- Cargo（随 Rust 工具链安装）

### 2.2 构建与运行

```bash
cargo build --workspace
cargo run -p dsh-runtime
```

默认以 FakeModel 启动，监听 `127.0.0.1:8090`：

```text
INFO dsh_runtime: dsh-runtime starting bind=127.0.0.1:8090 model="fake" tools=3 contract="1.0.0-draft.1" version="0.2.0"
```

### 2.3 调用示例

```bash
# 健康检查
curl -s http://127.0.0.1:8090/health

# 阻塞消息（回显）
curl -s -X POST http://127.0.0.1:8090/api/agent/message \
  -H 'content-type: application/json' \
  -d '{"agentId":"agent-1","content":[{"type":"text","text":"hello"}]}'

# 流式消息（SSE）
curl -sN -X POST http://127.0.0.1:8090/api/agent/stream \
  -H 'content-type: application/json' \
  -d '{"agentId":"agent-1","content":[{"type":"text","text":"time"}]}'

# 取消
curl -s -X POST http://127.0.0.1:8090/api/agent/cancel \
  -H 'content-type: application/json' \
  -d '{"agentId":"agent-1","runId":"run-xxx"}'
```

触发一次工具调用：以 `DSH_FAKE_SCRIPT=tool` 启动，FakeModel 会先请求 `clock` 工具，再总结工具结果。

### 2.4 配置

| 环境变量 | 默认 | 说明 |
| --- | --- | --- |
| `DSH_BIND_ADDR` | `127.0.0.1:8090` | 监听地址 |
| `DSH_MODEL` | `fake` | `fake` / `deepseek` |
| `DSH_FAKE_SCRIPT` | `echo` | `echo` / `tool` |
| `DSH_MAX_STEPS` | `8` | ReAct 步数上限 |
| `DSH_TOOL_TIMEOUT_MS` | `5000` | 单次工具超时 |
| `DSH_MODEL_TIMEOUT_MS` | `60000` | 模型超时 |
| `DSH_WORKSPACE_ROOT` | 当前目录 | `read_file` 允许根 |
| `DSH_READ_FILE_MAX_BYTES` | `262144` | 单文件读取上限 |
| `DSH_MODEL_BASE_URL` | `https://api.deepseek.com` | 仅 deepseek |
| `DEEPSEEK_API_KEY` | 无 | 仅 deepseek；缺失启动失败 |

启用 DeepSeek：

```bash
DEEPSEEK_API_KEY=... DSH_MODEL=deepseek cargo run -p dsh-runtime --features deepseek
```

### 2.5 测试与校验

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
node contracts/tools/validate.mjs
node contracts/tools/generate-openapi.mjs --check
```

---

## 3. 架构与分层

```text
apps/dsh-runtime        配置、装配、axum::serve、优雅关闭
        │
        ├── dsh-http       路由、DTO、错误映射、SSE 编码、traceId
        ├── dsh-case       ReAct 循环、ToolRegistry、RunRegistry、EventSink
        │        │
        │        └── dsh-domain   Session、ModelPort、Tool、RunEvent
        │                    │
        │                    └── dsh-types   契约类型、错误模型、ID
        └── dsh-infra      FakeModel/DeepSeek、内置工具、文件沙箱、内存存储
```

依赖方向固定为 `http → case → domain → types`，`infra → domain`；`dsh-types` 不依赖其它 workspace crate。

| crate | 职责 |
| --- | --- |
| `dsh-types` | 契约值对象、ID、错误码、JSON Schema 值 |
| `dsh-domain` | 会话聚合、模型端口、工具端口、运行事件 |
| `dsh-case` | ReAct 编排、工具注册、运行注册与取消、事件输出 |
| `dsh-http` | Axum 路由、DTO、错误映射、SSE 编码 |
| `dsh-infra` | 模型适配器、内置工具、沙箱、内存实现 |
| `apps/dsh-runtime` | 可执行入口与配置装配 |

---

## 4. HTTP 与 SSE API

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/health` | `{status, version, contractVersion}` |
| POST | `/api/agent/message` | 阻塞式 Agent 消息，返回 `AgentMessageResponse` |
| POST | `/api/agent/stream` | SSE 流式输出 |
| POST | `/api/agent/cancel` | 按 `runId` 或 `agentId` 取消 |

错误映射：`invalid_request→400`、`not_found→404`、`conflict→409`、`timeout→504`、`unavailable→503`、`invalid_response→502`、`internal→500`；错误体统一 `{code, message, retryable?, traceId?}`。

SSE 事件（对齐 `contracts/v1/sse/catalog.md`）：

```text
event: tool_call
id: 1
data: {"toolCallId":"call-clock-1","name":"clock","status":"started","arguments":{}}

event: chunk
id: 3
data: {"messageId":"msg-2","delta":"observed: ..."}

event: done
id: 4
data: {"finishReason":"stop"}
```

约定：`seq` 从 1 递增；每个流恰好一个终态（`done` 或 `error`）且最后出现；`keepalive` 为注释行、不占 seq。

---

## 5. 契约（contracts）

`contracts/` 与工作区版本独立演进：

- `contracts/VERSION`：契约版本，当前 `1.0.0-draft.1`
- `contracts/v1/`：schema（common/http/sse/plugin-host/plugin-manifest/persistence/config/external）与生成的 `openapi.yaml`
- `contracts/fixtures/`：8 组样例（4 组手写、4 组运行时导出）
- `contracts/tools/`：validate、record、replay、diff 与 `generate-openapi.mjs`
- `contracts/policy/`：兼容、版本与废弃规则

契约变更顺序：schema → fixture → `dsh-types` → `contracts/CHANGELOG.md`。

---

## 6. 技术栈

| 项 | 版本/选型 |
| --- | --- |
| 语言 | Rust 1.85+，edition 2024 |
| Web | Axum 0.8 |
| 异步 | Tokio 1（rt-multi-thread/macros/time/sync/net/signal） |
| 序列化 | serde 1 + serde_json 1 |
| 日志 | tracing 0.1 + tracing-subscriber 0.3 |
| DeepSeek | reqwest 0.12（可选 feature `deepseek`，rustls） |
| 契约校验 | Node.js + `contracts/tools/*.mjs` |

---

## 7. 质量门禁

| 门禁 | 命令 | 当前状态 |
| --- | --- | --- |
| 格式 | `cargo fmt --all --check` | 通过 |
| 静态检查 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过 |
| 测试 | `cargo test --workspace --all-features` | 51 passed |
| 契约校验 | `node contracts/tools/validate.mjs` | 29 schema / 52 refs / 8 fixtures，0 警告 |
| openapi 一致性 | `node contracts/tools/generate-openapi.mjs --check` | 通过 |

CI 见 `.github/workflows/ci.yml`，与上表命令一致。

---

## 8. 文档

- `docs/0.1.0/`：骨架版本四件套
- `docs/0.2.0/spec.md`：HTTP + ReAct 最小闭环需求与验收
- `docs/0.2.0/design.md`：分层、接口、选型与风险
- `docs/0.2.0/tasks.md`：编码任务与完成状态
- `docs/0.2.0/release-v0.2.0-development-notes.md`：验证结论与已知限制
- `contracts/README.md`：契约目录与录制流程
- `AGENTS.md` / `llms.txt`：AI 编码助手入口

---

## 9. 路线图

| 阶段 | 目标 | 说明 |
| --- | --- | --- |
| 0.1.0 | 骨架与契约基线 | 分层、类型、用例、常量、内存仓储、CI |
| 0.2.0 | 可运行 HTTP + ReAct 最小闭环 | Axum/Tokio、SSE、工具、Fake/DeepSeek、契约回放 |
| 0.2.1 | 插件宿主与持久化 | 插件 manifest/host、会话事件持久化与回放 |
| 后续 | 对齐参考源能力面 | 按参考源与 dsh-java 的升级项逐版对齐 |