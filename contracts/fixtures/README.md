# fixtures

行为契约。每个场景一个目录，包含 `meta.json`、输入和期望输出。

## 状态

- `handwritten-sample`：手写引导样例，未从基线录制，仅用于跑通工具链。
- `recorded`：由 `record-from-java.sh` 从指定 commit 录制，可回放对比。

当前样例全部为 `handwritten-sample`。首次录制后应替换或新增 `recorded` 场景，手写样例可保留为最小冒烟用例。

## 命名

```text
fixtures/<类别>/<场景>/<序号>-<语义>/
```

- 序号三位递增，不复用。
- 语义用小写短横线，例如 `001-basic`、`002-tool-error`、`003-cancel`。

## meta.json 必填

| 字段 | 说明 |
| --- | --- |
| `scenario` | 相对 fixtures 的路径 |
| `contracts` | 对应的 schema 相对路径 |
| `origin` | `handwritten-sample` 或 `recorded` |
| `recorded` | 是否来自真实录制 |
| `source` | 仓库、commit、端点；手写样例为 null |
| `normalized` | 被替换的字段 |
| `redacted` | 被脱敏的字段 |

## 规则

- 一个场景只验证一件事，失败信息才可定位。
- 不提交真实密钥、用户数据、模型原始输出。
- 录制后必须人工检查 `normalized` 与 `redacted` 是否足够。
- 归一化不得触碰错误码、事件数量、工具参数、审批结果和终态。
