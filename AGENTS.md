# AGENTS.md — dsh-rust

## Project
Rust 2024 (1.85+) **Agent Harness**: contract-first Cargo workspace with runnable HTTP/SSE, a minimal ReAct loop (model → tool → observation), built-in tools and in-memory stores. Version 0.2.0; contract version 1.0.0-draft.1.

## Layout
- `crates/dsh-types` — contract value objects, IDs, `Error`/`ErrorCode`, JSON schema values
- `crates/dsh-domain` — `Session`, `ModelPort`, `Tool`, `RunEvent`/`RunStatus`; depends on `dsh-types` only
- `crates/dsh-case` — `AgentService` (ReAct), `ToolRegistry`, `RunRegistry`, `EventSink`
- `crates/dsh-http` — Axum routes, DTOs, error mapping, SSE encoding, `serve`
- `crates/dsh-infra` — FakeModel, DeepSeek (feature `deepseek`), tools, file sandbox, in-memory stores
- `apps/dsh-runtime` — config, wiring, startup logging, graceful shutdown
- `contracts/` — `v1` schemas/openapi, fixtures, policy and tooling (single source of truth)

Dependency direction: `http -> case -> domain -> types`; `infra -> domain`. `dsh-domain` must not depend on axum/tokio; `dsh-types` must not depend on other workspace crates.

## Build & Test
- Build: `cargo build --workspace`
- Build all features: `cargo check --workspace --all-features`
- Test: `cargo test --workspace --all-features`
- Single crate: `cargo test -p dsh-case`
- Format: `cargo fmt --all --check`
- Lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Run: `cargo run -p dsh-runtime` (default `127.0.0.1:8090`, FakeModel)
- Contracts: `node contracts/tools/validate.mjs`
- OpenAPI: `node contracts/tools/generate-openapi.mjs --check`

## Conventions
- Rust edition 2024; MSRV 1.85; `rustfmt` defaults
- Errors flow through `dsh_types::{Error, ErrorCode}`; do not invent per-crate error enums
- Tools are synchronous `Tool` impls; `dsh-case` runs them via `spawn_blocking` + `tokio::time::timeout`
- SSE semantics follow `contracts/v1/sse/catalog.md`: increasing `seq`, exactly one terminal event last, keepalive excluded from seq
- Contract changes: schema + fixture first, then `dsh-types`, then `contracts/CHANGELOG.md`; regenerate openapi
- New third-party dependency must be recorded in `docs/0.2.0/design.md` §4.1 before use
- Docs are Chinese by default; keep `README.md` and `README.en.md` in sync

## Do Not
- Do not change SSE event names, ordering or terminal semantics without updating contracts and fixtures
- Do not introduce a real LLM as the default path (FakeModel stays default; DeepSeek is feature-gated)
- Do not bypass the `read_file` sandbox (canonicalize + `starts_with(root)` + size/UTF-8 checks)
- Do not log secrets (`DEEPSEEK_API_KEY`, tokens)
- Do not commit `target/`, `.idea/` or generated artifacts

## Key Docs
- `README.md`, `README.en.md`
- `docs/0.1.0/`, `docs/0.2.0/` (spec / design / tasks / release notes)
- `contracts/README.md`
- `llms.txt`