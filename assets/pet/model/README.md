# ReadMD Live2D model bundle contract

This directory contains the selected Arch-chan model, copied from
the upstream CC0 1.0 repository at the revision recorded in
`readmd.live2d.json`. It intentionally contains no Cubism Core or renderer:
the proprietary `live2dcubismcore.min.js` runtime is fetched and hash-pinned
at plugin build time and ships only inside the optional pet plugin ZIP (see
`packages/readmd-hermes-pet-adapter/assets/NOTICE.md`).

Do not place Hermes/Petdex artwork, generated sprite sheets, or unlicensed
assets here.

Only an original, redistributable model bundle may be placed in this directory.
It must contain `readmd.live2d.json`, every listed asset, and SHA-256 values for
each asset. `python -c "from src.readmd_modules.pet import verify_model_bundle; print(verify_model_bundle('assets/pet/model'))"`
must report `ready: true` before the desktop setting can enable the pet. The
bundled candidate reports `ready: true` with code `ready_for_platform_probe`
since the Cubism publication record was accepted on 2026-09-01 as
`accepted-2026-09-01-readmd-plugin-publisher`; note that Live2D rendering
additionally requires the plugin-stage Cubism Core and the native transparency
probe, both of which remain fail-closed.

The manifest must identify the author, asset licence, source/commission record,
redistribution authorisation, and the written Cubism publication-licence record.
The latter must not be `pending`, `unknown`, or `none`.
