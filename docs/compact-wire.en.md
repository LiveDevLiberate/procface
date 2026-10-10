# Compact wire format implementation notes

[简体中文](compact-wire.md) | English

Current/series/stream responses, process lists and process current/series/stream responses use compact data by default. JSONL exports also use compact batches. The old object-output branch has been removed. `wire=compact` may be specified explicitly, but omitting it has the same result. CLI, TSV and Prometheus output are unaffected. OpenSpec tasks 7.11–7.13 still require full acceptance.

The API major version remains 1. The wire schema has its own identifier, `procface-compact-v1`. Browser downloads remain human-readable.

## Batches and dictionaries

Batch layout: `[sequence, uptime, timestamp_unix, group_id, complete, diagnostics, dictionary_delta, samples, processes]`.

Sample layout: `[metric_id, entity_id, value, status_id]`.

- Metric definition: `[id, name, unit, kind]`. IDs come from the explicit published catalog, never collection order.
- Entity definition: `[id, name]`. IDs increase within a daemon session and are never reused.
- Group IDs 0–2: `system/process/trace`.
- Status IDs 0–6: `ok/unsupported/permission_denied/parse_error/exited/stale/discontinuity`.

Each encoded batch currently carries the dictionary subset it references, before its samples. Definitions may repeat, allowing decoding from any point within a history window. A future stream optimization may omit previously sent definitions, provided initial definitions and reconnection context are retained and measured payload savings justify the change.

Encoded batches hold strong references to entity definitions; the catalog holds weak references. Definitions can be reclaimed only after caches, queues and exports release all referencing batches. An entity name encountered after reclamation receives a new ID. Byte budgets must account for both batches and references. The browser stores expanded records and restores reconnection context from series responses containing dictionaries.

## Process records

Layout: `[entity_id, pid, starttime_ticks, name, state, uid, rss_bytes, threads, cpu_seconds, cpu_percent, sample_mask, cpu_status_id]`.

The seven `sample_mask` bits represent PID, name, state, RSS, thread count, CPU seconds and CPU percentage samples, in that order. Samples are merged only when they actually exist, values match and status can be preserved. The frontend reconstructs readable samples according to the mask, avoiding invented samples in filtered responses. CPU percentage status is stored separately: an initial `null/stale` must not become `null/ok`. Other exceptional samples, exited entities and `virtual_bytes` remain in the sample array.

## Validation and remaining work

The implementation record reports 16 passing offline Rust tests at the time it was written, including independent decoder checks for process deduplication, filtering, error states, structured values and maximum u64 round trips. A sample envelope is smaller than the original object representation. Tests also check stable catalog IDs, reference reclamation, non-reused IDs and rejection of duplicate IDs, unknown metrics and unit/kind conflicts. This count is historical, not a claim about the current test total.

The published metric catalog is `src/metric-catalog.tsv`. Existing IDs must not change or be reused; append explicit IDs for new metrics. Static IDs range from 1 through 2^32−1. `trace.thread.TID.stat` uses `2^32 + TID` in the reserved range, covering nonzero u32 TIDs while remaining within JavaScript's safe integer range. Thread names and Trace text retain their existing meaning. Dynamic definitions accompany their batches.

Parser coverage and fixture-based system/process/extended Trace round-trip tests are included. Diagnostic builds additionally cover missing sensitive files. Integration tests use `tests/compact_wire.py` as an independent decoder. `test_compact.py` covers default arrays, independently decodable batches, dynamic network devices, process filters, pagination metadata and JSONL. `tools/test_frontend_compact.cjs` checks IndexedDB, reconstructed process samples, statuses, u64 values and structured Trace in a real browser.

The original implementation record still lists strict handshake/schema compatibility, bounded recovery for unknown dictionaries, export boundaries, real-load payload measurements, four-architecture runtime checks and release-artifact verification as acceptance work. Consult current OpenSpec tasks and later verification records for progress; passing regressions alone do not replace that acceptance.
