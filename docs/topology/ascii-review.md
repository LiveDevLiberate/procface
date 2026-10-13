# Linux 子系统结构与指标落点：ASCII 审核稿

> **审核状态：待用户审核。** 本文件是 SVG 重画和前端接入的唯一结构前置稿。未收到六张图的审核结论前，不修改正式 SVG，也不把 SVG 接入单体 HTML。

本稿先核对结构、数据方向和统计范围；审核通过后才据此修改 SVG 与前端图模型。图是 Linux 的通用结构，不是 ProcFace 从设备上自动发现的实例拓扑。首版为 Scheduler / CPU、Memory / VM、VFS / Filesystem、Block I/O、Networking、IRQ / SoftIRQ 六张图；进程/Trace 是独立视图，以下节点、边、旁侧汇总和详情均不使用 PID/TID 指标。指标全量归属见 [metric-ownership.md](metric-ownership.md)。

## 审核回执

请对每一行回复“通过”或指出需要修改的节点、方向、条件分支或指标落点。未明确回复的项目保持待审核。

| 图 | 审核范围 | 当前状态 |
| --- | --- | --- |
| Scheduler / CPU | runqueue 的 `S/T` 结构、唤醒/选择/切换方向、CPU 与 load 指标只作系统或 CPU 汇总 | 待审核 |
| Memory / VM | 页表、页缓存索引、buddy 链表、回收集合、swap 条件路径及全局指标边界 | 待审核 |
| VFS / Filesystem | path walk、mount tree、dentry 映射、address space、buffered/direct I/O 和非块后端分支 | 待审核 |
| Block I/O | 逻辑/叠层设备、blk-mq 软件/硬件队列、retry 链表、驱动 ring 与 `disk.*` 实体归属 | 待审核 |
| Networking | TX/RX、Socket、协议、路由映射、qdisc、net_device、NAPI、ring 及旁侧 IRQ/softirq | 待审核 |
| IRQ / SoftIRQ | IRQ domain 映射、handler 链表、pending 位图、类型映射、CPU 执行路径及禁止推断的关联 | 待审核 |

审核结论写入本表后，才进入 SVG 阶段。单进程和 Trace 指标不参与六张子系统图审核。

## 统一符号

```text
(name)           子功能或设备；SVG 使用圆角矩形，可嵌套
Q[ ][ ][ ]       队列；连续矩形仅表示结构，不表示容量、深度或占用
L(o)->(o)       链表；箭头是元素关系，不是性能数据流
S{a,b,c}        集合或位图；不承诺 FIFO 顺序
M{k=>v}         键到值的映射、索引或查找表
T<root/...>      层级树或有序树
R[ ][ ][ ](wrap) 环形描述符队列；仅用于确有 ring 的设备路径
--->             可能的数据/控制路径；默认无直接接口计数
...>             条件路径；依内核配置、设备或工作负载而定
<---             完成、返回或反向路径
```

圆括号可套圆括号表示组件包含关系；结构图中的图标不等于当前 daemon 能测到其容量或活动。所有 `Q/L/S/M/T/R` 目前均无可用于填充格子、计算长度或命中率的 ProcFace 指标。一个名为 runqueue 的内核对象不能仅因名称而画成 FIFO 队列：调度类可能使用有序树、优先级队列或其他组织方式。下列 `M` 有些表示稳定的逻辑查找关系，具体内核容器可能随版本变化。

## 全局关系

```text
                           (Scheduler / CPU)
                        executes kernel work
                          /      |       \
                         v       v        v
(file syscalls) ---> (VFS) <--> (Memory / VM) ...> (Block I/O)
                       |                |                 ^
                       +---- bio ------>+-----------------+
                       |                                  ^
                       +-- local block FS only ------------+

(socket operations) <--> (Networking) <... (IRQ / SoftIRQ)
                            |     ^            |
                            +---->+-- CPU work -+
```

全局连线是子系统间的结构关系，不承载数值。文件系统可能不是块设备后端；换页和页缓存可能通过块层，网络接收可能经 IRQ/NAPI，也可能走轮询、虚拟接口或其他旁路。Scheduler / CPU 提供执行上下文，而非数据包或 bio 的固定中继站。

## 1. Scheduler / CPU

```text
(wakeup / timer / I/O completion)
                  |
                  v
          (CPU-local runqueue)
          +-- S{runnable by scheduling class}
          +-- T<fair-class order> ... version dependent
                  |
                  v
            (pick next) ---> (context switch) ---> (logical CPU)
                  ^                                  |
                  +<--- block / yield / preempt <---+

side: /proc/stat    cpu.running, cpu.blocked, cpu.context_switches*, cpu.forks*
side: /proc/loadavg load.*
CPU:  /proc/stat    cpu.usage, cpu.user, cpu.system, ... [cpuN or total]
side: /proc/pressure/cpu pressure.* [entity=cpu; optional]
```

