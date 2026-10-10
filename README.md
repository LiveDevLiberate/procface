# ProcFace

简体中文 | [English](README.en.md)

面向嵌入式 Linux 的 procfs 性能分析工具，使用 Rust 编写，不依赖 sysstat 或 BusyBox 命令。

- CLI 采样与单进程 Trace，支持 table、TSV、JSON 和 JSONL。
- daemon 提供实时 API；设备端默认不持久化，可选诊断构建启用 SQLite。
- 独立单体 HTML，中英双语，提供实时图表、类 top 进程列表、浏览器历史与数据导出。

当前为开发版，目标架构为 ARM32、ARM64、RISC-V64 和 x86_64，目标板兼容性仍待验证。

## 快速开始

使用 Rust 1.85+，在 Linux 上构建：

```sh
cargo build --release --locked
./target/release/procface sample --count 3 --format jsonl
./target/release/procface trace 1234 --count 3
```

本地调试网页，在两个终端分别运行：

```sh
./target/release/procface daemon --allow-origin http://localhost:8000 --allow-unsigned-frontend
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
```

打开 <http://localhost:8000/procface-web.html>，填写 daemon 地址 `http://127.0.0.1:9387` 和 stderr 打印的 token。默认每秒采样，daemon 只监听本机；局域网连接配置见使用指南。

## 文档与下载

- [在线网页](https://livedevliberate.github.io/procface/) · [下载](https://github.com/LiveDevLiberate/procface/releases)
- [使用指南](docs/usage.md)：CLI、daemon、API、诊断构建与测试
- [发布与签名校验](docs/release.md) · [紧凑协议](docs/compact-wire.md) · [国际化](docs/i18n.md)

[MIT 许可证](LICENSE)。ProcFace 独立实现，`upstream-sysstat` 仅供调研参考。
