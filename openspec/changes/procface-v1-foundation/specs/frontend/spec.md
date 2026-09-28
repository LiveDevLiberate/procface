# Spec Delta

## ADDED Requirements

### Requirement: 三标签页与 ASCII 终端风格

前端 MUST 使用“总览 / 进程 / Trace”三个标签页。公共连接栏 MUST 显示设备、连接状态、采样间隔与 uptime，连接成功后收起可重新展开的连接配置。历史会话选择、实时/回看和导出 MUST 放在公共工具栏。

总览 MUST 按 CPU、负载、内存、磁盘和网络分组展示图表，并提供展开更多指标的入口。进程页 MUST 使用紧凑的 top 风格表格，保留筛选、排序和选择 Trace 操作。Trace 页 MUST 展示当前 PID、开始/停止控制、进程曲线和可折叠的详细信息。

界面 MUST 采用等宽字体、黑白灰为主的配色、细线边框，无阴影和圆角；按钮使用类似 `[连接]` 的文本样式，状态以文字标识。图表 MUST 保留细线曲线和坐标，不强制转为字符画。颜色 MAY 辅助区分曲线、错误和断链，但 MUST NOT 作为唯一提示。数值列 MUST 右对齐并保持宽度稳定；实时刷新 MUST 避免先清空再显示造成的空白帧，保留正在查看的 Trace 展开状态。

#### Scenario: 切换标签页
- **WHEN** 用户在总览、进程和 Trace 标签页之间切换
- **THEN** 前端 MUST 只改变展示，普通数据订阅、当前 Trace 和历史保存继续运行，已有历史不得清空

#### Scenario: 实时刷新详细信息
- **WHEN** 新样本到达且用户已展开 Trace 详情
- **THEN** 页面 MUST 保留展开状态，更新数值而不造成整页布局跳动或图表空白帧

### Requirement: Single-file frontend

前端 MUST 作为独立单体 HTML 从 GitHub 发布，HTML、CSS、JavaScript 和图表逻辑全部内嵌，不依赖 CDN。网页启动时 MUST 允许填写 daemon IP、端口和 token。

同一正式 HTML 产物 MUST 在 GitHub Pages 的固定入口部署，并保留按版本下载的 Release 产物。在线入口更新后 MUST 继续执行 API 兼容检查；无法兼容时提供推荐版本信息。页面 MUST 支持本地文件和开发机静态 HTTP 托管入口，不得要求用户关闭浏览器安全机制。

#### Scenario: Offline frontend
- **WHEN** 用户离线打开 HTML 文件
- **THEN** 页面 MUST 能加载并显示连接配置界面

#### Scenario: 浏览器阻止设备连接
- **WHEN** 页面因跨域、本地网络权限或混合内容限制无法访问 daemon
- **THEN** 页面 MUST 显示连接失败及可能的配置检查项，提供本地文件显式放行或开发机静态托管说明；浏览器未提供具体原因时不得伪造确定诊断

### Requirement: 前端进程展示

前端 MUST 对普通采集提供的全部轻量进程记录执行排序、筛选和展示；默认显示前 16 个进程，并允许查看其他进程。展示数量不得改变后端采集范围。当前轮覆盖不足或数据陈旧时 MUST 明确提示。

#### Scenario: 查看默认列表之外的进程
- **WHEN** 用户扩大展示范围或筛选不在前 16 位的进程
- **THEN** 前端 MUST 能从已接收的完整轻量记录中展示目标进程，不要求启动 trace 才能发现该进程

### Requirement: API negotiation and session history

前端 MUST 先读取 capabilities 并检查 API 主版本；兼容后使用 SSE 接收实时样本，使用 IndexedDB 保存会话历史，断链时标记 gap，导出不得跨 gap 插值。

#### Scenario: Connection gap
- **WHEN** SSE 连接中断后恢复
- **THEN** 前端 MUST 记录断链区间并通过 series 补齐短窗口

#### Scenario: History no longer available
- **WHEN** 重连后断链区间的数据已经超出 daemon 内存窗口
- **THEN** 前端 MUST 保留无法补齐区间的 gap 标记，不得插值或伪造样本

### Requirement: 前端发布身份与校验边界

正式前端 MUST 在初次连接及重新建链时提交发布版本、构建 ID 和内嵌的签名版本声明，并在 daemon 接受建链且 API 兼容后开始数据订阅。离线使用 MUST 不依赖从 GitHub 获取声明。未签名开发前端仅可在 daemon 显式开启开发调试时建链，界面 MUST 持续显示调试状态。前端 MUST 显示建链拒绝原因。界面和文档 MUST 区分发布文件完整性、声明版本被接受及 token 鉴权，不得将自报摘要或声明校验成功显示为当前运行代码未经修改。

#### Scenario: 发布版本检查失败
- **WHEN** daemon 拒绝前端发布信息或 API 版本范围不兼容
- **THEN** 前端 MUST 停止数据订阅并显示原因及可用的推荐版本信息

### Requirement: Reconnect to active trace

前端 MUST 在重新连接后查询并显示 daemon 当前 trace 的 PID、进程身份与运行状态，允许用户显式停止当前 trace，再选择其他进程启动 trace；不得因重连自动替换当前 trace。

#### Scenario: Stop and switch after reconnect
- **WHEN** 用户重新接入并选择停止当前 trace、切换其他进程
- **THEN** 前端 MUST 等待 daemon 确认旧 trace 已停止后才启动新 trace；stopping 期间 MUST 显示停止中，普通视图与已接收历史 MUST 保留
