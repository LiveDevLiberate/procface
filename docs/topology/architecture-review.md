# 子系统结构核对：存储与网络

本文件先确定图中每条线的含义，再决定指标位置。图不是内核调用栈、运行时自动发现的设备拓扑，也不能把所有 Linux 配置压成一条固定路径。存储基准场景是**本地块文件系统上的普通文件 I/O**；网络基准场景是**本机 TCP/UDP Socket 收发**。其他路径必须画成条件分支，或明确标为未覆盖。

## 旧评审图需要纠正的地方

1. 存储图把“文件系统 → 页缓存 → 块层”画成固定串联。实际缓冲 I/O 使用文件的 `address_space`/页缓存，缓存未命中、脏页回写仍要通过文件系统的相应操作组织 I/O；direct I/O 则绕过通常的页缓存路径。页缓存不是独立替代文件系统的一站。
2. “页缓存 → 回写”不能表示所有缓存访问。只有脏数据等需要写出时才有写回；回写可异步发生，与触发该脏页的进程时间上分离。
3. 旧图的“上层请求队列 → 虚拟设备 → 下层请求队列”把设备叠层与 blk-mq 队列混为一谈。块设备可以叠加映射，映射后的 I/O 进入下层设备；**在具体设备的 blk-mq 中**，软件暂存队列是条件路径，硬件派发队列是另一种队列。`disk.in_flight` 不是这两个队列的深度。
4. 旧图把 `/proc/diskstats` 只放在最底部物理设备。它按**块设备实体**统计，实体可能是分区、虚拟块设备或下层设备；不应把这些实体默认相加或从名字推断父子关系。
5. 网络图把所有 RX 都画成“设备 → NAPI → IP → TCP/UDP → Socket”。这是本地交付的一个常见分支。桥接、IP 转发、AF_PACKET、虚拟接口、XDP 等可能绕开其中若干节点；并非每包都有 Socket 终点。
6. 网络图缺少驱动和网络层之间的二层接收分派，以及 TX 从 IP 到邻居/二层封装、qdisc、接口发送的关系。Netfilter 也不是一个处于固定单点的方框。
7. `/proc/net/dev` 只提供接口级计数。它不能给 TCP、IP、qdisc、NAPI、DMA ring 或特定进程的箭头标吞吐；`/proc/softirqs` 的 `NET_RX`/`NET_TX` 仍是按类型和 CPU 汇总。

## 存储图的基准路径

```text
进程 read/write/mmap
    → VFS 的 file_operations
    → 本地块文件系统的文件/映射操作
       ├─ buffered read: 页缓存命中 → 返回；未命中 → 文件系统 read_folio/readahead → bio
       ├─ buffered write: 写入页缓存并标脏；稍后的 writeback → 文件系统 writepages → bio
       └─ direct I/O: 文件系统 direct_IO/iomap 等路径 → bio（绕过通常页缓存）
    → 目标块设备（可能先经 dm/md/loop 等映射，再到下层块设备）
    → 该目标设备的块层提交 / blk-mq（可直接派发，或经软件暂存队列，再到硬件派发队列）
    → 对应驱动 / 控制器 / 设备
```

此示意只画请求向下的方向，读数据和完成通知还会返回。tmpfs、网络文件系统、原始块设备访问和不同驱动模型不应接到上述全套路径。具体内核版本、文件系统和驱动会改变中间实现，图中不写死具体函数作为所有设备的必经点。

指标落点：`memory.cached_bytes`、`memory.dirty_bytes`、`memory.writeback_bytes` 和 `vm.*` 放在系统级旁侧栏；每一个 `disk.*` 放在相应**块设备实体**上，可包含虚拟层和下层设备，但当前 ProcFace 无法可靠建立二者映射。`pressure.*` (`entity=io`) 也是系统资源压力，不属于某块设备。所有内部接口保持“未直接观测”。单 PID 的 `process.io.*` 只放在独立进程/Trace 视图，不进入存储子系统图。

## 网络图的基准路径

```text
TX：进程 → Socket → TCP/UDP → IP 输出/路由 → 邻居解析/二层封装
    → 可选 qdisc → net_device → 驱动/设备发送

RX：设备接收 → 驱动事件处理（常见为 IRQ + NAPI poll，可有 GRO）
    → net_device 接收入口 → 二层协议分派（可经 VLAN/bridge 等）
    → IP 本地交付 → TCP/UDP → Socket → 进程
```

RX 的二层分派可能进入桥接，IP 层可能进入转发，两者都不必进入本地 Socket。AF_PACKET、XDP、隧道、虚拟设备和环回还有不同旁路；先在图中以条件分支/未覆盖说明表示，不能把它们强制接到主路径。qdisc 可能采用 `noqueue` 等形式；队列格只表示存在队列的结构位置，不表示某个实例一定排队。

指标落点：`network.rx_*` 和 `network.tx_*` 都属于 `/proc/net/dev` 中**同一个接口实体**。RX/TX 方向可分开展示，但不得画成两块不同设备。`softirq.count_per_second` 的 `NET_RX`/`NET_TX` 与 `interrupts.count_per_second` 只放旁侧栏，当前不能对应到某个接口或 NAPI 实例。协议统计的 P1 提案未实现，不应在结构节点显示实时值。

## 符号规则

- 外层浅色区域表示子系统或路径范围，不是设备。
- 圆角矩形表示组件、设备或子功能；圆角矩形可以嵌套，内层表示包含关系。仅在可以确认实例关系时才把实际设备绑定到内层。
- 相邻的直角小矩形表示队列；数量和填充不能凭 `in_flight`、包速率等旁路指标推断。
- 实线表示基准场景路径，虚线表示条件分支；没有直接接口计数的线不写数值。
- 颜色只表示指标的统计范围或状态，不代表内核层级或性能好坏。

依据：[VFS](https://docs.kernel.org/filesystems/vfs.html)、[Page Cache](https://docs.kernel.org/mm/page_cache.html)、[blk-mq](https://docs.kernel.org/block/blk-mq.html)、[diskstats 字段](https://docs.kernel.org/admin-guide/iostats.html)、[NAPI](https://docs.kernel.org/networking/napi.html)、[网络接收扩展](https://docs.kernel.org/networking/scaling.html)、[Linux Bridge](https://docs.kernel.org/networking/bridge.html)、[网络接口统计](https://docs.kernel.org/networking/statistics.html)。
