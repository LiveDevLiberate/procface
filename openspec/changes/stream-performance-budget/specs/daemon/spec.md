# Spec Delta

## ADDED Requirements

### Requirement: daemon 必须提供性能预算可观测性

daemon MUST 在 health 中报告最近采样耗时、超预算轮次、窗口字节数、已发布批次数量和流量统计；字段缺失或统计不可用时 MUST 使用已有状态字段表达，不得阻塞采样。统计默认只保留内存窗口，不得引入设备持久化。

#### Scenario: 采样轮次超时
- **WHEN** 一轮采样耗时超过配置预算
- **THEN** health 增加超预算计数，当前批次按既有 complete/diagnostics 语义发布，后续轮次继续执行

#### Scenario: 流量统计
- **WHEN** SSE 或 chunked export 发布数据
- **THEN** health 能区分批次数、字节数和慢客户端关闭次数，统计不改变 v1 样本 schema

### Requirement: 性能诊断不得改变默认资源策略

daemon MUST 继续使用有界内存窗口、单活动 Trace 和慢客户端隔离；性能统计本身 MUST 有固定上限，并且浏览器断开、导出结束或 daemon 重启后不会无限增长。

#### Scenario: 长时间运行
- **WHEN** daemon 连续运行超过窗口保留时间
- **THEN** 指标历史按既有策略淘汰，性能统计仍保持有界，health 可继续读取
