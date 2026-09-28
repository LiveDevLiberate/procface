# ProcFace 规格入口

采用 OpenSpec `spec-driven` Markdown 工作流。规格内容以中文为主，保留工具要求的 Requirement、Scenario 等结构标记。

## 当前实施计划

统一计划位于 `changes/procface-v1-foundation/`：proposal 说明范围，design 说明架构，specs 定义可验证行为，tasks 跟踪实现。当前仍处于规划阶段，文档通过校验不代表功能已经实现。

`specs/` 与其他三个旧 change 保留历史内容；存在差异时，当前实施以统一计划的明确修订为准，不独立实施旧 change。实现完成后再同步主规格和归档，避免将历史内容误作最新决定。

## 平台与交付

正式目标为 ARM32 hard-float、ARM64、RISC-V64、x86_64，i686 为可选目标。计划最低支持 Linux 4.19，推荐 5.4+，实际目标板兼容性仍需验证。Rust 二进制直接读取 procfs，不依赖 BusyBox 或 sysstat 命令执行采集。

前端为单体 HTML，在 GitHub Pages 固定入口部署并通过 GitHub Release 提供匹配的可下载产物。设备端仅提供 API。

## 架构与验证

`procface-component.puml` 是带角色配色的横向 UML 组件图，描述共享采集核心、CLI、daemon、独立 trace 与浏览器数据流。

规格校验：`openspec validate --type change procface-v1-foundation --strict`。下一阶段使用 `openspec-apply-change` 按统一计划的首个实现里程碑推进。

