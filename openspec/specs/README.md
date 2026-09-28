# ProcFace 当前规格索引

ProcFace 的当前行为按四个能力域拆分：

- `daemon/`：可选的鉴权 HTTP 服务、内存边界和持久化策略。
- `cli/`：本地采集、输出、进程范围和兼容语义。
- `metrics/`：procfs 指标目录和计算规则。
- `frontend/`：静态网页、本地历史、断链和导出。

OpenSpec 要求的 `Requirement`、`Scenario` 和规范性关键字保留英文；解释、约束和产品内容使用中文。

提议中的变更放在 `openspec/changes/`，只有实现和验收后才合并到上述当前规格。
