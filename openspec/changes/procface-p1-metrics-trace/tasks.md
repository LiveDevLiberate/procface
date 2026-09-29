# Tasks

## 1. 目录与解析器

- [x] 1.1 扩展 metric catalog、紧凑字典和 capabilities，加入 PSI、中断、softirq、结构化 Trace 字段及单位。
- [x] 1.2 实现 PSI 文本解析、avg/total 状态和 total 速率基线。
- [x] 1.3 实现 interrupts/softirqs 动态实体解析、速率、回退和实体消失状态。
- [x] 1.4 实现 status、smaps_rollup、sched 和线程 stat 的字段级解析，覆盖缺失/非法字段。

## 2. Daemon 与 Trace

- [x] 2.1 将 P1 采样组接入主采样快照、health、current、series、stream 和 export。
- [x] 2.2 将结构化 Trace 字段接入 extended/diagnostic 开关，保留原始文本并执行预算与大小上限。
- [x] 2.3 验证权限错误、PID 复用、线程变化、文件消失、计数器回退和部分批次 complete/diagnostics。
- [x] 2.4 验证 P1 采样和 Trace 并行时主循环、SSE heartbeat、慢客户端和单活动 Trace 不受阻塞。

## 3. 前端分析体验

- [x] 3.1 增加压力、中断和 softirq 分组图表及状态提示。
- [x] 3.2 将 Trace 字段按内存、调度、I/O、FD、线程分组，显示权限和读取成本。
- [x] 3.3 对齐系统与 Trace 的 uptime 时间轴，保留 gap、历史、导出和重连语义。
- [x] 3.4 增加从异常总览选择 PID 到 Trace 的浏览器回归测试。

## 4. 验证

- [x] 4.1 扩展模拟 procfs 单元测试和 daemon 端到端测试。
- [x] 4.2 扩展紧凑协议、JSON/TSV 导出和 Prometheus 字段测试。
- [x] 4.3 运行前端 history、stream、layout 和真实 daemon 浏览器测试。
- [x] 4.4 更新中文指标覆盖和 Trace 权限/成本文档，运行 OpenSpec 严格校验。
