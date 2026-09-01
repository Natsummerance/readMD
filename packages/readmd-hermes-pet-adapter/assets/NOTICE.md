# Hermes fallback sprite provenance

`hermes-sprite.png` is copied byte-for-byte from:

- repository: `https://github.com/NousResearch/hermes-agent`
- revision: `fb27614addac115d55299bc6538ae112fd01f688`
- source path: `apps/desktop/public/hermes-sprite.png`
- SHA-256: `e328d387a2fca8c02452fa534da1a89bdb8be9292cc51ffa66905018e74097a3`
- license: MIT; see `third_party/hermes-agent-pet/LICENSE`

It is a fallback for exercising the copied Hermes sprite renderer. Its source
sheet has four 384x512 cells across two rows; the adapter reads those cells as
Hermes frames and does not claim that the asset is the selected Arch-chan
Live2D model.

## Arch-chan model (cubism-web renderer)

`models/arch-chan/` is staged from the repository bundle at
`assets/pet/model/readmd.live2d.json`, whose rights record is authoritative:

- model: Arch Chan, author RavioliMavioli; archival fork maintained by Speykious
- repository: `https://github.com/Speykious/arch-chan`
- revision: `ed43dcf7e88d56f79d2e42fecb084bf6923f44e0`
- license: CC0-1.0; see `ARCH-CHAN-CC0-1.0.txt`
- Cubism publication license: `accepted-2026-09-01-readmd-plugin-publisher`
  (ReadMD plugin-publisher record; model files ship only inside the optional
  pet plugin ZIP, never inside the base reader).

## Live2D Cubism Core (proprietary runtime)

The Live2D renderer requires Live2D's proprietary `live2dcubismcore.min.js`.
It is NOT stored in this repository. The build fetches it once from:

- source: `https://cubism.live2d.com/sdk-web/cubismcore/live2dcubismcore.min.js`
- recorded: 2026-09-01, 207,155 bytes
- SHA-256: `25ae938cb4fe282ce189b357bcc97e603d1e1f7ec78bf04150d401c23cdc792f`

The build (`scripts/build.mjs`) verifies this digest before copying the file to
`dist/vendor/` and fails closed on mismatch; `READMD_PET_CUBISM_CORE` may point
at a local copy but is hash-checked the same way. The runtime is distributed
under Live2D's Proprietary Software License Agreement and may only be used by
applications published through the Live2D publication review; it therefore
ships only inside the optional pet plugin ZIP together with this notice.
