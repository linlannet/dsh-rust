# DeepSeek Harness Rust v0.2.1 编码任务清单

> 版本：`v0.2.1` | 基线：v0.2.0 | 架构：既有六层 + Axum 接入层 | 契约版本：`1.0.0-draft.1`
>
> 本清单把 v0.2.1「会话事件持久化与回放」转化为可执行条目，同时收口 v0.2.0 遗留的技术债。
>
> **完成状态图例**：`[ ]` 待办 ｜ `[x]` 已完成
>
> **范围边界（强制）**：不新增 workspace 成员；新增依赖仅限本文件 §1 清单（须先写入 `docs/0.2.1/design.md` §4.1）；**插件宿主与审批矩阵不在本版**（顺延 0.2.2 / 0.2.3）；`dsh-domain` 不依赖 axum/tokio/rusqlite。

## 版本主题

从「能跑」到「能续」：进程重启后会话可续跑，运行过程可回放，流式输出内存可控。

## 与 spec §8 的关系

`docs/0.2.0/spec.md` §8 原把「插件宿主 / 持久化 / 审批矩阵」同列 0.2.1，本版拆分为：

| 版本 | 范围 |
| --- | --- |
| 0.2.1（本版） | 持久化与回放 + SSE 背压 + 技术债 |
| 0.2.2 | 插件宿主（stdio JSON-RPC，接 Java 宿主） |
| 0.2.3 | 审批与权限矩阵 |

---

## 1. 依赖与 workspace

> 依赖：无 ｜ 覆盖：design §4.1 ｜ 状态：已完成

- [x] 在 `docs/0.2.1/design.md` §4.1 登记新依赖并说明理由；验收：文档先于代码合并
- [x] workspace 增加 `rusqlite`（`0.40`，`bundled`）；验收：`cargo check --workspace` 通过
- [x] `dsh-infra` 引入 `rusqlite`；验收：`cargo check -p dsh-infra` 通过，`dsh-domain`/`dsh-types` 依赖树无 rusqlite
- [x] `dsh-http` 引入 `tokio`（已具备）与 `serde`（已具备），无新增；验收：`cargo tree -p dsh-http` 无反向依赖
- [x] CI 覆盖持久化测试；验收：既有 `cargo test --workspace --all-features` 已包含，无需新增步骤

---

## 2. 契约先行（contracts）

> 依赖：无 ｜ 覆盖：`contracts/v1/persistence`、`contracts/v1/http` ｜ 状态：已完成

- [x] 核对 `contracts/v1/persistence/session-event.schema.json` 与 `event-catalog.md` 的 12 类事件；验收：缺字段先补 schema，不先写代码
- [x] 新增 `contracts/v1/persistence/migrations/0001-baseline.sql`，对齐 `0001-baseline.md`：`session`、`session_message`、`session_event`、`run` 四表；验收：`sqlite3 :memory: < 0001-baseline.sql` 无错
- [x] `contracts/v1/http` 增加 `GET /api/sessions/{sessionId}/events` 定义（含分页参数 `afterSeq`、`limit`）；验收：schema 与响应示例齐备
- [x] 重跑 `node contracts/tools/generate-openapi.mjs --check`；验收：openapi 与路由表一致
- [x] fixtures 增加「重启续跑」与「事件回放」两组；验收：`node contracts/tools/validate.mjs` 通过
- [x] `contracts/CHANGELOG.md` 记录本次契约增量；验收：条目含变更类型与影响面

---

## 3. dsh-types 扩展

> 依赖：2 ｜ 覆盖：session-event.schema.json ｜ 状态：已完成

- [x] 新增 `SessionEvent` 值对象（`eventId`/`sessionId`/`seq`/`type`/`timestamp`/`payload`）；验收：序列化字段与 schema 逐键一致
- [x] `EventType` 常量集合覆盖 12 类事件；验收：单测断言字符串与 `event-catalog.md` 一致
- [x] 新增 `PersistenceError` 或复用 `ErrorCode` 补充 `STORAGE_UNAVAILABLE`；验收：`error-codes` 与 `common/error.schema.json` 同步

---

## 4. dsh-domain 扩展

> 依赖：3 ｜ 覆盖：ports ｜ 状态：已完成

- [x] 新增 `EventLog` 端口（`append`/`list_after`/`last_seq`），签名保持同步；验收：`cargo tree -p dsh-domain` 无 rusqlite / tokio
- [x] `SessionRepository` 保持同步签名不变，补充「不存在返回 `Ok(None)`」语义注释；验收：既有测试不改动即通过
- [x] 单测：`EventLog` 行为用内存实现验证 seq 单调；验收：新增 ≥ 2 个测试

---

## 5. dsh-infra 持久化（rusqlite）

> 依赖：1、2、3、4 ｜ 覆盖：M1 ｜ 状态：已完成

- [x] 新增 `SqliteStore`：连接管理（WAL、`busy_timeout`）、启动时执行 `0001-baseline.sql`（幂等）；验收：重复启动不报错 —— `crates/dsh-infra/src/store/`
- [x] 实现 `SqliteSessionRepository`（`load`/`save`，会话与消息同事务）；验收：写入后进程重启可读回
- [x] 实现 `SqliteEventLog`（`append` 分配 seq、`list_after` 按 seq 升序、`last_seq`）；验收：并发写入 seq 不重号
- [x] `append` 支持批量事务，减少 ReAct 循环写放大；验收：单 run 事件写入 ≤ 2 次事务 —— `EventLog::append_many` 单事务连续分配 seq
- [x] 单测：迁移幂等、seq 单调、事务回滚、回放重建；验收：新增 9 个测试，使用临时文件库
- [x] 保留 `InMemorySessionRepository` 作为测试替身；验收：`dsh-case` 测试仍用内存实现

