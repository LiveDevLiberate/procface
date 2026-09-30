# Proposal

## Why

P1 已接入采集与 SSE 发布，设备端还缺少每轮耗时和传输量的统一观测。旧 WSL 报告中的约 195/204 KiB/s 来自紧凑协议改造前，不能代表当前版本；需要以当前 compact 实现重建基线，再决定是否继续优化传输或采样。

## What Changes

- 为 daemon 增加采样轮次耗时、发布批次大小、流量和预算状态的 health/capabilities 信息。
- 为 SSE、JSONL、TSV 导出增加可选传输统计，保持 v1 默认响应兼容。
- 前端显示采样延迟、窗口占用和断链恢复状态，支持在测试中导出性能诊断。
- 增加固定进程数、Trace、慢客户端和长时间运行的性能基线脚本及验收阈值。

## Capabilities

### New Capabilities
- 无

### Modified Capabilities
- `daemon`: 增加性能预算、采样延迟和传输统计的可观测行为。
- `frontend`: 增加性能状态展示和诊断导出行为。

## Impact

影响 `src/daemon.rs`、`src/store.rs`、compact 编码与 HTTP 流处理、前端单体 HTML、性能测量脚本及 docs。默认采样指标、API 主版本和现有数据字段保持兼容，不引入新的运行时依赖。
