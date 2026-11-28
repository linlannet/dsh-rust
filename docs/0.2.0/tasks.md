# DeepSeek Harness Rust v0.2.0 编码任务清单

> 版本：`v0.2.0` | 基线：v0.1.0 | 架构：既有六层 + Axum 接入层 | 契约版本：`1.0.0-draft.1`
>
> 本清单把 `docs/0.2.0/spec.md` §5.1–§5.10 与 `design.md` 分层落位转化为可执行条目。
>
> **完成状态图例**：`[x]` 已完成（2026-11-27 实施）
>
> **任务统计**：13 个任务组，83 条全部完成；门禁结果见 `release-v0.2.0-development-notes.md`。
>
> **范围边界（强制）**：不新增 workspace 成员；新增依赖仅限 design §4.1 清单；不新增数据库表；`dsh-domain` 不依赖 axum/tokio。

---

## 1. 依赖与 workspace

> 依赖：无 ｜ 覆盖：design §4.1 ｜ 状态：待办

- [x] `dsh-types` 增加 `serde`（derive）与 `serde_json`；验收：`cargo check -p dsh-types` 通过 —— `crates/dsh-types/Cargo.toml`
- [x] `dsh-http` 增加 `axum`、`tokio`（rt-multi-thread/macros/time/sync/net）；验收：`cargo check -p dsh-http`
- [x] `dsh-case` 增加 `tokio`（time/sync，不启用 rt）；验收：依赖树无 `rt-multi-thread` 传播
- [x] `dsh-infra` 增加 `serde_json`；`deepseek` feature 增加 `reqwest`（rustls-tls）；验收：`cargo check -p dsh-infra --features deepseek`
- [x] `dsh-runtime` 增加 `tokio`（full 或显式 rt-multi-thread/macros/signal）与 `tracing-subscriber`；验收：`cargo check -p dsh-runtime`
- [x] 增加 `deepseek` feature 定义（默认关闭）；验收：`cargo check --all-features` 通过
- [x] CI 增加 `cargo check --all-features`；验收：workflow 更新 —— `.github/workflows/ci.yml`

---

## 2. dsh-types 扩展

> 依赖：1 ｜ 覆盖：spec §5.1/§5.8 ｜ 状态：待办

- [x] `ContentBlock` 增加 serde：`#[serde(tag = "type", rename_all = "lowercase")]`；验收：序列化为 `{"type":"text","text":...}`
- [x] `ToolDescriptor` 字段 rename 为 `inputSchema`/`outputSchema`；验收：JSON 键与契约一致
- [x] `Error` 增加 serde（`code` 字符串、`retryable` 可选）；验收：序列化符合 `common/error.schema.json`
- [x] `Id` 增加 serde（透明字符串 + 解析校验）；验收：非法 ID 反序列化失败
- [x] 保持 `ErrorCode::as_str` 小写下划线；验收：单测覆盖 7 个码

---

## 3. dsh-domain 扩展

> 依赖：2 ｜ 覆盖：spec §5.3/§5.6 ｜ 状态：待办

- [x] 新增 `model.rs`：`ModelPort`、`ModelRequest`、`ModelStep`、`ChatMessage`、`BoxFuture`；验收：`Arc<dyn ModelPort>` 可用 —— `crates/dsh-domain/src/model.rs`
- [x] 新增 `tool.rs`：`Tool` trait、`ToolOutput`、`ToolCallRequest`；验收：`Arc<dyn Tool>` 可用 —— `crates/dsh-domain/src/tool.rs`
- [x] 新增 `run.rs`：`RunEvent`、`RunStatus`、`FinishReason`、`Usage`、`RunRecord`；验收：枚举与 `enums.runStatus`/`finishReason` 对齐
- [x] `Session` 增加 `messages: Vec<ChatMessage>` 与 `active_run: Option<RunId>`；验收：字段可用（内存）
- [x] 领域层不依赖 axum/tokio；验收：`cargo tree -p dsh-domain` 无相关依赖
- [x] 单测：消息构造、运行状态迁移；验收：新增 ≥ 3 个测试

---

## 4. dsh-case ReAct 与注册表

> 依赖：3 ｜ 覆盖：spec §5.3/§5.6 ｜ 状态：待办

