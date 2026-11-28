# DeepSeek Harness Rust (dsh-rust)

> **TL;DR** · A Rust implementation of DeepSeek Harness: layered workspace + runnable HTTP/SSE service + minimal ReAct loop (model → tool → observation → stop), contract-first.
> **Keywords**: Agent Harness, ReAct, AI Agent, MCP, Plugin, Rust, Cargo, Axum, SSE, LLM
> **Upstream**: [deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) · **Java counterpart**: [dsh-java](https://github.com/linlannet/dsh-java)
> **For AI**: [llms.txt](./llms.txt) · [AGENTS.md](./AGENTS.md)

[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](#6-tech-stack)
[![Edition](https://img.shields.io/badge/edition-2024-blue.svg)](#6-tech-stack)
[![Version](https://img.shields.io/badge/version-0.2.1-blue.svg)](#9-roadmap)

---

[中文](./README.md) | **English**

A **Rust implementation of DeepSeek Harness**, aiming at a Rust reproduction where **everything is a plugin**, tracking the upstream project and the Java edition over time.

Version `0.2.1` adds **persistence and replay** on top of the 0.2.0 loop: sessions and events live in SQLite, a restarted process continues the same session, and events can be replayed page by page; streaming output is bounded by a byte budget.

- **Port**: `127.0.0.1:8090` (override with `DSH_BIND_ADDR`)
- **Contract version**: `1.0.0-draft.1` (independent from the product version, see `contracts/VERSION`)
- **License**: see the repository root

---

## Version history

All changes are recorded in [CHANGELOG](./CHANGELOG.md). This README keeps only stable architecture and usage notes.

---

## Table of contents

- [1. Introduction](#1-introduction)
- [2. Quick start](#2-quick-start)
- [3. Architecture and layering](#3-architecture-and-layering)
- [4. HTTP and SSE API](#4-http-and-sse-api)
- [5. Contracts](#5-contracts)
- [6. Tech stack](#6-tech-stack)
- [7. Quality gates](#7-quality-gates)
- [8. Documentation](#8-documentation)
- [9. Roadmap](#9-roadmap)

---

## 1. Introduction

0.2.1 delivers:

- **Runnable HTTP service**: health, blocking message, SSE stream, cancel, session event replay.
- **Minimal ReAct loop**: model step → tool call → observation → next step → stop; default max 8 steps.
- **Persistence and continuation**: sessions and event logs in SQLite; the same `sessionId` keeps its history across restarts.
- **Event replay**: six event families (`session.created` / `message.appended` / `run.started` / `tool.*` / `run.*`) paged by `seq`.
- **Dual model path**: scriptable FakeModel (default) and DeepSeek Chat Completions adapter (`deepseek` feature).
- **Built-in tools**: `echo`, `clock`, `read_file` (workspace sandbox, UTF-8, 256 KiB limit, 5s timeout).
- **Contract-aligned SSE**: `chunk` / `tool_call` / `done` / `error`; increasing seq, single terminal event, keepalive excluded; oversize output ends with `stream_budget_exceeded`.

Not included: plugin host, approvals, authentication, multi-tenancy (see `docs/0.2.1/spec.md` §8).

---

## 2. Quick start

### 2.1 Requirements

- Rust `1.85+` (edition 2024)
- Node.js `20+` (contract validation and openapi generation)
- Cargo

### 2.2 Build and run

```bash
cargo build --workspace
cargo run -p dsh-runtime
```

Starts with FakeModel on `127.0.0.1:8090`:

```text
INFO dsh_runtime: dsh-runtime starting bind=127.0.0.1:8090 model="fake" store="sqlite" db=./data/dsh-rust.db tools=3 stream_budget_bytes=8388608 contract="1.0.0-draft.1" version="0.2.1"
```

### 2.3 Examples

```bash
curl -s http://127.0.0.1:8090/health

curl -s -X POST http://127.0.0.1:8090/api/agent/message \
  -H 'content-type: application/json' \
  -d '{"agentId":"agent-1","content":[{"type":"text","text":"hello"}]}'

curl -sN -X POST http://127.0.0.1:8090/api/agent/stream \
  -H 'content-type: application/json' \
  -d '{"agentId":"agent-1","content":[{"type":"text","text":"time"}]}'

curl -s -X POST http://127.0.0.1:8090/api/agent/cancel \
  -H 'content-type: application/json' \
  -d '{"agentId":"agent-1","runId":"run-xxx"}'

curl -s 'http://127.0.0.1:8090/api/sessions/sess-xxx/events?afterSeq=0&limit=200'
```

Run with `DSH_FAKE_SCRIPT=tool` to trigger a `clock` tool call followed by a summary. Sessions and events go to `./data/dsh-rust.db` by default; set `DSH_STORE=memory` to fall back to in-memory only.

### 2.4 Configuration

| Variable | Default | Notes |
| --- | --- | --- |
| `DSH_BIND_ADDR` | `127.0.0.1:8090` | Listen address |
| `DSH_MODEL` | `fake` | `fake` / `deepseek` |
| `DSH_FAKE_SCRIPT` | `echo` | `echo` / `tool` |
| `DSH_MAX_STEPS` | `8` | ReAct step limit |
| `DSH_TOOL_TIMEOUT_MS` | `5000` | Per-tool timeout |
| `DSH_MODEL_TIMEOUT_MS` | `60000` | Model timeout |
| `DSH_WORKSPACE_ROOT` | current dir | `read_file` root |
| `DSH_READ_FILE_MAX_BYTES` | `262144` | Read limit |
| `DSH_MODEL_BASE_URL` | `https://api.deepseek.com` | deepseek only |
| `DEEPSEEK_API_KEY` | none | deepseek only; startup fails if missing |
| `DSH_STORE` | `sqlite` | `sqlite` / `memory`; `memory` is the rollback path |
| `DSH_DB_PATH` | `./data/dsh-rust.db` | Database path; parent directory is created |
| `DSH_STREAM_BUDGET_BYTES` | `8388608` | Per-run SSE byte budget |

```bash
DEEPSEEK_API_KEY=... DSH_MODEL=deepseek cargo run -p dsh-runtime --features deepseek
```

### 2.5 Test and validate

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
node contracts/tools/validate.mjs
node contracts/tools/generate-openapi.mjs --check
```

---

## 3. Architecture and layering

```text
apps/dsh-runtime        config, wiring, axum::serve, graceful shutdown
        │
        ├── dsh-http       routes, DTOs, error mapping, SSE encoding, traceId
        ├── dsh-case       ReAct loop, ToolRegistry, RunRegistry, EventSink
        │        │
        │        └── dsh-domain   Session, ModelPort, Tool, RunEvent
        │                    │
        │                    └── dsh-types   contract types, errors, IDs
        └── dsh-infra      FakeModel/DeepSeek, built-in tools, sandbox, memory stores
```

Dependency direction: `http → case → domain → types`; `infra → domain`; `dsh-types` has no workspace dependencies.

| Crate | Responsibility |
| --- | --- |
| `dsh-types` | Contract value objects, IDs, error codes, JSON Schema value |
| `dsh-domain` | Session aggregate, model/tool ports, run events |
| `dsh-case` | ReAct orchestration, tool registry, run registry/cancel, event sink |
| `dsh-http` | Axum routes, DTOs, error mapping, SSE encoding |
| `dsh-infra` | Model adapters, built-in tools, sandbox, in-memory stores |
| `apps/dsh-runtime` | Executable entry and configuration |

---

## 4. HTTP and SSE API

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/health` | `{status, version, contractVersion}` |
| POST | `/api/agent/message` | Blocking agent message |
| POST | `/api/agent/stream` | SSE stream |
| POST | `/api/agent/cancel` | Cancel by `runId` or `agentId` |
| GET | `/api/sessions/{sessionId}/events` | Paged session events by `seq` (`afterSeq`/`limit`) |

Error mapping: `invalid_request→400`, `not_found→404`, `conflict→409`, `timeout→504`, `unavailable→503`, `storage_unavailable→503`, `invalid_response→502`, `internal→500`; body `{code, message, retryable?, traceId?}`.

SSE rules: seq starts at 1 and increases; exactly one terminal event (`done` or `error`) appears last; keepalive is a comment and does not consume seq.

---

## 5. Contracts

- `contracts/VERSION`: `1.0.0-draft.1`
- `contracts/v1/`: schemas and generated `openapi.yaml`
- `contracts/fixtures/`: 8 cases (4 handwritten, 4 runtime-exported)
- `contracts/tools/`: validate, record, replay, diff, `generate-openapi.mjs`
- `contracts/policy/`: compatibility, versioning, deprecation

Change flow: schema → fixture → `dsh-types` → `contracts/CHANGELOG.md`.

---

## 6. Tech stack

| Item | Choice |
| --- | --- |
| Language | Rust 1.85+, edition 2024 |
| Web | Axum 0.8 |
| Async | Tokio 1 |
| Serialization | serde 1 + serde_json 1 |
| Logging | tracing 0.1 + tracing-subscriber 0.3 |
| DeepSeek | reqwest 0.12 (optional `deepseek` feature, rustls) |
| Storage | rusqlite 0.40 (`bundled`, `dsh-infra` only) with a `spawn_blocking` bridge |
| Contract tooling | Node.js + `contracts/tools/*.mjs` |

---

## 7. Quality gates

| Gate | Result |
| --- | --- |
| `cargo fmt --all --check` | Passing |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Passing |
| `cargo test --workspace --all-features` | 87 passed |
| `node contracts/tools/validate.mjs` | 31 schemas / 68 refs / 10 fixtures, 0 warnings |
| `node contracts/tools/generate-openapi.mjs --check` | Passing |

---

## 8. Documentation

- `docs/0.1.0/`: skeleton release four-piece set
- `docs/0.2.0/`: HTTP + minimal ReAct four-piece set
- `docs/0.2.1/spec.md`: persistence and replay requirements
- `docs/0.2.1/design.md`: storage choices, event model, blocking bridge, risks
- `docs/0.2.1/tasks.md`: coding tasks and status
- `docs/0.2.1/release-notes-v0.2.1.md` / `release-notes-v0.2.1.en.md`: user-facing release notes (features, behavior changes, upgrade guide)
- `docs/0.2.1/release-v0.2.1-development-notes.md`: verification and limitations
- `contracts/README.md`: contract layout and recording flow
- `AGENTS.md` / `llms.txt`: AI entries

---

## 9. Roadmap

| Phase | Goal | Notes |
| --- | --- | --- |
| 0.1.0 | Skeleton and contract baseline | Layering, types, use cases, constants, in-memory repo, CI |
| 0.2.0 | Runnable HTTP + minimal ReAct loop | Axum/Tokio, SSE, tools, Fake/DeepSeek, contract replay |
| 0.2.1 | Persistence and replay | Session/event storage on SQLite, restart continuation, event replay endpoint, SSE output budget |
| 0.2.2 | Plugin host | Plugin manifest/host, stdio JSON-RPC against the Java host |
| 0.2.3 | Approval and permission matrix | Tool policy, approval events, run suspension |
| Later | Align with upstream capabilities | Follow upstream and dsh-java upgrade items release by release |