`cpu.running` 和 `load.running` 是系统汇总，不是某个 CPU runqueue 的长度；`load.1m` 也不是当前队列深度。`cpu.context_switches*` 不能贴在 pick-next 到 CPU 的连线上。可比较运行队列的逻辑集合与 CPU 时间，但没有调度类内部结构的直接计数。[CFS 文档](https://docs.kernel.org/scheduler/sched-design-CFS.html)描述红黑树；[EEVDF 文档](https://docs.kernel.org/scheduler/sched-eevdf.html)说明新版选择策略，因此这里不把某种树当成所有内核的固定结构。

## 2. Memory / VM

```text
(memory access) ---> M{virtual address=>page-table entry} ---> (resident page)
          |
          +...> (page fault) ---> (file-backed mapping)
                     |                  M{file,index=>page/folio}
                     |                         |
                     |                         +...> (filesystem read) ...> (Block I/O)
                     +-------> (anonymous allocation)
                                       |
                              (per-CPU page set)
                                       |
                              (zone / buddy allocator)
                                 L(order-0 free blocks)
                                 L(order-1 free blocks) ...

(reclaim) <--- S{reclaim candidates} ...> (free pages / swap-out)
                                             swap-in <... (Block I/O)

side: /proc/meminfo  memory.* [system capacity/stock]
side: /proc/vmstat   vm.* [system paging, swap and faults]
side: /proc/pressure/memory pressure.* [entity=memory; optional]
```

页表映射、文件页缓存索引和空闲页链表是结构，当前没有对应长度/命中率。`memory.cached_bytes`、`memory.dirty_bytes` 和 `memory.writeback_bytes` 是全系统存量，不能填入一个文件映射；`vm.faults*` 也不能归给某一条分支。页缓存以文件索引查找缓存页，现代内核常用 folio/XArray；较旧内核的内部表示可能不同。buddy 分配器按 order 管理空闲区域。[页缓存](https://docs.kernel.org/mm/page_cache.html)、[物理内存](https://docs.kernel.org/mm/physical_memory.html)。

## 3. VFS / Filesystem

```text
(path operation: open/stat) ---> (VFS path walk)
                                   +-- T<mount topology>
                                   +-- M{parent,name=>dentry} ---> (inode)

(data operation on open file) ---> (VFS file operations)
                                        |
                                        v
                              (filesystem instance)
                               +-- (file address_space)
                               |     M{file offset=>cached page/folio}
                               |          +-- cache hit ---> return
                               |          +... cache miss / writeback ...> (bio)
                               +... (direct I/O) --------------------> (bio)
                               +... (non-block backend)              [other path]

                                        (bio) ...> (Block I/O)
side: memory.cached_bytes, memory.dirty_bytes, memory.writeback_bytes
      [system context only, owned by Memory / VM]
```

路径查找与已打开文件的数据操作是不同入口，不强制串联。dentry cache、mount tree 和文件页缓存确有查找结构，但当前 ProcFace 没有 VFS、单文件系统、挂载点或这些映射的吞吐/命中率。`bio` 只代表本地块后端的条件路径；tmpfs、网络文件系统等不能硬接到块设备。[VFS](https://docs.kernel.org/filesystems/vfs.html)、[路径查找](https://docs.kernel.org/filesystems/path-lookup.html)。

## 4. Block I/O

```text
(bio from filesystem / swap / raw block user)
                       |
                       v
              (logical block device)   [its own disk.* entity]
                       |
                       +...> (dm / md mapping)
                       |       M{logical sector=>lower target}
                       |                    |
                       |             (lower block device) [separate disk.* entity]
                       |                    |
                       +-- unstacked -------+
                                            v
                                   (request / blk-mq for target)
                                      M{CPU=>hardware context}
                                      +-- direct dispatch --------+
                                      +...> Q[ ][ ][ ] staging --+
                                                                 |
                                               Q[ ][ ][ ] hardware dispatch
                                                         |
                                               L(o)->(o) retry ... if busy
                                                         |
                                                      (driver)
                                                         |
                                            R[ ][ ][ ](wrap) ... device-specific
                                                         |
                                                      (device)

completion <--- driver <--- blk-mq <--- filesystem / caller
device: /proc/diskstats disk.* [one bdev or partition entity]
side:   /proc/pressure/io pressure.* [entity=io; optional]
```

虚拟/叠层设备与下层实体可能各自出现在 `/proc/diskstats`，不能由名称推断父子关系或相加。软件暂存队列是条件路径；硬件派发上下文是块层概念，驱动 ring 又是设备特定结构。blk-mq 文档明确描述软件/硬件队列及无法派发时的链表。`disk.in_flight` 是被统计设备的在途 I/O，不是任何 `Q` 或 `L` 的深度。`disk.read_*` 和 `disk.write_*` 只贴对应设备实体，不贴 bio、队列或 VFS-to-Block 连线。[blk-mq](https://docs.kernel.org/block/blk-mq.html)、[diskstats](https://docs.kernel.org/admin-guide/iostats.html)。

## 5. Networking

```text
TX: (socket) -> (TCP/UDP) -> (IP routing) -> (neighbor / L2)
                                  M{dst=>route}   M{next-hop=>L2}
                    -> (qdisc) ... Q[ ][ ][ ] -> (net_device: IFACE)
                    -> (driver) ... R[ ][ ][ ](wrap) -> (link)

RX: (link) -> R[ ][ ][ ](wrap) ... (driver / NAPI)
                    -> (net_device: SAME IFACE) -> (L2 demux)
                    -> (IP local delivery) -> (TCP/UDP)
                    -> Q[ ][ ][ ] socket receive ... (socket)

       L2 demux ...> (bridge / VLAN / packet socket)
       IP input ...> (forwarding)
       hooks ...> (Netfilter at several points, not one serial box)

IFACE: /proc/net/dev network.rx_*, network.tx_* [same interface entity]
side:  /proc/softirqs NET_RX / NET_TX [type and CPU; optional]
side:  /proc/interrupts [IRQ and CPU; optional]
```

RX/TX 是同一个接口的两个方向；`/proc/net/dev` 不能测 socket、IP、qdisc、NAPI 或 ring 的内部吞吐，也不能证明它们属于哪个进程。qdisc 可为 `noqueue`；物理网卡、virtio、veth 和 loopback 不能共用一条固定的 ring/IRQ 路径。协议和 socket 的 P1 指标仍只是待实现提案。[NAPI](https://docs.kernel.org/networking/napi.html)、[网络扩展](https://docs.kernel.org/networking/scaling.html)、[接口统计](https://docs.kernel.org/networking/statistics.html)。

## 6. IRQ / SoftIRQ

```text
(device event) ...> (interrupt controller)
                           |
                    M{hardware IRQ=>Linux IRQ}
                           |
                       (IRQ desc)
                       L(o)->(o) actions ... shared handlers
                           |
                    (hardirq on CPU)  [IRQ+CPU count]
                           |
                           +...> S{pending softirq bits by CPU}
                                      |
                               M{type=>softirq action}
                                      |
                               (softirq on CPU)  [type+CPU count]
                                      +...> (ksoftirqd) [deferred work]

other kernel paths --------...> S{pending softirq bits by CPU}
CPU:  /proc/stat cpu.irq, cpu.softirq [time fraction; cpuN or total]
IRQ:  /proc/interrupts interrupts.count* [IRQ+CPU]
type: /proc/softirqs softirq.count* [type+CPU]
```

IRQ domain 的映射、IRQ action 链表及 pending 位图是结构；当前没有它们的大小或关联关系指标。一次 IRQ 不等于一次同类型 softirq，也无法把 NET_RX 计数归给某个网络接口。某些设备或虚拟路径不经图示硬件中断入口。[generic IRQ](https://docs.kernel.org/core-api/genericirq.html)、[IRQ domain](https://docs.kernel.org/core-api/irq/irq-domain.html)。

## 指标绑定审查

| 图 | 可以有实时数值的位置 | 仅结构、不得填数值的位置 |
| --- | --- | --- |
| Scheduler / CPU | `cpu.*` 贴相同 CPU entity；`load.*`、调度全局字段放旁侧 | 运行集合/有序树、pick-next、切换箭头 |
| Memory / VM | `memory.*`、`vm.*`、memory PSI 放系统旁侧 | 页表、页缓存映射、buddy 链表、单次 fault/换页箭头 |
| VFS / Filesystem | 全系统缓存/脏页存量可作注明范围的旁注 | dcache、inode、文件系统实例、buffered/direct I/O 连线 |
| Block I/O | `disk.*` 贴相同块设备 entity；I/O PSI 放系统旁侧 | 设备映射、blk-mq 各队列/链表、驱动、连接线 |
| Networking | `network.*` 贴相同接口 entity；IRQ/softirq 仅旁注 | socket、协议、qdisc、NAPI、ring、连接线 |
| IRQ / SoftIRQ | IRQ+CPU、类型+CPU 的计数贴各自节点；CPU 时间贴 CPU/旁侧 | IRQ-to-softirq、IRQ-to-device、网络接口关联线 |

现有 `docs/topology/*.svg` 和 `web/procface-web.html` 的图模型早于此稿，不能视为已审核的结构定义。尤其要移除把进程聚合值放进子系统图、把设备计数放到中间连线、把 runqueue 画成 FIFO 等绑定。审查时先确认这六张图的组件、条件分支和符号语义，再进行 SVG 及前端修改。
