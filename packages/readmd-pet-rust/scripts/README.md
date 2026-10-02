# Offline desktop pet build and verification

The Windows host captures Raw Input in Rust and delegates dragging to user32.
The sprite presentation uses the original BongoCat standard model for Mochi's
keyboard desk; the other characters keep their ReadMD artwork. The presentation
uses the existing Tao/WRY and PIXI/Cubism renderer. It is not a new Rust GPU
renderer. Upstream attribution is in `third_party/bongocat/`.

Node 22.13+ can rebuild the tracked TypeScript and JavaScript without npm/Vite
installation. Supply a previous ReadMD renderer as the library/character cache:

```powershell
node packages/readmd-pet-rust/scripts/build-renderer.mjs --renderer-cache <previous-runtime>/renderer --output packages/readmd-hermes-pet-adapter/dist/renderer
$env:CARGO_NET_OFFLINE = 'true'
cargo build --offline --release --manifest-path packages/readmd-pet-rust/Cargo.toml
cargo run --offline --manifest-path rust/Cargo.toml -p xtask -- pet-package --skip-build --output <new-package-directory>
```

Keep cache and output directories different. The renderer build reads only the
four existing PIXI/Cubism chunks and six existing sprites from the cache;
ReadMD entry, stage, companion layer and BongoCat model come from tracked source.
No network, dependencies or directory deletion are needed. `CARGO_TARGET_DIR`
is honored by the native packager. `pet-package` rebuilds the presentation again
in the new package directory, preventing a stale dist entry from being shipped.

```powershell
cargo test --offline --manifest-path packages/readmd-pet-rust/Cargo.toml --lib
$env:READMD_PET_RENDERER_BUILD = '<absolute-renderer-build>'
# Use the project's already installed Playwright executable; do not install.
playwright test pet-bongo.spec.js --config ui-tests/playwright.config.js --project=desktop --retries=0
pwsh -NoProfile -File packages/readmd-pet-rust/scripts/smoke-windows.ps1 -Executable <absolute-host-exe> -RendererRoot <absolute-renderer-build>
```

The native smoke starts an isolated host with temporary bridge/state files. It
checks the drag threshold, actual movement, durable final position, stale
snapshot protection, acknowledgement and transparent-margin click-through.
It restores the pointer, closes only its own host and leaves its fixture for
inspection. It does not read or alter the user's settings or desktop pet data.

The browser checks cover modifier chords, fast press/release edges, independent
mouse buttons, stale input rejection, real Cubism hand/key layers, transparency,
the six characters at 320x420 and 480x560, and the optional Arch Chan stage.
