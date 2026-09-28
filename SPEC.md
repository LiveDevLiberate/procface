# 嵌入式 Linux 性能分析工具规格

- 状态：Draft 0.4
- 规格标准：ISO/IEC/IEEE 29148:2018（轻量裁剪版）
- 目标：嵌入式 Linux 性能分析，低依赖、按需运行、低扰动。
- 已确认约束：尽量只依赖 procfs 和 BusyBox 中的命令。
- 实现方向：BusyBox ash 兼容的 shell 脚本，awk 负责解析与计算；不引入独立编译型采集器。具体 applet 可用性须在目标设备验证。
- 本文取代 REWRITE_PLAN.md 中较宽的重写路线；后者仅保留作为早期分析背景。

## 0. 规格编写约定

本项目采用 **ISO/IEC/IEEE 29148:2018** 作为需求规格的主标准。该标准用于组织利益相关者目标、系统范围、约束、功能需求、质量需求、接口需求、验证方法和追踪关系；本文件按嵌入式工具规模做轻量裁剪，不声称完成该标准的完整过程合规。

需求使用唯一 ID，并采用以下规范性词语：

- **MUST**：首版验收必须满足。
- **SHOULD**：默认应满足；若目标板约束导致无法满足，必须记录例外和理由。
- **MAY**：可选能力，不作为首版验收条件。
- **MUST NOT**：明确禁止的行为。

这些词语按 RFC 2119 的常用含义解释。每条 MUST/SHOULD 需求都应有验证方法：`T` 测试、`I` 检查、`A` 分析或 `D` 演示。验收场景使用 Given/When/Then 表达，但不引入 Gherkin 运行时依赖。

需求 ID 范围：`STA` 利益相关者目标，`SYS` 系统需求，`FUN` 功能需求，`PER` 性能/资源需求，`INT` 接口需求，`CON` 约束，`VER` 验收需求。

### 0.1 顶层追踪

| 目标 | 需求覆盖 | 验证 |
|---|---|---|
| 嵌入式设备低扰动分析 | SYS-01、PER-01、PER-02 | T/A |
| 只依赖 procfs 和 BusyBox | CON-01、CON-02、INT-01 | I/T |
| 可定位 CPU、内存、I/O、网络和进程问题 | FUN-01～FUN-05 | T/D |
| 现场可通过串口/SSH 获取结果 | FUN-06、INT-02 | D/T |
| 结果可复核且不伪造数据 | SYS-02、FUN-07 | T/A |

## 1. 首版范围

首版提供系统概览、指定进程分析和有限时长的文本记录。默认只读 procfs 并写 stdout，不写设备文件、不启动后台服务、不需要 root。

| 模块 | 数据源 | 首版指标 |
|---|---|---|
| CPU | /proc/stat | 总体及每 CPU 的 user、nice、system、idle、iowait、irq、softirq、steal 百分比 |
| 内存 | /proc/meminfo | MemTotal、MemFree、MemAvailable、Buffers、Cached、SwapTotal、SwapFree |
| 系统活动 | /proc/loadavg、/proc/stat | load1/5/15、running/blocked、上下文切换和进程创建速率 |
| 磁盘 | /proc/diskstats | 读写请求/s、读写字节/s、读写 await、平均队列长度、busy 百分比 |
| 网络 | /proc/net/dev | 接口收发字节/s、包/s、错误和丢包增量 |
| 指定进程 | /proc/PID/stat、/proc/PID/status、/proc/PID/io | CPU tick/s、VmRSS/VmSize、缺页/s、上下文切换/s、读写字节/s |

模块按需开启。默认仅 CPU 总体、内存、负载与系统活动；每 CPU、磁盘、网络和指定进程由参数开启。首版不扫描全部进程，不扫描线程。

不纳入首版：sysfs、设备拓扑和温度频率、PSI、cgroup、perf/eBPF/ftrace、远程服务、Web UI、告警、JSON、压缩、自定义二进制格式、历史数据库、systemd 集成及完整 sysstat 命令兼容。磁盘可按名称筛选；不借助 sysfs 识别父设备/分区，不把所有行相加作为总磁盘吞吐。

## 2. 最小运行依赖

- BusyBox ash：shell 控制、参数处理、信号处理。
- BusyBox awk：文件解析、状态保存、增量及格式化。
- BusyBox sleep：采样等待；首版只要求整数秒。
- BusyBox date：可选展示时间；缺失时仍能根据 uptime 输出。
- shell 内建 printf、read、test 等；不依赖 Bash 专有语法。

