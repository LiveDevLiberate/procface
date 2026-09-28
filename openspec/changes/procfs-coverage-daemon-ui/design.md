# Design: procfs coverage and embedded daemon UI

## Context

The tool targets constrained embedded Linux systems. Runtime dependencies should remain small and the same normalized metric model must serve CLI, HTTP, and charts. Procfs files vary by kernel version and configuration, so collection is capability-driven rather than all-or-nothing.

## Goals / Non-Goals

**Goals:**

- Cover the procfs metric families that are readable on the target kernel.
- Preserve low overhead through configurable metric groups and sampling intervals.
- Provide a small authenticated HTTP API suitable for a separately hosted browser frontend.
- Keep the daemon free of bundled web assets; publish the frontend as a separate GitHub static package.

**Non-Goals:**

- Do not require sysfs, systemd, Python, Node.js, a database, or a cloud service.
- Do not promise every procfs file has identical fields across kernels.
- Do not implement a general-purpose web server or remote administration channel.
- Do not invoke or require external `iostat`, `mpstat`, or `pidstat` commands.

## Decisions

### Metric adapters

Use one adapter per procfs family. Initial families are `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`, `/proc/uptime`, `/proc/diskstats`, `/proc/net/dev`, `/proc/net/snmp`, `/proc/net/netstat`, `/proc/net/sockstat`, `/proc/net/softnet_stat`, `/proc/net/rpc`, `/proc/interrupts`, `/proc/softirqs`, `/proc/vmstat`, `/proc/pressure/*` when present, `/proc/sys/fs/*`, `/proc/sys/kernel/*`, `/proc/tty/*`, and selected `/proc/PID/*` files. The adapter reports a schema, raw counters, derived values, and capability/error state.

### Storage and retention

CLI streaming remains the default. Daemon mode MUST NOT be a long-term history store. It keeps the current sample, the previous baseline, and only a very short bounded window needed for live charts. The default window should be 60 seconds or less and the implementation MUST expose its actual memory cost; a target board may reduce it to zero/one sample. No database, daily file, or unbounded ring is created by default.

Long-term capture belongs on the developer workstation by default: a client polls `/api/v1/series` or consumes a live export stream and writes locally. The device daemon MUST NOT write metric data, history, cache, logs, pidfiles, or UI state unless the operator explicitly configures the optional SQLite backend. SQLite is disabled in the minimal build by default and requires an explicit writable database path, size limit, retention policy, and flush policy. The daemon MUST expose whether persistence is disabled or active. If SQLite is not configured, the HTTP client or shell redirection on the development machine owns long-term files.

### HTTP API

Bind to `127.0.0.1` by default. A non-loopback bind requires an explicit `--listen` value. All API routes require a bearer token on both loopback and LAN; there is no authentication bypass flag. The API is read-only:

```text
GET /                         static UI
GET /api/v1/capabilities      available metric groups and errors
GET /api/v1/metrics           current normalized sample
GET /api/v1/series?metric=...&entity=...&from=...&to=...
GET /api/v1/health            process and collector health
GET /api/v1/export?format=jsonl|tsv&metrics=...
```

`/api/v1/series` is for chart refreshes: it returns only the short in-memory window for a selected metric/entity. `/api/v1/export` is for developer capture: it streams current/future samples as chunked JSONL or TSV; the workstation can keep the connection open and write the stream locally. The browser keeps chart points in its own memory and discards them when the page closes. A non-follow export may return only the retained short window. The export endpoint MUST use the same filters and permission model as the series endpoint and MUST NOT expose device paths or raw history files.

Responses are JSON except for the explicit export stream. Query ranges, metric/entity counts, request body size (there are no write bodies), and response size are bounded. Unknown metric names return 404; invalid ranges return 400. No endpoint accepts a command, path, shell fragment, or arbitrary file name. Long-term persistence is the client's responsibility; the daemon's in-memory window is intentionally short.

### UI

Publish a single self-contained HTML file with inline CSS and JavaScript and no CDN dependency. The daemon does not serve this file. The UI has only two primary views:

1. **Charts**: one chart per selected metric, with a short time window, current value, unit, and timestamp.
2. **Top**: a live sortable table for processes or other entities, similar to `top`, showing rank, entity name/ID, CPU, memory, I/O and selected metric columns.

