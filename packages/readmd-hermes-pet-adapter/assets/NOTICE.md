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
Live2D model. Arch-chan remains separately license-gated until an official
Cubism redistribution review is completed.