- [x] `ToolRegistry`：注册/按名查找/排序目录；验收：目录顺序稳定 —— `crates/dsh-case/src/tool_registry.rs`
- [x] `RunRegistry`：注册、按 runId/agentId 取消、会话占用 guard；验收：并发/幂等用例 —— `crates/dsh-case/src/run_registry.rs`
- [x] `EventSink`：seq 分配、`finish_once` 单终态；验收：重复 finish 被拒绝 —— `crates/dsh-case/src/event_sink.rs`
- [x] `AgentRunner`：ReAct 循环（模型 → 工具 → 回填 → 再推理）；验收：单步/工具/上限用例 —— `crates/dsh-case/src/runner.rs`
- [x] 步数上限 `max_steps` 与终止原因映射；验收：超限 `finishReason=length`
- [x] 工具执行：`spawn_blocking` + 超时 + 错误映射；验收：超时 `timeout`、失败 `failed`
- [x] 重复调用保护（同名同参 > 2）；验收：返回 `conflict`
- [x] 取消检查点（每步与每次工具前后）；验收：取消后不再产生 chunk

---

## 5. FakeModel

> 依赖：3 ｜ 覆盖：spec §5.4 ｜ 状态：待办

- [x] `FakeModel` 实现 `ModelPort`（`echo` 脚本：回显文本 → `stop`）；验收：单测 —— `crates/dsh-infra/src/model/fake.rs`
- [x] `tool` 脚本：首步返回 `clock` 工具调用，次步总结文本；验收：集成用例
- [x] `DSH_FAKE_SCRIPT` 选择脚本；验收：配置生效
- [x] 无网络依赖；验收：断网 `cargo test` 通过

---

## 6. DeepSeek 适配器（feature）

> 依赖：1、3 ｜ 覆盖：spec §5.4 ｜ 状态：待办

- [x] `DeepSeekModel` 实现 `ModelPort`（feature `deepseek`）；验收：`--features deepseek` 编译 —— `crates/dsh-infra/src/model/deepseek.rs`
- [x] 配置校验：未启用 feature 或缺少 key 时启动失败并给出指引；验收：错误信息可读
- [x] 超时（60s）与错误映射（`timeout`/`unavailable`）；验收：mock 服务器用例
- [x] 最小字段消费：增量文本 + 工具调用；验收：响应解析单测
- [x] 密钥不写日志；验收：代码审查（全链路无密钥日志输出）

---

## 7. 内置工具与沙箱

> 依赖：3 ｜ 覆盖：spec §5.5 ｜ 状态：待办

- [x] `echo` 工具（`{text}` → `{text}`）；验收：单测 —— `crates/dsh-infra/src/tools/echo.rs`
- [x] `clock` 工具（无参 → `{now}` RFC3339 UTC）；验收：格式单测 —— `crates/dsh-infra/src/tools/clock.rs`
- [x] `read_file` 工具（`{path}` → `{path, content, bytes}`）；验收：正常读取单测 —— `crates/dsh-infra/src/tools/read_file.rs`
- [x] `ReadFileSandbox`：canonicalize + starts_with + `..`/符号链接拒绝；验收：逃逸用例 —— `crates/dsh-infra/src/sandbox.rs`
- [x] 大小上限（默认 256 KiB，先 metadata 后读）；验收：超限 `invalid_request` + `details.limit`
- [x] UTF-8 校验；验收：二进制文件 `invalid_request`
- [x] 工具描述与输入 Schema 对齐 `tool.schema.json`；验收：序列化断言
- [x] 工具目录快照测试；验收：名称/描述/参数稳定

---

## 8. dsh-http 路由与 DTO

> 依赖：1、2、4 ｜ 覆盖：spec §5.1 ｜ 状态：待办

- [x] `AppState` 与 `Router`：4 条路由挂载；验收：路由测试 —— `crates/dsh-http/src/router.rs`
- [x] DTO：`AgentMessageRequest`/`AgentMessageResponse`/`AgentCancelRequest`/`HealthResponse`；验收：序列化匹配契约 —— `crates/dsh-http/src/dto.rs`
- [x] 请求校验：`agentId` 必填、`content` 非空且仅文本；验收：400 用例
- [x] `HttpError` + `IntoResponse` 映射表；验收：7 个码→状态码用例 —— `crates/dsh-http/src/error.rs`
- [x] `traceId` 生成（时间+计数）并写入错误体；验收：错误体含 traceId —— `crates/dsh-http/src/trace.rs`
- [x] `/health` 返回 status/version/contractVersion；验收：集成测试
- [x] 阻塞消息端点打通 FakeModel；验收：200 + 契约结构

---

## 9. SSE 与取消

> 依赖：8 ｜ 覆盖：spec §5.2/§5.6 ｜ 状态：待办

