# 废弃流程

## 标记

1. 在 `CHANGELOG.md` 记录废弃项、替代方案和最早移除版本。
2. 在 schema 描述或 `v1/.../deprecated.md` 中标注。
3. HTTP 响应按需增加 `Deprecation` 与 `Sunset` 头；SSE 在 `catalog.md` 标注。
4. 记录保留期从进入 `deprecated` 的那个 MINOR 版本开始计算，至少保留两个 MINOR 版本。

## 行为

- 废弃项在 `v1` 生命周期内保持行为不变。
- 不得在废弃期内改变废弃字段的类型、默认值或错误语义。
- 移除只发生在新 MAJOR 目录，`v1` 保留只读快照。

## 插件协议

- 废弃方法需同时提供替代方法，并在 `initialize` 的 `capabilities` 中声明。
- 宿主不得在未协商的情况下停止发送仍被对方声明支持的事件。
