# ReadMD pet-rust Phase 0 spike

This directory is the only permitted location for the v1.4.7 Rust migration
spike. It is intentionally disconnected from the production Python/Electron
host. The crate exercises the parts that can be checked without a desktop
session: bounded snapshot parsing, typed DIP geometry, durable command
envelopes, and generation-aware state reconciliation.

`PlatformBackend` is a contract surface, not a platform implementation. No
Wayland, GNOME, Win32, Cocoa, Tao, or WRY window is created here. The document
requires physical evidence for those backends, so this spike must stay
`Planned`/`BLOCKED` until the Phase 0 matrix has real machine evidence.

## Local smoke check

With Rust 1.85 or newer installed:

```text
cargo test --manifest-path experiments/pet-rust/Cargo.toml
cargo run --manifest-path experiments/pet-rust/Cargo.toml --bin pet-rust-spike
```

The dependency set follows the v1.4.7 Cargo specification, while the crate's
default binary remains protocol-only. Declaring these platform libraries does
not certify a backend or authorize a production host; physical evidence is
still required before any platform implementation can be promoted.
