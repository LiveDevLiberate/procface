# Spec Delta

## ADDED Requirements

### Requirement: Procfs 指标必须按能力和状态发布

P1 采集 MUST 支持 `/proc/pressure/cpu`、`/proc/pressure/memory`、`/proc/pressure/io`、`/proc/interrupts` 和 `/proc/softirqs`（文件存在时）。PSI 的 avg10、avg60、avg300 MUST 为 gauge，total MUST 为 counter；中断和 softirq 的累计字段 MUST 可计算速率。缺失文件、字段缺失、权限错误、解析错误和计数器回退 MUST 使用既有 status 枚举表达，不得转换为零。

#### Scenario: PSI 文件存在
- **WHEN** `/proc/pressure/cpu` 包含 some/full 的 avg10、avg60、avg300 和 total
- **THEN** daemon 发布对应 CPU PSI gauge、total counter 及可用速率，并以 `uptime_s` 计算间隔

#### Scenario: PSI 文件不支持
- **WHEN** 设备没有 `/proc/pressure/io`
- **THEN** capabilities 和样本报告 `unsupported`，CPU、内存及其他系统指标继续发布

#### Scenario: 动态中断实体
- **WHEN** `/proc/interrupts` 或 `/proc/softirqs` 出现新的 IRQ 或类型
- **THEN** daemon 先发布实体字典增量，再发布该实体的累计值和速率

### Requirement: Trace 必须提供结构化进程诊断

Trace MUST 在显式选择 PID 后，尽可能结构化发布 `/proc/PID/status`、`/proc/PID/smaps_rollup`、`/proc/PID/sched`、`/proc/PID/io`、FD 数量和线程 stat 的稳定字段。无法读取或超过预算的字段 MUST 单独报告状态；不得因单个字段失败丢弃完整批次。原始文本可作为诊断字段保留，但不得替代可查询的关键数值字段。

#### Scenario: 结构化内存字段
- **WHEN** `smaps_rollup` 可读并包含 RSS、PSS、匿名、文件和共享内存字段
- **THEN** Trace 发布 bytes 单位的结构化字段，并保留对应采样 uptime

#### Scenario: Trace 权限不足
- **WHEN** 目标 PID 的 `status`、`io` 或 `smaps_rollup` 返回 permission denied
- **THEN** 仅对应字段标记 `permission_denied`，其他可读字段仍发布

#### Scenario: Trace 超时
- **WHEN** 线程、FD 或 smaps 读取超过本轮预算
- **THEN** 批次保留 `complete=false` 和 diagnostics，未完成字段标记 `stale` 或 `skipped_expensive`，不得发布伪造零值
