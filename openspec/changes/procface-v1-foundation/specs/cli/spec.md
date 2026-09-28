# Spec Delta

## ADDED Requirements

### Requirement: Native output contract

CLI MUST 使用 `sample`、`trace`、`daemon` 和 `capabilities` 原生命令，并支持 table、TSV、JSON、JSONL。CLI 的 table、TSV、JSON 和 JSONL 默认 MUST 输出带字段名、指标名、实体名、单位和状态的可读格式；CLI 不直接输出 API 紧凑数组。间隔不得小于 1 秒。

#### Scenario: JSONL sampling
- **WHEN** 用户执行 `procface sample --format jsonl`
- **THEN** stdout MUST 只输出统一样本记录，诊断写 stderr

### Requirement: 本地复用采集核心

`sample`、`trace` 和 `capabilities` MUST 直接复用本地 procfs 采集核心，无需启动或连接 daemon。CLI 与 daemon MUST 使用相同解析器、进程身份和样本模型，避免重复实现采集逻辑。

#### Scenario: 无 daemon 的本地采样
- **WHEN** 用户在未启动 daemon 的 Linux 设备上执行 sample
- **THEN** CLI MUST 直接读取本地 procfs 并按指定格式输出


