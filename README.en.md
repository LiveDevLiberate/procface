# ProcFace

[简体中文](README.md) | English

A procfs performance analysis tool for embedded Linux, written in Rust without sysstat or BusyBox command dependencies.

- CLI sampling and single-process Trace, with table, TSV, JSON and JSONL output.
- A daemon providing live APIs, with no persistence by default; SQLite is available in diagnostic builds.
- A standalone Chinese/English HTML frontend with live charts, a top-style process table, browser history and data export.

Currently a development release targeting ARM32, ARM64, RISC-V64 and x86_64. Target-board compatibility still needs verification.

## Quick start

Build on Linux with Rust 1.85+:

```sh
cargo build --release --locked
./target/release/procface sample --count 3 --format jsonl
./target/release/procface trace 1234 --count 3
```

For local frontend development, run these commands in separate terminals:

```sh
./target/release/procface daemon --allow-origin http://localhost:8000 --allow-unsigned-frontend
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
```

Open <http://localhost:8000/procface-web.html>, enter `http://127.0.0.1:9387` and the token printed to stderr. Sampling defaults to one second and the daemon listens on loopback only. See the user guide for LAN configuration.

## Documentation and downloads

- [Documentation site (中文 / English)](https://livedevliberate.github.io/procface-docs/)
- [Hosted frontend](https://livedevliberate.github.io/procface/) · [Downloads](https://github.com/LiveDevLiberate/procface/releases)
- [User guide](docs/usage.en.md): CLI, daemon, API, diagnostic builds and testing
- [Releases and signature verification](docs/release.en.md) · [Compact protocol](docs/compact-wire.en.md) · [Internationalization](docs/i18n.en.md)

[MIT license](LICENSE). ProcFace is an independent implementation; `upstream-sysstat` is a research reference only.
