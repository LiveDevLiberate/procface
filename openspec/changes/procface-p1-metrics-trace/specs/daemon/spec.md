# Spec Delta

## ADDED Requirements

### Requirement: daemon 必须提供可控的 P1 采样和 Trace 状态

daemon MUST 将 P1 系统采样纳入现有有界调度和 immutable snapshot 流程；P1 采样失败、部分缺失或预算不足 MUST 只降低本轮 complete 状态，不得停止主循环。Trace MUST 继续使用独立 worker、单活动生命周期、PID + starttime 身份和既有权限开关。

#### Scenario: P1 部分失败
- **WHEN** softirq 文件解析失败但 CPU、内存和 PSI 可读
- **THEN** daemon 发布错误状态的 softirq 样本，同时继续发布其他组并增加 diagnostics

#### Scenario: Trace 与系统采样并行
- **WHEN** Trace 正在读取 smaps_rollup 或线程数据
- **THEN** 系统采样、SSE heartbeat 和 health sequence 继续增长，Trace 超时只影响 Trace 批次

#### Scenario: 进程身份变化
- **WHEN** Trace 期间 PID 退出或被复用
- **THEN** daemon 丢弃受影响本轮、结束当前 Trace，并保留普通系统历史和断链语义
