# Proposal

## Why

当前前端以指标卡片和时间序列为主，用户仍需自行把指标映射到 Linux 子系统和数据流。借鉴 profiler 的组件/接口性能图，在不增加 daemon 架构知识的前提下，让开发者能够按子系统观察组件内部指标和组件之间的接口指标。

## What Changes

- 在总览中增加 Linux 子系统摘要入口。
- 为每个子系统提供独立的性能数据流图：节点显示组件指标，连线显示接口指标。
- 使用前端内置的静态图定义和稳定 metric ID 绑定，不新增 daemon 架构图 API。
- 使用单体 HTML 内嵌 SVG/CSS/JavaScript 实现缩放、节点/连线选择、状态颜色和指标详情。
- 对 unsupported、permission_denied、stale、discontinuity 和缺失绑定提供明确显示。

## Capabilities

### New Capabilities

无。该能力属于独立前端的现有展示能力。

### Modified Capabilities

- `frontend`: 增加 Linux 子系统概览和组件数据流图要求。

## Impact

- 影响 `web/procface-web.html` 及前端浏览器测试。
- 不修改 daemon、API v1、采集器、紧凑协议或指标命名。
- 不增加外部 JavaScript/CSS 依赖；图形采用单体 HTML 内嵌 SVG。
