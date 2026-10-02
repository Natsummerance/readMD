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

The original Mochi, Hermes, Amber, Moss, Cache Capy and Niu Lai sprites keep
their own artwork. Their desk poses retarget arm and palm cutouts from the
unchanged source sheets. Host-delivered original sheets use the same rig;
other imported sheets use their named action rows, frame counts and dimensions,
with interaction regions derived from their visible pixels. BongoCat is an
explicit choice in the overlay menu and does not replace Mochi by default.

Arch-chan uses its original Cubism mouse parameters, with a reversible mesh
rig for keyboard/mouse desk contact. Its shoulders, sleeves and skin retain the
original texture. The renderer restores all vertex changes before the next
Cubism update and when leaving the desk pose. The sprite and Live2D routes
share the native Rust input capture, drag threshold, head-petting and durable
window bounds. Model drawing still uses the packaged Cubism/PIXI WebView.

Offline renderer rebuild (Node 22.13+, an existing packaged library cache):

```text
node packages/readmd-pet-rust/scripts/build-renderer.mjs --renderer-cache EXISTING/renderer --output FRESH/renderer
```

The desktop regression suite is `ui-tests/pet-bongo.spec.js`, with
`READMD_PET_RENDERER_BUILD` pointing to that output. Native Windows drag checks:

```text
pwsh -NoProfile -File packages/readmd-pet-rust/scripts/smoke-windows.ps1 -Executable HOST.exe -RendererRoot FRESH/renderer -Character mochi
pwsh -NoProfile -File packages/readmd-pet-rust/scripts/smoke-windows.ps1 -Executable HOST.exe -RendererRoot FRESH/renderer -Character arch-chan -Renderer live2d
```

These smoke checks use isolated data/bridge directories and cover the held
pointer, below-threshold motion, persisted position, stale bridge snapshots,
position acknowledgement and transparent margins.
