# BongoCat reference and adaptation

- Repository: https://github.com/ayangweb/BongoCat
- Revision: `e5922f3c71716ba06531324f4ab5f8066b2cc487`
- License: Apache-2.0 (full text in LICENSE).
- ReadMD adaptation date: 2026-10-02.

The Raw Input byte decoder, physical scan-code/HID mapping and modifier
normalization in `packages/readmd-pet-rust/src/input/windows.rs` are adapted
from `crates/bongocat-platform/src/windows.rs` at the pinned revision.
ReadMD retains its own Tao/WRY renderer ABI, window lifecycle and state storage.

The held-control/reconciliation behavior in `src/input/pressed.rs` follows
`crates/bongocat-runtime/src/input_state/`: first down edges trigger attacks,
repeats do not add held controls, mouse and keyboard releases are independent,
two missing system-state samples release a captured control, and lifecycle
resets clear all held controls. The elapsed-time cursor interpolation follows
`shared/behavior/input-semantics.md` (`1 - 0.75^(seconds * 60)`).

Native placement follows BongoCat's Windows caption-drag contract. ReadMD's
Rust input worker detects the drag threshold, user32 moves the window, and the Rust host
persists the measured position after `WM_EXITSIZEMOVE`. Polling never moves it.

The classic cat, keyboard, mouse and key overlays in
`packages/readmd-pet-rust/models/bongocat-standard/` are copied unchanged from
upstream `resources/models/standard/` at the same revision. The upstream project
credits MMmmmoko's Bongo-Cat-Mver as the original model/project inspiration.
ReadMD adds only `keys.json`, a sorted inventory of those existing key images.
The models are packaged under `models/bongocat-standard/`, with this license and
notice in `licenses/bongocat/`. No upstream Cubism SDK binaries are copied;
the renderer reuses ReadMD's existing packaged Cubism runtime and PIXI chunks.
The remaining ReadMD characters retain their full authored sprite actions or
Live2D portrait. Only BongoCat has keyboard/mouse layers; no extra forearms or
instruments are attached to the other characters.
`assets/pet/bongocat-preview.png` is a preview rendered from the unchanged
bundled cat model and its original instrument artwork.
Input identities live only in memory; no keyboard text or input sequence is
written to logs, health reports or settings.
