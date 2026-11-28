# dsh-rust v0.2.1 发布说明

> 发布日期：2026-11-28 ｜ 上一版本：v0.2.0 ｜ 契约版本：`1.0.0-draft.1`
>
> 主题：**从能跑到能续**——会话与事件落盘，重启可续跑，过程可回放。

## 一、亮点

1. **持久化**：会话、消息、事件、运行记录写入 SQLite 单文件库，进程重启不丢数据。
2. **重启续跑**：同一 `sessionId` 再次请求会带上历史消息参与推理，事件 `seq` 接续不重号。
3. **事件回放**：新增 `GET /api/sessions/{sessionId}/events`，按 `seq` 分页，`afterSeq`/`limit` 可调。
4. **流式预算兜底**：单次运行输出超过 `DSH_STREAM_BUDGET_BYTES`（默认 8 MiB）时，以唯一终态 `error(stream_budget_exceeded)` 收尾，不再无限增长。
5. **存储可切换**：`DSH_STORE=memory` 一键回到纯内存模式，作为持久化问题的回滚通道。

## 二、新增能力

### 2.1 HTTP 端点

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/health` | 健康检查，返回 `version=0.2.1` 与契约版本 |
| POST | `/api/agent/message` | 阻塞式对话（不落流式预算） |
| POST | `/api/agent/stream` | SSE 流式对话 |
| POST | `/api/agent/cancel` | 按 `runId` 或 `agentId` 取消 |
| GET | `/api/sessions/{sessionId}/events` | **新增**：会话事件回放 |

回放示例：

```bash
# 第一页
curl -s 'http://127.0.0.1:8090/api/sessions/sess-xxx/events?afterSeq=0&limit=200'

# 翻页：用上一页最后一条的 seq 作为 afterSeq
curl -s 'http://127.0.0.1:8090/api/sessions/sess-xxx/events?afterSeq=200&limit=200'
```

响应形如 `{"sessionId":"…","events":[…],"lastSeq":5}`；会话不存在返回 `404 not_found`（不返回空数组），参数非法（负数、超 1000、非数字）返回 `400 invalid_request`。

### 2.2 落库事件

| 事件 | 触发时机 |
| --- | --- |
| `session.created` | 新建会话时（续跑不重复） |
| `message.appended` | 用户消息、助手消息、工具结果消息 |
| `run.started` | 运行登记完成 |
| `tool.requested` / `tool.completed` | 工具调用开始 / 结束（含 `isError`） |
| `run.completed` / `run.failed` | 运行成功结束 / 失败或取消 |

### 2.3 数据表

`schema_migration`（迁移记录）、`session`、`session_message`、`session_event`、`run`。
迁移脚本在 `contracts/v1/persistence/migrations/`，启动时按编号幂等执行。

### 2.4 配置项

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `DSH_STORE` | `sqlite` | `sqlite` / `memory` |
| `DSH_DB_PATH` | `./data/dsh-rust.db` | 目录不存在时自动创建 |
| `DSH_STREAM_BUDGET_BYTES` | `8388608` | 单 run SSE 字节预算 |

0.2.0 的其他环境变量与默认值保持不变。启动日志新增 `store=`、`db=`、`stream_budget_bytes=`，不输出任何密钥。

### 2.5 错误码

新增两个稳定错误码（登记于 `contracts/v1/common/error.schema.json#/$defs/errorCode`）：

| 错误码 | HTTP 状态 | 场景 |
| --- | --- | --- |
| `storage_unavailable` | 503 | 库不可用、读写失败 |
| `stream_budget_exceeded` | 500（仅作兜底） | SSE 输出超预算，实际以流内终态出现 |

## 三、行为变化

1. **默认落盘**：0.2.0 完全内存态，0.2.1 默认在启动目录生成 `./data/dsh-rust.db`。如不希望落盘，设置 `DSH_STORE=memory`。
2. **事件延迟可见**：单 run 的事件分两批提交（开始一次、结束一次），运行中的工具事件要到 run 结束才可回放；流式实时输出仍走 SSE，不受影响。
3. **运行留存**：已结束运行的查询记录由「超过 1024 条整体清空」改为「淘汰最旧、保留最近 1024 条」。
4. **关闭行为**：`Ctrl-C` 后会最多等待 5 秒让在途运行写完数据再退出。

## 四、破坏性变更（仅影响库使用者）

| 项 | 变更 |
| --- | --- |
| `dsh-case` | `AgentService::new` 增加 `events: Arc<dyn EventLog>` 参数 |
| `dsh-http` | `AppState` 新增 `stream_budget_bytes` 字段；用 `AppState::new(service).with_stream_budget(n)` 覆盖，原有 `new` 调用仍可用 |
| `dsh-domain` | 新增 `EventLog` 端口（`append`/`append_many`/`list_after`/`last_seq`，同步签名） |
| 依赖 | 新增 `rusqlite 0.40`（`bundled`），仅 `dsh-infra` 引用；`dsh-domain` 依赖树仍无 axum/tokio/rusqlite |

使用可执行文件的部署方式不受影响，仅需注意第三节的默认落盘行为。

## 五、升级指南

1. **版本**：workspace 全部 crate 为 `0.2.1`，依赖升级后执行 `cargo build`。
2. **首次启动**：无需手工建库，启动时自动建目录、建表并记录迁移版本。只读目录会以非零码退出并给出可读错误。
3. **备份**：数据库为单文件 + WAL，备份时请一并复制 `*.db`、`*.db-wal`、`*.db-shm`，或在停机后复制。
4. **回滚**：
   - 想回到纯内存：`DSH_STORE=memory`；
   - 想回到 0.2.0 代码：直接切回旧版本即可，0.2.0 不读数据库文件，残留 `data/` 目录可安全删除。
5. **容量**：事件只追加、不清理，长期运行请按需归档数据库文件（容量治理在后续版本提供）。

## 六、验证情况

| 门禁 | 结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过 |
| `cargo test --workspace --all-features` | 87 passed |
| `node contracts/tools/validate.mjs` | 31 schema / 68 refs / 10 fixtures / 0 警告 |
| `node contracts/tools/generate-openapi.mjs --check` | 与提交版本一致 |

端到端验证（FakeModel）：新建会话落 5 条事件可回放；重启后同 `sessionId` 能续上前文且 `seq` 接续；`DSH_STORE=memory` 不生成数据库文件；只读目录启动退出码为 1；`Ctrl-C` 后输出 `shutdown drain complete`。

## 七、已知问题

1. DeepSeek 适配器未做真实联调，默认路径仍为 FakeModel。
2. 工具超时后 `spawn_blocking` 任务不可取消，工具自身需设上限。
3. 单 run 事件为两批提交，运行期的中间事件无法即时回放。
4. 无事件容量治理、无鉴权，服务默认仅监听回环地址。
5. 契约仍为 `1.0.0-draft.1`，待 Java 基线与 Rust 双向录制后再评估升 `1.0.0`。

## 八、下一步

| 版本 | 计划 |
| --- | --- |
| 0.2.2 | 插件宿主（stdio JSON-RPC，接 Java 宿主） |
| 0.2.3 | 审批与权限矩阵、鉴权与多租户 |
| 后续 | 事件归档/清理、可插拔存储后端、契约升版 |

---

- 技术细节与选型依据：`docs/0.2.1/release-v0.2.1-development-notes.md`
- 需求与验收：`docs/0.2.1/spec.md`
- 契约增量：`contracts/CHANGELOG.md`
