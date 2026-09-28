# 初版验证记录（2026-09-28）

本次推进属于有效进展：修改代码、增加可重复测试并生成八个开发预览包。目标尚未完成，不归档 change。

## 已取得的证据

- WSL Ubuntu 24.04，Rust 1.97.1；默认与 all-features 单元测试均通过，7 项。
- cargo clippy --all-features --all-targets -- -D warnings 通过。
- tests/test_e2e.py：Linux x86_64 普通与诊断静态 musl 产物均通过。覆盖四种 CLI 格式、300 进程无截断、token/CORS、API 兼容拒绝、导出、单 trace 409、停止切换、PID 复用停止、SQLite 损坏隔离。
- tests/test_release.py：真实临时 Ed25519 签名，验证完整文件与握手；拒绝 HTML/清单篡改、声明字段不一致、无效 token、签名失败降级。测试私钥在临时目录销毁，不作为发布密钥。
- tests/test_stream.py：SSE 序号、15 秒心跳、连接数量上限及断开释放、历史分页、follow 导出、最后一次 PID 阻塞读取超过预算后整轮跳过，全部通过。
- 发布顺序增加短时互斥，序号分配、缓存提交、广播通知顺序一致；不在此锁内序列化或进行网络 I/O。
- .github/workflows/check.yml 已配置本地测试对应命令和四架构双构建矩阵。没有远端仓库，不宣称 GitHub Actions 已运行。

## 静态构建

使用 cargo-zigbuild 0.23.4 与 Zig 0.14.1。file 检查均为对应架构静态 ELF（RISC-V 为 static PIE）。

| 目标 | 普通版字节 | 诊断版字节 |
|---|---:|---:|
| ARMv7 hard-float | 1622728 | 2768976 |
| ARM64 | 1581520 | 2736296 |
| RISC-V64 | 1657584 | 2588400 |
| x86_64 | 1736408 | 2918216 |

dist/preview-index.json 记录八个归档及二进制 SHA-256。归档包含二进制、MIT LICENSE、使用说明、未签名开发 HTML 和 build.json。它们明确标为 preview，不是正式签名 Release；Pages 尚未上线。构建之后仅调整了 SQLite 参数范围判断的等价写法，产物需在最终验收后统一重建。

## 仍需完成的验收

- ARM32、ARM64、RISC-V64 实际运行、最低内核及真实目标板内存/CPU 预算验证；仅编译通过不代表运行保证。
- 确认 RISC-V 最低 4.20 与原计划 4.19 的平台差异。
- 慢客户端网络拥塞回收、跨多个缓存窗口的断线 gap 完整性、并发发布压力回归。
- 采集源权限/非法字段、trace 每项能力及线程信息的覆盖度；预算与 PID 身份竞态的进一步边界测试。
- SQLite 滚动淘汰、大小边界、重启保留与正常关闭 flush 验证。
- 当前前端的完整浏览器回归：恢复历史、CSV/TSV/JSON 精度、gap 不插值、离线文件与在线入口。上一轮页面测试和截图不替代本轮全部变更后的验收。
- 发布密钥、实际项目 GitHub 地址、正式 Release 和 Pages 配置。用户目标是完成项目代码；这些外部发布输入不妨碍继续完成本地代码和验证。

## 下一步

### 后续边界修复与验证

