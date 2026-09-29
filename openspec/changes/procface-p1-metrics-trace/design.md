# Design

## Context

当前 P0 采样器已经有统一 `SampleBatch`、状态枚举、动态字典和 uptime 速率基线。Trace worker 可读取原始 procfs 文本，但前端只能按文本块展示；`capabilities` 中的 PSI、协议、中断和 softirq 目前只做探测。设计必须继续使用 procfs、固定 1 秒系统采样、独立 Trace worker、有界缓存和 API v1。

## Goals / Non-Goals

**Goals:**

- 用字段级状态表达 procfs 文件或字段缺失、权限拒绝、解析失败、预算跳过和计数器回退。
- 让 P1 系统异常与选定 PID 的 Trace 样本共享 uptime、session、sequence 和 gap 语义。
- 结构化展示最有价值的进程内存、调度、I/O、线程和 FD 信息，同时限制读取成本。

**Non-Goals:**

- 不读取或默认持久化完整 `smaps`、每个 FD 的符号链接目标和所有网络协议表。
- 不改变 API 版本、认证、CORS、采样间隔下限、历史窗口或 Trace 单活动约束。

## Decisions

1. **P1 指标按可选采样组加入**：增加 `pressure`、`interrupts`、`softirq` 组；缺少来源生成 `unsupported`，不阻塞其他组。相比把所有 procfs 文件塞进默认循环，这能控制嵌入式设备开销并保持能力探测准确。
2. **压力值使用原生字段**：PSI 的 `avg10/60/300` 作为 gauge，`total` 作为 counter 并计算速率；不把压力百分比转换成自定义阈值，阈值交给前端。
3. **中断与 softirq 使用动态实体**：IRQ 名称、softirq 类型和 CPU 编号进入实体字典，原始累计值与速率都保留。相比固定字段表，这兼容内核新增类型。
4. **Trace 采用结构化字段加原文双轨**：`status`、`smaps_rollup`、`sched` 和线程 stat 解析出稳定字段，原始文件仍按受限文本保留。解析失败只影响对应字段，不丢弃整个 Trace 批次。
5. **成本分层**：status、schedstat、io 和线程主 stat 为 extended；smaps_rollup、线程详细字段和 FD 遍历受 Trace 预算控制；敏感文件继续只在 diagnostic 构建和显式开关可用。相比后台自动扫描，显式 Trace 能避免泄露和全局开销。
6. **前端按样本时间对齐**：系统图表和 Trace 图表都使用 `uptime_s`，只在同一 session 内对齐；跨 gap 或缺失状态不连线。相比使用浏览器墙上时间，这不受 RTC、时区和断线影响。

## Risks / Trade-offs

- [内核字段变化] -> 使用字段探测、动态字典和未知字段诊断，不把缺失当零。
- [smaps_rollup 或线程读取过慢] -> 独立 worker、每轮预算、单文件上限，超时整轮标记 incomplete。
- [权限不足造成 Trace 不完整] -> 每个字段携带 permission_denied，并在前端显示读取状态和建议权限。
- [实体数量过多增加紧凑字典] -> 复用 session 字典、限制单轮实体/文本数量，超限保留 diagnostics 和 gap。

## Migration Plan

先加入解析器、目录和后端测试，再接入紧凑编码，最后增加前端分组。旧 API v1 客户端可以继续读取已有指标；不认识的新 metric 只显示为未知字段，不改变既有样本。回滚时删除新增采样组和前端分组即可，P0 数据和历史格式保持兼容。
