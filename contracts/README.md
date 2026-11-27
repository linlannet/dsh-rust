# dsh-rust contracts

外部可观测协议的单一真源。只描述跨进程、跨仓库、可回放的边界，不描述 Rust crate 内部类型。

## 状态说明

| 状态 | 含义 |
| --- | --- |
| draft | 结构已设计，字段仍可能随基线录制调整 |
| frozen | 已随 tag 发布，只能做兼容新增 |
| deprecated | 仍可用，计划在下一个大版本移除 |

当前所有 schema 均为 `draft`，`fixtures` 中的样例标记为 `handwritten-sample`，尚未从 Java 基线录制。

## 目录

```text
contracts/
├── VERSION              契约版本（与产品、插件协议版本独立）
├── CHANGELOG.md         所有可见变更
├── policy/              兼容、版本、废弃规则
├── v1/                  大版本目录，发布后不原地修改
├── fixtures/            录制与回放样例
└── tools/               校验、录制、回放、差分
```

## 快速开始

```bash
node contracts/tools/validate.mjs
bash contracts/tools/record-from-java.sh --scenario contracts/fixtures/http/agent-message/001-basic --endpoint message --request ./request.json
bash contracts/tools/replay-rust.sh --scenario contracts/fixtures/http/agent-message/001-basic --out /tmp/rust-out
bash contracts/tools/diff-contract.sh expected.json actual.json
```

## 契约清单

| 契约 | 生产者 | 消费者 | 状态 |
| --- | --- | --- | --- |
| `v1/http/*` | Java / Rust Runtime | desktop、mobile、第三方 | draft |
| `v1/sse/*` | Java / Rust Runtime | desktop、mobile | draft |
| `v1/plugin-host/*` | Rust Runtime | JVM 插件宿主 | draft |
| `v1/plugin-manifest/*` | 插件作者 | JVM 插件宿主 | draft |
| `v1/persistence/*` | Runtime | 审计、回放、迁移工具 | draft |
| `v1/config/*` | 部署者 | Runtime | draft |

## 录制流程

```text
Java 基线 + 假模型/假工具
   │ record-from-java.sh
   ▼
fixtures（脱敏、归一化、带 commit）
   ├── Rust Runtime 回放 → diff-contract.sh
   ├── Java Runtime 回放 → 防基线漂移
   └── desktop 契约测试复用同一批样例
```

归一化只允许时间戳、随机 ID、耗时、绝对路径；错误类型、事件数量、工具参数、审批结果、终态不得归一化。

## 不放入本目录

- crate 内部类型与模块 API；
- Java 源码或整份 DTO 复制；
- MCP、A2A 完整第三方 schema（`v1/external` 只记版本与引用）；
- 真实密钥、用户数据、模型原始输出；
- 构建产物与二进制。
