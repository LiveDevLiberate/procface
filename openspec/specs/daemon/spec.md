# Daemon 规格

## 目的

提供可选的设备端 HTTP 服务，用于向独立前端和开发机提供性能数据。daemon 只提供 API，不负责网页文件；默认不要求 systemd，也不保存数据。

## Requirements

### Requirement: 运行方式和监听地址

daemon MUST 通过显式子命令启动。默认 MUST 只监听 `127.0.0.1`；局域网监听必须显式指定地址。未启动 daemon 时，CLI MUST 仍可独立运行。

### Requirement: 鉴权

所有设备数据 API 请求 MUST 携带 Bearer token，包括 loopback、capabilities、health 和流式接口。独立前端文件不由 daemon 提供；所有设备数据 API MUST 鉴权。token 通过 `--token TOKEN` 指定；未指定时 MUST 使用操作系统随机源生成 32 字节 token，并在启动 stderr 显示一次。token 只保存在内存中，不写设备存储，也不得放入 URL。

### Requirement: 采集和内存

daemon MUST 在没有浏览器连接时继续按配置采集。默认采样间隔为 1 秒、内存窗口为 60 秒、最大浏览器连接数为 1。`--interval`、`--history-seconds` 和 `--max-clients` MUST 可配置且有上下限。超过连接数时返回 503，不得影响已有连接或采集器。

### Requirement: 持久化

设备持久化默认关闭。daemon MUST NOT 写指标、缓存、日志、pidfile 或 UI 状态。SQLite MUST 作为独立构建产物提供，不得进入默认最小二进制；设备上允许使用 SQLite 构建。启用时必须显式指定数据库路径、大小上限、保留策略和 flush 策略，并在能力接口中报告持久化状态。默认大小上限为 16 MiB，达到上限时 MUST 删除最旧数据；大小上限必须可配置。

### Requirement: API

```text
GET /api/v1/capabilities     指标、构建特性和持久化状态
GET /api/v1/current           当前样本
GET /api/v1/series            有界内存时间序列
GET /api/v1/stream            SSE 实时样本流
GET /api/v1/export            HTTP chunked JSONL/TSV 流
GET /api/v1/health            采集健康状态
GET /metrics                  Prometheus exposition metrics
```

数据 API 只读且有界；唯一允许的控制操作是受鉴权保护、结构化的 trace 生命周期接口。任何接口都不得接受命令或任意文件路径。

### Requirement: Prometheus compatibility

daemon MUST expose `GET /metrics` using the Prometheus text exposition format (`text/plain; version=0.0.4`). Metric names、labels、单位和 counter/gauge 类型 MUST 稳定。该端点用于 Prometheus/Grafana 抓取，不承担 ProcFace 前端的历史查询。ProcFace 自有前端继续使用 `/api/v1/*` JSON/stream 接口。

#### Scenario: Grafana scrape

- **GIVEN** Grafana 使用 Prometheus data source 指向 daemon
- **WHEN** Prometheus 抓取 `/metrics`
- **THEN** daemon MUST 返回合法的 HELP/TYPE 和样本行
- **AND** 认证策略 MUST 与其他设备数据 API 一致

Grafana 专用 JSON API 或 Grafana data source plugin 不属于首版范围。

### Requirement: 发布边界

release 构建 MUST 保留 daemon、普通 procfs 指标和鉴权 API。网页前端作为独立单体 HTML 文件发布，不打包进 daemon；敏感进程读取器和路由 MUST 从 release 产物中移除，只 MAY 在诊断构建中通过显式选项启用。

### Requirement: 进程采集范围

daemon MUST 提供 `--process-scope none|self|pid-list`。默认值为 `self`，扫描当前用户可见的全部进程，但默认只读取与 Linux `top` 对齐的轻量字段（`/proc/PID/stat`、必要的 `status` 字段）。daemon MUST 使用有界的 `--process-result-limit`（默认 256）保留/返回候选结果，并支持 `--process-budget-ms` 预算。`none` 禁用进程采集；`pid-list` 只采集显式 PID。

### Requirement: 流协议

daemon MUST provide /api/v1/stream as Server-Sent Events (SSE) for live browser charts. The stream MUST send normalized samples and explicit connected/heartbeat/error events, and MUST apply backpressure or close a slow client. /api/v1/export?format=jsonl and /api/v1/export?format=tsv MUST use HTTP chunked transfer for workstation capture and MUST NOT require the daemon to retain the full session.

### Requirement: 独立前端连接

The daemon MUST accept API requests from a separately hosted single-file HTML frontend. The frontend supplies the daemon host, port, and bearer token at runtime and uses the `/api/v1` schema; the daemon MUST NOT assume that the page is served from its own origin. CORS MUST be denied by default. LAN or separately hosted frontend access MUST explicitly configure one or more allowed origins (for example `--cors-origin`). Origin allowance MUST NOT bypass bearer authentication.






### Requirement: SSE reconnect and short-window recovery