- SQLite 正常退出增加 shutdown：关闭生产队列，排空待写入批次并等待最终事务完成；daemon 退出前调用。真实 daemon 在 30 秒 flush 到期前退出后，数据库内仍有已采集样本，端到端测试通过。
- 已有数据库先检查页数和大小，再创建 schema；超限时字节不变。单元测试覆盖拒绝修改 128 KiB 已有数据库、64 KiB 限制下 80 批样本滚动淘汰、时间保留与退出 flush。
- 历史丢失增加 lost_through_sequence 水位：报告窗口中部因淘汰或超大批次产生的缺口，而非只比较最早记录。停止 trace 后仍由主采样时钟清理其过期数据，HTTP 回归验证窗口外 current 返回 404 且 series 标记 gap。
- 缓存估算纳入 Vec/String 已分配 capacity，避免只计使用长度而忽略保留容量。该估算针对当前标量/文本样本，不代表整个进程 RSS 硬上限。
- 最新 all-features 单元测试为 12 项，全部通过；Clippy 严格检查通过。更新后的诊断端到端测试和 SSE/窗口测试通过。
- dist 内预览包早于这些边界修复，尚未重新打包；最终交付前需要统一重建，不能将旧包视为最新实现。

继续修复验收暴露的问题并补足测试，逐项更新 tasks.md。未勾选任务可能已有部分实现，只有行为和指定验证均完成才勾选。

### 前端历史一致性回归

- 图表按采样时间与序列排序并去重；旧批次不覆盖新的进程列表或 Trace 文本。补偿后实时流重放旧批次不会使视图倒退。
- 实时序列跳跃和心跳序列超前会触发有界 series 补偿；缺失批次保留 gap，图表不跨 gap 连线。旧连接的异步读取通过连接代次检查，避免污染新连接视图。
- `node tools/test_frontend_history.cjs` 在 Edge 无头浏览器通过：离线 file 页面、API 不兼容只请求 capabilities 并显示推荐版本、乱序去重、IndexedDB、可恢复与不可恢复区间、旧连接隔离以及 u64 最大值原样导出。
- 更新页面后重新运行 `node tools/test_browser.cjs`，连接 WSL 实际 daemon，实时图表、Trace 重连接管与停止、JSON 历史导出通过，未发生页面异常。该测试使用本地 HTTP 来源；不代表 GitHub Pages 在线入口已验收。
- 已将独立前端历史回归加入 CI 配置，远端工作流尚未执行。任务 7.1、7.2 完成，总进度 11/49；完整网络拥塞和多窗口重连仍需继续验证。

### TCP 背压、CLI 与退出顺序

- `tests/test_stream.py` 新增真实 TCP 小接收窗口：HTTP 握手后不再读数据，分别占满 SSE 与两个导出名额。确认阻塞连接自动释放，期间 health 序列增长且 Trace 仍运行；测试保留原客户端 socket 打开，避免将主动断开误当作服务器回收。
- 同一测试验证 JSONL/TSV follow、TSV HTTP chunked 编码、15 秒心跳、历史过期水位及连接断开后采集继续。更新后二次执行通过。
- series 在同一发布边界复制普通历史、Trace 历史和丢失水位，随后释放锁再过滤与序列化，避免并发发布导致响应水位与批次不一致。
- CLI 新增 Unix 时间戳与 uptime、有限轮数、未知指标、PID 列表、显式用户范围、单 PID Trace、默认无色和 table 显式颜色、human 不影响 TSV、读取错误只写 stderr 的验证。普通与诊断构建端到端测试均通过。
- 退出顺序改为主采样结束、Trace worker 结束、SQLite 最终 flush。诊断测试用 FIFO 暂停 Trace 读取，发送 SIGINT 后确认 daemon 等待；解除阻塞后正常退出，数据库包含最后一批 Trace 文本。
- 单元测试 12 项通过；格式和 Clippy 严格检查通过。任务 3.2、3.3、3.4、4.4、4.6、4.7 完成，总进度 17/49。上述 TCP 测试不替代目标板内存测量，也不代表全部前端长时间重连场景已验收。

### P0 准确性与安全边界

