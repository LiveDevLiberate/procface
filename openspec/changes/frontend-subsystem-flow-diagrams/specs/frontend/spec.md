# Spec Delta

## ADDED Requirements

### Requirement: Linux 子系统性能数据流图

前端 MUST 提供 Linux 子系统级性能概览，并允许进入单个子系统的数据流图。概览只显示子系统状态和少量摘要指标；子系统图 MUST 使用节点表示组件、使用有方向连线表示组件之间的性能接口。节点内部显示组件指标，连线显示接口指标，不能把所有指标堆叠在同一层级。

#### Scenario: 总览显示子系统
- **WHEN** 前端已完成 capabilities 协商并收到系统样本
- **THEN** 前端显示可用的 Scheduler、Memory、VFS/Filesystem、Block I/O、Networking、IRQ/SoftIRQ 和 Process 子系统入口，并为不可用子系统显示状态原因

#### Scenario: 进入子系统图
- **WHEN** 用户选择 Networking 子系统
- **THEN** 前端打开 Networking 数据流图，显示网络组件节点、组件内部指标和组件间接口指标，并保留总览与历史会话状态

### Requirement: 前端静态图定义

组件、连线、布局、显示名称和 metric ID 绑定 MUST 由单体 HTML 前端静态维护。daemon MUST NOT 提供或解释组件图定义；前端 MUST 只使用统一 metrics schema 中的 metric、entity、unit 和 status 填充图形。

#### Scenario: 后端不提供架构图
- **WHEN** daemon 只返回标准指标和 capabilities，不返回架构图描述
- **THEN** 前端仍能渲染内置子系统图，并根据可用 metric 显示数据或状态占位

#### Scenario: 新增前端图布局
- **WHEN** 发布新版本前端增加 VFS 子系统图布局
- **THEN** 不需要升级 daemon 或修改 API，只要绑定的 metric ID 在兼容范围内即可运行

### Requirement: 节点和接口指标状态

图中每个节点和连线 MUST 显示当前绑定指标的值、单位和状态；绑定多个指标时 MUST 显示摘要值并允许查看详情。unsupported、permission_denied、parse_error、stale、discontinuity、exited 和 incomplete MUST 使用可区分的文字或颜色状态，缺失值不得渲染为零。图例 MUST 说明颜色含义，颜色不能是唯一状态信息。

#### Scenario: 接口吞吐显示
- **WHEN** Block I/O 图的 VFS-to-Block 连线绑定 `disk.read_bytes_per_second` 和 `disk.write_bytes_per_second`
- **THEN** 连线显示当前吞吐值和 bytes/second 单位，点击后显示绑定指标的短期趋势

#### Scenario: 指标不支持
- **WHEN** 组件绑定的 metric 状态为 `unsupported`
- **THEN** 组件保留在图中但显示 unsupported 和原因，不显示伪造的数值或零值

### Requirement: 静态图交互与资源边界

图形 MUST 内嵌在单体 HTML 中，不依赖 CDN、图形库或 daemon 新接口。前端 MUST 支持节点/连线选择、返回子系统总览、缩放或等价的可读性操作，并复用现有 IndexedDB、断链 gap、历史导出和 SSE 订阅。切换图形视图不得停止采样、清除历史或停止 Trace。

#### Scenario: 图形视图切换
- **WHEN** 用户从子系统图切换到指标图表或 Trace
- **THEN** 当前 SSE 连接、普通历史和活动 Trace 保持不变，返回数据流图时保留上次视图位置或默认布局

#### Scenario: 单体 HTML 离线打开
- **WHEN** 用户直接打开下载的单体 HTML
- **THEN** 图形布局、节点名称、图例和无数据占位可以正常显示；只有连接 daemon 和实时值需要网络访问
