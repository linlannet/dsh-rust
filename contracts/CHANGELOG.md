# Changelog

契约版本与产品版本独立。破坏性变更必须新开大版本目录。

## [Unreleased]

### Added

- 新增 `v1/common/error.schema.json#/$defs/errorCode`：登记稳定错误码集合（含 `storage_unavailable`、`stream_budget_exceeded`），只允许追加。
- 新增 `v1/http/session-events.request.schema.json` 与 `v1/http/session-events.response.schema.json`：`GET /api/sessions/{sessionId}/events` 的路径/查询参数与分页响应（`events` + `lastSeq`）。
- 新增 `v1/persistence/migrations/0001-baseline.sql`：随附于 `0001-baseline.md` 的 DDL 落地（`schema_migration`/`session`/`session_message`/`session_event`/`run`），幂等且只建表建索引。
- 路由表新增 `GET /api/sessions/{sessionId}/events`，`v1/openapi.yaml` 同步再生成。
- 新增 2 组 fixtures：回放分页（`http/session-events/001-replay-page`）、重启续跑（`persistence/002-restart-continuation`）。
- 新增 4 组运行时导出 fixtures：阻塞工具调用、流式文本、流式工具调用、取消。
- 新增 `tools/generate-openapi.mjs`，从路由表生成 `v1/openapi.yaml` 并支持 `--check`。
- 初始化 `v1` 骨架：common、http、sse、plugin-host、plugin-manifest、persistence、config、external。
- 初始化 `fixtures` 样例与 `tools` 校验脚本。

### Compatibility

- 纯增量：既有请求/响应字段、SSE 事件名与终态语义均未变更；新增端点不替换任何现有端点。
- 契约版本保持 `1.0.0-draft.1`，字段仍以 Java 基线录制结果为准。

## [1.0.0-draft.1] - 2026-11-27

### Added

- 首个草案版本。字段以 Java 基线录制结果为准，录制前不作为稳定承诺。