不依赖 Python、Perl、jq、GNU 专有命令、编译器、包管理器、联网或额外动态库。BusyBox 可以裁剪 applet，不能仅凭“安装了 BusyBox”认定依赖满足。实现前须用目标设备验证命令、选项和 awk 能力；缺少必需项时启动失败并给出具体名称。不强制要求 awk system()、外部排序或每条记录启动一个进程。

## 3. CLI 契约

暂定入口为 procface：

```text
procface [--interval SECONDS] [--count N]
             [--cpu-all] [--disk [NAME,...]] [--net [NAME,...]]
             [--pid PID,...] [--format table|tsv]
             [--proc-root PATH] [--clk-tck N]
```

- interval：正整数秒，默认 1。
- count：输出的采样窗口数，默认 10；0 表示持续运行，Ctrl-C 结束。
- 首先读取基线，等待一个 interval 后输出第一个窗口；count 不包含基线。
- 未指定模块时采集默认指标；可选模块作为附加项。
- table 为默认输出，适配普通终端，无颜色和光标控制。
- tsv 用于记录和后处理；首版不提供 JSON，避免新增序列化依赖和复杂转义。
- proc-root 默认为 /proc，用于注入测试快照。
- clk-tck 为可选正整数，只影响进程 CPU 百分比换算，不能猜测为 100。
- 所有诊断信息进入 stderr，不混入数据输出。

用例：

```sh
procface --interval 1 --count 10
procface --interval 2 --count 30 --disk mmcblk0 --net eth0
procface --pid 123 --interval 1 --count 60 --format tsv > /tmp/capture.tsv
```

输出目录由调用者选择；/tmp 可能不是 tmpfs。重定向到持久存储属于用户明确选择，工具不自动轮转或清理文件。持续运行并重定向会持续增长，应使用有限 count 控制采集量。

## 4. 采样与计算语义

### 4.1 时间与误差

每轮读取 /proc/uptime 作为设备运行时间基准，用相邻实际读数之差计算每秒速率，不用 sleep 参数充当实际间隔。uptime 的精度和包含休眠的语义需在报告中说明；首版不测量亚秒尖峰或硬实时延迟。

记录采集起止 uptime、实际窗口间隔和样本序号。各 proc 文件顺序读取，样本不是原子快照。sleep 和解析会引入漂移，首版不承诺绝对定时或不丢样。实际间隔明显超过请求间隔时标记 delayed；初始阈值为大于请求间隔的 1.5 倍。

uptime 缺失、解析失败或间隔非正时不计算速率，报告错误。日期为可选标签，不能用于速率计算或样本排序。

### 4.2 CPU

分母为相邻 CPU tick 总量增量，不重复加入已包含在 user/nice 中的 guest/guest_nice。总体和每 CPU 独立计算。分母为零时百分比为 N/A；字段下降时标记 discontinuity，本窗口不计算该 CPU 的百分比，下一窗口重新建立可用增量。iowait 不能等同于磁盘饱和。

### 4.3 内存

保持 proc 字段名称和定义；以文件标示的 kB 换算为字节。缺少 MemAvailable 时输出 N/A，不自行用 free+cached 代替。若显示 used，定义为 MemTotal-MemAvailable。无 swap 与无法读取 swap 字段必须区分。

### 4.4 磁盘和网络

速率使用计数器差值/实际间隔。diskstats 的 sector 按 512 字节换算；await 为相应请求耗时增量/完成请求数增量，无完成请求时为 N/A。平均队列长度使用 weighted I/O time 增量/间隔；busy 百分比使用 I/O time 增量/间隔，不能作为并行设备饱和度结论。

磁盘用 major:minor 和名称作为观察键，网络用接口名。检测到消失再出现时重建基线；同名实体在两个采样之间被替换且计数器继续增长的情况无法可靠识别，应列为已知限制。

### 4.5 进程

只读取指定 PID；以 PID 和 stat 的 starttime 识别进程。stat 的 comm 可含空格及括号，不能直接对整行按空格切分。RSS/虚拟内存读取 status 的 VmRSS/VmSize，避免依赖 page size 换算。CPU 默认报告 user/system tick/s；提供 clk-tck 后才输出单核为 100% 的 CPU 百分比，多线程进程可以超过 100%。未提供时百分比为 N/A。