- 修复读取 disk/network 等来源失败时把已知实体误标为 exited：现在输出实际 unsupported/permission_denied/parse_error，重新出现时重新建立速率基线。`/proc/stat` 缺少的固定字段保留为 unsupported。
- 14 项单元测试通过。新增磁盘扇区到 bytes、增量速率、计数器回退、来源丢失/重新出现/实体退出；扩充 CPU 回退与恢复、uptime 回退、meminfo/vmstat 单位及非法字段、loadavg 与 capabilities 隔离测试。
- 本次测试在 WSL UID 1000 下运行，chmod 000 的实际文件访问产生 permission_denied；其他指标继续有效。未使用 root 跳过权限断言的结果作为验收证据。
- Prometheus 端到端测试验证 HELP/TYPE 唯一声明、合法序列语法、counter/rate 类型、device/interface 标签、默认排除进程及显式开启后的 PID/进程名转义。普通与诊断运行均通过；测试等待首轮采集完成，避免将 HTTP 已就绪误当作数据已就绪。
- `tests/test_security.py` 在普通和诊断二进制通过：默认禁用未签名调试与跨域、null origin 单独放行、完整 Origin 精确匹配、无数据预检、token/API 检查保留、调试不会开启敏感采集或 SQLite、随机 token 仅通过 stderr 输出。默认监听地址另由 CLI 解析测试确认。
- 正式签名与篡改拒绝测试再次通过；Clippy 严格检查通过。新增安全测试纳入 CI 配置。任务 2.1、2.2、2.3、2.5、4.1、4.8、4.10、4.11、8.1 完成，累计 26/49。

### Trace 生命周期与查询

- 修复进程历史查询在 limit 之后才按 PID 过滤的问题：现在先匹配 PID 再分页；current 按目标 PID 返回窗口内最近记录。停止或退出的 Trace 不再接受新的专属 stream 订阅。
- 新增 `tests/test_trace.py`，普通和诊断构建均通过。使用无写入者的敏感 FIFO 验证普通读取不会打开该文件；诊断显式开关才读取敏感内容。验证 basic/extended 分层、线程文本、permission_denied、4097 个 fd 触发 skipped_expensive/stale。
- 通过阻塞 sched 读取确认 Trace 线程 nice=10、普通采集继续、默认预算超时后整批清空；停止中返回 stopping 并拒绝替换（409），读取结束后才进入 idle。普通历史不因停止或切换而删除。
- 验证进程 capabilities/current/series/stream、PID 筛选分页、客户端断开后 Trace 继续、PID 复用结束任务及新任务 CPU/I/O 基线为 stale；进程退出后停止专属订阅。
- 普通进程以 1 ms 测试预算触发整轮丢弃，health 累加跳轮且系统数据继续发布；正常预算下仍采集全部 300 个进程。查询拒绝非法 limit、时间范围、NaN、过多/过长过滤器及未知 group。
- 普通与诊断端到端测试通过；14 项单元测试、Clippy 严格检查通过。验证 SQLite 仅诊断构建提供能力，默认无路径、16 MiB、1 小时、30 秒 flush；普通版尝试启用时拒绝且不创建文件。
- 任务 4.2、4.3、4.5、5.1–5.6、6.1 完成，累计 36/49。已向用户询问正式发布仓库与目标板环境，尚未收到这些外部输入；本地前端与 SQLite 验证继续推进。

### 前端完整流程

