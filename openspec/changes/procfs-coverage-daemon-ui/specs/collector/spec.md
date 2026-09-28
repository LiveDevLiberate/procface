# Delta specification: procfs coverage and daemon UI

## ADDED Requirements

### Requirement: Capability-driven procfs coverage

The collector MUST expose every supported procfs metric family implemented for the target release and MUST report unavailable, permission-denied, and parse-error states per family and field. A missing optional procfs file MUST NOT stop unrelated collection.

#### Scenario: Kernel without PSI

- **GIVEN** `/proc/pressure` is absent
- **WHEN** the collector starts
- **THEN** capabilities MUST mark PSI as unsupported
- **AND** CPU, memory, load, disk, network, and process groups MUST continue when their sources are readable

### Requirement: Optional daemon mode

The product MUST provide an opt-in daemon mode that runs the same collector model as the CLI and listens on a configurable HTTP address and port. CLI-only operation MUST remain available without starting the daemon.

#### Scenario: Default daemon bind

- **GIVEN** the daemon starts without an explicit listen address
- **WHEN** it becomes ready
- **THEN** it MUST bind only to loopback
- **AND** it MUST expose a health endpoint and capabilities endpoint

### Requirement: Configurable bounded sampling and clients

Daemon mode MUST default to a one-second sampling interval, a 60-second in-memory chart window, and one concurrent browser connection. The CLI MUST provide configuration for interval, retained window duration, and maximum browser connections. The collector MUST continue sampling when no browser is connected. Values MUST be bounded and invalid or excessive values MUST fail with an actionable error.

#### Scenario: Browser disconnect

- **GIVEN** the daemon is sampling and the only browser disconnects
- **WHEN** no client remains connected
- **THEN** collection MUST continue at the configured interval
- **AND** the next browser MUST see current data without requiring a collector restart

#### Scenario: Connection limit

- **GIVEN** `--max-clients 1` and one browser/API client is connected
- **WHEN** a second browser/API connection is attempted
- **THEN** the daemon MUST return 503 or an equivalent capacity error
- **AND** the active client and collector MUST continue unaffected

### Requirement: Read-only HTTP API

The daemon MUST provide versioned read-only JSON endpoints for capabilities, current metrics, historical series, and health. It MUST provide `/api/v1/stream` as SSE for live charts and `/api/v1/export?format=jsonl` or `format=tsv` as HTTP chunked streams for workstation capture. It MUST reject invalid routes, unbounded ranges, request bodies, and any attempt to execute commands or access arbitrary files.

#### Scenario: Bounded series query

- **GIVEN** a valid metric, entity, and time range within configured limits
- **WHEN** a client requests `/api/v1/series`
- **THEN** the daemon MUST return samples in timestamp order
- **AND** it MUST report whether samples were dropped or delayed

#### Scenario: Historical export

- **GIVEN** a valid bounded time range and metric filter within the retained history
- **WHEN** a client requests the export endpoint with `format=tsv` or `format=csv`
- **THEN** the daemon MUST stream only matching samples with a downloadable content type
- **AND** it MUST include schema/column information in the export header
- **AND** it MUST never expose a local history filename or arbitrary filesystem path

### Requirement: Browser visualization

The daemon MUST provide only the authenticated API; the UI is a single self-contained HTML file released from GitHub. The UI MUST provide only two primary views: metric charts and a live Top-style sortable entity table. It MUST discover capabilities, render charts for available metric groups, and visibly distinguish unsupported, permission-denied, parse-error, and missing data states. The first release MUST NOT include a dashboard builder, alert center, log viewer, arbitrary query editor, or multi-level navigation.

#### Scenario: Optional metric group

- **GIVEN** the kernel does not provide a metric group
- **WHEN** the UI loads capabilities
- **THEN** it MUST hide or disable that group with a reason
- **AND** it MUST continue rendering available groups

#### Scenario: Minimal chart view

- **GIVEN** a client selects one available metric
- **WHEN** the chart view is displayed
- **THEN** the UI MUST show that metric's time series, current value, unit, and timestamp
- **AND** it MUST NOT require a configurable dashboard or external chart service

#### Scenario: Top-style entity view

- **GIVEN** process or device samples are available
- **WHEN** the Top view is displayed
- **THEN** the UI MUST show a compact sortable table with rank, entity identity, and selected resource columns
- **AND** it MUST refresh through bounded API polling without starting a new collector

#### Scenario: Frontend export formats

- **GIVEN** the browser has collected session samples
- **WHEN** the user selects a range and chooses TSV, CSV, or JSON export
- **THEN** the browser MUST generate the file locally from its IndexedDB/session data
- **AND** the export MUST include schema information and connection-gap markers

#### Scenario: Connection gap marking

