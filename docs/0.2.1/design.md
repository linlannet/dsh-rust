# DeepSeek Harness Rust v0.2.1 实现方案文档

> 版本：`v0.2.1` ｜ 基线：`v0.2.0` ｜ 契约版本：`1.0.0-draft.1`
>
> 目标：在不破坏 0.2.0 契约与分层的前提下，引入 SQLite 持久化、事件回放、重启续跑与 SSE 输出预算。
>
> 对应需求：`docs/0.2.1/spec.md`；任务拆解：`docs/0.2.1/tasks.md`。

---

# 一、需求与存量功能关系分析

## 1.1 0.2.0 存量与 0.2.1 增量

| 能力 | 0.2.0 | 0.2.1 增量 |
| --- | --- | --- |
| 会话存储 | `InMemorySessionRepository` | 新增 `SqliteSessionRepository`，端口签名不变 |
| 事件日志 | 仅运行期内 `RunEvent` 推送 | 新增 `EventLog` 端口 + SQLite 实现 |
| 回放 | 无 | 新增 `GET /api/sessions/{sessionId}/events` |
| 续跑 | 仅进程内有效 | 重启后依 `DSH_DB_PATH` 续跑 |
| 流式输出 | 无界通道，无上限 | 单 run 字节预算 + 终态降级 |
| 运行留存 | `finished` 超 1024 条整体清空 | 改为有序淘汰保留最近 1024 条 |

## 1.2 存量约束

1. `dsh-domain` 不得依赖 axum/tokio；本版追加：**不得依赖 rusqlite**。
2. `dsh-types` 不依赖任何 workspace crate。
3. SSE 事件名、顺序、唯一终态语义不得改变（新增错误码属增量）。
4. 新依赖必须先登记于本文 §4.1。

---

# 二、总体设计

## 2.1 分层落位（不新增 workspace 成员）

| 层 | 本版职责 |
| --- | --- |
| `dsh-types` | `SessionEvent`、`EventType` 常量、`STORAGE_UNAVAILABLE` 错误码 |
| `dsh-domain` | `EventLog` 端口（同步签名）；`SessionRepository` 保持不变 |
| `dsh-case` | 在运行阶段写事件；对仓储/日志调用做 `spawn_blocking` 桥接；`finished` 淘汰策略 |
| `dsh-http` | 回放路由与 DTO；SSE 输出预算 |
| `dsh-infra` | `SqliteStore`、`SqliteSessionRepository`、`SqliteEventLog`；保留内存实现 |
| `apps/dsh-runtime` | 配置项、启动迁移、装配与关闭前 flush |

依赖方向不变：`http → case → domain → types`，`infra → domain`。

## 2.2 关键接口

### 2.2.1 事件日志端口

```rust
pub trait EventLog: Send + Sync {
    /// 追加事件并分配 seq；返回分配后的完整事件。
    fn append(&self, event: SessionEvent) -> Result<SessionEvent, Error>;
    /// 按 seq 升序读取 afterSeq 之后至多 limit 条。
    fn list_after(&self, session_id: &SessionId, after_seq: u64, limit: usize)
        -> Result<Vec<SessionEvent>, Error>;
    /// 读取指定会话当前最大 seq；无事件返回 0。
    fn last_seq(&self, session_id: &SessionId) -> Result<u64, Error>;
}
```

决策：保持**同步签名**。`dsh-domain` 不引 tokio，异步化会在领域层强行塞入运行时耦合；SQLite 本身是同步驱动，桥接放在 `dsh-case` 边界（与既有 `execute_tool` 一致）。

### 2.2.2 会话仓储边界

`SessionRepository` 签名不变（`load`/`save` + `Option` 语义），仅新增 SQLite 实现。选择不变更签名，避免连带修改 0.2.0 的 10 个 case 层测试与全部依赖注入点。

### 2.2.3 存储与阻塞桥接

```text
AgentService::run  ──spawn_blocking──▶  SqliteSessionRepository / SqliteEventLog
                    ◀── JoinHandle ────  (rusqlite 同步调用)
```

1. 每个阻塞调用用 `tokio::task::spawn_blocking` 包裹，避免占用运行时工作线程。
2. 仓储与事件日志共用 `SqliteStore`（`Arc<Mutex<Connection>>`），保证同一连接串行化，规避 SQLite 单写者限制。
3. 不使用 `block_in_place`（会在单线程运行时下 panic，且阻塞调度）。

### 2.2.4 续跑与写事件时序

