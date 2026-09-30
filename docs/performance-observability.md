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

## 尚未完成

- 慢客户端计数目前只覆盖广播队列落后，尚未覆盖传输层写超时；不能作为全部慢连接关闭次数。
- 窗口覆盖时间、最后恢复结果、前端导出统计和完整浏览器长时间运行验证。
- 相同构建配置下的优化前后 CPU、RSS、传输量对照，以及 300/3000 进程场景。

旧 `performance-pc-wsl.md` 中约 195/204 KiB/s 是紧凑协议改造前数据。当前 compact 的初步记录见 `compact-wire-wsl.md`；两者场景不同，不能据此宣称压缩率或当前性能。PC/WSL 结果均不替代嵌入式目标板验收。

本轮已执行 Rust 单元测试、默认构建端到端测试、前端 layout/history 和默认构建 clippy。task 文件只勾选已有对应证据的子项，整体 change 尚未完成。
