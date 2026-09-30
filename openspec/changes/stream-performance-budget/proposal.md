# Proposal

## Why

当前 P1 已能完整采集并通过 SSE 发布，但 WSL 基线显示实时文本流约 195 KiB/s，Trace 时约 204 KiB/s；设备端还缺少每轮耗时、丢弃和传输量的统一观测。需要先建立性能预算和可复现实测入口，再决定是否继续压缩字段或调整采样策略。

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
