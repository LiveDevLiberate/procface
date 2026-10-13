# ProcFace 子系统指标归属草案

本表用于核准结构图，而不是定义新的采集接口。以当前 `src/metric-catalog.tsv`、`src/collector.rs`、`src/p1.rs` 和 `src/trace.rs` 为准。结构参考 [Linux Storage Stack Diagram](https://www.thomas-krenn.com/en/wiki/Linux_Storage_Stack_Diagram) 的分层、分支表达方式；这里不复制原图，也不假定目标设备具备原图中的全部模块。

逐个静态 metric ID 的机器可校验归属见 [`metric-taxonomy.tsv`](metric-taxonomy.tsv)。修改 catalog、采集来源或子系统落点时，必须运行 `node tools/check_metric_taxonomy.mjs`；本表的字段族说明用于人审，TSV 是覆盖性校验的基准。

## 标注规则

- **组件观测**：子系统图指标只归属于明确的 CPU、网卡、IRQ 或块设备等非单进程实体，可标在该实体节点内。
- **子系统汇总**：指标只覆盖整个系统或资源类别，应放在层旁的汇总栏；不能写到单个组件或箭头上。
- **结构节点**：Linux 中真实存在，但当前 ProcFace 没有该层的直接计数。节点可画，标明“未直接观测”。
- **关系线**：表示 Linux 架构中的可能路径。只有确实测到该接口本身，才在线上显示数值。来自路径终点的统计不能冒充中间接口流量。
- **队列符号**：相邻的连续矩形表示队列；格数只是结构图符号，不表示真实容量。`disk.in_flight` 是设备在途 I/O 数，不是上层或下层队列深度；当前不能用它填充队列格。
- **组件符号**：圆角矩形表示子功能或设备；父组件可包含子组件，连线应连接到承担该路径的具体组件。嵌套表达结构关系，不表示当前设备已启用该分支；不能仅凭同一组指标推断实例父子关系。
- **进程边界**：子系统图的节点、连线、旁侧汇总和详情跳转不使用任何 PID/TID 指标。`process.*`、`trace.*` 以及按 PID/TID 统计的动态字段只在独立的进程/Trace 视图展示。用户空间等节点可以保留正式结构，但不显示单进程数值。
- **可用性**：默认组为 `cpu,memory,load,time,disk,network,vm`；`pressure,interrupts,softirq` 是可选采集组。`time.*` 用于样本时间轴，不作为内核组件性能值。
- 速率来自相邻采样的计数器差分；首次采样、计数器回退、来源不可用或采集不完整时，不应显示假数值。以下仅列代表指标，完整字段以 catalog 为准。

## 六个子系统的层级

| 子系统 | 正式结构图中的层/组件 | 当前可放置的指标 | 范围与来源 | 图中边界 |
| --- | --- | --- | --- | --- |
| Scheduler / CPU | 任务、可运行队列、调度器、逻辑 CPU | `cpu.running`, `cpu.blocked`, `load.*`; `cpu.context_switches_per_second`, `cpu.*` | 系统/每 CPU `/proc/stat`；系统 `/proc/loadavg` | 系统上下文切换和负载不能归到某个调度器队列；调度器内部队列没有直接指标。 |
| Memory / VM | 地址空间、缺页、页缓存、回收/换页、物理内存 | `vm.faults_per_second`, `vm.major_faults_per_second`, `vm.swap_*`, `vm.page_*`; `memory.*`; `pressure.*` (`entity=memory`) | 系统 `/proc/vmstat` 和 `/proc/meminfo`、可选 `/proc/pressure/memory` | 全局页故障、缓存和回收不能归因到某个进程或内存区；无页缓存命中率。 |
| VFS / Filesystem | 用户文件操作、VFS、页缓存、具体文件系统 | 旁侧系统汇总：`memory.cached_bytes`, `memory.dirty_bytes`, `memory.writeback_bytes` | 系统 `/proc/meminfo` | VFS、某一挂载点、某一种文件系统均无直接吞吐/时延；缓存指标是全局量，不能作为 VFS-to-Block 接口流量。 |
| Block I/O | 块层、可选的虚拟块设备、驱动、块设备/分区 | `disk.read_*`, `disk.write_*`, `disk.in_flight`, `disk.io_time_ms_per_second`, `disk.weighted_io_time_ms_per_second`; `pressure.*` (`entity=io`) | 每个 `/proc/diskstats` 设备实体；可选系统 `/proc/pressure/io` | `/proc/diskstats` 提供每设备统计，不提供 VFS、文件系统、驱动、控制器之间的流量；不能随意相加分区与其父设备或叠层设备。 |
| Networking | Socket、TCP/UDP/IP、队列/qdisc、接口/驱动 | `network.rx_*`, `network.tx_*` | 每接口 `/proc/net/dev` | 当前只有接口统计；无 Socket/协议/队列级数值，不能将接口流量归给进程。协议统计仍是未实现的 P1 提案。 |
| IRQ / SoftIRQ | 硬件/中断控制器、IRQ handler、softirq 类型、CPU | `interrupts.count_per_second`, `softirq.count_per_second`; `cpu.irq`, `cpu.softirq` | 可选每 IRQ/CPU `/proc/interrupts`，每类型/CPU `/proc/softirqs`；每 CPU `/proc/stat` | IRQ 与 softirq 计数可分别标节点，不能推出某个 IRQ 触发了某类 softirq；CPU 时间是聚合值。 |

进程/Trace 是独立的正交视图，不属于上述六张子系统图。其单进程指标不能自动分配给系统级块设备、网络接口或 IRQ。

## 指标字段族的主归属

以下覆盖当前 `src/metric-catalog.tsv` 的静态字段族。**主归属**决定指标默认出现在哪张图；其他图若引用同一系统级指标，只能作为注明范围的旁侧背景信息，不能将其变成局部组件或接口值。**观测边界**是 procfs 实际统计的位置，不等于图中的所有结构连线。

| catalog 字段 / ID | 主归属与层级 | 统计实体 / 设备 | 实际观测边界 | procfs 来源 |
| --- | --- | --- | --- | --- |
| `time.*` / 1–2 | 样本元数据；不属于子系统 | 整个采样批次 | 时间轴与采样间隔，非性能接口 | `/proc/uptime` 与采样器时钟 |
| `cpu.usage`–`cpu.steal` / 10–18 | Scheduler / CPU → 逻辑 CPU 时间分布 | 系统合计或单个逻辑 CPU | CPU 时间分类；`iowait`、`irq`、`softirq` 可供其他图旁注，但不是设备、IRQ 或接口计数 | `/proc/stat` |
| `cpu.context_switches*`、`cpu.forks*`、`cpu.running`、`cpu.blocked` / 20–25 | Scheduler / CPU → 系统调度活动 | 全系统 | 调度事件累计/速率与当前任务数；不是某个运行队列深度 | `/proc/stat` |
| `load.*` / 60–64 | Scheduler / CPU → 系统负载 | 全系统 | 负载均值、可运行任务数和任务总数 | `/proc/loadavg` |
| `memory.*` / 40–52 | Memory / VM → 物理内存、页缓存、脏页、Slab、Swap | 全系统；无单内存设备实体 | 容量和存量，不测 VFS 或块设备接口流量 | `/proc/meminfo` |
| `vm.*` / 70–81 | Memory / VM → 全局分页、换页、缺页 | 全系统 | VM 事件与全局页读写量；不按文件系统或块设备归因 | `/proc/vmstat` |
| `disk.read_*`、`disk.write_*` / 90–101 | Block I/O → 被统计的块设备/分区 | 单个 `/proc/diskstats` entity；可为虚拟或物理块设备、分区 | 该 entity 的读写请求、字节和累计耗时；不是 VFS-to-Block 连线 | `/proc/diskstats` |
| `disk.io_time_*`、`disk.weighted_io_time_*`、`disk.in_flight` / 102–106 | Block I/O → 被统计的块设备/分区 | 单个块设备 entity | 设备活动时间与在途请求；`in_flight` 不是队列容量/深度 | `/proc/diskstats` |
| `disk.available` / 107 | Block I/O → 采集可用性 | 采集组；非设备性能值 | 来源是否可用 | `/proc/diskstats` |
| `network.available` / 108 | Networking → 采集可用性 | 采集组；非接口性能值 | 来源是否可用 | `/proc/net/dev` |
| `network.rx_*`、`network.tx_*` / 110–125 | Networking → `net_device` 接口统计 | 单个网络接口 entity；可能为物理、虚拟或 loopback | 该接口的收发字节、包、错误和丢弃；不测 Socket、协议栈、qdisc 或驱动内部连线 | `/proc/net/dev` |
| `pressure.some.*`、`pressure.full.*` / 300–309 | `entity=cpu` 属 Scheduler；`entity=memory` 属 Memory；`entity=io` 属 Block I/O 的系统旁注 | 对应资源类别的全系统压力；非单 CPU/设备 | 资源等待时间与比例，不归因到具体设备或接口 | `/proc/pressure/{cpu,memory,io}` |
| `interrupts.count*` / 310–311 | IRQ / SoftIRQ → IRQ 处理 | IRQ 与 CPU 组合 | 中断次数；不证明后续 softirq 或网络接口归属 | `/proc/interrupts` |
| `interrupts.available` / 312 | IRQ / SoftIRQ → 采集可用性 | 采集组 | 来源是否可用 | `/proc/interrupts` |
| `softirq.count*` / 313–314 | IRQ / SoftIRQ → softirq 类型 | 类型与 CPU 组合 | softirq 执行次数；不测设备队列或特定 IRQ 到 softirq 连线 | `/proc/softirqs` |
| `softirq.available` / 315 | IRQ / SoftIRQ → 采集可用性 | 采集组 | 来源是否可用 | `/proc/softirqs` |

VFS / Filesystem 当前没有独占的直接测量字段。它可以画正式结构，并在旁侧引用 Memory / VM 的系统级页缓存、脏页与回写**存量**，但不能把这些值当作某文件系统或 VFS-to-Block 接口的吞吐。`pressure.*` 的 metric ID 在三类资源间复用，必须同时检查 `entity` 才能确定归属。

| catalog 字段 / ID | 独立视图归属 | 统计范围 / 来源 |
| --- | --- | --- |
| `process.*` / 140–148 | 进程列表或进程详情 | 单 PID；`/proc/<pid>/stat`、`status`、`fd` 等 |
| `process.io.*` / 160–173 | 单 PID Trace | `/proc/<pid>/io` |
| `trace.*` / 200–220 | 单 PID Trace 原始诊断与状态 | `/proc/<pid>/*` 或 Trace 会话元数据 |
| `process.status.*` / 320–335 | 单 PID Trace | `/proc/<pid>/status` |
| `process.memory.*` / 340–353 | 单 PID Trace | `/proc/<pid>/smaps_rollup` |
| `process.sched.*` / 360–364 | 单 PID Trace | `/proc/<pid>/sched` |
| 动态线程字段 | 单 PID Trace | 单 TID；`/proc/<pid>/task/<tid>/*` |

上述进程字段即使与 CPU、内存或块 I/O 有关，也不进入对应子系统图。其 PID/TID 统计范围不能转换成全系统或设备接口值。

### 动态线程字段

线程字段不占用静态 catalog 行，而是在 Trace 会话发现线程时按 TID 生成。它们属于独立的进程/Trace 视图，不能进入 Scheduler、Memory、Block I/O 或其他子系统图。

| 动态字段模式 | 语义 | 统计实体 | procfs 来源 | 紧凑协议 ID |
| --- | --- | --- | --- | --- |
| `trace.thread.<tid>.stat` | 线程 `/proc/<pid>/task/<tid>/stat` 原文 | 单 TID | `/proc/<pid>/task/<tid>/stat` | `2^32 + tid` |
| `trace.thread.<tid>.state` | 线程状态字符 | 单 TID | `/proc/<pid>/task/<tid>/stat` | `2^34 + tid` |
| `trace.thread.<tid>.cpu_seconds` | 由线程 utime/stime 换算的 CPU 秒数 | 单 TID | `/proc/<pid>/task/<tid>/stat` | `2^35 + tid` |

`<tid>` 必须是不带前导零的十进制正整数，且处于 `u32` 范围内；动态 ID 按上述规则确定，不占用静态 metric catalog。采样点的 `entity` 使用所属进程的身份键，TID 则包含在 metric 名称中；解码和展示时必须同时保留两者。线程文件读取失败时记录诊断并跳过该线程，不用零值填充。

## 存储栈展开图的候选层

从上到下画层级，读、写和直接 I/O 可分支，但不以箭头暗示现有指标已测量每一跳。

| 层 | 组件或路径 | 图内指标位置 | 观测判断 |
| --- | --- | --- | --- |
| 1. 用户空间 | 进程读写、文件映射、直接 I/O 请求 | 无 | 保留结构节点，不显示单进程 I/O 值；当前没有系统级的用户空间到 VFS 接口计数。 |
| 2. VFS | syscall 入口、VFS、挂载/文件系统选择 | 无 | 结构节点；当前不能区分 ext4、f2fs、overlayfs 等具体文件系统的吞吐。 |
| 3. 文件数据路径 | page cache、dirty page、writeback、direct I/O 旁路 | 旁侧系统汇总：`memory.cached_bytes`, `memory.dirty_bytes`, `memory.writeback_bytes`, `vm.page_in_bytes_per_second`, `vm.page_out_bytes_per_second` | 页面/系统级统计，不是缓存命中率、某文件系统回写量或 direct I/O 吞吐。 |
| 4. 块设备抽象 | 文件系统提交 I/O、上层块层、可选 device-mapper/虚拟设备、下层块层 | 有实体时可显示该块设备的 `disk.*` | 映射后请求会重新进入下层块层；`diskstats` 实体名不构成父子关系图，不使用名称猜测拓扑。 |
| 5. 设备侧 | 块驱动、控制器、物理或虚拟块设备、分区 | 对应实体 `disk.read_*`, `disk.write_*`, `disk.in_flight`, `disk.io_time_ms_per_second`, `disk.weighted_io_time_ms_per_second` | 数值归属于被统计的设备实体。`io_time` 是设备忙碌时间累计差分，可估算忙碌比例，但不是请求时延或 VFS 耗时。 |
| 旁侧观测 | 系统 I/O 等待 | `pressure.some.avg10`, `pressure.full.avg10` (`entity=io`), `cpu.iowait`, `cpu.blocked` | 资源压力、CPU 状态和阻塞任务汇总；不表示某块设备导致的等待。 |

特别注意：旧版 OpenSpec 曾把 `disk.read_bytes_per_second` / `disk.write_bytes_per_second` 绑定到 VFS-to-Block 连线，与指标范围冲突。现已将规范改为把这两个值放到**块设备节点**；VFS-to-Block 连线只画结构路径和“未直接观测”。前端实现仍需随后调整。

## 网络栈展开图的候选层

网络图应把发送和接收画成两条相反方向的路径，结构参考用户提供的 [Linux Network Stack 图](https://wiki.altlinux.ru/Images.www.altlinux.org/5/5c/LinuxNetworkStack.png)。具体拓扑以 Linux 内核文档为准；当前 ProcFace 的实时指标主要落在接口层。

| 层 | 发送 TX | 接收 RX | 当前指标归属 |
| --- | --- | --- | --- |
| 用户空间 | 进程调用 Socket API | 进程从 Socket 读取 | 当前没有每进程网络字节数或包数；`process.fd_count` 不能当作 Socket 数。 |
| Socket / 传输层 | Socket 缓冲、TCP/UDP | TCP/UDP、Socket 缓冲 | 结构节点，当前没有已实现的协议或 Socket 统计；`p1-network-protocol-metrics` 仍是提案。 |
| 网络层 | IP 路由、可选 Netfilter | 可选 Netfilter、IP 路由/本地交付 | 结构节点，当前没有逐层吞吐或丢包归因。 |
| 设备无关层 | qdisc、`net_device` | `net_device`、上行协议交付 | `/proc/net/dev` 按接口 entity 提供 `network.tx_*`、`network.rx_*`。这是接口统计，不是 qdisc、IP 或驱动内部统计。 |
| 驱动 / 设备 | 驱动提交 TX、物理或虚拟接口 | 设备 RX、驱动、常见的 NAPI 处理 | 驱动队列、DMA ring、NAPI poll 目前未直接观测；物理网卡、loopback、veth 等路径不应强行画成同一种硬件。 |
| 旁侧观测 | NET_TX softirq 类型 | NET_RX softirq 类型，可能关联 IRQ | 可选 `softirq.count_per_second` 按类型/CPU、`interrupts.count_per_second` 按 IRQ/CPU；没有可靠的接口归属关系。 |

接口实体可显示 `network.rx_bytes_per_second`、`network.rx_packets_per_second`、`network.rx_errors_per_second`、`network.rx_dropped_per_second` 及对应 `tx_*` 指标。即使网卡有统计值，也不能据此判定某个数据包经过了哪一个 Socket、Netfilter hook、qdisc 或物理端口。

## 绘图前仍需的结构事实

`procfs` 指标能确认某个设备/接口的**活动**，通常不能单独确认它处于哪条内部路径。因此首版图应采用 Linux 通用结构，按设备能力隐藏不适用的分支；不能仅凭 `diskstats` 名称判断 device-mapper、文件系统、控制器和物理盘之间的映射。需要实例化真实设备拓扑时再考虑 `/sys`、挂载信息等额外来源，这属于另一个采集范围决定。

依据：[内核 procfs 文档](https://docs.kernel.org/filesystems/proc.html)、[块设备统计字段](https://docs.kernel.org/admin-guide/iostats.html)、[网络接口统计](https://docs.kernel.org/networking/statistics.html)、[NAPI 文档](https://docs.kernel.org/networking/napi.html)、[PSI 文档](https://docs.kernel.org/accounting/psi.html)、[VFS 概览](https://docs.kernel.org/filesystems/vfs.html)。
