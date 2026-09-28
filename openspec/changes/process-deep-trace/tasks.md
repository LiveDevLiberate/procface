# Tasks

- [ ] Define the supported per-process file/field catalog and sensitivity/cost tiers.
- [ ] Add fixtures for stat parsing with spaces/parentheses, status, io, statm, sched, smaps_rollup, and PID reuse.
- [ ] Implement process identity and lifecycle state machine.
- [ ] Implement low-cost snapshot adapters and counter deltas.
- [ ] Implement optional medium/diagnostic adapters with explicit flags and limits.
- [ ] Implement process capabilities/current/series/export API routes.
- [ ] Add minimal process detail charts and top-style thread table to the UI.
- [ ] Add forbidden-file and secret-redaction tests.
- [ ] Measure trace overhead on target boards with and without threads/smaps.
- [ ] Update baseline specs and archive this change after acceptance.