前端断开后重新连接时 MUST 先通过 `/api/v1/series` 请求短期窗口，再连接 `/api/v1/stream` 接收新样本。daemon MUST 保留不超过配置窗口的样本，不得为重连客户端保存完整历史。SSE MUST 发送 heartbeat 和 error 事件；慢客户端 MUST 施加背压或主动断开。

### Requirement: Prometheus process opt-in

`/metrics` MUST 默认只导出系统级指标。进程指标只有在显式设置 `--prometheus-process` 后才导出；该选项必须在 capabilities 中报告。启用后仍 MUST 遵守当前进程采集范围和最大进程数限制，并使用有限的 `pid`、`comm` labels。

### Requirement: Capabilities response

`GET /api/v1/capabilities` MUST 返回 `schema_version`、ProcFace 版本、API 版本、推荐前端版本、构建 profile、目标架构/系统、功能开关和指标组能力。每个指标组 MUST 提供 `available`、支持的 `metrics` 列表；不可用时 MUST 提供 `reason`。`reason` MUST 使用 `unsupported`、`permission_denied`、`parse_error`、`disabled` 或 `not_compiled`。

示例：

```json
{
  "schema_version": 1,
  "procface_version": "0.1.0",
  "build_profile": "release",
  "target": "aarch64-linux-musl",
  "features": {
    "daemon": true,
    "prometheus": true,
    "prometheus_process": false,
    "sqlite": false,
    "process_trace": false,
    "diagnostic_sensitive": false
  },
  "metric_groups": {
    "cpu": {"available": true, "metrics": ["cpu.usage", "cpu.user"]},
    "psi": {"available": false, "reason": "unsupported"}
  }
}
```

### Requirement: Health response

`GET /api/v1/health` MUST 返回 daemon 状态、uptime、collector 状态、最近采样耗时、late/dropped 计数、内存窗口使用量、客户端数量和持久化状态。顶层 `status` MUST 使用 `ok`、`degraded` 或 `error`：采样延迟、丢样本或部分指标不可用时至少为 `degraded`；采集器停止或无法提供当前样本时为 `error`。

```json
{
  "status": "ok",
  "uptime_s": 12345.6,
  "collector": {"running": true, "interval_s": 1.0, "last_duration_ms": 8.4, "late_samples": 0, "dropped_samples": 0},
  "memory": {"history_samples": 60, "history_limit": 60, "estimated_bytes": 184320},
  "clients": {"active": 1, "limit": 1},
  "persistence": {"mode": "disabled"}
}
```

### Requirement: Current and series API

`GET /api/v1/current` MUST 返回当前 `session_id`、`sequence`、`uptime_s` 和样本数组；默认返回当前可用的 P0 系统指标，并 MAY 通过 `metrics`、`entities` 参数筛选。`GET /api/v1/series` MUST 支持按 `metric`、`entity`、`from_uptime`、`to_uptime` 和 `limit` 查询短期内存窗口，结果按 `sequence` 升序，并返回 `gaps` 和 `truncated` 标记。默认 `limit` 为 120，最大为 3600；超出窗口不得读取设备文件或伪造历史。

### Requirement: SSE stream

`GET /api/v1/stream` MUST 使用 SSE。事件类型至少包括 `connected`、`sample`、`heartbeat` 和 `error`；heartbeat 默认间隔为 15 秒。SSE 只发送连接建立后的新样本，断线恢复由客户端通过 `/api/v1/series` 补齐。慢客户端超过有界缓冲后 MUST 被主动断开，不能导致采集器阻塞。

### Requirement: Chunked export

`GET /api/v1/export?format=jsonl` 和 `format=tsv` MUST 使用 HTTP chunked transfer。JSONL 每行一个统一样本；TSV 第一行 MUST 是固定字段名。接口 MUST 支持有限的 `metrics`、`entities`、`from_uptime`、`to_uptime` 和 `follow=1` 参数。`follow=1` 持续发送未来样本，但 daemon 不得缓存完整会话；客户端断开后必须释放导出资源。查询范围、过滤器数量和并发导出连接必须有界。

### Requirement: Process API

`GET /api/v1/processes` MUST 默认只返回当前用户可见进程，默认结果上限为 256；后端 MUST 按请求的 Top 指标维护有限结果集；前端负责最终展示排序。超过结果上限时 MUST 返回 `truncated: true`，但不得因此停止扫描剩余可见进程。无法读取的 PID 可以跳过，但必须计入诊断统计。进程身份 MUST 同时包含 `pid` 和 `/proc/PID/stat` 的 `starttime_ticks`；规范化 entity 使用 `process:<pid>:<starttime_ticks>`。

单进程接口 MUST 提供：

```text
GET /api/v1/processes/{pid}/capabilities
GET /api/v1/processes/{pid}/current
GET /api/v1/processes/{pid}/series
GET /api/v1/processes/{pid}/stream
```

进程字段分为 `basic`、`extended` 和 `diagnostic` 三层。默认只启用 basic；extended 和 diagnostic 必须显式配置并受采集预算、响应大小、线程数和内存窗口限制。敏感 diagnostic 字段仅在诊断构建并显式开启后可用。PID 退出或被复用时，旧实体必须标记为 `exited`，不得跨实体计算速率。


