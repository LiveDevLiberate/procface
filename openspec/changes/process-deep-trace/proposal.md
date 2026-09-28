# Proposal: single-process deep trace mode

## Why

System-wide metrics identify pressure, but embedded failures often come from one business process. We need a mode that follows one PID, captures as much readable `/proc/PID` information as practical, and presents it as a compact process detail view with charts and a top-like timeline.

## Scope

- Explicit PID/TID selection; no implicit full-process scan.
- Snapshot and interval metrics from readable `/proc/PID` files.
- Capability discovery per file and field, with cost and sensitivity classification.
- Process identity protection using PID plus starttime; detect exit and PID reuse.
- Optional thread breakdown when explicitly enabled.
- Shared daemon API and minimal UI detail view; CLI TSV remains available.
- No device-side persistence or flash writes.

## Out of scope

No memory reads, state-changing proc files, arbitrary path traversal, full core dump, continuous `smaps` by default, or automatic tracing through perf/eBPF/ftrace.

