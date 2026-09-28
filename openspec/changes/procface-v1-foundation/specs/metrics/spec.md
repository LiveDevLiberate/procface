# Spec Delta

## ADDED Requirements

### Requirement: P0 metrics and schema

首阶段 MUST 实现 CPU、内存、负载、uptime、磁盘、网络、VM 和基础进程指标。所有输出 MUST 使用统一样本字段：`schema_version`、`session_id`、`sequence`、`uptime_s`、`timestamp_unix`、`metric`、`entity`、`value`、`unit`、`kind`、`status`。

#### Scenario: Unsupported procfs field
- **WHEN** 目标内核缺少可选 procfs 文件
- **THEN** 样本 MUST 保留并标记 `unsupported`，不得填零

### Requirement: Prometheus mapping

系统指标 MUST 默认导出为 `procface_` 前缀、带单位的 Prometheus 指标；进程指标默认关闭，只有显式选项才导出。

#### Scenario: Process export opt-in
- **WHEN** 未启用进程 Prometheus 选项
- **THEN** `/metrics` MUST 不返回进程 PID labels