---

## 6. dsh-case 事件写入与阻塞桥接

> 依赖：5 ｜ 覆盖：M1 ｜ 状态：已完成

- [x] `AgentService` 注入 `EventLog`，在 `session.created`、`message.appended`、`run.started`、`run.completed`/`run.failed`、`tool.requested`/`tool.completed` 处落事件；验收：一次工具调用产生完整事件序列
- [x] 仓储与事件日志调用统一经 `spawn_blocking` 包裹（复用 `execute_tool` 模式）；验收：`cargo test -p dsh-case` 通过，无 `block_in_place` —— 见 `crates/dsh-case/src/blocking.rs`
- [x] 事件写入失败不改变运行结论，转为 `Error` 事件并记日志；验收：注入失败实现后 run 仍以原 `finishReason` 结束
- [x] 会话续跑：`load` 命中时复用历史 `messages`；验收：新增测试「同 sessionId 二次调用可看到首轮内容」
- [x] 清理 `RunRegistry.finished` 策略：改为有序淘汰保留最近 1024 条；验收：新增测试断言插入 1100 条后仍保留最新 1024 条

---

## 7. dsh-http 回放端点

> 依赖：2、5 ｜ 覆盖：M1 ｜ 状态：已完成

- [x] `routes` 增加 `SESSIONS_EVENTS`；验收：`routes_are_absolute` 测试覆盖新常量 —— `crates/dsh-http/src/lib.rs`
- [x] `GET /api/sessions/{sessionId}/events`：支持 `afterSeq`、`limit`（默认 200、上限 1000）；验收：非法参数返回 `invalid_request`
- [x] 会话不存在返回 `not_found`，不返回空数组；验收：契约测试覆盖
- [x] 新增 DTO 并复用既有错误映射；验收：`cargo test -p dsh-http` 通过

---

## 8. SSE 输出预算（技术债）

> 依赖：无 ｜ 覆盖：M2 ｜ 状态：已完成

- [x] `RunEventStream` 统计已发字节，超过 `DSH_STREAM_BUDGET_BYTES`（默认 8 MiB）时停止并补发终态 `error`（新错误码 `STREAM_BUDGET_EXCEEDED`）；验收：超限响应仍以唯一终态事件收尾
- [x] 事件码写入 `contracts/v1/common/error.schema.json` 与 `error-codes`；验收：`generate-openapi.mjs --check` 通过 —— 落地为 `$defs/errorCode`，`v1/sse/catalog.md` 同步预算终态规则
- [x] 单测：构造超限事件流，断言终态与 seq 连续性；验收：新增测试覆盖
- [x] 在 `docs/0.2.1/design.md` 记录「工具超时后 `spawn_blocking` 任务不可取消」的已知边界；验收：文档条目存在

---

## 9. runtime 配置

> 依赖：5 ｜ 覆盖：M1 ｜ 状态：已完成

- [x] `Config` 增加 `db_path`（`DSH_DB_PATH`，默认 `./data/dsh-rust.db`）、`store_kind`（`DSH_STORE`，`sqlite` 默认 / `memory`）与 `stream_budget_bytes`；验收：非法值返回明确错误
- [x] `DSH_STORE=memory` 时装配内存实现，作为持久化回滚通道；验收：该模式下服务可用且不生成数据库文件
- [x] 启动时创建父目录并执行迁移，失败则以非零码退出；验收：只读目录启动报错可读
- [x] 启动日志打印 `db=`、`contract=`、`version=`，不打印密钥；验收：日志无 `DEEPSEEK_API_KEY`
- [x] 优雅关闭前 flush 待写事件；验收：Ctrl-C 后事件不丢 —— 事件按 run 提交，关闭时有限等待在途 run

---

## 10. 测试与门禁

> 依赖：全部 ｜ 覆盖：M3 ｜ 状态：已完成

- [x] `cargo fmt --all --check` 通过
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过
- [x] `cargo test --workspace --all-features` 通过，测试数由 51 增至 ≥ 65 —— 实际 87
- [x] 回放差分测试：内存实现与 SQLite 实现产生的事件序列完全一致；验收：逐事件比对通过 —— `crates/dsh-case/tests/event_log_parity.rs`
- [x] 契约回放测试纳入 CI；验收：`cargo test -p dsh-http --test contract_replay` 通过 —— CI 增加显式步骤
- [x] 按 spec §5 逐条补齐验收测试：消息 `ordinal` 冲突回滚、并发 `seq` 不重号、回放连续翻页、读失败 `storage_unavailable`；验收：见 §5.1/§5.2/§5.3/§5.6

---

## 11. 版本与文档

> 依赖：10 ｜ 状态：已完成

- [x] workspace 版本 `0.2.0` → `0.2.1`；验收：`cargo metadata` 一致
- [x] 新增 `docs/0.2.1/{spec,design,tasks}.md` 与 `release-v0.2.1-development-notes.md`；验收：tasks 与本清单同步
- [x] `CHANGELOG.md` 按 crate 记录；`README.md`/`README.en.md` 路线图与能力边界同步；验收：两版 README 一致
- [x] `AGENTS.md`/`llms.txt` 补充持久化相关约定（DB 路径、迁移、禁止绕过 `EventLog`）；验收：AI 入口可发现

---

## 附：里程碑与出口

| 里程碑 | 内容 | 出口条件 |
| --- | --- | --- |
| M1 持久化 | §2–§7、§9 | 重启续跑 + 事件回放差分测试通过 |
| M2 技术债 | §8 | 超限终态测试通过；已知边界入档 |
| M3 收口 | §10、§11 | 全部门禁绿，版本与文档一致 |

> 不变更项：SSE 事件名与终态语义、`FakeModel` 默认路径、`read_file` 沙箱规则。
