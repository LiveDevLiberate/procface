# Tasks

## 1. Daemon metrics

- [x] 1.1 记录系统、进程和 Trace 采样耗时，并在 health 返回最近值、最大值和超预算计数；用模拟 procfs 测试超时轮次继续运行。
- [x] 1.2 统计 SSE、JSONL、TSV 编码后的批次数和字节数，以及慢客户端关闭次数；用 stream 测试验证计数有界且不改变样本 schema。
- [x] 1.3 将性能统计加入 capabilities 的可选特性说明，并验证旧字段和 v1 路径保持兼容。

## 2. Frontend and tooling

- [x] 2.1 在静态 HTML 连接状态区显示采样延迟、窗口覆盖、断链和恢复结果；运行 layout/history 浏览器测试。
- [x] 2.2 扩展性能测量脚本覆盖无客户端、SSE、SSE+Trace、慢客户端和大进程数，输出 CPU、RSS、窗口字节和传输字节。
- [x] 2.3 更新中文性能文档，明确 PC/WSL 基线不是嵌入式验收；运行端到端和前端回归测试。

## 3. Verification

- [ ] 3.1 运行 cargo fmt、cargo test、cargo clippy、Python daemon/stream 测试和浏览器测试。
- [x] 3.2 用 OpenSpec strict 校验 change，并记录优化前后传输与内存数据。
