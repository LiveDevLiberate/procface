# 指标规格

指标目录见 [procfs-catalog.md](procfs-catalog.md)。目录定义 P0/P1/P2 指标族、来源路径、单位、计数器语义、读取成本、权限和缺失行为。

首个稳定层级包括：

- `/proc/uptime`、`/proc/stat`、`/proc/loadavg`
- `/proc/meminfo`、`/proc/vmstat`、`/proc/swaps`
- `/proc/diskstats`
- `/proc/net/dev`

诊断层级增加 PSI、协议/socket 计数器、中断、softirq 和显式选择的进程文件。所有 procfs 字段都必须做能力探测，因为内核版本、配置、命名空间、权限和挂载选项会改变结果。

## 统一样本 Schema

所有 CLI、daemon、SSE、JSONL 和前端接口 MUST 使用同一套规范化样本模型。字段如下：

```json
{
  "schema_version": 1,
  "session_id": "…",
  "sequence": 42,
  "uptime_s": 12345.6,
  "timestamp_unix": null,
  "metric": "cpu.usage",
  "entity": "cpu0",
  "value": 37.2,
  "unit": "percent",
  "kind": "gauge",
  "status": "ok"
}
```

`metric` 使用稳定的点号命名，例如 `cpu.usage`、`memory.available_bytes`、`disk.read_bytes_per_second`。`entity` 表示 CPU、磁盘、接口或进程等对象；系统级指标使用 `system`。`value` MUST 保留有效数值精度，无法提供时为 `null`。

`status` MUST 使用以下枚举：`ok`、`unsupported`、`permission_denied`、`parse_error`、`exited`、`stale`、`discontinuity`。错误或缺失状态不得伪造为零值。

速率字段的计算时间基准是相邻样本的 `/proc/uptime` 差值；`timestamp_unix` 仅为可选墙上时间，不能作为速率计算依据。

## P0 首阶段指标

首阶段 MUST 实现以下低成本指标组：

- CPU：总使用率、user、system、idle、iowait、steal、每 CPU 使用率、上下文切换、fork 次数、运行队列和阻塞进程。
- 内存：总量、可用量、空闲量、buffers、cached、swap 总量和可用量，以及内存/Swap 使用率。
- 负载：1/5/15 分钟负载、运行进程数和总进程数。
- 时间：uptime 和实际采样间隔。
- 磁盘：读写完成次数、读写字节、读写耗时和进行中的 I/O。
- 网络：收发字节、包、错误和丢包。
- VM：page in/out、swap in/out、fault 和 major fault。
- 基础进程：PID、名称、状态、CPU 时间、RSS 和线程数。

P0 数据源分别为 `/proc/stat`、`/proc/meminfo`、`/proc/loadavg`、`/proc/uptime`、`/proc/diskstats`、`/proc/net/dev`、`/proc/vmstat`、`/proc/PID/stat` 和 `/proc/PID/status`。可选文件或字段缺失时 MUST 通过 capabilities 和样本 status 报告，不得阻止其他指标采集。

## P0 计算规则

- CPU 使用率 MUST 根据相邻 `/proc/stat` counter 增量计算：`100 * (total_delta - idle_delta) / total_delta`。
- 内存使用量 SHOULD 使用 `MemTotal - MemAvailable`；`MemAvailable` 缺失时必须报告计算状态，不得静默替换为零。
- 磁盘字节数 MUST 由 sectors 乘以 512 计算，并在 schema 中标记单位为 bytes。
- 所有计数器下降、设备重置或 uptime 回退 MUST 产生 `discontinuity`，并重新建立速率基线。

P1/P2 指标包括中断、softirq、网络协议计数器、socket、PSI、完整线程信息和敏感进程字段，首阶段只要求能力探测，不要求默认采集。

## TSV 字段顺序

TSV MUST 使用固定列顺序：

```text
schema_version	session_id	sequence	uptime_s	timestamp_unix	metric	entity	value	unit	kind	status
```

缺失值输出为空；错误样本仍保留记录，并通过 `status` 表达原因。JSON 和 JSONL 使用同名字段。`kind` MUST 为 `gauge`、`counter` 或 `rate`。

## Prometheus 映射

`/metrics` MUST 将稳定指标映射为 `procface_` 前缀和下划线命名。例如：`cpu.usage` 映射为 `procface_cpu_usage_percent`，`memory.available_bytes` 映射为 `procface_memory_available_bytes`，原始累计计数器映射为以 `_total` 结尾的 counter。Prometheus 指标名 MUST 包含单位，label 只用于实体区分：CPU 使用 `cpu`，磁盘使用 `device`，网络使用 `interface`。

系统级指标 MUST 默认导出到 `/metrics`。进程指标默认不导出；只有显式启用进程 Prometheus 导出后，才允许使用 `pid` 和 `comm` label，避免默认产生高基数时间序列。


