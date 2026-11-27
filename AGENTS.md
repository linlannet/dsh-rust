# AGENTS.md — dsh-rust

## Project
Rust 2024 (1.85+) **Agent Harness** skeleton: contract-first Cargo workspace with domain ports, use cases, HTTP/SSE constants and an in-memory adapter. Version 0.1.0; contract version 1.0.0-draft.1.

## Layout
- `crates/dsh-types` — contract value objects, identifiers, error model (no internal deps)
- `crates/dsh-domain` — session aggregate and ports; depends on `dsh-types` only
- `crates/dsh-case` — use cases, request validation, session orchestration
- `crates/dsh-http` — HTTP/SSE route and event constants (server not wired yet)
- `crates/dsh-infra` — adapters; currently the in-memory session repository
- `apps/dsh-runtime` — executable entry and self-check output
- `contracts/` — `v1` schemas, fixtures, policy and tooling; single source of truth for cross-process protocols

Dependency direction: `case -> domain -> types`; `http`/`infra` depend on `domain` and `types`. Never let `dsh-types` depend on another workspace crate.

## Build & Test
- Build: `cargo build --workspace`
- Test: `cargo test --workspace`
- Single crate: `cargo test -p dsh-case`
- Format: `cargo fmt --all --check`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Run: `cargo run -p dsh-runtime`
- Contracts: `node contracts/tools/validate.mjs`

## Conventions
- Rust edition 2024; MSRV 1.85; formatting via `rustfmt` defaults
- Errors flow through `dsh_types::{Error, ErrorCode}`; do not invent per-crate error enums
- Domain and types stay free of web frameworks, async runtimes, database drivers and model SDKs
- Contract changes: schema + fixture first, then `dsh-types`, then `contracts/CHANGELOG.md`
- `contracts/VERSION` is independent from the Cargo workspace version
- Docs are Chinese by default; keep `README.md` and `README.en.md` in sync

## Do Not
- Do not add a third-party dependency without explicit justification
- Do not bind Axum/Tokio/SQLx before the selection is recorded in `docs/0.1.0/design.md`
- Do not edit `contracts/v1` schemas without updating fixtures and the contract CHANGELOG
- Do not log secrets (API keys, tokens, passwords)
- Do not commit `target/`, `.idea/` or generated artifacts

## Key Docs
- `README.md`, `README.en.md`
- `docs/0.1.0/` (spec / design / tasks / release notes)
- `contracts/README.md`
- `llms.txt`