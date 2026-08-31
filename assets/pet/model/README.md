# ReadMD Live2D model bundle contract

This directory intentionally ships without a Live2D model. Do not place a
sample model, Hermes/Petdex artwork, generated sprite sheet, or unlicensed
asset here.

Only an original, redistributable model bundle may be placed in this directory.
It must contain `readmd.live2d.json`, every listed asset, and SHA-256 values for
each asset. `python -c "from src.readmd_modules.pet import verify_model_bundle; print(verify_model_bundle('assets/pet/model'))"`
must report `ready: true` before the desktop setting can enable the pet.

The manifest must identify the author, asset licence, source/commission record,
redistribution authorisation, and the written Cubism publication-licence record.
The latter must not be `pending`, `unknown`, or `none`.