进程退出为 exited，新启动时间为 replaced；不跨不同进程计算增量。io 无权限时仍输出可读的 CPU、内存字段。

### 4.6 计数器精度

计数器下降视为 discontinuity，输出 N/A 并重新建立基线，不猜测回绕位宽。由于常见 awk 数值表示存在大整数精度边界，实现不得直接将任意 64 位累计值转换为浮点后相减：原始值按十进制字符串保存，以精确十进制比较/减法取得增量后换算速率。实现和测试必须覆盖超过 2^53 的累计值及很小的增量。

## 5. 输出契约

TSV 采用固定长表，字段为：

```text
schema_version  sample  uptime_s  interval_s  module  entity  metric  value  unit  status
```

上列字段实际以 TAB 分隔，首行是表头。schema_version 初始为 1。每行表示一个实体的一个指标；缺失值写 N/A，状态为 unsupported、permission_denied、read_error、discontinuity 或 exited 等。正常状态为 ok，迟到窗口为 delayed；若同时有字段错误，优先保留错误状态。每轮还输出 collector 模块的 duration_s 指标以记录采集耗时。

字段值中的反斜线、TAB、CR、LF 分别转义为 \\、\t、\r、\n；不得破坏行结构。数值使用点作为小数分隔符，不使用千分位。原始计数器可作为 *_total 指标随选中模块输出，以便复核公式；字段清单与单位在实现前建立单独指标表。

不建立内部历史存储。TSV 可直接保存在设备指定路径，或由开发机通过现有 SSH 重定向保存。文件截断时最后一条不完整行可由分析方忽略；没有持久化、断电恢复或无损流传输承诺。

## 6. 错误与资源约束

- 单个模块/字段不可用时继续其他采集，明确状态，不能填 0 伪装有效数据。
- 相同持久性错误只在首次出现和状态变化时输出 stderr，避免刷屏。
- 参数错误退出码 1，必需命令或基准数据源不可用为 2，检测到的输出 I/O 错误为 3；正常完成为 0，SIGINT 为 130。SIGPIPE 的具体行为须在目标 ash 上验证并记录。
- 只保存前后快照及有界实体状态，不保留整个会话。实体数量、PID 参数数和输入文件大小的限制需在实现前根据板卡确定；超限必须报错，不静默截断。
- 默认无文件写入、无后台任务、无全进程周期扫描。不声称对被测系统零影响。
- 性能验收采用目标板实测，而非预设通用 1% CPU/8 MiB 阈值。记录默认模式和开启磁盘/网络/PID 模式的 CPU 时间、峰值 RSS、采集耗时、输出量，以及每窗口外部进程启动次数。

## 7. 测试与验收

1. 在目标 BusyBox ash 下运行，无 Bash/GNU 专有依赖；记录 BusyBox 版本和可用 applet。
2. 默认执行输出 10 个窗口，所有速率使用实际 uptime 间隔。
3. 对固定 procfs 快照序列验证 CPU、磁盘、网络、进程公式及 TSV 字段；测试夹具调度由开发环境提供，无需设备安装测试框架。
4. 覆盖计数器超过 2^53、下降、首轮基线、CPU 变化、设备消失/再出现、进程退出/PID 复用。
5. 覆盖缺失 MemAvailable、无 swap、缺少 diskstats、PID io 不可读和 stat 中含空格/括号的 comm。
6. 在只读根文件系统、普通用户条件下运行，除显式 stdout 重定向外无文件写入。
7. 有限采集及持续运行一小时后，内存不随样本数量增长；Ctrl-C 能退出并结束自身启动的子进程。
8. 在目标板验证慢输出、存储写满、信号和管道关闭，记录实际错误行为；性能分析报告包含采集开销。

## 8. 待确认

- 目标板 CPU/架构、内存、Linux 版本及 BusyBox 版本/配置。
- 是否确实具备 ash、awk、sleep；date 为可选。
- 主要分析 CPU、内存、I/O、网络还是具体业务进程。
- 可接受的采样间隔、采集时长和资源开销。
- 是否可提供内核 USER_HZ（CLK_TCK）以展示进程 CPU 百分比；默认保留 tick/s。

在这些信息缺失时，按整数秒采样、按需模块、默认 10 个窗口、不写文件、不要求 root 的最小方案推进规格。

> 迁移说明：项目现采用 OpenSpec spec-driven 目录作为规范来源。请优先阅读 openspec/project.md 和 openspec/specs/collector/spec.md；本文件保留为历史草案。

