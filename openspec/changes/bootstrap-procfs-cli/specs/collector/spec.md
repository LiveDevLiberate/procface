# Delta specification: bootstrap procfs CLI collector

## ADDED Requirements

### Requirement: Low-dependency CLI baseline

The first implementation MUST provide a finite-sampling Rust CLI with ProcFace-native commands and options, and table/TSV/JSON/JSONL output. It MUST NOT require or invoke external `iostat`, `mpstat`, or `pidstat` commands. Daemon/UI and device-side history are separate features.

#### Scenario: Real procfs smoke run

- **GIVEN** a device has readable `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`, and `/proc/uptime`
- **WHEN** the user runs a finite collection
- **THEN** the CLI MUST emit the requested number of windows
- **AND** it MUST exit without creating a device-side file

### Requirement: Injectable procfs root

The CLI MUST accept a procfs root override for deterministic fixture tests. All adapters MUST resolve source paths below that root and MUST NOT read host paths when the override is set.

#### Scenario: Fixture isolation

- **GIVEN** `--proc-root fixtures/proc` is supplied
- **WHEN** collection runs
- **THEN** all metric reads MUST come from that directory
- **AND** a missing fixture file MUST produce an explicit capability/error state

### Requirement: Stable sample output

The CLI MUST provide table, TSV, JSON, and JSONL output with stable metric IDs, units, sample number, actual interval, and status. JSON and JSONL MUST preserve calculated numeric precision. It MUST write diagnostics to stderr and MUST NOT mix prose into TSV data.

#### Scenario: Discontinuity output

- **GIVEN** a counter decreases between two fixture snapshots
- **WHEN** a rate is rendered
- **THEN** the value MUST be `N/A` with `discontinuity` status
- **AND** the CLI MUST continue collecting unrelated metrics



### Requirement: Monotonic sampling time

The CLI MUST calculate rates using measured `/proc/uptime` differences. It MUST default to uptime output, MAY add Unix timestamps with `--timestamp unix`, and MUST NOT require timezone or localization data. JSON and JSONL MUST preserve calculated numeric precision.

#### Scenario: Wall-clock jump

- **GIVEN** the device wall clock changes between two samples
- **WHEN** the next sample is computed
- **THEN** rates MUST use the uptime interval
- **AND** the wall-clock change MUST NOT create a false counter delta

### Requirement: Native command model

The CLI MUST expose `sample` and `trace` as ProcFace-native operations. It MUST NOT reproduce the historical command syntax of `iostat`, `mpstat`, or `pidstat`.

#### Scenario: Native system sample

- **GIVEN** the user requests `procface sample --metrics cpu,memory`
- **WHEN** arguments are parsed
- **THEN** the CLI MUST select the requested metric groups using the ProcFace option model
- **AND** it MUST not require any sysstat executable
