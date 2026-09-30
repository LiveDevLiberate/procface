# Spec Delta

## ADDED Requirements

### Requirement: 前端必须显示性能和数据完整性状态

前端 MUST 在连接状态区显示采样延迟、窗口覆盖、断链数量和最后一次恢复结果；性能状态异常时 MUST 明确显示 degraded 或 incomplete，并保留现有图表和 Trace 数据。

#### Scenario: 采样延迟升高
- **WHEN** daemon health 报告采样耗时超过间隔
- **THEN** 前端显示 degraded 状态和最近耗时，不隐藏已有样本

#### Scenario: 导出性能诊断
- **WHEN** 用户导出 JSONL 或 TSV 历史
- **THEN** 文件继续使用既有可读格式，并可在前端看到导出批次数和字节数
