# PC / WSL 开销基线

测试日期：2026-09-28。AMD Ryzen 7 7800X3D，WSL2 分配 8 个逻辑 CPU、约 7.76 GiB RAM，Linux 6.18.33.2。测试普通版静态 x86_64 musl 二进制，大小 1,743,160 字节，SHA-256 见原始 JSON。SQLite 关闭、间隔 1 秒、当前用户范围，约 9 个进程；每轮 692 个系统指标及 72 个进程样本。用户原有 daemon 保持运行。

每场景独立启动，预热 60 秒，再测量 30 秒；CPU 来自 /proc/PID/stat 的增量，100% 表示占满一个逻辑核。RSS 每秒读取，VmHWM 记录内核内存高水位。Trace 目标为 sleep，extended 与 threads 开启，共 36 个 Trace 样本，不能代表复杂应用。

| 场景 | CPU（单核） | 结束 RSS | VmHWM | 线程 | SSE 数据量 |
|---|---:|---:|---:|---:|---:|
| 无客户端 | 0.26% | 5.91 MiB | 6.10 MiB | 3 | 0 |
| 一个自动重连 SSE 客户端 | 0.33% | 5.97 MiB | 6.48 MiB | 3 | 194.95 KiB/s |
| SSE + Trace | 0.40% | 7.41 MiB | 7.98 MiB | 4 | 203.54 KiB/s |

三项有效测量的 health 均为 errors=0、skipped_rounds=0、dropped_batches=0。普通缓存计量约 3.96 MiB，Trace 缓存约 1.21 MiB；这些计量不是进程 RSS。缓存同时受时间和字节限制，配置 60 秒不保证总能保留完整 60 秒。当前接口未记录每轮精确耗时，不能由 CPU 百分比推导采集延迟或宣称预算余量。

## 发现的问题

早期真实 HTTP SSE 在约 30 秒发生 EOF，原因是 daemon 的传输层 30 秒 socket read timeout 误作用于长连接。现已改为由 Hyper 单独限制请求头读取 30 秒，SSE/跟随导出保持连接，并通过 32 秒双心跳回归验证。写侧仍保留 5 秒超时，慢客户端不会阻塞采集。

首次测量的客户端没有重连，60 秒后的测量窗口无 SSE 数据，不可作为持续订阅结果。保留在 resources-pc-wsl.json 并标注无效场景；客户端结果来自 resources-pc-wsl-clients.json。该脚本客户端只验证 HTTP 接收，不执行浏览器握手、series 补偿或绘图，故还未覆盖完整前端开销。

以实际接收速率外推，无 Trace 的 SSE 原始文本约 685 MiB/小时；Trace 场景约 716 MiB/小时。这是传输文本体积，不是 IndexedDB 实测占用，也不是设备持久化写入量，未计 HTTP/TCP 开销。样本 JSON 重复携带元数据，带宽与浏览器历史策略值得优化，当前无需优先优化 PC CPU。

## 范围与复现

这是小进程数、无明显负载的短时 PC 基线，非嵌入式验收、压力测试或泄漏测试。不含浏览器 CPU/RSS/IndexedDB、WSL 虚拟机总内存、复杂进程 smaps 等 Trace 成本。短时的 0.1% 级差异只用于量级判断。

```sh
python3 tools/measure_pc.py --binary dist/local-x86_64/procface --output /tmp/procface-pc.json
```

脚本只读取自身启动的测试 daemon 资源，用真实 /proc 数据；测试输出不保存 token 或进程详细内容。每场景约 90 秒。后续优先修复 SSE 周期断流，再测试真实浏览器长时间运行与较大进程数。
