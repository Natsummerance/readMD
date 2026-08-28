# ReadMD V2.3.7 update deck

This directory is the source of truth for the V2.3.6 → V2.3.7 poster deck. It has no bundled product mockups: every page resolves to a distinct screenshot captured by `showcase/showcase.spec.js` or a separately audited website capture.

```powershell
npm run capture --prefix showcase
npm run build:v237-update --prefix showcase
npm run verify:v237-update --prefix showcase
```

The generated deck is intentionally outside version control at `showcase/output/v237-update/`. It contains 14 PNG posters, a static gallery, Chinese and English update lists, and an evidence manifest with source and output SHA-256 values. The renderer refuses to build if a screenshot is missing, altered, duplicated, outside `showcase/`, or mismatched with the requested `v2.3.7` capture.

To render the English deck without changing the source data:

```powershell
node showcase/update-v237/render-deck.mjs --locale en --output showcase/output/v237-update-en
```
