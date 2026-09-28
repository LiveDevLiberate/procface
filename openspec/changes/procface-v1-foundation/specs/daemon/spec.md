# Spec Delta

## ADDED Requirements

### Requirement: API v1 data service

daemon MUST 提供 capabilities、health、current、series、stream、export 和 Prometheus `/metrics` 接口，默认监听 127.0.0.1，所有数据接口使用 Bearer token。CORS 默认拒绝，允许来源必须显式配置。

#### Scenario: Unauthorized request
- **WHEN** 请求未携带有效 token
- **THEN** daemon MUST 返回 401 且不得返回设备数据

### Requirement: 全量轻量进程采集

普通模式 MUST 默认采集当前用户授权范围内所有可读进程的轻量 stat/status 信息；不得以固定 256 个进程或 Top-K 截断采集结果，也不得按 CPU、内存或 I/O 在后端排序。读取失败 MUST 跳过相应 PID 并在 stderr 报告；单轮超出采集预算 MUST 跳过该轮并报告覆盖状态，不得把部分结果声明为完整。系统指标 MUST 不等待进程扫描完成。

#### Scenario: 超过历史进程数量限制
- **WHEN** 授权范围内可读进程超过 256 个且本轮在资源预算内完成
- **THEN** daemon MUST 提供全部轻量进程记录，由前端排序、筛选和决定展示数量

### Requirement: 显式前端来源配置

daemon MUST 默认拒绝跨域，并允许显式配置 GitHub Pages 或开发机静态站点的完整 Origin。允许本地文件来源 MUST 使用单独的显式开关，默认关闭；该开关只放行 `Origin: null`，不能将其识别为某个可信本地 HTML 文件，且 MUST 继续验证 token。预检 MUST 只返回协议许可，不返回设备数据。

#### Scenario: 本地文件来源默认被拒绝
- **WHEN** 请求的 Origin 为 null 且未开启本地文件来源开关
- **THEN** daemon MUST 不返回允许该来源的 CORS 响应，即使其他站点已在允许列表中

### Requirement: Single trace worker

daemon MUST 同时只允许一个 trace。trace 使用独立 worker、独立间隔和预算；重复启动返回 409，必须显式停止。采样间隔不得小于 1 秒。

#### Scenario: Trace isolation
- **WHEN** trace 读取高成本 procfs 文件
- **THEN** 普通采集、HTTP 发布和历史窗口 MUST 继续工作

#### Scenario: Trace survives frontend reconnect
- **WHEN** 前端断开后携带有效 token 重新连接
- **THEN** daemon MUST 保持当前 trace 独立运行，并允许前端查询当前 trace 的 PID、进程身份与运行状态及显式停止它

#### Scenario: Switch traced process
- **WHEN** 前端停止当前 trace 并请求追踪另一个进程
- **THEN** daemon MUST 在旧 worker 完成停止后才允许新 trace 启动；处于 stopping 时启动请求 MUST 返回 409，普通采集与历史 MUST 不受影响

### Requirement: Slow client isolation

daemon MUST 使用有界客户端发送队列，网络发送不得阻塞采集；积压超过上限时 MUST 断开慢客户端。重连只补偿仍在内存窗口内的数据，窗口外缺失 MUST 明确标记。

#### Scenario: Client cannot keep up
- **WHEN** 客户端消费速度不足导致发送队列超过上限
- **THEN** daemon MUST 断开该客户端并释放其队列，继续普通采集和当前 trace

### Requirement: Version negotiation

capabilities MUST 返回 `procface_version`、`api_version`、`api_compatibility`、`schema_version` 和推荐前端版本。API 主版本不兼容时，前端 MUST 在数据请求前停止连接。

#### Scenario: Incompatible frontend
- **WHEN** 前端支持范围与 daemon API 范围无交集
- **THEN** 前端 MUST 禁止 current、series 和 stream 请求

### Requirement: 前端发布版本建链检查

daemon MUST 提供 `POST /api/v1/frontend/handshake`。前端 MUST 先读取 capabilities 检查兼容范围，再握手，接受后才开始数据订阅；握手响应 MUST 包含接受结果或可识别的拒绝原因，不得授予超出 Bearer token 的权限。原有脚本导出与 Prometheus 客户端不要求提交 HTML 版本声明，仍须遵守其 token 鉴权与来源规则。

正式前端建链时 MUST 提交前端版本、构建 ID 及内嵌的签名版本声明。该声明 MUST 包含版本、构建 ID 和 API 兼容范围，不包含最终 HTML 文件摘要。daemon MUST 使用受信任发布公钥验证声明签名及字段一致性，并接受 API 版本范围兼容的正式发布版本；不得要求前端与 daemon 软件版本完全一致。检查失败时 MUST 拒绝该前端建链并返回可识别原因。此检查 MUST 不替代 Bearer token 鉴权，也不得被描述为实际运行代码未被修改的证明。

#### Scenario: 发布信息不一致
- **WHEN** 前端提交的版本或构建 ID 与签名版本声明不一致，或声明签名无效
- **THEN** daemon MUST 拒绝前端建链并返回对应错误

#### Scenario: 有效声明不能代替鉴权
- **WHEN** 请求携带有效签名版本声明但未携带有效 token
- **THEN** daemon MUST 返回 401，不返回设备数据

#### Scenario: 软件版本不同但接口兼容
- **WHEN** 正式前端与 daemon 软件版本不同，但声明签名可信且 API 范围兼容
- **THEN** daemon MUST 接受该版本检查，不得仅因软件版本不同拒绝建链

### Requirement: 未签名前端开发调试

daemon MUST 提供默认关闭的显式开发调试开关，允许没有签名版本声明的本地开发前端建链。开发调试 MUST 保留 token、CORS 及 API 兼容检查，并在 stderr、能力响应及前端界面中明确显示已启用；不得因签名校验失败自动进入调试模式。此开关 MUST 不启用敏感 procfs 或 SQLite 能力。

#### Scenario: 显式连接未签名开发前端
- **WHEN** 用户开启开发调试，未签名前端来源被允许、token 有效且 API 兼容
- **THEN** daemon MUST 允许调试建链并明确标记调试状态

#### Scenario: 未开启开发调试
- **WHEN** 未签名前端请求建链且开发调试关闭
- **THEN** daemon MUST 拒绝建链，普通 token 鉴权不得自动豁免版本声明检查

