# procface

嵌入式 Linux 性能分析工具，目标是通过 procfs 和 BusyBox 基础命令，以低扰动方式进行现场分析。

## 约束

- 目标环境是裁剪版嵌入式 Linux。
- 正式目标架构：ARM32 hard-float、ARM64、RISC-V64、x86_64；x86 32 位列为可选目标。所有架构支持必须通过交叉编译和目标板实测确认。
- 最低支持 Linux 4.19，推荐 Linux 5.4 或更高；不保证 Linux 2.6 等过老内核。procfs 能力必须运行时探测。
- 运行时优先只依赖 `/proc`、BusyBox `ash`、`awk`、`sleep` 和 shell 内建能力。
- 交付形态为 Rust 二进制；release 优先使用静态 musl，不要求目标设备安装 Rust、Python、Node.js 或额外运行时。构建必须明确 libc、链接方式和最低内核 ABI。正式 Rust 目标为 `armv7-unknown-linux-musleabihf`、`aarch64-unknown-linux-musl`、`riscv64gc-unknown-linux-musl` 和 `x86_64-unknown-linux-musl`；`i686-unknown-linux-musl` 为可选目标。
- BusyBox 版本暂未锁定，必须建立目标设备版本矩阵并验证 ash/awk/sleep 行为；BusyBox 不是 Rust daemon 的运行时依赖。
- 系统进程列表默认仅包含当前用户可见的进程；禁止默认扫描全部用户进程。单进程深度追踪必须显式指定 PID。
- 默认只写 stdout/stderr；不要求 root，不依赖 systemd、Python、jq、sysfs 或数据库。设备端持久化默认关闭；SQLite 仅作为显式可配置的独立构建能力，默认关闭。
- 首版支持 table、TSV、JSON 和 JSONL，使用 ProcFace 原生命令和参数，不复制 sysstat 命令格式。
- 所有指标缺失、权限不足、解析失败和计数器重置都必须显式表达，不能填 0 伪装有效数据。
- 命令统一使用 `procface`；构建能力通过 `--help` 的特性清单公开，不通过不同命令名区分版本。
- 进程深度诊断和敏感信息读取属于诊断构建能力，不进入面向终端用户的 release 软件包。
- release 构建保留 daemon、普通 procfs 指标和鉴权 API；单体 HTML 前端独立发布，敏感诊断读取器从 release 产物中移除。
- 不复制 `iostat`、`mpstat`、`pidstat` 的命令格式，不依赖或调用 sysstat 外部命令。
- 前端负责本地历史数据和 TSV/CSV/JSON 导出；许可证采用 MIT。

## 当前基线

- 详细能力规格：`openspec/specs/collector/spec.md`
- 原始调研与宽范围计划：`REWRITE_PLAN.md`
- 早期单文件规格：`SPEC.md`，后续以 OpenSpec 目录下的规格为准。

## 变更流程

每项实现或行为变化都在 `openspec/changes/<change-name>/` 下创建：

1. `proposal.md`：说明为什么改、改什么和影响范围。
2. `specs/`：描述目标行为的增量规格。
3. `design.md`：跨模块或有技术取舍时记录实现方案。
4. `tasks.md`：可执行的实现和验证清单。

变更完成并验证后，将最终行为同步到 `openspec/specs/`，再归档变更。

- 已确认认证：默认监听 127.0.0.1；局域网监听需显式设置。网页输入 token，通过 Authorization Bearer 请求头访问所有设备 API。token 由命令行设置，未设置时自动生成 32 字节随机凭据；设备不持久化 token。敏感进程数据仅诊断构建显式启用后可用。