- **GIVEN** the browser loses the daemon connection between two samples
- **WHEN** the connection is restored
- **THEN** the frontend MUST record a disconnected interval
- **AND** charts and exports MUST show a gap rather than interpolate values

### Requirement: Embedded resource bounds

Daemon mode MUST use only a short bounded in-memory chart window and bounded HTTP responses by default. It MUST NOT write metric data, history, cache, logs, pidfiles, or UI state to flash or any other local filesystem unless the operator explicitly enables the optional SQLite backend. SQLite support MAY be omitted from the minimal build. When enabled, it MUST require an explicit database path, byte limit, retention policy, and flush policy; the daemon MUST report persistence state. Default configuration MUST be measurable and overridable for the target board. The daemon MUST expose collector duration and late/dropped sample counters. Without SQLite, long-term capture MUST be performed by a client consuming the export stream and saving data off-device.

#### Scenario: History limit

- **GIVEN** the short chart buffer reaches `max_samples`
- **WHEN** a new sample is collected
- **THEN** the oldest sample MUST be evicted
- **AND** the process memory MUST remain bounded independently of runtime duration

#### Scenario: Long-term workstation capture

- **GIVEN** a developer needs a 30-minute diagnostic capture
- **WHEN** the workstation opens `/api/v1/export?format=tsv&follow=1`
- **THEN** the daemon MUST stream new samples without retaining the full session
- **AND** the workstation can save the stream locally for later analysis

#### Scenario: No device-side history

- **GIVEN** the daemon runs continuously on a device with writable flash
- **WHEN** samples are collected and a browser is disconnected
- **THEN** the daemon MUST retain only the bounded in-memory chart window and counter baselines
- **AND** it MUST NOT create or append a metric history file on the device

#### Scenario: Explicit SQLite persistence

- **GIVEN** the build includes SQLite and the operator supplies `--sqlite PATH` with size and retention limits
- **WHEN** the daemon starts
- **THEN** it MAY persist samples only within that configured database and limits
- **AND** the health/capabilities response MUST report SQLite as active
- **AND** it MUST fail startup if the path or limits are invalid rather than silently falling back to unbounded writes

### Requirement: Safe network exposure

The daemon MUST bind to 127.0.0.1 by default. LAN listening MUST require explicit configuration. Every device-data API MUST require a bearer token regardless of bind address, without an unsafe bypass. Static assets MAY be public. Sensitive readers MUST require a diagnostic build and explicit opt-in independently of authentication.

#### Scenario: Default generated credential

- **WHEN** the daemon starts without `--token`
- **THEN** it MUST generate a token from 32 bytes of OS cryptographic randomness, display its 64-character hexadecimal encoding once on startup stderr, and keep it only in memory
- **AND** it MUST fail startup if secure generation fails

#### Scenario: Explicit credential

- **WHEN** the operator supplies `--token TOKEN`
- **THEN** the daemon MUST use that token without echoing or persisting it
- **AND** diagnostic command-line output MUST redact that credential

#### Scenario: Browser authentication

- **WHEN** the user enters a token in the static website
- **THEN** API and streaming requests MUST submit it in the Authorization Bearer header
- **AND** the frontend MUST retain it only in page memory, excluding browser databases and exports

#### Scenario: Missing or invalid credential

- **WHEN** an API request supplies no token or an incorrect token
- **THEN** the daemon MUST return 401 without returning device data
- **AND** this MUST also apply to loopback, capabilities, health and sensitive-process endpoints

#### Scenario: Development sensitive access

- **GIVEN** a valid API token
- **WHEN** sensitive process fields are requested
- **THEN** access MUST additionally require an included development feature and explicit runtime opt-in
- **AND** no additional interactive confirmation MUST be required after that opt-in

### Requirement: Native command model

ProcFace MUST use its native `sample`, `trace`, `daemon`, and `capabilities` command model. It MUST NOT require, invoke, or shell out to `iostat`, `mpstat`, `pidstat`, or the sysstat package.

#### Scenario: Native invocation

- **GIVEN** a user invokes a supported common `mpstat`-style option through ProcFace
- **WHEN** the command is parsed
- **THEN** ProcFace MUST select its own CPU metric view
- **AND** execution MUST succeed when external sysstat commands are absent

### Requirement: Release feature boundary

The release build MUST include the daemon, authenticated HTTP API, and normal procfs metrics. The separately released frontend is not part of the daemon binary. It MUST omit sensitive process diagnostic readers and routes. Diagnostic builds MAY include them behind explicit opt-in.

## MODIFIED Requirements

### Requirement: Minimal runtime dependencies

CLI mode MUST continue to meet the existing BusyBox/procfs dependency goal. Daemon/UI mode MAY add a small HTTP implementation and static assets, but MUST NOT require Python, Node.js, a database, systemd, or a cloud service.