- [x] `RunEvent` → SSE 编码（事件名、data JSON、`id: seq`）；验收：事件流快照 —— `crates/dsh-http/src/sse.rs`
- [x] 心跳 `:keepalive` 且不占 seq；验收：空闲流用例
- [x] 单终态保证（done/error 二选一且最后）；验收：异常路径用例
- [x] 断连触发取消（stream drop）；验收：客户端断开用例
- [x] 取消端点：按 runId/agentId 广播；验收：`done(cancelled)`
- [x] 取消幂等与已完成运行的冲突策略；验收：重复取消用例
- [x] 取消后释放会话占用；验收：可再次发起运行

---

## 10. runtime 配置与启动

> 依赖：8、9、5、7 ｜ 覆盖：spec §5.9 ｜ 状态：待办

- [x] 环境变量解析与默认值（§5.9 表）；验收：默认启动 —— `apps/dsh-runtime/src/config.rs`
- [x] 依赖装配（模型选择、工具注册、仓储、注册表）；验收：启动日志含模型/工具数 —— `apps/dsh-runtime/src/main.rs`
- [x] `axum::serve` 监听 `DSH_BIND_ADDR`；验收：`curl /health`
- [x] 优雅关闭（`tokio::signal`）；验收：Ctrl-C 退出码 0
- [x] 启动日志不打印密钥；验收：日志断言
- [x] 配置错误快速失败（非法端口/步数/大小）；验收：错误信息可读

---

## 11. 契约 fixtures、回放与 openapi

> 依赖：8、9 ｜ 覆盖：spec §5.8 ｜ 状态：待办

- [x] 导出运行事件为 NDJSON 并归一化（时间/ID/耗时/绝对路径）；验收：可直接作为 fixture —— `contracts/tools/export-rust.sh`
- [x] 新增 fixtures：阻塞消息、流式文本、工具调用、取消；验收：`validate.mjs` 识别 —— `contracts/fixtures/http/agent-message/*`、`contracts/fixtures/sse/*`
- [x] 回放测试：读取 fixture 驱动运行时并逐事件比对；验收：新增 4 组（总计 8 组）通过 —— `crates/dsh-http/tests/contract_replay.rs`
- [x] openapi 由路由表生成或以脚本生成；验收：与提交版本一致 —— `contracts/v1/openapi.yaml`
- [x] CI 增加 openapi 再生成校验；验收：无差异
- [x] `contracts/CHANGELOG.md` 记录新增 fixtures 与生成规则；验收：条目存在
- [x] 评估契约版本是否升 `1.0.0`（录制通过后）；验收：结论记录于 release notes —— 结论：保持 `1.0.0-draft.1`

---

## 12. 测试与门禁

> 依赖：全部 ｜ 覆盖：spec §5.10 ｜ 状态：待办

- [x] 单元测试：状态机、沙箱、SSE 排序、错误映射；验收：新增 ≥ 25 个测试
- [x] 集成测试：health/阻塞/流式/取消/并发冲突；验收：`cargo test` 全绿
- [x] 负面用例：越界路径、超限文件、非法 JSON、缺字段、模型超时、工具超时；验收：全部覆盖
- [x] `cargo fmt --all --check`；验收：通过
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`；验收：通过
- [x] `cargo test --workspace --all-features`；验收：通过
- [x] `node contracts/tools/validate.mjs`；验收：0 警告

---

## 13. 版本与文档

> 依赖：12 ｜ 覆盖：spec §7 ｜ 状态：待办

- [x] `[workspace.package] version` 升 `0.2.0`（实施启动时）；验收：`cargo metadata` 一致 —— `Cargo.toml`
- [x] `CHANGELOG.md` 新增 0.2.0 分节（新增/更新/破坏性/限制）；验收：条目齐全
- [x] 更新 `README.md`/`README.en.md`：端口、端点、配置、快速开始示例；验收：命令可执行
- [x] 更新 `AGENTS.md`/`llms.txt`：新依赖、新 crate 文件、门禁命令；验收：与实际一致
- [x] 回填 `docs/0.2.0/release-v0.2.0-development-notes.md` 的验证结论与限制；验收：数据真实
- [x] 契约版本与产品版本核对；验收：`CONTRACT_VERSION` 与 `contracts/VERSION` 一致

---

## 附：里程碑与出口

| 里程碑 | 出口 |
| --- | --- |
| M1 | `/health` 与阻塞端点可跑（FakeModel echo） |
| M2 | ReAct + 三工具 + 沙箱负面用例通过 |
| M3 | SSE/取消/并发验收通过 |
| M4 | fixtures 回放 + openapi + 四条门禁全绿 |

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
node contracts/tools/validate.mjs
cargo run -p dsh-runtime
curl -s http://127.0.0.1:8090/health
```