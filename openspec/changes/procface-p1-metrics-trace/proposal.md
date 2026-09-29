# Proposal

## Why

当前 ProcFace 已能采集 P0 procfs 指标并提供单进程 Trace，但 `capabilities` 已探测而未发布的 PSI、中断和 softirq 数据会让用户误以为系统没有压力指标；Trace 也主要是原始文本，难以比较内存、调度、线程和 I/O。现在补齐一组低频、高诊断价值的 P1 数据，可以让“总览发现异常 -> 选择进程 -> Trace 对齐分析”形成完整闭环。

## What Changes

- 发布 CPU、内存、I/O PSI 指标，并保留内核缺失、权限和解析状态。
- 发布 `/proc/interrupts` 与 `/proc/softirqs` 的累计值和速率。
- 将进程 `status`、`smaps_rollup`、`sched`、线程 stat 和 FD 结果转换为结构化 Trace 字段，同时保留原始文本诊断能力。
- 前端按压力、内存、调度、I/O、文件描述符和线程分组展示 Trace，并与系统样本使用相同 uptime 轴。
- 保留现有 API v1、紧凑字典、历史 IndexedDB、断链 gap 和权限/成本状态语义。

## Capabilities

### New Capabilities

- 无。本变更扩展已有指标、daemon 和前端能力。

### Modified Capabilities

- `metrics`: 增加 P1 系统压力、中断/softirq 与结构化 Trace 指标要求。
- `daemon`: 增加 P1 采样组、Trace 字段和预算/权限状态要求。
- `frontend`: 增加 P1 图表、Trace 分组和系统/进程时间线对齐要求。

## Impact

- 影响 `src/collector.rs`、`src/parsers.rs`、`src/trace.rs`、`src/metric-catalog.tsv`、`src/compact.rs`、`src/daemon.rs` 和 `web/procface-web.html`。
- API 仍为 v1；新增指标进入能力字典，不改变既有字段含义。
- 需要增加模拟 procfs、daemon API、紧凑协议和浏览器测试；不新增运行时依赖。
- 完整 `smaps`、每个 FD 路径、全部网络协议表和高频全线程深度采样不在本变更范围内。
