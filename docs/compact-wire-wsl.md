# 紧凑协议开销记录

测试日期：2026-09-29。环境为 WSL2 Ubuntu-24.04，使用 `target/debug/procface` 和模拟 procfs fixture。该记录用于验证接口量级，不代表目标板的带宽或内存上限。

fixture 运行约 1.2 秒时，响应体大小如下：

| 接口 | 响应体 | 批次数/行数 |
|---|---:|---:|
| `/api/v1/current` | 11,947 B | 1 个 envelope |
| `/api/v1/series?limit=1000` | 17,577 B | 1 个 envelope |
| `/api/v1/export?format=jsonl` | 36,324 B | 4 行 |

运行约 3.2 秒后，断线补偿请求的大小如下：

| 请求 | 响应体 | 批次数 |
|---|---:|---:|
| `/api/v1/series?after=0&limit=1000` | 28,753 B | 8 |
| `/api/v1/series?after=1&limit=1000` | 23,645 B | 7 |
| `/api/v1/series?after=2&limit=1000` | 23,165 B | 6 |

紧凑 JSON 的批次主体使用数组字段和每批次字典引用；JSONL 每行仍是独立 envelope，便于开发机直接保存和独立解码。当前没有保留旧 wire 格式，因此这里记录绝对大小和补偿开销，不做不具可比性的旧格式压缩率对比。

复现：

```sh
python3 tests/test_compact.py --binary target/debug/procface
```
