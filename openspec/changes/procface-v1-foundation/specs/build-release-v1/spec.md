# Spec Delta

## Purpose

定义 ProcFace 在嵌入式目标上的构建产物、兼容性边界和诊断能力隔离，确保最小发布版本可部署且诊断版本可扩展。

## ADDED Requirements

### Requirement: Rust target matrix

release MUST 提供 ARM32 hard-float、ARM64、RISC-V64 和 x86_64 的静态 musl 产物；x86 32 位 MAY 作为可选产物。最低支持 Linux 4.19，推荐 Linux 5.4+。

#### Scenario: Target artifact
- **WHEN** 发布 ARM64 版本
- **THEN** 产物 MUST 使用 `aarch64-unknown-linux-musl` 并可在目标 Linux 4.19+ 上启动

### Requirement: Build feature boundary

默认 release MUST 不包含 SQLite 和敏感诊断读取器。诊断构建 MUST 默认包含 SQLite 编译能力，但数据库写入和敏感读取仍须显式启用。SQLite 使用 Cargo feature 编译开关。

#### Scenario: Diagnostic build
- **WHEN** 诊断构建执行 `--help`
- **THEN** MUST 报告 SQLite 能力已编译，但持久化默认关闭

### Requirement: 前端发布文件完整性

发布流程 MUST 为最终单体 HTML 生成 SHA-256 清单及清单签名，清单 MUST 关联前端版本、构建 ID、文件名、文件摘要及支持的 API 版本范围。发布文档 MUST 提供通过可信发布公钥在打开 HTML 前验证签名与文件摘要的方法；私钥 MUST 不进入前端或 daemon 产物。校验失败的文件 MUST 不作为可信发布产物使用。

构建流程 MUST 先生成不含最终文件摘要的签名版本声明并内嵌到 HTML，再对最终 HTML 字节生成外部签名校验清单，避免自引用。GitHub Pages 与对应 Release MUST 使用字节一致的 HTML 产物，不得在生成摘要后重新注入版本或修改页面。

#### Scenario: 双入口发布一致
- **WHEN** 同一正式版本发布到 GitHub Pages 和 GitHub Release
- **THEN** 两处 HTML MUST 匹配外部签名清单中的同一摘要，并携带同一签名版本声明

#### Scenario: 发布文件被修改
- **WHEN** 下载的 HTML 与签名清单内摘要不匹配
- **THEN** 独立校验流程 MUST 报告失败，不得将该文件标记为完整性校验通过

#### Scenario: 清单被替换
- **WHEN** 清单签名无法通过可信发布公钥验证
- **THEN** 独立校验流程 MUST 拒绝该清单，即使文件与其中摘要一致

### Requirement: MIT license

所有发布二进制和前端包 MUST 附带 MIT 许可证文本。

#### Scenario: Release package
- **WHEN** 用户下载 release 包
- **THEN** 包内 MUST 包含 `LICENSE` 文件
