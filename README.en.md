# ProcFace

[简体中文](README.md) | English

A procfs performance analysis tool for embedded Linux. The Rust binary collects data directly, without sysstat or BusyBox commands. The frontend is a standalone HTML file; the device serves only APIs.

Version 0.1.0 is a development release. Initial implementations include the CLI, P0 metrics, daemon, single-process Trace, optional SQLite storage and frontend. Acceptance work is tracked in `openspec/changes/procface-v1-foundation/tasks.md`.

## Build

Use Rust 1.85+; a Linux development machine is recommended.

```sh
cargo build --release --locked
./target/release/procface --help
./target/release/procface sample --count 3 --format jsonl
./target/release/procface sample --metrics process --count 1 --format json
./target/release/procface trace 1234 --count 3 --threads
```

The default sampling interval is one second; shorter intervals are rejected. Rates use uptime. JSON requires a finite `--count`; use JSONL, TSV or table for continuous output. stdout contains data only; diagnostics go to stderr. The CLI does not collect processes by default. With the process group enabled, it defaults to the current user's processes, supports `--user` and `--pids`, and does not truncate results to a backend Top-K. `--proc-root` accepts a fixture directory for testing.

Targets are ARM32 hard-float, ARM64, RISC-V64 and x86_64. Linux 4.19 is the planned baseline, but Rust's RISC-V musl target requires at least 4.20. Minimum-kernel and target-board compatibility still need independent verification.

## Frontend and daemon

The development HTML is unsigned and requires explicit development configuration:

```sh
./target/release/procface daemon --allow-origin http://localhost:8000 --allow-unsigned-frontend
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
```

Open `http://localhost:8000/procface-web.html` on the development machine and enter the daemon address and random token printed to stderr. The device does not need Python. The daemon listens on `127.0.0.1:9387` by default. For LAN access, explicitly set `--listen 0.0.0.0:9387` and configure `--allow-origin` for the page's origin. An origin contains only scheme, host and port, with no path or trailing slash. Opening the HTML directly requires `--allow-file-origin`, which is off by default.

Development configuration retains token authentication, CORS and API checks; it does not enable sensitive readers. Released frontends contain a signed version declaration. Configure the daemon's trusted key using `--frontend-public-key` or the build-time `PROCFACE_RELEASE_KEY` environment variable. A signed declaration cannot prove that the browser's running code has not been modified.

The frontend supports Chinese and English through the language selector. Switching language preserves the daemon connection, Trace and local history. It provides live charts, a process table showing 16 rows by default with browser-side filtering and sorting, independent Trace, IndexedDB history, CSV/TSV/JSON downloads and per-chart PNG export. Disconnecting does not stop daemon collection or Trace; reconnect to stop the active Trace and select another PID. Browsers may restrict local-network access; check permissions and origin settings without disabling browser security.

Three tabs—Overview, Processes and Trace—use monospace text and bracket-style buttons. Overview groups CPU, load, memory, disk, network and pressure charts; API metric filters also expose interrupts and soft IRQs. Selecting a PID on the process tab opens Trace. Trace displays status, memory, scheduling, I/O, threads and raw procfs fields, including read status. Switching tabs preserves subscriptions and history. Connection settings collapse after connecting; the shared status bar retains the device, interval, uptime and connection status.

The hosted frontend is available at [GitHub Pages](https://livedevliberate.github.io/procface/).

## API

Data requests use `Authorization: Bearer TOKEN`.

| Path | Purpose |
|---|---|
| GET /api/v1/capabilities | Build and API capabilities |
| POST /api/v1/frontend/handshake | Signed frontend declaration / development handshake |
| GET /api/v1/health | Collection, cache and persistence status |
| GET /api/v1/current | Latest batches |
| GET /api/v1/series | Bounded history; after, from, to, limit, group, metric, entity |
| GET /api/v1/stream | SSE live stream and heartbeats |
| GET /api/v1/export?format=jsonl | Chunked export; also supports format=tsv and follow=1 |
| GET /api/v1/processes | Lightweight process list |
| GET/POST/DELETE /api/v1/trace | Query, start or stop the single active Trace |
| GET /api/v1/processes/PID/{capabilities,current,series,stream} | Process diagnostics |
| GET /metrics | Prometheus; process metrics require --prometheus-process |

Example Trace request body:

```json
{"pid":1234,"interval":1,"extended":true,"threads":false,"sensitive":false,"budget_ms":500}
```

Starting a Trace while one is running or stopping returns 409. Collection budgets are checked cooperatively between reads and cannot guarantee interruption of a blocked kernel call. Default history is 60 seconds, with a 4 MiB ordinary cache and 2 MiB Trace cache; measurements may change these defaults. Cache byte budgets are not a limit on total process RSS.

## Diagnostic build and persistence

```sh
cargo build --release --locked --features diagnostic
./target/release/procface daemon --sqlite-path ./history.db
```

The default build excludes SQLite and sensitive readers. The diagnostic build includes those capabilities, but still requires explicit `--sqlite-path` and `--diagnostic-sensitive` flags. SQLite defaults to 16 MiB, one-hour retention and a 30-second flush interval; configure these with `--sqlite-max-bytes`, `--sqlite-retention-seconds` and `--sqlite-flush-seconds`. Errors are reported to stderr and health status without stopping live collection.

SQLite retention uses confirmed elapsed uptime rather than RTC. Restarting the daemon within the same Linux `boot_id` includes downtime in record age. After a device reboot, or when `boot_id` is unavailable, unknown powered-off time is not inferred; old records continue aging during the next run. The size limit always applies. Upgrading an older development database establishes a conservative age baseline before normal eviction.

## Release verification

These tools run only on the development machine and require Python and OpenSSL:

```sh
python3 tools/release_frontend.py build --key /secure/path/release.key --version 0.1.0 --build-id COMMIT_ID
python3 tools/release_frontend.py verify --html dist/procface-web.html --manifest dist/frontend-manifest.json --public-key TRUSTED_BASE64_KEY
```

The builder embeds a signed declaration and then creates an externally signed SHA-256 manifest for the final HTML. The private key is never included in artifacts. Pages and Release use identical HTML bytes. `tools/package_release.py` assembles Release attachments and a matching Pages directory; it does not upload or deploy automatically. See the [release guide](docs/release.en.md), [compact wire format](docs/compact-wire.en.md) and [internationalization policy](docs/i18n.en.md).

## Verification

```sh
cargo test --locked
cargo build --locked
python3 tests/test_e2e.py --binary target/debug/procface
python3 tests/test_release.py --binary target/debug/procface
cargo build --locked --features diagnostic
python3 tests/test_e2e.py --binary target/debug/procface --diagnostic
npm ci --prefix tools
node tools/test_browser.cjs
node tools/test_frontend_layout.cjs
```

Browser integration tests default to a web server on port 8765, daemon on port 8766 and a fixed test token. Override settings using `PROCFACE_WEB_URL`, `PROCFACE_DAEMON_URL` and `PROCFACE_BROWSER_CHANNEL`. Do not use test tokens on real devices.

MIT license. `upstream-sysstat` is a research reference only; ProcFace is an independent implementation.
