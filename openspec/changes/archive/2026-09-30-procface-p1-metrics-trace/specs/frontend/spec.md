# Spec Delta

## ADDED Requirements

### Requirement: 前端必须让 P1 指标和 Trace 可分析

前端 MUST 展示 PSI、中断和 softirq 的可用时间序列，并在状态异常时显示 unsupported、permission_denied、stale、discontinuity 或 incomplete。Trace 页面 MUST 将结构化字段按内存、调度、I/O、文件描述符和线程分组，并与总览图表共享 uptime 时间轴。前端 MUST 保留现有历史 IndexedDB、断链 gap、不跨 gap 连线、导出和 Trace 重连停止行为。

#### Scenario: 总览发现压力后进入 Trace
- **WHEN** 用户在 PSI 或中断图表发现异常并从进程列表选择 PID
- **THEN** 前端切换到 Trace，保留总览历史，并显示该 PID 在相同 uptime 区间的 CPU、内存和 I/O 数据

#### Scenario: Trace 字段部分不可读
- **WHEN** 某个结构化字段状态为 permission_denied 或 skipped_expensive
- **THEN** 前端在对应分组显示状态原因，不把字段渲染为零，并继续显示其他分组

#### Scenario: 时间线存在断链
- **WHEN** SSE 序号缺失或 series 超出 daemon 窗口
- **THEN** 系统和 Trace 图表保留 gap 标记，曲线不跨 gap 插值，历史导出包含断链记录
