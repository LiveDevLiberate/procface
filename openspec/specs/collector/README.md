# Current specifications

The current ProcFace behavior is split into four capability specifications:

- `daemon/`: optional authenticated HTTP service, memory bounds, persistence policy.
- `cli/`: local collection, output, process scope, compatibility semantics.
- `metrics/`: procfs metric catalog and formulas.
- `frontend/`: static UI, browser-local history, gaps, and export.

The older `collector/spec.md` remains as historical context; new requirements should target one of the four capability domains above.