- `tools/test_frontend_history.cjs` 增加默认 16 行、CPU/内存排序、全部 20 行、筛选列表之外的进程、覆盖不足提示、IndexedDB 回看与实时切换、JSON/CSV/TSV 下载、u64 精度和 PNG 文件签名验证。展示选择不改变原始批次中的完整进程列表；检查导出和存储不包含连接 token。
- 修复历史回看与停止 Trace 的过期异步响应；Trace 状态显示启动标识。增加 stopping 期间开始按钮禁用、确认 idle 后才可开始的浏览器断言。
- 新增 `tools/test_frontend_stream.cjs`，用实际 HTTP/SSE 驱动页面：服务端 slow_client 事件、直接销毁 TCP 响应、自动重连、窗口内补偿、窗口外保留 gap、每次重新握手、签名拒绝及推荐版本显示、daemon 会话切换后重新协商。该测试加入 CI。
- 修复同毫秒 gap ID 冲突导致 IndexedDB 覆盖记录，改为 getRandomValues 生成标识（可在普通 HTTP 来源使用）。修复初次连接时缺口范围覆盖已恢复历史的问题；新增首个可用样本之后不得继续标为 gap 的断言。
- 三个浏览器测试均通过；真实 WSL daemon 测试增加重连后停止并切换另一个 PID，再停止的完整操作。更新 docs/procface-ui.png 并完成视觉检查。
- 结合已通过的实际 TCP 背压、CORS/Bearer、Prometheus 和 SQLite 损坏隔离测试，完成任务 7.3–7.8、8.3，累计 43/49。GitHub Pages 真实在线入口、目标板与正式发布仍未验证；6.2 的跨重启保留语义还需完善。

### SQLite 保留与最新预览产物

- SQLite 增加持久化运行时长时钟，以 boot_id 识别 Linux 启动周期。跨 daemon 会话继续淘汰；同 boot_id 的停机时间计入年龄。不同 boot_id 或不可获取时不猜测断电时间，以随后可确认的 uptime 增量继续老化，大小上限独立生效。
- 旧开发数据库增加 retention_time 字段与时钟表，无法恢复年龄的旧记录使用保守基线。重启测试覆盖多次打开同一数据库、会话改变、设备重启与过期删除。15 项单元测试、诊断端到端及 Clippy 通过，任务 6.2 完成，累计 44/49。
- 四架构普通/诊断静态 musl 产物全部重建。最新普通版字节数 ARMv7 1628944、ARM64 1586880、RISC-V64 1663808、x86_64 1742952；诊断版分别为 2781240、2748024、2600512、2932072。file 确认目标架构和静态 ELF；x86_64 两种构建实际端到端测试通过。
- dist 八个 unsigned-development-preview 包已替换旧产物，验证包摘要、二进制摘要、当前 HTML 摘要及 LICENSE 一致。它们仍不是正式签名 Release，不代表另外三种架构已实际运行。
- 新增 tools/measure_resources.py 与 docs/resources-host.json。WSL x86_64 静态普通版、模拟 procfs、1 秒采样、每场景约 4 秒：1 进程 RSS 1.91 MiB；300 进程 5.52 MiB；3000 进程峰值 10.52 MiB，整批超出默认 4 MiB 缓存被丢弃 5 次；300 进程加 200 KiB Trace 文本 RSS 6.60 MiB。CPU 单核占比分别约 0、0.73%、6.54%、0.72%；短时低于一个时钟 tick 的读数不代表零开销。
- 上述宿主机数据表明缓存上限不等于 RSS 上限，3000 进程配置需更大缓存才能保留普通进程批次。暂不凭短时宿主机结果修改嵌入式默认值；目标板长时测量仍为未完成任务 8.7。

### 正式发布组装准备

- 新增 tools/package_release.py：校验最终 HTML 外部签名、内嵌声明签名及字段一致、Cargo 前端版本、API 范围、八个 ELF 的架构和无动态加载器；输出八个归档、Release assets、Pages 目录和 SHA256SUMS。输出不得覆盖已有目录，临时组装成功后才整体交付。
- 新增 tests/test_package.py，使用临时 Ed25519 密钥和现有八个真实静态二进制，通过归档摘要、LICENSE、HTML 字节一致、无私钥以及拒绝篡改/覆盖检查；测试结束销毁临时密钥与测试签名产物。
- 新增 docs/release.md，说明同提交构建、公钥信任、目录组装、实际上传和线上一致性验证。未将自报 build_id 作为二进制来源证明，也未自动向远端发布。
- 该工作完成本地发布工具准备，但不代表 8.4/8.6 已通过：目标仓库、长期发布密钥、实际 Release/Pages 和目标设备输入仍缺失，总进度保持 44/49。

