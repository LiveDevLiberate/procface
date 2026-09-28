# Proposal: full procfs coverage and embedded daemon UI

## Why

The current scope covers a small set of procfs metrics and one-shot CLI analysis. Embedded troubleshooting also needs broad visibility into the metrics exposed by procfs and a mode that can be viewed remotely without installing a separate analysis stack on the device.

## What changes

- Expand the metric catalog to cover available procfs families, with capability detection and explicit unsupported/permission states.
- Keep the CLI as the primary low-dependency interface.
- Add an optional daemon mode that samples metrics and exposes a local HTTP API on a configurable address and port.
- Add a static browser UI that renders charts from the API; assets may be embedded in the binary/script package or served from a configured directory.
- Make daemon mode opt-in and resource-bounded. It must not be required for CLI use.
- Keep device persistence disabled by default, while allowing an explicitly configured SQLite history backend for boards that have suitable writable storage.
- Define security defaults: loopback bind by default, no shell execution through HTTP, bounded request/response sizes, and optional token authentication for non-loopback binds.

## Scope boundaries

The daemon is for local device diagnostics, not a multi-tenant monitoring server. The first UI provides time-series charts and tables for collected data; it does not provide alert rules, remote code execution, package management, or arbitrary file browsing.

## Impact

This change adds a long-running process, HTTP parsing, bounded in-memory retention, and static UI assets. It introduces additional implementation and security work, so the daemon and UI remain optional build/runtime features.

