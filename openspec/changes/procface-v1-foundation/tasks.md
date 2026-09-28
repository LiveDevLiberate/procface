# Tasks

## 1. Rust 工程与构建

- [x] 1.1 创建 Rust workspace、`procface` 二进制入口和模块边界，并用宿主机 `cargo check` 验证工程可编译
- [x] 1.2 配置 release、diagnostic 和 `sqlite` Cargo feature，验证最小构建不链接 SQLite、诊断构建报告 SQLite 能力
- [x] 1.3 配置 ARM32、ARM64、RISC-V64、x86_64 musl 目标和 CI 交叉编译，验证四个目标均生成可执行文件
- [x] 1.4 加入 MIT LICENSE、版本信息和 `--help` 特性清单，验证 release/diagnostic 的能力显示不同

## 2. Procfs P0 采集

- [x] 2.1 实现 `/proc/uptime`、`/proc/stat`、`/proc/loadavg` 解析及 uptime 基线，使用固定样例测试计数器增量和回退
- [x] 2.2 实现 `/proc/meminfo`、`/proc/vmstat` 解析和 bytes/seconds 单位，测试缺失字段与非法数值状态
- [x] 2.3 实现 `/proc/diskstats`、`/proc/net/dev` 解析和速率计算，测试设备重置与接口消失
- [x] 2.4 实现统一样本模型、`kind`、status 枚举和 JSON/JSONL/TSV 序列化，测试固定 TSV 列顺序和数值精度
- [x] 2.5 实现 capabilities 指标组探测，验证 unsupported、permission_denied 和 parse_error 不会停止其他指标

## 3. CLI

- [x] 3.1 实现 `sample`、`capabilities`、`trace`、`daemon` 原生命令解析，验证未知参数只写 stderr 并返回非零状态
- [x] 3.2 实现 table、TSV、JSON、JSONL 输出以及 `--timestamp unix`，测试 stdout 不混入诊断文本
- [x] 3.3 实现 `--interval` 最低 1 秒、`--count`、`--metrics` 和 `--proc-root`，验证亚秒间隔被拒绝
- [x] 3.4 实现当前用户进程范围、PID 列表和单进程 trace CLI，验证默认不扫描其他用户

## 4. Daemon 与采样调度

- [x] 4.1 实现默认 127.0.0.1 监听、Bearer token、随机 token stderr 展示和显式 CORS 来源，测试未授权返回 401
- [x] 4.2 实现系统指标主采样循环、60 秒有界窗口和 health 计数，验证浏览器断开后仍继续采集
- [x] 4.3 实现 top 风格全量当前用户进程轻量扫描和 500 ms 预算，测试超过 256 个进程时无数量截断、后端不排序，超时跳过本轮且系统指标继续发布
- [x] 4.4 实现 immutable snapshot、有界队列和独立 HTTP 发布循环，测试慢客户端不会阻塞采集
- [x] 4.5 实现 capabilities、health、current、series API，验证 limit、范围和过滤器上限
- [x] 4.6 实现 SSE connected/sample/heartbeat/error 事件和断线补偿约定，测试 15 秒 heartbeat 与慢客户端断开
- [x] 4.7 实现 chunked JSONL/TSV export 和 `follow=1`，测试客户端断开后资源释放且不保存完整会话
- [x] 4.8 实现 Prometheus `/metrics` 和进程导出显式开关，验证 HELP/TYPE、单位和系统指标默认集合
- [x] 4.9 实现前端握手与签名版本声明检查，验证软件版本不同但 API 兼容时接受，签名错误、字段不一致和 API 不兼容时拒绝，有效声明不能绕过 token
- [x] 4.10 实现默认关闭的未签名前端开发调试开关，验证 token、CORS 和 API 检查仍生效，签名错误不自动降级，调试状态可见且不启用敏感诊断
- [x] 4.11 实现 GitHub Pages 完整 Origin 配置与独立的 null origin 放行开关，测试默认拒绝、本地文件显式允许和预检不泄露设备数据

## 5. Trace 与进程深度诊断

- [x] 5.1 实现单活动 trace 状态、POST/GET/DELETE 生命周期和重复启动 409，测试停止后普通历史不变
- [x] 5.2 实现独立低优先级 trace worker、独立间隔、500 ms 预算和合作式停止，测试高成本读取不阻塞主采样
- [x] 5.3 实现 PID + starttime 身份、退出和 PID 复用处理，测试速率基线不会跨实体继承
- [x] 5.4 实现 basic/extended/diagnostic 字段分层和诊断构建敏感开关，测试 release 不打开敏感文件
- [x] 5.5 实现进程 capabilities/current/series/stream 响应，测试权限错误、退出和 skipped_expensive 状态
- [x] 5.6 验证浏览器断开后 trace 继续，重连可查询和停止当前 trace；stopping 期间新请求返回 409，停止完成后可切换 PID

