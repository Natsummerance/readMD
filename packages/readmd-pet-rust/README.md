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

Only BongoCat has a keyboard/mouse scene. It is an explicit library/menu choice
and does not replace Mochi. The other characters retain their full authored art:

- Mochi, Amber and Moss blink and respond with their original wave frames.
- Hermes plays its original spell sequence on interaction and during work,
  with pauses between casts rather than spinning through orientation frames.
- Cache Capy stays calm, acknowledges interaction and uses its thoughtful poses.
- Niu Lai waves and celebrates; both v2 sheets use their original look directions.
- Imported sheets keep named rows, per-row frame counts, dimensions and timing.

Task completion triggers a finite response; work, failure and rest select only
actions that the sheet actually supplies. Alpha geometry keeps complete frames
and props visible and derives the native interaction regions from the artwork.
Key presses do not give these sprites artificial typing animations or sounds.
Settings use an 8–48% range (22% default); all presentations shrink, including
Live2D and BongoCat. Only opaque pixels receive pointer input. A merged scanline
mask updates as authored poses change, and an empty mask passes through. Menus
and dialogue panels receive input only while open; their buttons do not start a
pet drag. Native input honors window occlusion when the pet is not on top.

Always-on-top, position lock, quiet mode, speech hints and BongoCat sound are
persisted by the Rust kernel. Clicking opens localized interaction controls;
character selection and pet/feed/play/rest/wake commands persist through the
same durable bridge. The right-click menu opens ReadMD, clipboard intake,
character selection and settings. Native file drops retain full OS paths,
acknowledge receipt and reach the reader/conversion inbox.

Native hit regions convert CSS coordinates into window coordinates, so WebView
zoom does not offset dragging or head-petting. Readiness waits for the host's
character snapshot and actual painted art; requesting state does not mark the
renderer ready before a sprite or model finishes loading.

Arch-chan keeps its full portrait, follows the pointer with its eyes and head,
blinks, breathes, smiles and blushes when petted. Its brief greeting uses the
original model's mouse-hand parameter. No Cubism mesh vertices are retargeted.
The sprite and Live2D routes share native Rust dragging, head-petting and durable
window bounds. Model drawing uses the packaged Cubism/PIXI WebView.

Offline renderer rebuild (Node 22.13+, an existing packaged library cache):

```text
node packages/readmd-pet-rust/scripts/build-renderer.mjs --renderer-cache EXISTING/renderer --output FRESH/renderer
```

Desktop regression suites are `ui-tests/pet-bongo.spec.js`,
`ui-tests/pet-overhaul.spec.js` and `ui-tests/pet-window-settings.spec.js`, with
`READMD_PET_RENDERER_BUILD` pointing to that output. Native Windows drag checks:

```text
pwsh -NoProfile -File packages/readmd-pet-rust/scripts/smoke-windows.ps1 -Executable HOST.exe -RendererRoot FRESH/renderer -Character mochi
pwsh -NoProfile -File packages/readmd-pet-rust/scripts/smoke-windows.ps1 -Executable HOST.exe -RendererRoot FRESH/renderer -Character arch-chan -Renderer live2d
```

These smoke checks use isolated data/bridge directories and cover the held
pointer, below-threshold motion, persisted position, stale bridge snapshots,
position acknowledgement, actual model pixels, locked dragging, topmost
switching and transparent margins. The smoke runner uses an ephemeral WebView2
debug port and Node built-ins to choose a real opaque point; production never
enables that port.
