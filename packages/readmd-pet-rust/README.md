# ReadMD native desktop-pet host

`readmd-pet-rust` is the production independent desktop runtime. ReadMD's
Python `PetRuntimeOrchestrator` starts this executable first and gives it the
existing renderer bundle from `renderer/`. Electron remains an explicit
compatibility fallback.

The process topology is:

```text
ReadMD -> Python PetRuntimeOrchestrator -> readmd-pet-rust -> Tao native window -> WRY WebView -> existing renderer
```

Build a signed-by-hash package from the repository root with:

```text
python packages/readmd-pet-rust/scripts/build-package.py
```

The generated `ReadMD-Pet-Rust.zip` contains the executable, renderer, sprite
assets, Live2D models/vendor runtime, and `runtime-manifest.json`. The installer
promotes it atomically to `<ReadMD>/plugins/pet/readmd-rust-host/`.