## 6. SQLite 诊断构建

- [x] 6.1 实现 `sqlite` feature、显式路径和 16 MiB 默认上限，验证最小二进制不提供 SQLite 运行时能力
- [x] 6.2 实现 1 小时保留、30 秒 flush 和最旧数据淘汰，测试可配置大小上限和边界事务
- [x] 6.3 实现独立 SQLite worker、损坏/写入失败 stderr 报告和 persistence error health 状态，验证实时 API 继续工作

## 7. 单体 HTML 前端

- [x] 7.1 创建无外部依赖的单体 HTML，验证离线打开能显示 daemon IP、端口和 token 配置界面
- [x] 7.2 实现 capabilities API 版本协商和不兼容阻止连接，测试推荐前端版本提示
- [x] 7.3 实现 fetch SSE、series 断线补偿、IndexedDB 会话历史和 gap 标记，测试不跨断链插值
- [x] 7.4 实现 P0 图表、前端排序筛选、默认前 16 个及剩余进程查看和单进程 trace 视图，测试展示不改变采集范围、覆盖不足有提示、视图切换不清空历史
- [x] 7.5 实现 TSV/CSV/JSON 本地导出，验证 token 不进入 URL、IndexedDB、导出文件或日志
- [x] 7.6 实现重连后当前 trace 状态恢复、停止与切换操作，验证等待停止完成且保留普通视图历史
- [x] 7.7 验证慢客户端被断开后的重连补偿，窗口外缺失保留 gap 且不插值
- [x] 7.8 实现初次连接及重新建链的版本、构建 ID 和内嵌签名版本声明提交及拒绝原因展示，支持显式开发调试状态，避免将检查结果描述为运行代码完整性证明
- [ ] 7.9 验证 GitHub Pages、本地文件和开发机静态托管入口，提供跨域及浏览器网络限制排查提示，离线建链不依赖 GitHub
- [ ] 7.10 后续前端优化阶段实现“总览 / 进程 / Trace”三标签页、公共连接与历史工具栏及 ASCII 终端风格；验证分组图表、数值列稳定、刷新无空白帧、Trace 展开状态保留，以及切页不影响订阅、Trace 和历史保存。本轮只确认规范，不实施或发布布局改造
- [x] 7.11 后续协议优化阶段保持 API v1，实现 API 统一紧凑 JSON 和建链 schema 兼容校验，不保留旧预览线格式分支：稳定 metric/entity/group/status 目录、动态字典增量先于样本、complete/diagnostics 保留、current/series/stream/export 统一解码、前端展开显示和旧数据版本拒绝；API TSV、CLI 和前端下载保持可读格式，测量 API JSON 大小与重连补偿开销

- [x] 7.12 实现字典引用生命周期与安全回收，验证 session 内编号不复用、浏览器/持久化历史独立解码、未知编号有界恢复及失败缺口标记
- [x] 7.13 固定 processes 数组字段位置并消除同批次重复进程样本，验证可读导出字段完整以及 Trace 文本、列表、结构化值和大整数精度往返

## 8. 集成验证与发布

- [x] 8.1 在模拟 procfs 上覆盖缺失文件、权限错误、解析错误、计数器回退和 PID 复用场景
- [ ] 8.2 在四个正式架构上验证启动、P0 采集、daemon API、SSE 和 trace 资源预算
- [x] 8.3 验证断网重连、CORS、Bearer token、Prometheus 抓取和 SQLite 损坏隔离
- [ ] 8.4 生成匹配版本的 musl 二进制、诊断二进制和单体 HTML GitHub Release 包，并核对 LICENSE 与 API 版本字段
- [x] 8.5 先生成内嵌签名版本声明，再对最终 HTML 生成外部 SHA-256 签名清单并提供独立校验说明；验证修改 HTML、清单或签名时失败，产物不含签名私钥
- [ ] 8.6 配置 GitHub Pages 固定入口部署正式 HTML，验证与 Release 产物字节一致且在线入口更新后仍执行 API 范围检查
- [ ] 8.7 实测基础内存、不同进程数、trace 和慢客户端积压峰值，据此调整缓存及队列默认字节预算和淘汰策略，记录目标板测量条件与结果

## 首个实现里程碑

先完成 Rust 工程骨架、procfs P0 解析与统一样本模型，以及无需 daemon 的 CLI 输出；随后推进 daemon、trace、前端和发布流程。完成状态仅依据实现与验证证据更新，文档校验不代表功能已实现。验证记录见 docs/verification-2026-09-28.md。

