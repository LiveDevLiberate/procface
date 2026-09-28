# Tasks

- [ ] Confirm implementation form: BusyBox ash/awk script layout and install path.
- [ ] Define metric catalog: IDs, units, source files, counter/gauge type, and error states.
- [ ] Create representative procfs fixtures, including malformed and missing files.
- [ ] Implement exact decimal counter delta handling in awk-compatible code.
- [ ] Implement `/proc/uptime` interval and delayed-window detection.
- [ ] Implement CPU and system activity collection.
- [ ] Implement memory collection.
- [ ] Implement diskstats and net/dev collection with entity baseline handling.
- [ ] Implement CLI options, signal handling, and exit codes.
- [ ] Implement table and TSV renderers.
- [ ] Add fixture-based golden tests and a real-host smoke command.
- [ ] Measure CPU time, RSS, process launches, and output volume on a representative board.
- [ ] Review and merge the delta spec into the baseline after acceptance.


- [ ] Implement uptime-based time semantics, optional --timestamp unix, RTC-unavailable states, and JSON precision preservation.