### Requirement: Top-aligned process collection

普通进程指标 MUST 对齐 Linux `top` 的轻量采集模型：扫描当前用户全部可见 PID，默认只读取 `/proc/PID/stat` 和必要的 `/proc/PID/status` 字段，使用相邻采样计算 CPU 等基础速率，并维护有界 Top-K 结果。扩展 procfs 文件和敏感字段不得在普通进程列表中默认读取。`trace --pid PID` 才允许按配置读取 extended/diagnostic 字段。

### Requirement: Trace isolation

前端切换普通视图与单进程 trace 时，daemon MUST 继续运行系统指标和 top 风格普通进程采集，不得清空或暂停普通历史窗口。trace MUST 使用独立采集任务、预算和扩展字段缓冲区；trace 开始时为选定 PID 建立新的 counter baseline，不得伪造开启前的扩展字段历史。资源不足时 MUST 优先保证系统指标和普通进程采集，trace 可返回 `degraded`、`skipped_expensive` 或 `timed_out`。

### Requirement: Sampling floor and scheduling

所有系统、普通进程和 trace 采样间隔 MUST 不小于 1 秒；更高频率性能分析不属于 ProcFace，建议使用 perf。系统指标采集优先级最高；普通进程扫描超时时跳过本轮；trace 使用 daemon 内独立的低优先级 worker、独立间隔和有界任务队列，不得阻塞系统指标采集、普通进程扫描或 HTTP 发布。trace 间隔同样不得小于 1 秒。

### Requirement: Single active trace

daemon 默认同时只允许一个活动 trace。前端 MUST 显示当前 trace 的 PID、进程身份和状态；活动 trace MUST 持续运行，直到用户显式停止、目标进程退出或 daemon 重启。重复启动请求 MUST 返回 `409 Conflict`，不得隐式替换当前 trace。

daemon MUST 提供受 Bearer token 保护的 trace 生命周期接口：

```text
POST   /api/v1/trace          启动指定 PID 的 trace，可设置 interval_s
DELETE /api/v1/trace          停止当前 trace
GET    /api/v1/trace           查询当前 trace 状态
```

这些接口只接受结构化 PID 和有限配置，不接受命令、路径或 shell 内容。停止 trace 后，普通采集和历史窗口 MUST 保持不变。


### Requirement: SQLite build and failure isolation

Rust Cargo feature 是编译期依赖开关：`sqlite` feature 启用 SQLite 依赖和持久化模块，默认 feature 集合不得包含它；发布时应产生不含 SQLite 的最小二进制和可选的 SQLite 二进制。运行时 `--sqlite PATH` 只对包含该 feature 的二进制有效。

SQLite 数据库损坏、不可打开或写入失败时，daemon MUST 在 stderr 输出明确错误，并在 `/api/v1/health` 和 `/api/v1/capabilities` 报告 `persistence.mode=error`。实时 procfs 采集、短期内存窗口、CLI、JSON/SSE/export 和 Prometheus API MUST 继续工作；只有设备端持久化停止。daemon 不得自动删除损坏数据库或静默创建替代文件。达到数据库大小上限时 MUST 按时间删除最旧数据后继续写入；删除失败时持久化进入 error 状态并保留实时功能。

### Requirement: Frontend compatibility

独立单体 HTML 前端 MUST 声明其支持的 API 主版本范围（例如 `v1` 或 `min=1,max=1`）。daemon capabilities MUST 返回当前 API 主版本和匹配的 `recommended_frontend_version` 和 `api_compatibility`；该字段只提供兼容信息，不要求设备访问 GitHub。前端发现 daemon API 版本不在支持范围内时 MUST 在读取设备数据前停止连接，并显示版本不兼容信息。

每个 GitHub Release MUST 同时发布匹配版本的 daemon 二进制和单体 HTML 前端，版本号保持一致。API 主版本变化时必须发布新的前端兼容范围；同一主版本内的向后兼容字段扩展不得强制旧前端失效。

### Requirement: Version negotiation fields

capabilities MUST 使用独立字段表达版本：

```json
{
  "procface_version": "0.1.7",
  "api_version": "v1",
  "api_compatibility": {"min_major": 1, "max_major": 1},
  "schema_version": 1,
  "recommended_frontend_version": "0.1.7"
}
```

`procface_version` 是软件发布版本，`api_version` 是 HTTP API 主版本，`schema_version` 是统一样本字段版本。三者不得互相替代。

### Requirement: Trace worker isolation

daemon MUST 为当前 trace 使用单独的 worker。trace worker 负责扩展 procfs 读取、解析和序列化前的样本构造；网络发布、SQLite 写入和主采样循环不得在 trace worker 中同步执行。trace worker 必须支持合作式停止、单轮超时和有界内存；停止请求到达后不得启动下一轮读取。trace worker 的异常只影响当前 trace，并在 trace 状态和 health 中报告。
