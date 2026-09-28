# Proposal: bootstrap procfs CLI collector

## Why

The daemon and minimal UI depend on correct procfs parsing, counter delta handling, capability states, and a stable sample schema. These pieces should be implemented and tested through a low-dependency CLI before adding HTTP and browser concerns.

## Scope

- Rust binary CLI entrypoint with ProcFace-native commands and a BusyBox-friendly deployment environment.
- Injectable `/proc` root for fixture tests.
- CPU, memory, load/system activity, block I/O, and network adapters.
- Actual uptime-based intervals and discontinuity handling.
- Table and TSV output with stable columns and explicit status values.
- No daemon, HTTP server, UI, filesystem history, sysfs, or JSON in this change.
- Default process views are limited to processes visible to the invoking user; no all-user scan.

## Exit criteria

A developer can run finite collection against a fixture or a real `/proc`, compare rates with known counter changes, and redirect TSV to a workstation without device-side file creation.


- CLI 不复制 iostat、mpstat、pidstat 命令格式；输出支持 table、TSV、JSON 和 JSONL。

