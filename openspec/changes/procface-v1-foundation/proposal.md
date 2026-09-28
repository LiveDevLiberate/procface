# Proposal

## Why

ProcFace 目前已经确定了嵌入式 Linux 性能分析的核心边界，但现有 OpenSpec change 分散在 CLI、daemon/UI 和单进程 trace 三个草案中，范围相互重叠，无法直接进入实现。需要建立一个统一的 v1 foundation change，冻结首个可运行版本的接口、采集策略、资源边界和发布形态。

## What Changes

- 建立 Rust 实现和 ARM32、ARM64、RISC-V64、x86_64 的静态 musl 构建目标；x86 32 位作为可选目标。
- 实现 Linux 4.19+ 的 P0 procfs 指标采集和统一样本 schema。
- 提供原生 `sample`、`trace`、`daemon`、`capabilities` CLI，不依赖 sysstat 外部命令。
- 提供默认监听 127.0.0.1、Bearer token、CORS 显式配置的 daemon API v1。
- 提供 top 风格的全量当前用户进程轻量采集，以及独立 worker 的单进程 trace；不在后端按 Top-K 截断或按指标排序，前端默认展示前 16 个进程。
- 提供 current、series、SSE stream、JSONL/TSV chunked export、Prometheus `/metrics` 接口。
- 发布独立的单体 HTML 前端，并在 GitHub Pages 固定入口部署匹配的正式产物；支持本地文件显式放行和开发机静态托管。
- 前端通过 capabilities 协商 API 版本，并提交签名版本声明；daemon 接受可信签名且 API 兼容的正式前端，不要求软件版本完全一致。显式开发调试开关允许未签名前端，保留 token、CORS 和 API 兼容检查。
- 发布 HTML 的独立 SHA-256 签名校验清单；前端保存浏览器本地历史并支持导出。
- 诊断构建默认包含 SQLite 编译能力，但持久化仍须显式启用；release 构建不包含 SQLite 和敏感诊断读取器。
- 采用 MIT 许可证。

## Capabilities

### New Capabilities

- `build-release-v1`: Rust/musl 构建矩阵、release/diagnostic 边界和 SQLite 可选构建。

### Modified Capabilities

- `cli`: 将 CLI 行为统一到 ProcFace 原生命令、统一 schema 和时间/格式输出。
- `daemon`: 补充 v1 API、进程采集、trace 隔离、资源预算和版本协商要求。
- `frontend`: 补充单体 HTML 交付、SSE/导出、API 版本检查和进程视图要求。
- `metrics`: 补充 P0 指标、schema、TSV 字段和 Prometheus 映射。
## Impact

- 新增 Rust crate、procfs 解析器、采样调度器、HTTP/SSE 服务、Prometheus 导出器和单体 HTML 前端。
- 新增可选 `sqlite` Cargo feature 和独立诊断构建产物；默认最小构建不引入 SQLite。
- 目标设备需要可读 procfs；daemon 运行时不依赖 BusyBox、systemd、Python 或 Node.js。
- 现有三个重叠 change 应在本 change 实现完成后归档，避免重复 apply。

