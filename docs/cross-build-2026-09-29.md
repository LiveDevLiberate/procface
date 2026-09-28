# 四架构交叉构建验证

2026-09-29，在 WSL2 Ubuntu-24.04 使用 Rust 1.97.1、cargo-zigbuild 0.23.4、Zig 0.14.1 完成普通版和 diagnostic 版四架构构建。Rust 源码对应 `5dfd00f`；构建期间的工作区修改仅涉及网页、浏览器测试和 CI。各二进制大小和 SHA-256 见 [原始记录](cross-build-2026-09-29.json)。

普通版位于 `target/TARGET/release/procface`，诊断版位于 `target/diagnostic/TARGET/release/procface`。所有目标均通过发布组装器的 ELF 位宽、机器类型和无动态加载器检查。

已执行：

```sh
cargo zigbuild --locked --release --target x86_64-unknown-linux-musl
cargo zigbuild --locked --release --target armv7-unknown-linux-musleabihf --target aarch64-unknown-linux-musl --target riscv64gc-unknown-linux-musl
cargo zigbuild --locked --release --features diagnostic --target-dir target/diagnostic --target x86_64-unknown-linux-musl --target armv7-unknown-linux-musleabihf --target aarch64-unknown-linux-musl --target riscv64gc-unknown-linux-musl
python3 tests/test_compact.py --binary target/x86_64-unknown-linux-musl/release/procface
python3 tests/test_e2e.py --binary target/diagnostic/x86_64-unknown-linux-musl/release/procface --diagnostic
python3 tests/test_package.py --release-root target --diagnostic-root target/diagnostic
```

组装测试使用临时 Ed25519 密钥，验证八个归档、前端签名、摘要、LICENSE、Pages 与附件 HTML 字节一致，篡改和覆盖发布目录被拒绝。临时产物和测试密钥已自动删除。没有创建正式 Release 或更新 Pages。

x86_64 musl 已在 WSL 实际运行；ARM32、ARM64、RISC-V64 本次仅验证构建与 ELF，尚无对应目标板的启动、采样、SSE、Trace 和资源测量证据，任务 8.2、8.4、8.6、8.7 保持未完成。
