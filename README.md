# ProcFace

面向嵌入式 Linux 的 procfs 性能分析工具。Rust 二进制直接采集，不依赖 sysstat 或 BusyBox。网页为独立单体 HTML，设备只提供 API。

当前为 0.1.0 开发版，尚未正式发布。CLI、P0 指标、daemon、trace、SQLite 可选写入与前端已有初版实现；完整验收仍按 openspec/changes/procface-v1-foundation/tasks.md 推进。

## 构建

使用 Rust 1.85+，建议 Linux 开发机：

~~~sh
cargo build --release --locked
./target/release/procface --help
./target/release/procface sample --count 3 --format jsonl
./target/release/procface sample --metrics process --count 1 --format json
./target/release/procface trace 1234 --count 3 --threads
~~~

默认 1 秒采样，拒绝更短间隔；速率使用 uptime。JSON 要求有限 --count，持续输出可用 JSONL、TSV 或 table。stdout 只输出数据，诊断写 stderr。CLI 默认不采集进程；启用 process 后默认当前用户，支持 --user 和 --pids，不做后端 Top-K 截断。--proc-root 支持测试目录。

正式目标为 ARM32 hard-float、ARM64、RISC-V64 和 x86_64。计划最低 Linux 4.19，但 Rust RISC-V musl 目标最低为 4.20。最低内核和目标板兼容性仍待独立验证。

## 网页与 daemon

开发网页未签名，需要显式开发调试：

~~~sh
./target/release/procface daemon --allow-origin http://localhost:8000 --allow-unsigned-frontend
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
~~~

在开发机打开 http://localhost:8000/procface-web.html，填写 daemon 地址和 stderr 打印的随机 token。设备无需 Python。默认监听 127.0.0.1:9387；局域网访问显式设置 --listen 0.0.0.0:9387，按网页来源配置 --allow-origin。Origin 仅含协议、主机和端口，不含路径或尾斜线。双击 HTML 需要 --allow-file-origin，默认关闭。

开发调试保留 token、CORS 和 API 检查，不启用敏感读取。正式前端使用内嵌签名版本声明，daemon 通过 --frontend-public-key 或构建环境变量 PROCFACE_RELEASE_KEY 信任发布公钥。签名声明不能证明浏览器运行代码未被修改。

前端提供实时图表、默认前 16 个进程及前端筛选排序、独立 trace、IndexedDB 历史、CSV/TSV/JSON 导出与 PNG 单图下载。断线不停止 daemon 或 trace；重连可停止原 trace，再切换 PID。浏览器可能限制本地网络访问，需检查权限与来源配置，不要关闭浏览器安全机制。

界面分为“总览 / 进程 / Trace”三页，使用等宽字体与方括号按钮。总览按 CPU、负载、内存、磁盘和网络分组；进程页选择 PID 后进入 Trace 页。切页不会停止订阅或清空历史。连接成功后配置自动收起，公共栏保留设备、采样间隔、uptime 和断链状态。

## 接口

数据请求均使用 Authorization: Bearer TOKEN。

| 路径 | 用途 |
|---|---|
| GET /api/v1/capabilities | 构建与 API 能力 |
| POST /api/v1/frontend/handshake | 前端签名声明 / 开发握手 |
| GET /api/v1/health | 采集、缓存、持久化状态 |
| GET /api/v1/current | 最近批次 |
| GET /api/v1/series | 有界历史；after、from、to、limit、group、metric、entity |
| GET /api/v1/stream | SSE 实时流及心跳 |
| GET /api/v1/export?format=jsonl | 分块导出，支持 format=tsv、follow=1 |
| GET /api/v1/processes | 轻量进程列表 |
| GET/POST/DELETE /api/v1/trace | 查询、启动、停止唯一 trace |
| GET /api/v1/processes/PID/{capabilities,current,series,stream} | 进程诊断 |
| GET /metrics | Prometheus；进程需 --prometheus-process |

启动 trace 的 JSON：

~~~json
{"pid":1234,"interval":1,"extended":true,"threads":false,"sensitive":false,"budget_ms":500}
~~~

已有 trace 或 stopping 时返回 409。采集预算是读取之间的合作式检查，不保证中断阻塞内核调用。默认历史 60 秒，普通缓存 4 MiB、trace 缓存 2 MiB，数值待测量调整；缓存字节预算不是整个进程 RSS 上限。

## 诊断构建与持久化

~~~sh
cargo build --release --locked --features diagnostic
./target/release/procface daemon --sqlite-path ./history.db
~~~

默认构建不含 SQLite 或敏感读取器。诊断构建包含能力，但仍须显式指定 --sqlite-path、--diagnostic-sensitive。SQLite 默认 16 MiB、1 小时保留、30 秒 flush；对应 --sqlite-max-bytes、--sqlite-retention-seconds、--sqlite-flush-seconds。错误报告到 stderr/health，不影响实时采集。

SQLite 保留期按可确认的 uptime 累计，不依赖 RTC。同一 Linux boot_id 内重启 daemon，停机期间计入数据年龄；设备重启或 boot_id 不可读时，不推测未知的断电时长，旧数据随新一次运行继续老化。容量限制始终生效。旧版开发数据库首次升级时为历史记录建立保守年龄基线，后续按该时钟淘汰。

## 发布文件校验

以下工具只运行在开发机，需要 Python 与 OpenSSL：

~~~sh
python3 tools/release_frontend.py build --key /安全路径/release.key --version 0.1.0 --build-id COMMIT_ID
python3 tools/release_frontend.py verify --html dist/procface-web.html --manifest dist/frontend-manifest.json --public-key 可信Base64公钥
~~~

先生成内嵌签名声明，再对最终 HTML 生成外部 SHA-256 签名清单。发布私钥不进入产物。Pages 与 Release 应使用同一 HTML 字节。项目仓库为 [LiveDevLiberate/procface](https://github.com/LiveDevLiberate/procface)，已公开；Pages 发布流程与公钥已配置，但线上旧预览尚未更新为当前实现，状态和步骤见 [发布操作](docs/release.md)。

正式目录组装使用 `tools/package_release.py`，生成 Release 附件与字节一致的 Pages 目录；具体构建、上传与验收步骤见 [发布操作](docs/release.md)。该工具不自动上传或部署。

## 验证

~~~sh
cargo test --locked
cargo build --locked
python3 tests/test_e2e.py --binary target/debug/procface
python3 tests/test_release.py --binary target/debug/procface
cargo build --locked --features diagnostic
python3 tests/test_e2e.py --binary target/debug/procface --diagnostic
npm ci --prefix tools
node tools/test_browser.cjs
~~~

浏览器测试默认使用 8765 网页服务、8766 daemon 和固定测试 token。可用 PROCFACE_WEB_URL、PROCFACE_DAEMON_URL、PROCFACE_BROWSER_CHANNEL 覆盖配置。测试 token 不用于实际设备。

MIT 许可证。upstream-sysstat 仅供调研参考，ProcFace 独立实现。

