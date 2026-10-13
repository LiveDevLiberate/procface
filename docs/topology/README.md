# ProcFace 子系统结构审核

> **状态：待用户审核。** 先确认 ASCII 图的结构、数据方向和指标归属，再重画 SVG 并接入前端。

审核入口：

1. [六张子系统 ASCII 图与统一结构符号](ascii-review.md)
2. [指标归属、层次与数据流边界](metric-ownership.md)
3. [逐项静态指标分类表](metric-taxonomy.tsv)
4. [存储与网络结构核对说明](architecture-review.md)

六张图分别为 Scheduler / CPU、Memory / VM、VFS / Filesystem、Block I/O、Networking、IRQ / SoftIRQ。`Q` 表示队列、`L` 表示链表、`S` 表示集合或位图、`M` 表示映射、`T` 表示树、`R` 表示环形描述符队列；圆角矩形表示组件或设备。所有符号只表示结构，不暗示现有指标能读取其容量或活动。

子系统图排除单进程和单线程指标。未直接观测的节点和接口不得借用上下游数值。审核结论请按 `ascii-review.md` 顶部的六行回执给出。
