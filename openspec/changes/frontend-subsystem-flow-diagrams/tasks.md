# Tasks

## 1. 图模型与基础渲染

- [x] 1.1 定义前端子系统、节点、连线和 metric 绑定数据结构，并验证所有绑定使用现有稳定 metric ID
- [x] 1.2 实现无外部依赖的 SVG 图渲染、状态图例、缺失占位和基础缩放，验证离线打开不报错
- [x] 1.3 实现总览子系统入口和单个子系统图切换，验证切换不重置 SSE、IndexedDB 历史或 Trace 状态

## 2. 指标绑定与交互

- [x] 2.1 为 Scheduler、Memory、VFS/Filesystem、Block I/O、Networking、IRQ/SoftIRQ 和 Process 建立静态图定义，并验证无 daemon 架构图请求
- [x] 2.2 实现节点/连线主指标摘要、单位、状态文字和颜色图例，验证 unsupported、permission_denied、stale、discontinuity 和 incomplete 不显示为零
- [x] 2.3 实现节点/连线点击到既有指标图表和详情面板，验证多指标绑定、实体筛选和历史 gap 保持一致
- [x] 2.4 增加键盘焦点、aria-label 和文字状态，验证颜色不是唯一可用信息

## 3. 验证与发布

- [x] 3.1 增加浏览器布局、离线、断链、历史和 Trace 保持测试，验证单体 HTML 不引入 CDN 或运行时依赖
- [x] 3.2 运行前端回归、`cargo test --locked` 和打包检查，验证 daemon/API/紧凑协议没有被修改
- [x] 3.3 验收后将行为同步到 `openspec/specs/frontend/spec.md`，并用 `openspec validate frontend-subsystem-flow-diagrams --strict` 验证
