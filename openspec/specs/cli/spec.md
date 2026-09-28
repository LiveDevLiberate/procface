# CLI 规格

## 目的

为嵌入式 Linux 提供本地性能分析，不依赖 sysstat 外部命令、daemon 或设备端历史存储。

## Requirements

### Requirement: 统一二进制和特性说明

所有构建的可执行文件都 MUST 使用 `procface` 名称。`procface --help` MUST 列出编译进来的特性、默认值、安全策略以及诊断构建专有且当前不可用的特性。

### Requirement: procfs 采集

CLI MUST 从 procfs 读取已支持的指标。可选文件缺失时仍 MUST 继续采集其他指标；不得用伪造的零值替代 unsupported、permission-denied、parse-error 或 discontinuity 状态。

### Requirement: 采样

CLI MUST 支持默认 1 秒间隔和有限采样次数，任何采样间隔不得小于 1 秒。速率 MUST 使用 `/proc/uptime` 测得的实际间隔计算。计数器下降时 MUST 输出 `N/A` 和 discontinuity 状态，并重新建立基线。

### Requirement: 进程范围

CLI 默认 MUST NOT 采集进程指标。启用 process 指标时，默认范围 MUST 为当前用户可见进程；不得默认扫描所有用户。单进程深度追踪 MUST 显式指定 PID。进程筛选、排序、Top 数量和线程展开属于前端展示行为，不属于 CLI 采集器。

### Requirement: 输出

CLI MUST 支持 table、TSV、JSON 和 JSONL 输出。有限采样使用 JSON 文档；持续采样使用 JSONL，每个样本一行。数据写入 stdout，运行错误写入 stderr；输出必须使用 metrics 规格定义的统一样本 schema，包含稳定的指标 ID、entity、uptime、可选 Unix 时间戳、值、kind 和状态。JSON 和 JSONL MUST 保留计算后的有效精度；table 才进行展示格式化。CLI 不得要求外部 `iostat`、`mpstat` 或 `pidstat` 命令。

### Requirement: ProcFace 原生命令结构

ProcFace MUST 使用自己的命令和参数模型，不复制 `iostat`、`mpstat` 或 `pidstat` 的命令格式，也不要求用户安装 sysstat。系统采集、进程列表和单进程追踪通过 ProcFace 原生子命令表达。

### Requirement: 时间语义

速率计算 MUST 使用相邻采样的 `/proc/uptime` 实际差值。默认输出 uptime；CLI MAY 通过 `--timestamp unix` 增加 Unix 时间戳。时间戳不可用时必须输出 null 和明确状态。CLI MUST NOT 依赖本地化或设备时区；墙上时间不得用于速率计算或唯一排序依据。

#### Scenario: 不可靠 RTC

- **GIVEN** 设备 RTC 未设置、发生回拨或 Unix 时间不可用
- **WHEN** CLI 继续采样
- **THEN** CPU、内存、磁盘和网络速率 MUST 继续使用 uptime 正常计算
- **AND** Unix 时间字段 MUST 为 null 或带不可用状态
- **AND** 不得跨 uptime 回退计算 counter 增量







## Interface

```text
procface [sample|trace|daemon|capabilities] [OPTIONS]
  --interval SECONDS
  --count N
  --metrics GROUP,...
  --format table|tsv|json|jsonl
  --timestamp unix
  --proc-root PATH
```

具体参数范围通过对应的 change proposal 维护。


