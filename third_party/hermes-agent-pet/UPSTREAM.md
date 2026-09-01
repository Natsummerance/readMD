# Hermes Agent desktop pet module — exact upstream snapshot

Source repository: `https://github.com/NousResearch/hermes-agent`

Pinned revision: `fb27614addac115d55299bc6538ae112fd01f688`
License: MIT; the original complete text is retained as `LICENSE`.

This is an unmodified, path-preserving copy of the upstream desktop pet module:

- `apps/desktop/electron/pet-overlay-ipc.ts`
- `apps/desktop/src/components/pet/`
- `apps/desktop/src/app/pet-overlay/`
- `apps/desktop/src/store/pet.ts`
- `apps/desktop/src/store/pet-overlay.ts`
- `apps/desktop/src/app/contrib/hooks/use-pet-bridge.ts`

ReadMD must consume this vendor snapshot through an explicit bridge. Do not edit
files below `apps/`; refresh the whole snapshot from the recorded revision
instead. The upstream module depends on Hermes's Electron/React application
shell and its pet state APIs, so it cannot be treated as a Python module or
silently replaced by an HTML approximation.
