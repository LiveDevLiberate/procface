# sysstat 重写分析与实施计划

## 1. 项目边界

上游 sysstat 是 Linux 性能统计工具集合，核心不是单个命令，而是一条数据链：

`/proc + /sys + /dev` → 采集快照 → 两次快照计算增量/速率 → 本地历史存储 → CLI/JSON/CSV/XML/SVG 输出。

主要命令及职责：

| 命令 | 职责 | 重写优先级 |
|---|---|---:|
| `sadc` | 采集快照并写入活动数据文件 | P0 |
| `sar` | 读取实时或历史数据并展示 | P0 |
| `iostat` | CPU、块设备、分区 I/O | P0 |
| `mpstat` | 每 CPU/NUMA CPU 统计 | P1 |
| `pidstat` | 进程/线程 CPU、内存、I/O、上下文切换 | P1 |
| `sadf` | 历史数据转换为 JSON/CSV/XML/SVG | P1 |
| `sa1/sa2` | 定时采集和日报脚本 | P2 |
| `tapestat/cifsiostat` | 磁带/CIFS 专项统计 | P2 |

上游 README 明确列出的覆盖范围包括 CPU、块设备、内存/交换、虚拟内存、进程、IRQ、网络、NFS、套接字、负载、TTY、电源、USB、文件系统和 PSI 等；默认采样间隔为 10 分钟，但支持低至 1 秒。上游采用 GPL-2.0，应在重写前明确是否复用代码、测试数据或仅复刻命令语义。

## 2. 上游结构观察

当前实现是 C + Autotools 的单体式工程：

- `rd_stats.c` 集中读取 `/proc`、`/sys`、传感器和设备信息。
- `activity.c`、`sa.h` 定义活动类型、字段和内存布局。
- `sadc.c` 负责采集循环、文件锁、文件头、记录写入和轮转。
- `sar.c` 负责历史文件读取、时间范围筛选、平均值和报告输出。
- `iostat.c`、`mpstat.c`、`pidstat.c` 各自维护快照、设备/进程列表和格式化逻辑。
- `raw_stats.c`、`json_stats.c`、`xml_stats.c`、`svg_stats.c` 将数据直接渲染成不同格式。
- 大量回归测试通过伪造 `/proc`、`/sys` 树和固定输出文件验证兼容性。

上游的优点是低开销、架构覆盖广、命令语义稳定；主要重写机会是把“读取内核文件”“计算派生指标”“存储”“格式化”“命令参数”拆成稳定接口，减少每个命令重复维护的状态机。

## 3. 重写目标架构

建议采用 Rust 实现守护采集核心和共享库，CLI 先保持 sysstat 命令名与常用参数兼容。目录建议：

```text
sysstat-next/
  crates/
    sysstat-core/       # 时间、计数器、速率、实体 ID、错误模型
    sysstat-source/     # Procfs/Sysfs/Netlink/Proc connector 数据源
    sysstat-model/      # 规范化快照和指标 schema
    sysstat-store/      # WAL/分段文件、压缩、保留策略、版本迁移
    sysstat-format/     # table/json/csv/prometheus 输出
    sysstat-cli/        # sar/iostat/mpstat/pidstat/sadf 入口
    sysstat-agent/      # 长期采集服务和 systemd 单元
  tests/
    fixtures/proc/      # 固定 procfs/sysfs 快照
    golden/             # 可接受的 JSON/表格结果
```

核心接口应类似：

```text
Source::snapshot() -> RawSnapshot
Normalizer::normalize(raw) -> Snapshot
DeltaEngine::between(previous, current) -> Sample
Store::append(sample)
Renderer::render(sample, OutputFormat)
```

采集器只负责读取原始计数器；速率、百分比和平均值统一由 DeltaEngine 计算。所有计数器使用 `u128/u64` 加溢出和重置检测，时间使用单调时钟计算间隔，展示时间另存 wall clock。

## 4. 数据源与指标分层

第一阶段只依赖标准 Linux 接口：

- `/proc/stat`：全局和每 CPU CPU time、上下文切换、进程创建、中断。
- `/proc/meminfo`、`/proc/vmstat`、`/proc/uptime`、`/proc/loadavg`：内存、分页、负载。
- `/proc/diskstats` 与 `/sys/block/*/stat`：块设备 I/O；设备枚举使用稳定的 major/minor 和 sysfs 路径。
- `/proc/net/dev`、`/proc/net/snmp`、`/proc/net/netstat`、`/proc/net/softnet_stat`：网络。
- `/proc/<pid>/{stat,status,io,sched,schedstat,smaps_rollup}`：进程指标。
- `/sys/class/net`、`/sys/class/block`、`/sys/devices/system/cpu`：设备属性、热插拔和 CPU 拓扑。
- `/proc/pressure/*`：CPU、内存、I/O PSI。

