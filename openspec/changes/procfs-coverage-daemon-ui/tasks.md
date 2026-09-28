# Tasks

- [ ] Confirm target boards, kernel versions, libc/toolchain, RAM budget, and browser/client count.
- [ ] Inventory procfs files present on representative targets and classify metric families.
- [ ] Define normalized metric IDs, units, entity IDs, and capability/error schema.
- [ ] Implement adapters and fixture tests for each procfs family in the first release tier.
- [ ] Implement shared sampler, bounded ring buffer, late/dropped counters, and capability endpoint.
- [ ] Define optional SQLite feature/build flag, schema, size/retention limits, and flash-write warning.
- [ ] Implement persistence-disabled default and explicit `--sqlite` configuration reporting.
- [ ] Add `--interval`, `--history-seconds`, and `--max-clients` validation with safe upper/lower bounds.
- [ ] Implement bounded read-only HTTP server and `/api/v1` JSON routes.
- [ ] Implement loopback default, mandatory API bearer authentication, CLI token configuration, secure auto-generation, and a minimal browser token input.
- [ ] Build static no-CDN UI with capability discovery and charts for CPU/memory/load/disk/network/process.
- [ ] Implement the separately hosted GitHub frontend: daemon IP/port/token form, SSE chart stream, IndexedDB session storage, disconnect-gap events, and TSV/CSV/JSON browser export.
- [ ] Define and implement native `sample`, `trace`, `daemon`, and `capabilities` CLI options without sysstat compatibility aliases.
- [ ] Verify release build retains daemon/UI while excluding sensitive diagnostic readers and routes.
- [ ] Add daemon resource benchmark on target board and document measured defaults.
- [ ] Add security and malformed-request tests.
- [ ] Update baseline `openspec/specs/collector/spec.md` after acceptance and archive this change.


- [ ] Test unauthorized requests, generated-token restart invalidation, token redaction, streaming headers, and exclusion of credentials from browser history/export.
- [ ] Test zero-client continued sampling, browser disconnect/reconnect, and one-client rejection behavior.




