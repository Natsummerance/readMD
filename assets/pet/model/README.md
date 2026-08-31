# ReadMD Live2D model bundle contract

This directory contains the selected Arch-chan model candidate, copied from
the upstream CC0 1.0 repository at the revision recorded in
`readmd.live2d.json`. It intentionally contains no Cubism Core or renderer:
the desktop pet remains disabled until the separate Cubism publication record
and native-window evidence have both been completed.

Do not place Hermes/Petdex artwork, generated sprite sheets, or unlicensed
assets here.

Only an original, redistributable model bundle may be placed in this directory.
It must contain `readmd.live2d.json`, every listed asset, and SHA-256 values for
each asset. `python -c "from src.readmd_modules.pet import verify_model_bundle; print(verify_model_bundle('assets/pet/model'))"`
must report `ready: true` before the desktop setting can enable the pet. The
bundled candidate is expected to report `cubism_publication_license_pending`
until that approval is recorded; this is a deliberate fail-closed state, not a
working renderer.

The manifest must identify the author, asset licence, source/commission record,
redistribution authorisation, and the written Cubism publication-licence record.
The latter must not be `pending`, `unknown`, or `none`.
