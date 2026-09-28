# 前端规格

## 目的

提供独立发布的极简静态网页，用于连接 ProcFace daemon 实时分析。网页不要求随 daemon 部署，设备不负责保存网页历史，浏览器负责当前会话数据。

## Requirements

### Requirement: 极简视图

UI MUST 只提供三类主要视图：

1. 指标图表：当前值、单位、时间戳和短期趋势；
2. 类似 top 的可排序表：当前用户进程或选定实体；
3. 显式选择 PID 后的单进程详情视图。

首版 MUST NOT 提供 dashboard builder、告警中心、账号系统、日志查看器或任意查询编辑器。

### Requirement: 本地会话历史

前端 MUST 使用 IndexedDB 或等价的浏览器本地存储保存会话样本。daemon 不得依赖长期内存历史。历史按设备和会话隔离。

### Requirement: 断链标记

前端 MUST 记录连接、断开和重连事件。图表 MUST 显示断链间隔，不得跨断链插值。导出文件 MUST 保留断链标记和时间戳。

### Requirement: 导出

前端 MUST 将选定的本地数据导出为 TSV、CSV 和 JSON。导出在浏览器/开发机本地完成，不触发设备写文件。

### Requirement: 鉴权

网页 MUST 在读取设备数据前请求 token，并通过 `Authorization: Bearer <token>` 发送。token MUST 只保存在页面内存中，不得进入 URL、IndexedDB、导出文件或日志。

### Requirement: 静态交付

UI MUST 不依赖 CDN、运行时包管理器、外部字体或其他网络服务；唯一网络依赖是用户配置的 ProcFace API。前端作为独立单体 HTML 文件从项目 GitHub 发布，网页启动时 MUST 允许用户填写 daemon IP、端口和 token。daemon 不负责提供网页文件。GitHub 发布文件 MUST 将 HTML、CSS、JavaScript 和图表逻辑全部内嵌，下载后可离线打开。

### Requirement: 外部监控兼容

Grafana 等外部监控系统 SHOULD 通过 daemon 的 Prometheus `/metrics` 端点接入。ProcFace 前端不依赖 Grafana，也不要求设备运行 Prometheus。

### Requirement: 进程数据展示

前端 MUST 对后端提供的进程样本执行用户筛选、PID 筛选、CPU/内存/I/O 排序和 Top 数量截断；默认显示前 16 个进程。线程展开由前端控制。后端只负责按照采集范围读取和推送原始进程样本。

### Requirement: 实时连接

前端 MUST 使用 /api/v1/stream 的 SSE 接收实时样本，断线后 MUST 自动重连并记录断链区间。前端不得使用原生 EventSource 拼接 token 到 URL；应使用可设置 Authorization header 的 fetch 流式实现。




### Requirement: Schema compatibility

前端 MUST 按 metrics 规格解析统一样本 schema，并根据 `status` 显示 unsupported、permission_denied、parse_error、exited、stale 和 discontinuity。前端不得把缺失值转换为零，也不得跨 discontinuity 或断链区间插值。

### Requirement: API version negotiation

单体 HTML MUST 在连接前读取 `/api/v1/capabilities`，比较自身支持的 API 主版本范围与 daemon 返回的 `api_compatibility.min_major`/`max_major`。仅当两个范围有交集且包含 daemon 当前 `api_version` 时才允许继续请求；版本不兼容时 MUST 禁止后续数据请求，并显示当前 daemon 版本、前端版本和推荐匹配版本；前端不得尝试猜测字段或降级解析未知主版本。

前端 MUST 在单体 HTML 中声明 `supported_api_major_min` 和 `supported_api_major_max`。协商成功后，所有请求固定使用协商出的 `/api/v<major>/` 路径；协商失败时不得尝试其他主版本或猜测字段。
