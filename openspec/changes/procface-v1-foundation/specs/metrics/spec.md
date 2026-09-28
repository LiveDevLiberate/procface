# Spec Delta

## ADDED Requirements

### Requirement: P0 metrics and schema

首阶段 MUST 实现 CPU、内存、负载、uptime、磁盘、网络、VM 和基础进程指标。API 机器可读数据 MUST 使用统一紧凑协议：批次为 `[sequence, uptime, timestamp_unix, group_id, complete, diagnostics, dictionary_delta, samples]`，样本为 `[metric_id, entity_id, value, status_id]`。指标、实体、分组和状态的名称、单位及类型 MUST 通过能力目录或字典解析；指标编号全局稳定、只增不复用，实体编号在 session 内稳定。

当新进程、网卡或磁盘出现时，`dictionary_delta` MUST 先于引用新编号的样本发送。`current`、`series`、`stream` 和 JSON/JSONL 导出 MUST 携带足以独立解码的初始目录及后续字典增量；重连补偿不得引用客户端未收到的实体或指标编号。`complete` 和 `diagnostics` MUST 保留本轮覆盖完整性及跳过、超时和解析错误信息。

CLI 输出使用展开后的可读 schema，不直接暴露 API 数组位置；API 紧凑数组的位置和含义属于协议，API 版本或 schema 版本变化时 MUST 显式变更版本，不得静默改变。

此次紧凑协议迁移 MUST 保持 `/api/v1` 和 API 主版本 1，撤下旧预览 Release，不提供旧预览对象格式的兼容分支。紧凑线格式 MUST 有显式 schema 标识，前端建链 MUST 验证线格式兼容性；旧前端或旧 daemon 不得仅因 API 主版本相同而误判兼容。既有本地历史 MUST 按自身 schema 解码或明确拒绝，不能按新数组格式误读。

#### Scenario: 旧预览客户端建链
- **WHEN** 客户端与 daemon 都声明 API v1，但支持的线格式 schema 不兼容
- **THEN** 建链 MUST 明确拒绝并提示匹配版本，不得尝试解析不兼容数据

#### Scenario: Unsupported procfs field
- **WHEN** 目标内核缺少可选 procfs 文件
- **THEN** 样本 MUST 保留并标记 `unsupported`，不得填零

### Requirement: Prometheus mapping

系统指标 MUST 默认导出为 `procface_` 前缀、带单位的 Prometheus 指标；进程指标默认关闭，只有显式选项才导出。

#### Scenario: Process export opt-in
- **WHEN** 未启用进程 Prometheus 选项
- **THEN** `/metrics` MUST 不返回进程 PID labels

### Requirement: API and human-readable export boundary

API JSON/JSONL MUST 使用上述紧凑协议。TSV API、CLI 的 table/TSV/JSON/JSONL 以及前端下载 MUST 使用带字段名、指标名、实体名、单位和状态的可读格式；Prometheus `/metrics` MUST 保持 Prometheus 文本格式。

#### Scenario: 可读导出
- **WHEN** 用户通过 TSV API、CLI 或前端下载导出采样数据
- **THEN** 输出 MUST 展开指标与实体名称，不要求用户额外查询字典才能理解记录

#### Scenario: 离线解码紧凑历史
- **WHEN** JSON/JSONL API 导出的历史包含运行中新增的实体
- **THEN** 文件 MUST 包含 session、schema 和解码所需目录，在首次引用前提供对应字典定义，离线解码不得依赖 daemon 在线
