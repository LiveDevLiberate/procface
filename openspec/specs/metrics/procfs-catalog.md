# procfs 指标目录（Draft 0.1）

本文根据 Linux kernel 官方 `/proc` 文档整理可采集内容。procfs 文件和字段会随内核版本、配置、架构、命名空间和挂载选项变化；实现必须做能力探测，不能把某个发行版上的完整文件树当成保证。参考：[The /proc Filesystem](https://docs.kernel.org/filesystems/proc.html)、[PSI](https://docs.kernel.org/accounting/psi.html)。

## 分层策略

- **P0 基础**：嵌入式设备最常见，默认采集或低成本开启。
- **P1 诊断**：性能定位价值高，但字段/权限/内核配置更容易变化。
- **P2 专项**：特定协议、文件系统、驱动或调试场景，按需读取。
- **禁止默认读取**：可能昂贵、敏感或会改变内核状态的接口。

## P0：系统核心

| 文件 | 指标 |
|---|---|
| `/proc/uptime` | uptime、所有 CPU idle 时间；采样实际间隔基准 |
| `/proc/stat` `cpu*` | user、nice、system、idle、iowait、irq、softirq、steal、guest、guest_nice；总 tick 和每 CPU tick |
| `/proc/stat` `intr` | 总中断和各 IRQ 累计次数 |
| `/proc/stat` `ctxt` | 上下文切换累计次数 |
| `/proc/stat` `btime` | 启动时间（Unix 秒） |
| `/proc/stat` `processes` | 创建的进程/线程累计数 |
| `/proc/stat` `procs_running` | 当前 runnable 线程数 |
| `/proc/stat` `procs_blocked` | 当前等待 I/O 的阻塞进程数 |
| `/proc/stat` `softirq` | 总 softirq 及各类型累计次数 |
| `/proc/loadavg` | load1、load5、load15、运行数/总数、最近 PID |
| `/proc/meminfo` | MemTotal、MemFree、MemAvailable、Buffers、Cached、SwapCached、Active/Inactive、Unevictable、SwapTotal/Free、Dirty、Writeback、AnonPages、Mapped、Shmem、Slab、SReclaimable、SUnreclaim、Committed_AS、CommitLimit、Vmalloc*、PageTables、KernelStack、HugePages*（存在时） |
| `/proc/vmstat` | pgpgin/pgpgout、pswpin/pswpout、pgfault/pgmajfault、pgalloc/pgfree、pgscan/pgsteal、oom_kill、反向映射、THP、压缩/回收等内核虚拟内存累计计数（按字段能力发现） |
| `/proc/diskstats` | 每块设备/分区读写完成数、合并数、扇区数、请求耗时、进行中 I/O、加权 I/O 时间、discard/flush（字段存在时） |
| `/proc/net/dev` | 每接口收发 bytes、packets、errs、drop、fifo、frame、compressed、multicast、colls、carrier 等 |

CPU 百分比使用相邻 CPU tick 差值；`guest`/`guest_nice` 已包含在 user/nice 中，不能重复计入。Linux 文档特别指出 `/proc/stat` 的 iowait 在特定条件下可能下降且不可靠，应保留原始值并允许 discontinuity。

## P1：压力、网络协议和系统资源

| 文件/目录 | 指标 |
|---|---|
| `/proc/pressure/cpu` | some/full（CPU full 系统级可能为零）、avg10/60/300、total us |
| `/proc/pressure/memory` | some/full、avg10/60/300、total us |
| `/proc/pressure/io` | some/full、avg10/60/300、total us |
| `/proc/net/snmp` | IP、ICMP、TCP、UDP 协议累计计数；按两行 header/value 配对解析 |
| `/proc/net/netstat` | TcpExt、IpExt 等扩展网络统计；按 header/value 配对解析 |
| `/proc/net/sockstat`、`sockstat6` | sockets 使用量、TCP/UDP/RAW/FRAG 内存和数量（字段依内核变化） |
| `/proc/net/softnet_stat` | 每 CPU 网络输入队列处理、丢弃、time_squeeze、收到的 RPS 等十六进制计数 |
| `/proc/interrupts` | 每 IRQ 每 CPU 次数、设备/类型文本；可计算 IRQ 速率和热点 CPU |
| `/proc/softirqs` | HI、TIMER、NET_TX、NET_RX、BLOCK、TASKLET、SCHED、HRTIMER、RCU 等每 CPU 次数 |
| `/proc/sys/fs/file-nr` | 已分配文件句柄、空闲文件句柄、最大文件句柄（只读查询） |
| `/proc/sys/fs/dentry-state` | dentry 状态计数（只读查询） |
| `/proc/sys/kernel/pty/nr` | 当前 pseudo-terminal 数量 |
| `/proc/swaps` | swap 设备/文件、类型、大小、已用、优先级 |
| `/proc/mounts` 或 `/proc/self/mountinfo` | 挂载点、文件系统类型、挂载选项；用于文件系统清单，不等价于使用率 |

PSI 官方格式是 `some/full avg10 avg60 avg300 total`，其中 total 是累计 stall 微秒；缺少 `/proc/pressure` 时应标记 unsupported。

## P1：指定进程/线程

对明确选择的 PID/TID 读取，默认不扫描全系统进程：

| 文件 | 指标 |
|---|---|
| `/proc/PID/stat` | pid、comm、state、ppid、进程组/session、minor/major faults、utime/stime、cutime/cstime、priority、nice、线程数、starttime、vsize、rss、信号、processor、实时调度字段（字段位置须按官方格式解析） |
| `/proc/PID/status` | Name、State、Tgid/Pid/PPid、Uid/Gid、Threads、VmPeak/VmSize/VmRSS/VmHWM、RssAnon/File/Shmem、VmData/Stk/Exe/Lib/PTE/Swap、HugetlbPages、voluntary/nonvoluntary ctxt switches、Capabilities、Seccomp、NSpid 等（字段存在时） |
| `/proc/PID/io` | rchar/wchar、syscr/syscw、read_bytes/write_bytes、cancelled_write_bytes；可能需要权限 |
| `/proc/PID/statm` | size、resident、shared、text、lib、data、dt；精度和语义受内核影响，优先 status/smaps_rollup |
| `/proc/PID/smaps_rollup` | 汇总 RSS/PSS、匿名/文件/共享内存、dirty/clean、swap、referenced 等；读取成本高，按需开启 |
| `/proc/PID/fd` | 打开文件描述符数量；目录遍历有成本，按需开启 |
| `/proc/PID/sched` | 调度延迟、运行统计等；字段和开销受内核配置影响，P2 |
| `/proc/PID/wchan` | 阻塞内核等待点；依赖符号配置且可能受权限影响 |
| `/proc/PID/cmdline` | 命令行展示，不作为数值指标；可能为空 |
| `/proc/PID/task/TID/*` | 线程级 stat/status/io/sched 等，按显式 `--threads` 开启 |

`/proc/PID/stat` 的 `comm` 可能含空格和括号，不能用简单空格切分。使用 PID + starttime 识别实体，避免 PID 复用跨进程计算。

## P2：网络和协议专项

Linux 官方 `/proc/net` 文档列出以下可选内容，是否存在取决于内核配置、网络命名空间和协议模块：

- `tcp`, `tcp6`, `udp`, `udp6`, `raw`, `raw6`, `unix`：socket 表；可统计连接状态和 socket 数，但读取量可能很大。
- `route`, `ipv6_route`, `arp`, `if_inet6`：路由、邻居和地址清单，适合按需快照，不适合高频轮询。
- `snmp`, `snmp6`, `netstat`, `sockstat`, `sockstat6`：协议计数器。
- `dev_mcast`, `igmp`, `igmp6`, `wireless`, `psched`：多播、无线和调度专项数据。
- `rpc/`、`nfs/`：RPC/NFS 客户端或服务端活动，存在时采集。

## P2：文件系统、驱动和调试

| 文件/目录 | 可获取信息 | 策略 |
|---|---|---|
| `/proc/fs/ext4/*` | ext4 allocator 等调试统计 | 按需，文件系统专用 |
| `/proc/sysvipc/*` | SysV IPC msg/sem/shm 资源 | 低频快照 |
| `/proc/modules` | 已加载模块列表及引用计数 | 低频快照 |
| `/proc/devices`、`/proc/partitions` | 设备和分区清单 | 低频快照；不替代 sysfs 属性 |
| `/proc/cpuinfo` | CPU 型号、特性、bogomips 等静态信息 | 启动时一次 |
| `/proc/version`、`/proc/cmdline`、`/proc/bootconfig` | 内核版本、启动参数、boot config | 启动时一次，注意敏感信息 |
| `/proc/zoneinfo`、`/proc/buddyinfo`、`/proc/pagetypeinfo` | 内存 zone、伙伴分配器、页类型 | 调试/低频，输出可能大 |
| `/proc/slabinfo` | slab cache 对象、活动/总数、大小 | 调试/低频，可能需要权限 |
| `/proc/vmallocinfo` | vmalloc 区域 | 调试/低频，输出可能大 |
| `/proc/allocinfo` | 内核分配点累计信息（新内核/配置） | 调试/低频 |
| `/proc/tty/*` | TTY driver、line discipline、serial 状态 | 设备专项 |
| `/proc/rtc`、`/proc/apm` | RTC/APM 信息（若存在） | 平台专项 |
| `/proc/scsi/*` | SCSI/ATA 控制器和设备信息 | 平台专项、低频 |
| `/proc/consoles` | 注册控制台及能力 | 启动/诊断一次 |

## 明确不纳入“只读指标”

- `/proc/sys/*` 主要是可写 sysctl 参数；除少量只读资源计数外，不应由分析工具写入。
- `/proc/PID/mem`、`clear_refs` 等接口可能读取或改变进程状态，禁止自动使用。
- `/proc/kcore`、`pagemap`、`mem`、`maps/smaps` 可能敏感、昂贵或需要特殊权限；只在显式诊断模式下考虑。
- `cmdline`、`environ`、环境和命令行可能包含密码/token，UI/API 默认应脱敏或只显示进程名。

## 实现优先级建议

1. **P0 第一版**：`uptime`、`stat`、`meminfo`、`loadavg`、`vmstat`、`diskstats`、`net/dev`。
2. **P1 第二版**：指定 PID `stat/status/io`、`pressure/*`、`snmp/netstat`、`sockstat`、`interrupts/softirqs`、`swaps`。
3. **P2 第三版**：NFS/RPC、协议 socket 表、文件系统/内存分配器/驱动专项。

每个指标字段必须记录：`metric_id`、procfs 路径、字段名/位置、单位、counter/gauge 类型、最小已验证内核版本、权限条件、读取成本等级、缺失状态和派生公式。

