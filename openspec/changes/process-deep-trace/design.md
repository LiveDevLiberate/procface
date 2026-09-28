# Design: single-process deep trace

## Context

Linux documents many per-task procfs files, but availability and permissions vary. Some files are cheap counters, some are expensive snapshots, and some expose sensitive data or alter state. The trace mode must maximize useful coverage without making “all information” an unbounded or unsafe operation.

## Goals / Non-Goals

**Goals:**

- Capture every allowed, readable, known per-process file in the configured cost tier.
- Keep a stable normalized identity and status for the traced process.
- Make expensive fields explicit and opt-in.
- Serve a compact process detail view from the same daemon API.

**Non-Goals:**

- Do not promise literally every kernel-specific file or field.
- Do not read process memory or write procfs control files.
- Do not store a trace on the device.
- Do not include sensitive diagnostic readers in the production release artifact.

## Decisions

### File tiers

**Always/low cost:** `stat`, `status`, `io`, `statm`, `cmdline` (display name only), `cgroup` (if readable), and task identity metadata.

**Opt-in medium cost:** `sched`, `schedstat`, `smaps_rollup`, `fd` count, `limits`, `loginuid`, `mountinfo`-related context where available, and explicit thread summaries.

**Explicit diagnostic only:** full `smaps`, `maps`, `numa_maps`, `wchan`, `stack`, `pagemap`, `kpage*`, `mem`, `environ`, `cwd`, `root`, and fd target names. These may be sensitive, expensive, restricted, or contain secrets. `clear_refs` and any writable proc file are forbidden.

The sensitive diagnostic tier MUST be compiled only into a diagnostic build profile. A release build MUST omit the readers and routes, not merely hide them behind a runtime flag. The executable name remains `procface` for every build. `procface --help` MUST print a feature list showing whether process tracing, medium-cost readers, sensitive diagnostics, daemon, and UI are compiled in, plus the default security posture. In a diagnostic build, each sensitive capability requires an explicit command-line/configuration flag (for example `--diagnostic-sensitive`) without an additional interactive confirmation. The output and `/api/v1/processes/.../capabilities` response MUST identify that sensitive mode is active.

The implementation may discover unknown regular files under `/proc/PID`, but it MUST classify them as unknown and skip them unless a future schema explicitly allows them. It MUST never follow symlinks or construct paths from untrusted HTTP input.

### Identity and lifecycle

At start, read PID `stat` and capture starttime, boot/session identity if available, and command name. Every interval re-check starttime. If the directory disappears, emit `exited`; if the same PID appears with another starttime, emit `replaced` and reset all deltas.

### Data model and API

Expose a process trace as capabilities plus snapshots/deltas:

```text
GET /api/v1/processes/{pid}/capabilities
GET /api/v1/processes/{pid}/current
GET /api/v1/processes/{pid}/series?metric=...&from=...&to=...
GET /api/v1/processes/{pid}/export?format=tsv&follow=1
```

The PID path is validated as a decimal identifier and is not used as a filesystem path outside the daemon's internal proc root. Responses contain process identity, lifecycle state, timestamp, metric values, unit, source file, cost tier, and status. The daemon holds only the current/previous snapshot and the same short chart window as system metrics; clients own long-term storage. Trace collection runs in one isolated low-priority worker with cooperative cancellation and bounded queues.

### UI

Add a minimal process detail view, reachable after an explicit PID selection. It contains: identity/status header, current resource summary, selectable time-series charts, a compact table of readable fields grouped by CPU/memory/I/O/scheduling/identity, and an optional thread table. It must show unsupported/permission/expensive/skipped reasons inline and never display raw environment or command-line secrets by default. The UI MUST NOT expose sensitive diagnostic controls in release builds; diagnostic builds must show a prominent diagnostic-mode marker before enabling them.

## Risks / Trade-offs

- [Large per-process surface] → tiered files, capability discovery, bounded output.
- [Secrets in cmdline/environ/fd] → display command basename only; sensitive files disabled by default.
- [PID reuse] → starttime identity and lifecycle states.
- [High cost of smaps/threads] → explicit flags and measured resource limits.
- [Kernel-specific files] → known schema plus unknown/unsupported status.