每个指标定义应包含：`metric_id`、单位、来源、单调/瞬时属性、实体类型、最小内核版本、权限要求、重置规则。这样可以让新内核字段以插件方式加入，而不改变存储格式主干。

## 5. 存储设计

不要直接复制上游私有二进制布局。建议定义带版本的自描述记录：

- 文件头：magic、schema version、host boot id、HZ、页面大小、架构、创建时间。
- 记录：采样时间、采集耗时、活动 bitset、实体字典增量、压缩后的 typed columns。
- 每个分段文件按天或大小滚动，写入采用临时文件 + `fsync` + 原子 rename。
- 读取器支持 schema 迁移；损坏尾记录可截断恢复。
- 默认保留 30 天，支持按天数、总字节和最小可用空间清理。

JSON/CSV/Prometheus 是导出协议，不作为内部主存储。Prometheus 输出应明确 counter 与 gauge 类型，避免把已经计算过的 rate 再标成 counter。

## 6. CLI 兼容策略

先兼容高频用法，而不是一次性复刻全部历史选项：

1. `iostat -x 1 5 -o JSON`
2. `mpstat -P ALL 1 5 -o JSON`
3. `pidstat -d -r -u 1 5 -o JSON`
4. `sar -u -r -n DEV 1 5`
5. `sar -f <file> -s <time> -e <time> -o json`

使用统一参数解析和统一 schema；表格输出只是一种 renderer。错误码、SIGINT 行为、无权限字段和设备热插拔行为要在兼容测试中固定。

## 7. 分阶段实施

### Phase 0：基线（1 周）

- 固定支持的 Linux 发行版、最低内核和 CPU 架构。
- 收集上游命令样例、`--help`、JSON 输出和真实 `/proc` 快照。
- 确认许可证策略和命令兼容范围。

### Phase 1：最小可用采集器（2–3 周）

- Rust workspace、错误模型、时钟抽象。
- CPU、内存、负载、磁盘、网络 5 类 source。
- 内存中 DeltaEngine。
- `sysstat-next sample --json` 和 `iostat`/`mpstat` 的最小 JSON 输出。

### Phase 2：持久化与 sar（2–3 周）

- 版本化分段存储、轮转、锁和恢复。
- `agent` systemd 服务/定时采集。
- `sar -f` 查询、时间过滤、平均值和 JSON/CSV 导出。

### Phase 3：进程与扩展指标（3–4 周）

- pidstat CPU/内存/I/O/线程。
- IRQ、网络协议、虚拟内存、PSI、文件系统。
- 设备和 CPU 热插拔检测。

### Phase 4：兼容和生态（持续）

- sadf XML/SVG、sa1/sa2、tapestat/cifsiostat。
- Golden tests、模糊测试、性能回归、发行版打包。

## 8. 必须提前解决的技术风险

- `/proc/<pid>/stat` 的 `comm` 可包含空格和括号，不能用简单 split 解析。
- 计数器会回绕、重置或因设备重连改变；设备身份不能只用名称。
- CPU online/offline、容器 PID namespace、权限和 cgroup 会导致实体集合变化。
- 不同架构的 HZ、页大小、字节序和 32 位计数器必须进入样本元数据。
- 采集间隔小于 1 秒时，读取成本和调度抖动会污染结果；需要记录采集耗时并支持跳过慢采样。
- 历史文件写入不能阻塞实时 CLI；Store 应使用单独 writer 或无锁队列。

## 9. 第一批验收标准

- 在 x86_64 Linux 上连续运行 24 小时，无崩溃、无不可恢复坏文件。
- CPU、内存、磁盘、网络的 JSON 数值与 `/proc` 原始计数器推导结果一致。
- 设备热插拔、CPU offline/online、进程退出和计数器重置均不会产生负速率。
- `iostat/mpstat/sar` 在 SIGINT、无权限和空数据场景下有稳定退出码和可读错误。
- 采集器自身 CPU 占用目标低于 1%，常驻内存目标低于 32 MiB（不含历史缓存）。

## 10. 当前建议

先实现“采集核心 + JSON CLI + 版本化存储”，再补表格兼容和长尾指标。这样可以尽快验证数据模型、Linux 接口和长期运行稳定性，同时避免被上游数百个历史选项拖慢。

