# 性能统计开销对照

比较新增观测字段前的 `12993e6` 与加入统计后的 `83254e5`。两者均在同一 WSL 主机以相同 Cargo.lock、Rust stable 工具链、默认 feature 和 debug profile 编译；本次没有修改 compact 样本协议。原始数据及二进制 SHA-256 见 [resources-performance-comparison.json](resources-performance-comparison.json)。

使用模拟 procfs，每个场景独立启动，预热 2 秒，测量约 10 秒；慢客户端测量约 15 秒。每 200 ms 读取资源；CPU 百分比以单核为 100%。Trace 场景额外包含 200 KiB 的 sched 原始文本，用于模拟较大诊断负载，其内容不是有效 sched 数值字段。SSE 接收字节包含事件 framing，区别于 health 的 payload 计数。慢客户端完成响应头后不读负载，接收字节为 0 不代表 daemon 没有生成数据。

| 场景 | CPU 前 / 后 (%) | 结束 RSS 前 / 后 (MiB) | SSE 前 / 后 (KiB/s) |
|---|---:|---:|---:|
| 1 进程，无客户端 | 0.00 / 0.10 | 9.02 / 9.09 | 0 / 0 |
| 300 进程，无客户端 | 1.00 / 1.10 | 13.21 / 14.47 | 0 / 0 |
| 3000 进程，无客户端 | 9.58 / 9.88 | 22.67 / 22.18 | 0 / 0 |
| 300 进程 + SSE | 1.70 / 1.60 | 16.06 / 16.07 | 35.26 / 35.26 |
| 300 进程 + SSE + Trace | 2.40 / 2.40 | 19.00 / 19.23 | 258.71 / 258.71 |
| 300 进程 + 慢 SSE + Trace | 1.06 / 1.13 | 18.03 / 18.75 | 不读负载 |

3000 进程场景前后都丢弃 12 批；默认缓存字节上限无法容纳该模拟场景的普通进程整批，因此不能把这个场景当作完整采集的容量验收。300 进程场景没有整批丢弃，但字节限制会提前淘汰历史：结束时普通窗口约 2 秒、Trace 约 3 秒，配置 60 秒不保证保留足够的实际历史。当前慢客户端场景记录一次 write_timeout，后续采样仍继续。

这轮证据仅支持：统计功能没有改变这组负载的 SSE 流量，能揭示缓存覆盖不足和写超时。单次、短时、debug、模拟 procfs 的 CPU/RSS 差异不能证明统计的准确增量成本，更不能宣称性能提升、长期无泄漏或嵌入式达标。旧报告约 195/204 KiB/s 来自不同场景和旧协议，不能用作压缩率分母。

## 复现

```sh
python3 tools/measure_resources.py \
  --binary target/debug/procface \
  --baseline-binary target/performance-baseline-build/debug/procface \
  --output docs/resources-performance-comparison.json \
  --seconds 10 --warmup 2
```

baseline 由 `git archive 12993e6` 提取到独立的 target 子目录，使用同一工具链执行 `cargo build --locked`。支持更长 warmup/seconds；每轮输出包含采样状态、丢批、RSS、CPU 和接收字节，不能只看平均值忽略丢批。

真实浏览器联调使用 Windows Edge 连接 WSL daemon，HTTP 和 file 两个入口的连接、实时图表、Trace 重连接管、停止和导出通过。WSL 自身缺少 Playwright Chromium，runner 增加 `--node` 与 `--file-url` 参数以复用 Windows 浏览器。该联调为短时功能回归，不是浏览器长时间稳定性测量。
