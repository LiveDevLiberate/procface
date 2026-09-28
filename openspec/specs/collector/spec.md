# Collector capability specification

## Purpose

定义 procface 在嵌入式 Linux 设备上的 procfs 采集、采样计算和文本输出行为。本文是当前行为的来源规格；未来改动通过 OpenSpec change 提议后合并。

## Requirements

### Requirement: Minimal runtime dependencies

procface MUST run in the target shell environment using procfs, BusyBox-compatible `ash`, `awk`, `sleep`, and shell built-ins. It MUST NOT require Python, Perl, jq, GNU-only utilities, sysfs, systemd, a database, or network access.

#### Scenario: Read-only embedded root filesystem

- **GIVEN** the root filesystem is read-only and the caller is not root
- **WHEN** procface runs a finite collection
- **THEN** it MUST collect readable metrics and write data only to stdout/stderr
- **AND** it MUST NOT create a pidfile, cache, log, or temporary file

### Requirement: Procfs system metrics

The collector MUST support CPU, memory, load/system activity, block I/O, network, and explicitly selected process metrics using procfs files.

#### Scenario: Default collection

- **GIVEN** `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`, and `/proc/uptime` are readable
- **WHEN** the caller runs the default command
- **THEN** the collector MUST emit CPU, memory, load, context-switch, and process-creation metrics
- **AND** it MUST continue when optional disk, network, or process files are unavailable

### Requirement: Explicit sampling interval and count

The command MUST accept a positive integer interval in seconds and a finite count, with defaults of one second and ten windows. Count MUST describe emitted measurement windows and MUST NOT include the initial baseline read.

#### Scenario: Finite sample

- **GIVEN** interval is `1` and count is `2`
- **WHEN** collection starts
- **THEN** it MUST read a baseline, wait for an actual interval, and emit two windows
- **AND** each rate MUST use the measured `/proc/uptime` difference

### Requirement: Correct counter delta handling

The collector MUST calculate rates from counter deltas divided by measured uptime interval. If a counter decreases, it MUST emit `N/A` with a discontinuity status and establish a new baseline; it MUST NOT emit a negative rate or guess a counter width.

#### Scenario: Counter reset

- **GIVEN** a subsequent `/proc/stat`, `/proc/diskstats`, or `/proc/net/dev` counter is lower than its baseline
- **WHEN** the next window is computed
- **THEN** the affected metric MUST be `N/A`
- **AND** the following window MAY report a rate using the new baseline

### Requirement: Selected process collection

The collector MUST inspect only explicitly selected PIDs and MUST identify a process using PID plus `/proc/PID/stat` starttime. It MUST NOT scan every process by default.

#### Scenario: PID reuse

- **GIVEN** the selected PID exits and a new process reuses the PID
- **WHEN** the new starttime differs
- **THEN** the collector MUST mark the entity as replaced and MUST NOT calculate a cross-process delta

### Requirement: Stable TSV output

The TSV format MUST contain `schema_version`, `sample`, `uptime_s`, `interval_s`, `module`, `entity`, `metric`, `value`, `unit`, and `status` columns. Missing values MUST be represented as `N/A`; diagnostics MUST go to stderr.

#### Scenario: Permission-limited metric

- **GIVEN** `/proc/PID/io` is unreadable while `/proc/PID/status` is readable
- **WHEN** process collection runs
- **THEN** memory and readable process metrics MUST be emitted
- **AND** I/O metrics MUST be emitted as `N/A` with `permission_denied`

### Requirement: Bounded resource use

The collector MUST retain only baseline/current snapshots and bounded entity state. It MUST NOT retain all samples in memory, start an unbounded process per metric, or silently truncate configured PID/entity input.

#### Scenario: Long finite collection

- **GIVEN** collection runs for one hour with TSV redirected by the caller
- **WHEN** samples are emitted
- **THEN** the collector's memory usage MUST remain bounded independently of sample count

## Interface

```text
procface [--interval SECONDS] [--count N]
             [--cpu-all] [--disk [NAME,...]] [--net [NAME,...]]
             [--pid PID,...] [--format table|tsv]
             [--proc-root PATH] [--clk-tck N]
```

The exact option spelling and BusyBox applet behavior are implementation details of the first change proposal, but behavior above is normative.