The page must remain visually minimal: one compact header, a metric/entity selector, the chart or table, a small status line, and a download control. It MUST NOT add a general dashboard builder, card grid, alert center, account-management UI, arbitrary query editor, log viewer, or multi-level navigation in the first release. The UI discovers capabilities, hides unavailable metrics, and shows missing/permission/error states inline. It must work on a current desktop browser connecting over the device network. A text-only device does not need to render the UI; CLI remains complete.

Charts should use inline SVG or a small bundled renderer; no CDN, runtime package manager, canvas framework, or external font is required. The Top view refreshes through bounded API polling and defaults to a short refresh interval that is configurable by the page, not by spawning a new collector.

The browser stores session samples in IndexedDB and exports the selected range as TSV, CSV, or JSON. Each session contains explicit `connected`, `disconnected`, and `reconnected` events. A disconnect interval is represented as a gap marker with start/end (or open-ended) timestamps; exporters MUST preserve it and MUST NOT interpolate values across the gap. The UI visibly marks gaps on charts and in exported metadata.

The frontend connects to a user-entered daemon IP, port, and token. `/api/v1/stream` uses SSE for live charts. `/api/v1/export?format=jsonl` and `format=tsv` use HTTP chunked transfer for workstation capture. ProcFace does not copy sysstat command-line formats or require the sysstat package.

### Scheduling and resource limits

Use one collector loop and one HTTP server loop with bounded channels or equivalent. Collection continues when zero browsers are connected and is independent of browser request timing. Defaults are: 1 s sampling interval, 60 s in-memory chart window, one concurrent browser connection, and 256 KiB response limit. The CLI MUST expose `--interval`, `--history-seconds`, and `--max-clients`; values are bounded by build/runtime safety limits. Optional persistence uses `--sqlite PATH`, `--sqlite-max-bytes`, `--sqlite-retention`, and `--sqlite-flush`; it is rejected when SQLite support is not compiled in. The implementation must expose actual collector duration and dropped/late sample counts. A connected follow export must apply backpressure or terminate cleanly; it must not queue unbounded samples. When the client limit is reached, new browser/API connections receive 503 without affecting the active client or collector.

### Security

Run as the invoking user, avoid privilege escalation, and do not follow symlinks outside procfs during metric reads. Default loopback binding prevents accidental exposure. The token is configured at daemon startup with `--token TOKEN`, or generated automatically when omitted. Generated tokens contain 32 bytes of OS cryptographic randomness encoded as 64 hexadecimal characters. Generation failure prevents startup; timestamps and process IDs are not substitutes. The generated token is displayed once on the startup console (stderr) and otherwise stays in process memory; it is not written to a device file. User-provided tokens are not echoed. Command-line tokens can be visible through process arguments, so diagnostic cmdline rendering must redact this option. Restarting with automatic generation invalidates the old token.

## Risks / Trade-offs

- [Kernel variance] → capability discovery and per-field null/error states.
- [HTTP attack surface] → minimal read-only routes, bounded parsing, loopback default, token auth.
- [Flash/RAM pressure] → in-memory bounded ring by default; optional SQLite is explicit, size/retention bounded, and disabled in the minimal build.
- [Browser compatibility] → static no-CDN UI with a small supported-browser baseline; CLI is fallback.
- [Full procfs breadth] → classify families by tier and ship metrics incrementally without blocking the core.

## Open Questions

- Target board CPU/RAM budget and required maximum number of simultaneous browser clients.
- Exact CORS/origin allowlist defaults for the separately hosted frontend.
- Whether token auth is sufficient or TLS termination is provided by an existing device proxy.


### Browser token flow

The static page shows one compact token input before connecting. It sends `Authorization: Bearer <token>` with each API request, including the stream. It never puts credentials in URLs, IndexedDB, exported recordings, localStorage or logs. The token remains only in page memory and is cleared on disconnect; reloading the page requires entering it again. Missing/incorrect credentials return 401 without device data. The backend compares tokens in constant time with bounded credential length and limits failed-auth traffic.

Static assets may be loaded without a token; every device-data API, including capabilities and health, requires authentication. No account database, password reset, cookie session, or token-setting HTTP endpoint is introduced. Token validation is simply an authenticated API request. A diagnostic build plus explicit sensitive-reader opt-in remains necessary even after authentication; possessing a token does not enable omitted build features. No extra interactive confirmation is required after explicit configuration.

If SSE is selected, use a fetch-based streaming client to carry the Authorization header rather than putting the token in a native EventSource URL. For a separately hosted static frontend, allow only configured origins and authorize API data requests; preflight responses reveal no device data. TLS or an SSH tunnel is a separate transport decision: a bearer token alone does not encrypt LAN traffic.





