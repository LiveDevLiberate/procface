# 性能观测实现进度

2026-09-30，`stream-performance-budget` 实现中。P1 的归档已存在；主规格全量严格校验此前存在结构错误，归档不代表这些错误已经解决。

## 已实现字段

`/api/v1/health.performance.sampling` 包含 system、process、trace 三个固定对象：

- `rounds`：本次 daemon 会话内采样尝试次数，包括读取失败。
- `last_sample_us`：最近一次采集调用耗时，未采样时为 null。
- `max_sample_us`：会话内最大耗时。
- `budget_us`：系统使用采样间隔；进程和 Trace 使用各自预算。
- `over_budget_rounds`：实际耗时大于对应预算的累计次数。

使用单调时钟计时，耗时不含发布编码、网络等待及 SQLite 写入。固定三组统计每轮更新一次，计数饱和，不保存逐轮日志。

`performance.transport` 分别记录 sse、jsonl、tsv 的 `batches` 和 `payload_bytes`。这些值表示交给响应流的样本负载，不是客户端确认接收字节或 TCP 流量；SSE 不计事件 framing、握手和心跳，TSV 不计列头，JSONL 计换行。重放和多订阅分别计数；进程重启清零。

前端每次 health 请求结束后等待 5 秒再请求，断开时取消；未知统计显示不支持。最近耗时超过预算时显示 degraded，恢复后显示 ok；历史超预算计数不导致永久降级。

`performance.windows` 按 system、process、trace 返回缓存首尾 uptime、`span_seconds` 和 `lost_through_sequence`。空窗口为 null，单批次跨度为 0；跨度不保证中间连续，丢失水位用于辅助判断，实际缺口仍以 series/gap 为准。前端显示窗口跨度、最近恢复结果及本地导出的批次数和字节数。

`performance.slow_clients` 统计广播队列落后导致的流关闭；`write_timeouts` 统计传输层写超时导致的连接关闭，覆盖 SSE 和跟随导出。两个计数分别保留，不能相加当作去重后的客户端人数。正常广播关闭、主动断连和请求头超时不计作写超时。

## 尚未完成

- 完整浏览器长时间运行验证。
- 相同构建配置下的优化前后 CPU、RSS、传输量对照，以及 300/3000 进程场景。

旧 `performance-pc-wsl.md` 中约 195/204 KiB/s 是紧凑协议改造前数据。当前 compact 的初步记录见 `compact-wire-wsl.md`；两者场景不同，不能据此宣称压缩率或当前性能。PC/WSL 结果均不替代嵌入式目标板验收。

已执行 Rust 单元测试、默认构建端到端测试、前端 layout/history、默认构建 clippy 和真实 TCP 背压测试；后者断言慢 SSE/导出释放后 write_timeouts 增加。task 文件只勾选已有对应证据的子项，整体 change 尚未完成。
