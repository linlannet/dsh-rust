# DeepSeek Harness Rust (dsh-rust)

> **TL;DR** · A Rust skeleton of DeepSeek Harness: a contract-first Cargo workspace (types → domain ← case → http/infra), event-session ports with an in-memory adapter, and a runnable runtime entry.
> **Keywords**: Agent Harness, ReAct, AI Agent, MCP, Plugin, Rust, Cargo, LLM
> **Upstream**: [deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) · **Java counterpart**: [dsh-java](https://github.com/linlannet/dsh-java)
> **For AI**: [llms.txt](./llms.txt) · [AGENTS.md](./AGENTS.md)

[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](#5-tech-stack)
[![Edition](https://img.shields.io/badge/edition-2024-blue.svg)](#5-tech-stack)

---

[中文](./README.md) | **English**

A **Rust implementation of DeepSeek Harness**, aiming at a Rust reproduction where **everything is a plugin**, tracking the upstream project and the Java edition over time.

Version `0.1.0` is a **skeleton release**: it fixes the layering, contracts and the smallest closed loop. It does not start an HTTP server, call a model, or implement a plugin runtime yet. The trade-off is "contract-first, minimal dependencies" so that no web framework or database driver is locked in before the selection is settled.

- **Port**: none yet (the HTTP server arrives in a later version)
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
- [4. Contracts](#4-contracts)
- [5. Tech stack](#5-tech-stack)
- [6. Quality gates](#6-quality-gates)
- [7. Documentation](#7-documentation)
- [8. Roadmap](#8-roadmap)

---

## 1. Introduction

dsh-rust reproduces the core shape of DeepSeek Harness in Rust. This version delivers a **compilable, testable, runnable skeleton**:

- **Contract-first**: `contracts/v1` is the single source of truth for cross-process, cross-repository protocols; Rust types map one-to-one to schemas.
- **Layered workspace**: `dsh-types` (contract types) → `dsh-domain` (models and ports) ← `dsh-case` (use cases) → `dsh-http` / `dsh-infra` (transport and adapters).
- **Minimal closed loop**: session create/load/save, request validation, route constants, in-memory repository and a runtime entry.
- **Quality gates**: `cargo fmt`, `cargo clippy -D warnings`, `cargo test` and contract schema validation, all mirrored in CI.

Not included in this version: HTTP server, model calls, the ReAct state machine, plugin host, database persistence, approvals and tool execution. These are listed as boundaries in `docs/0.1.0/spec.md` and arrive in later versions.

---

## 2. Quick start

### 2.1 Requirements

- Rust `1.85+` (edition 2024)
- Node.js `20+` (only for `contracts/tools/validate.mjs`)
- Cargo (installed with the Rust toolchain)

### 2.2 Build and run

```bash
cargo build --workspace
cargo run -p dsh-runtime
```

`dsh-runtime` prints the product version, contract version and fixed route constants to verify wiring and request validation:

```text
dsh-runtime 0.1.0 contract 1.0.0-draft.1
routes health=/health message=/api/agent/message stream=/api/agent/stream
session store empty: true
```

### 2.3 Test and validate

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node contracts/tools/validate.mjs
```

---

## 3. Architecture and layering

```text
apps/dsh-runtime        executable entry (wiring and self-check)
        │
        ├── dsh-case       use-case orchestration, validation, session creation
        │        │
        │        └── dsh-domain   session aggregate + ports (SessionRepository)
        │                    │
        │                    └── dsh-types   contract value objects, errors, IDs
        ├── dsh-http       HTTP/SSE route and event constants
        └── dsh-infra      adapters (in-memory session repository only)
```

The dependency direction is fixed at `case → domain → types`; `http`/`infra` depend on `domain` and `types` to implement outer capabilities; `dsh-types` has no internal dependencies and is the minimal base.

| Crate | Responsibility | Status |
| --- | --- | --- |
| `dsh-types` | Contract types, error codes, identifiers | Implemented |
| `dsh-domain` | Session aggregate and port definitions | Implemented |
| `dsh-case` | Run-request validation, session ensure | Implemented |
| `dsh-http` | Route and SSE constants | Constants fixed, server not wired |
| `dsh-infra` | In-memory session repository | Implemented |
| `apps/dsh-runtime` | Executable entry and self-check output | Implemented |

---

## 4. Contracts

`contracts/` evolves independently from the workspace:

- `contracts/VERSION`: contract version, currently `1.0.0-draft.1`
- `contracts/v1/`: major-version schemas (common/http/sse/plugin-host/plugin-manifest/persistence/config/external)
- `contracts/fixtures/`: recording and replay samples (handwritten for now)
- `contracts/tools/`: validate, record-from-Java, Rust replay and diff scripts
- `contracts/policy/`: compatibility, versioning and deprecation rules

Change flow: update schema and fixture first, then `dsh-types`, then `contracts/CHANGELOG.md`.

---

## 5. Tech stack

| Item | Version / choice |
| --- | --- |
| Language | Rust 1.85+, edition 2024 |
| Build | Cargo workspace (`crates/*`, `apps/*`) |
| Lint | workspace lints: `unsafe_code = forbid`, `dbg_macro = deny` |
| Contract validation | Node.js + `contracts/tools/validate.mjs` |
| Web/async/storage | **Not selected yet** (no Axum/Tokio/SQLx lock-in during the skeleton phase) |

---

## 6. Quality gates

| Gate | Command | Status |
| --- | --- | --- |
| Format | `cargo fmt --all --check` | Passing |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | Passing |
| Unit tests | `cargo test --workspace` | 11 tests passing |
| Contract validation | `node contracts/tools/validate.mjs` | 29 schemas / 46 refs / 4 fixtures, 0 warnings |

CI lives in `.github/workflows/ci.yml` and uses the same commands.

---

## 7. Documentation

- `docs/0.1.0/spec.md`: requirements (what and acceptance criteria)
- `docs/0.1.0/design.md`: design (layering, models, choices and risks)
- `docs/0.1.0/tasks.md`: coding task list with completion state
- `docs/0.1.0/release-v0.1.0-development-notes.md`: development notes
- `contracts/README.md`: contract layout and recording flow
- `AGENTS.md` / `llms.txt`: AI coding assistant entries

---

## 8. Roadmap

| Phase | Goal | Notes |
| --- | --- | --- |
| 0.1.0 | Skeleton and contract baseline | This version: layering, types, use cases, constants, in-memory repository, CI |
| 0.2.0 | Runnable HTTP + minimal ReAct loop | Select web/async stack, wire SSE, tool registry and a fake model |
| 0.3.x | Plugin host and persistence | Plugin manifest/host, session event persistence and replay |
| Later | Align with upstream capabilities | Follow upstream and dsh-java upgrade items release by release |