```text
请求进入
  ├─ runs.register（会话占用，冲突则 conflict）
  ├─ sessions.load(sessionId)      ← 命中则复用历史 messages
  ├─ 追加 message.appended(user)
  ├─ 循环 step：
  │    ├─ run.started（仅首步）
  │    ├─ model.step
  │    ├─ tool.requested → 执行 → tool.completed
  │    └─ 终态：run.completed | run.failed
  ├─ sessions.save（会话与消息同事务）
  └─ 返回 / 推流
```

事件写入失败不改变运行结论：捕获错误后仅发 `Error` 事件并记日志（spec §5.6）。

## 2.3 数据模型与表结构

以 `contracts/v1/persistence/migrations/0001-baseline.sql` 为准：

| 表 | 关键约束 |
| --- | --- |
| `session` | 主键 `id`；`agent_id` 可空（领域 `Session` 当前无该字段）；`status` 默认 `'active'`；`updated_at` 由写入方维护 |
| `session_message` | `UNIQUE(session_id, ordinal)`；`ordinal` 由 `MAX(ordinal)+1` 在事务内计算；外键指向 `session` |
| `session_event` | `UNIQUE(session_id, seq)`；`payload_json` 存 JSON 文本；**不设外键** |
| `run` | 主键 `run_id`；`finish_reason`/`error_json` 可空；**不设外键** |

> 外键取舍：事件与运行是只追加的溯源事实，允许先于会话投影写入（回放不依赖会话行存在），
> 因此不设外键；`session_message` 属于会话投影，保留外键以保证一致性。

索引：

1. `idx_run_session_started (session_id, started_at)` —— 运行历史查询，唯一显式创建的索引。
2. `UNIQUE(session_id, seq)` 已隐含回放主路径索引，不再重复创建 `idx_session_event_session_seq`。
3. `UNIQUE(session_id, ordinal)` 已隐含续跑主路径索引，不再重复创建 `idx_session_message_session_ordinal`。

> 实现记录：`0001-baseline.sql` 已按此落地；重复执行幂等，`sqlite3` 实测二次执行无错，重复 `seq` 被唯一约束拒绝。

**seq 分配**：进程内按会话加锁 + 库内 `UNIQUE(session_id, seq)` 兜底；冲突时重试一次后失败，避免静默产生重号。

**留存**：本版不删除事件；容量治理列入后续版本。

## 2.4 HTTP 回放端点设计

