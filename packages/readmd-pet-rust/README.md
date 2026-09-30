# ReadMD native desktop-pet host

`readmd-pet-rust` is the production independent desktop runtime. The ReadMD
Rust kernel's pet runtime orchestrator starts this executable first and gives
it the existing renderer bundle from `renderer/`. Electron remains an explicit
compatibility fallback.

The process topology is:

```text
ReadMD (Rust kernel) -> pet runtime orchestrator -> readmd-pet-rust -> Tao native window -> WRY WebView -> existing renderer
```

Build a signed-by-hash package from the `rust/` directory with:

```text
cargo xtask pet-package [--platform windows|macos|linux] [--arch x86_64|aarch64] [--skip-build] [--output DIR]
```

The generated `ReadMD-Pet-Rust.zip` contains the executable, renderer, sprite
assets, Live2D models/vendor runtime, and `runtime-manifest.json`. The installer
promotes it atomically to `<ReadMD>/plugins/pet/readmd-rust-host/`.
