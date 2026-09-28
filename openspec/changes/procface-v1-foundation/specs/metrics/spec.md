# Spec Delta

## ADDED Requirements

### Requirement: P0 metrics and schema

首阶段 MUST 实现 CPU、内存、负载、uptime、磁盘、网络、VM 和基础进程指标。所有 API 和 CLI 的机器可读输出 MUST 使用统一紧凑协议：批次为 `[sequence, uptime, timestamp_unix, group_id, samples]`，样本为 `[metric_id, entity_id, value, status_id]`。指标、实体、分组和状态的名称、单位及类型 MUST 通过能力目录或协议文档解析；指标编号全局稳定、只增不复用，实体编号在 session 内稳定。

CLI 的 table、`--expanded` 输出和独立解码工具 MAY 将紧凑记录展开为字段名。紧凑数组的位置和含义属于协议，API 版本或 schema 版本变化时 MUST 显式变更版本，不得静默改变。

#### Scenario: Unsupported procfs field
- **WHEN** 目标内核缺少可选 procfs 文件
- **THEN** 样本 MUST 保留并标记 `unsupported`，不得填零

### Requirement: Prometheus mapping

系统指标 MUST 默认导出为 `procface_` 前缀、带单位的 Prometheus 指标；进程指标默认关闭，只有显式选项才导出。

#### Scenario: Process export opt-in
- **WHEN** 未启用进程 Prometheus 选项
- **THEN** `/metrics` MUST 不返回进程 PID labels
