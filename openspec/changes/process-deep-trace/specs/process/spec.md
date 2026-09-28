# Delta specification: single-process deep trace

## ADDED Requirements

### Requirement: Unified command name and feature disclosure

All build profiles MUST install the same command name, `procface`. The `--help` output MUST include a feature list that identifies compiled-in capabilities and their default state, including process tracing, sensitive diagnostics, daemon, HTTP authentication, and UI. The command MUST report when a requested feature is absent from the current build.

#### Scenario: Release help

- **GIVEN** the release build is invoked with `--help`
- **WHEN** help text is rendered
- **THEN** it MUST list normal procfs metrics and CLI capabilities
- **AND** it MUST explicitly mark sensitive diagnostics as unavailable or diagnostic-only

#### Scenario: Development help

- **GIVEN** a diagnostic build is invoked with `--help`
- **WHEN** help text is rendered
- **THEN** it MUST list the available process trace and diagnostic options
- **AND** it MUST state that sensitive reads require explicit opt-in

### Requirement: Explicit single-process selection

The product MUST provide a process trace mode that requires an explicit PID and is the only mode that enables extended or diagnostic procfs readers. It MUST NOT scan or trace all processes as part of this mode. Process identity MUST combine PID with `/proc/PID/stat` starttime.

#### Scenario: Missing PID

- **GIVEN** the user starts process trace without a PID
- **WHEN** argument validation runs
- **THEN** the command MUST fail with an actionable error
- **AND** it MUST not scan `/proc` for candidate processes

### Requirement: Current-user process listing boundary

System-wide process views MUST scan processes owned by the invoking user and visible under the active procfs namespace, using the lightweight field set of Linux `top`. They MUST NOT scan or display all users' processes unless a separate future privileged mode is explicitly specified. Single-process deep trace remains an explicit PID operation.

#### Scenario: Default Top process view

- **GIVEN** the user opens the process view without a user filter
- **WHEN** the process list is collected
- **THEN** it MUST include only processes owned by the invoking user
- **AND** it MUST not enumerate all users' `/proc/PID` directories

### Requirement: Broad readable proc coverage

For the selected PID, the tracer MUST attempt all supported low-cost per-process procfs files and MUST expose a capability result for each file/field as ok, unsupported, permission_denied, parse_error, exited, replaced, or skipped_expensive. It SHOULD support opt-in medium-cost files and explicit diagnostic files.

#### Scenario: Partial permissions

- **GIVEN** `status` and `stat` are readable but `io` and `sched` are not
- **WHEN** a trace snapshot is collected
- **THEN** readable fields MUST be returned
- **AND** blocked files MUST be reported with permission_denied without failing the snapshot

### Requirement: Process lifecycle identity

The tracer MUST identify a process by PID plus `/proc/PID/stat` starttime and MUST never calculate deltas across an exit or PID reuse.

#### Scenario: PID reused

- **GIVEN** the selected PID exits and another process receives the same PID
- **WHEN** the next snapshot observes a different starttime
- **THEN** the state MUST be replaced
- **AND** all counter-derived rates MUST be N/A until a new baseline is established

### Requirement: Procfs collection mode`r`n`r`nProcFace MUST define a runtime procfs mode. `normal` is the default and excludes sensitive or writable procfs files. `diagnostic` is available only in a diagnostic build and requires an explicit opt-in flag such as `--diagnostic-sensitive`. Linux procfs itself has no standard development mode; this mode is a ProcFace policy.`r`n`r`n### Requirement: Safe file boundary

The tracer MUST NOT read or write `mem`, `clear_refs`, `pagemap`, `kpage*`, or other writable proc files in the normal mode. Sensitive fields such as `environ`, fd targets, cwd, root, and raw cmdline MUST be disabled by default and MUST NOT be returned by the default UI. Sensitive readers MAY exist only in a diagnostic build, MUST require an explicit opt-in flag, and MUST be absent from the release build artifact.

#### Scenario: Diagnostic safety

- **GIVEN** a client requests the default process trace
- **WHEN** the tracer enumerates process files
- **THEN** it MUST skip forbidden and sensitive files
- **AND** it MUST not alter process state

#### Scenario: Release build excludes sensitive diagnostics

- **GIVEN** the product is built with the release profile
- **WHEN** a client requests a sensitive process capability or passes a diagnostic flag
- **THEN** the capability MUST be unavailable as an unsupported build feature
- **AND** no sensitive procfs file MUST be opened

#### Scenario: Explicit diagnostic mode

- **GIVEN** a diagnostic build was started with the sensitive diagnostic opt-in
- **WHEN** an authorized developer requests a supported sensitive field
- **THEN** the tracer MAY read that field subject to kernel permissions and configured limits
- **AND** the API/UI MUST mark the result as sensitive diagnostic data

### Requirement: Bounded process trace resources

The tracer MUST allow only one active trace session per daemon, bound the trace to one process, thread count, response size, sampling interval (minimum one second), and its own in-memory history. Starting or stopping a trace MUST NOT clear the daemon ordinary process or system metric history. Full `smaps`, thread summaries, and diagnostic files MUST require explicit opt-in and MUST report their measured collection duration.

#### Scenario: Long-running trace

- **GIVEN** a process is traced for one hour
- **WHEN** the daemon is connected to a browser
- **THEN** device memory MUST remain bounded by the configured short window
- **AND** no trace data MUST be written to flash

### Requirement: Process detail API and visualization

The daemon MUST expose `/api/v1/processes/{pid}/capabilities`, `current`, `series`, and `stream` endpoints for the selected process. The UI MUST provide a minimal process detail view with identity/lifecycle status, current CPU/memory/I/O summary, selectable charts, and an optional sortable thread table.

#### Scenario: Process exits

- **GIVEN** the traced process exits
- **WHEN** the UI refreshes the process detail view
- **THEN** it MUST show exited status and retain the last bounded in-memory points
- **AND** it MUST stop presenting new rates as current values