### 2.4.1 路由

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/api/sessions/{sessionId}/events` | 按 `seq` 升序分页读取事件 |

### 2.4.2 参数与错误

| 参数 | 默认 | 约束 |
| --- | --- | --- |
| `afterSeq` | 0 | 非负整数 |
| `limit` | 200 | 1–1000 |

| 情况 | 返回 |
| --- | --- |
| 会话不存在 | `not_found` |
| 参数非法 | `invalid_request` |
| 存储读取失败 | `storage_unavailable` |

### 2.4.3 响应外形

```json
{
  "sessionId": "sess-1",
  "events": [{ "eventId": "evt-1", "sessionId": "sess-1", "seq": 1,
               "type": "session.created", "timestamp": "2026-11-28T00:00:00Z", "payload": {} }],
  "lastSeq": 1
}
```

`lastSeq` 为本次返回的最后一条事件的 `seq`；无事件时为 0，便于客户端递增游标。

## 2.5 SSE 输出预算设计

1. `RunEventStream` 维护已发送字节计数；`encode` 后累加 `data` 与 `event` 字段长度。
2. 超过 `budget` 时：停止转发源事件 → 发送唯一终态 `error(STREAM_BUDGET_EXCEEDED)` → 结束流。
3. `seq` 在终止前持续递增，不跳号；心跳（`keepalive`）不计入预算与 `seq`。
4. 未选择「有界 channel + await」的原因：`EventSink::emit` 为同步签名，改为异步会波及 case 层与全部测试；预算方案以更小改动达成内存上限目标。

## 2.6 配置

| 变量 | 默认 | 说明 |
| --- | --- | --- |
| `DSH_DB_PATH` | `./data/dsh-rust.db` | 数据库文件路径，目录不存在则创建 |
| `DSH_STORE` | `sqlite` | `sqlite` / `memory`；`memory` 为回滚通道 |
| `DSH_STREAM_BUDGET_BYTES` | `8388608` | 单 run 事件字节预算 |

## 2.7 契约映射

| 契约文件 | 关联实现 |
| --- | --- |
| `contracts/v1/persistence/session-event.schema.json` | `dsh-types::SessionEvent` |
| `contracts/v1/persistence/event-catalog.md` | `dsh-case` 事件落点 |
| `contracts/v1/persistence/migrations/0001-baseline.sql` | `SqliteStore` 启动迁移 |
| `contracts/v1/http/*` | 回放端点 DTO 与错误映射 |
| `contracts/v1/openapi.yaml` | 路由表生成并 CI 校验 |

---

# 三、模块级详细设计

## 3.1 dsh-types

- 新增 `session_event.rs`：`SessionEvent`（serde，camelCase）、`EventType` 常量集合。
- `ErrorCode` 增加 `StorageUnavailable`，字符串 `storage_unavailable`；`StreamBudgetExceeded` → `stream_budget_exceeded`。
- 约束：不引入除 `serde`/`serde_json` 外的依赖。

## 3.2 dsh-domain

- 新增 `ports::EventLog`（见 §2.2.1）。
- `SessionRepository` 不变。
- 约束：`cargo tree -p dsh-domain` 不得出现 `rusqlite`、`tokio`、`axum`。

## 3.3 dsh-case

- `AgentService` 注入 `EventLog`，在 §2.2.4 列出的节点写事件。
- 新增 `blocking.rs`（内部）：`fn block_on_store<T>(f) -> Result<T, Error>` 统一封装 `spawn_blocking` + JoinError 映射为 `internal`。
- `EventBuffer` 按 run 聚合事件，分两次 `append_many` 落库（开始：会话创建/用户消息/运行开始；结束：工具调用、助手消息与终态），单 run 至多 2 次事务。
- `RunRegistry.finished`：`HashMap` 改 `VecDeque<RunId>` + `HashMap` 索引，超限淘汰最旧（保留最近 1024 条）。
- 事件写入失败：`warn!` + 追加 `Error` 事件，不改变 `finishReason`。
- 时间戳统一使用 `dsh_types::now_rfc3339`（纯标准库实现），`dsh-infra` 不再保留私有副本。

## 3.4 dsh-http

- `routes::SESSIONS_EVENTS` 新常量；`router` 增加 handler，参数经 `Path`/`Query` 提取并统一映射为 `invalid_request`。
- `dto` 增加 `SessionEventsQuery` / `SessionEventsResponse`；响应事件直接复用 `dsh_types::SessionEvent`（序列化字段已对齐契约），不再单设 DTO。
- 会话不存在由 `AgentService::session_events` 返回 `not_found`，端点不回空数组。
- `sse::RunEventStream` 增加 `budget` 与 `emitted_bytes` 字段；`sse_response` 从 `DSH_STREAM_BUDGET_BYTES` 取值，`sse_response_with_budget` 供测试与后续配置直传。

## 3.5 dsh-infra

```text
src/store/
  mod.rs        SqliteStore（连接、PRAGMA、迁移、事务辅助）
  session.rs    SqliteSessionRepository
  event_log.rs  SqliteEventLog
  migration.rs  读取并执行 migrations 目录脚本
```

- `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;`
- 迁移记录表 `schema_migration(version TEXT PRIMARY KEY, applied_at TEXT)`。
- 内存实现保留：`InMemorySessionRepository`、新增 `InMemorySessionEventLog`（测试与 `DSH_STORE=memory`）。
  命名上刻意区别于既有的 `InMemoryRunEventLog`——后者是单次运行的 SSE 广播，前者对应领域 `EventLog` 端口。

## 3.6 apps/dsh-runtime

- `Config` 增加 `db_path`、`store_kind`、`stream_budget_bytes`。
- 启动顺序：解析配置 → 建目录 → 打开库 → 迁移 → 装配 → 监听。
- 关闭：`ctrl_c` 后等待在途 run 结束（有限等待）再退出。

实现记录：

1. `SqliteStore::open` 自带建目录与迁移，`runtime` 只负责把失败转成可读错误并以非零码退出。
2. `stream_budget_bytes` 经 `AppState::with_stream_budget` 注入 SSE，运行时不读环境变量两次。
3. 每个 run 结束即提交会话投影与事件，因此「关闭前 flush」等价于「等待在途 run」：`ctrl_c` 后每 50 ms 检查一次 `RunRegistry`，最多等 5 秒，超时只告警不阻塞退出。
4. 启动日志字段：`bind`、`model`、`store`、`db`、`tools`、`stream_budget_bytes`、`contract`、`version`；内存模式 `db=<memory>`，不输出任何密钥。

---

# 四、技术选型与约束

## 4.1 新增依赖清单

| 依赖 | 版本 | 作用域 | 理由 | 备选 |
| --- | --- | --- | --- | --- |
| `rusqlite` | `0.40`（`bundled`） | `dsh-infra` | 同步 SQLite 驱动，内嵌 SQLite 无需系统库；与 `spawn_blocking` 桥接契合 | `sqlx`（异步但依赖链更长、需绑 tokio）、`sled`（非 SQL、迁移不可见） |

> 版本以实现时 `Cargo.lock` 解析结果为准；若与 MSRV 1.85 冲突则回退至兼容版本，并在本表更新。
>
> 实现记录：`rusqlite 0.43` 要求 rustc 1.89，超出本仓 MSRV 1.85，故锁定 `0.40.2`。

**约束**：

1. `rusqlite` 仅允许出现在 `dsh-infra`；`dsh-domain`/`dsh-types` 依赖树不得出现。
2. 不引入 `async-trait`：端口的异步化通过 `spawn_blocking` 解决。
3. 不引入 ORM 与迁移框架；迁移为纯 SQL 脚本，由 `SqliteStore` 顺序执行。

## 4.2 版本与 feature

- workspace 版本 `0.2.0 → 0.2.1`。
- `deepseek` feature 语义不变（默认关闭）。
- 不新增 feature 开关；存储切换用运行时配置 `DSH_STORE`，避免 2^n 编译组合。

## 4.3 与既有约束的冲突与处理

| 冲突 | 处理 |
| --- | --- |
| 0.2.0 声明「不新增数据库表」 | 本版显式变更：引入 SQLite 四表，已在 spec §6 与本表记录 |
| `dsh-infra` 原仅依赖 serde_json | 新增 rusqlite（bundled），编译时间上升，已在 §六 风险登记 |
| `EventSink::emit` 同步签名限制背压 | 改用字节预算方案，不改动端口签名（§2.5） |

---

# 五、测试策略

| 类型 | 范围 | 关键用例 |
| --- | --- | --- |
| 单测（infra） | 迁移幂等、seq 单调、事务回滚、分页读取 | 临时文件库，测试后删除 |
| 单测（case） | 续跑复用历史、事件写入失败降级、finished 淘汰 | 内存实现 |
| 单测（http） | 回放参数校验、`not_found`、预算超限终态 | 契约驱动 |
| 集成 | `/health`、阻塞、流式、取消 | 沿用 0.2.0 |
| 差分 | 内存 vs SQLite 事件序列逐条一致 | 同一输入双实现比对 |
| 契约 | fixtures 回放 + openapi 再生成 | `node contracts/tools/*.mjs` |

门禁：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets --all-features -- -D warnings`、`cargo test --workspace --all-features`、契约校验。

---

# 六、风险与回滚

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| `rusqlite` bundled 拉长首次编译 | CI 时间上升 | 缓存 `~/.cargo`；必要时改系统 SQLite |
| SQLite 单写者导致写等待 | 高并发下延迟 | WAL + `busy_timeout` + 单连接串行化；后续评估连接池 |
| 事件写放大拖慢 ReAct | 每步多一次 IO | 批量事务（单 run ≤ 2 次）；必要时改异步队列 |
| 迁移与代码漂移 | 启动失败 | 迁移仅来自 `contracts/`，CI 校验建表可用性 |
| 预算误杀长回答 | 用户感知截断 | 默认 8 MiB 足够；错误码可诊断、可配置 |
| 工具超时后 `spawn_blocking` 任务不可取消 | 超时后工具线程继续跑到自然结束，占用阻塞线程池 | 已知边界：`tokio::time::timeout` 只放弃等待，不中断阻塞任务；工具内部须自设上限。事件日志与仓储调用同理，均为短事务，影响可控 |

**回滚**：

1. 运行时 `DSH_STORE=memory` 一键退回无持久化路径（内存实现保留）。
2. 代码回滚仅需回退 `dsh-infra` 的 SQLite 实现与 runtime 装配，端口与 `dsh-domain` 不受影响。

---

# 七、实施计划

| 阶段 | 内容 | 出口 |
| --- | --- | --- |
| P1 | 契约（migration、端点定义、fixtures） | `validate.mjs` 通过 |
| P2 | `dsh-types` + `dsh-domain` 端口 | `cargo test -p dsh-types -p dsh-domain` 通过 |
| P3 | `dsh-infra` SQLite 实现 | 6 个 infra 测试通过 |
| P4 | `dsh-case` 事件落点与续跑 | 续跑/降级/淘汰测试通过 |
| P5 | `dsh-http` 回放与预算 | 契约测试通过 |
| P6 | runtime 装配与关闭 flush | 重启续跑集成测试通过 |
| P7 | 门禁、版本、文档 | 全绿 |

> 与 `tasks.md` 的对应：P1→§2，P2→§3/§4，P3→§5，P4→§6，P5→§7/§8，P6→§9，P7→§10/§11。
