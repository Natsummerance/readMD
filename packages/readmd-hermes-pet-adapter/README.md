# ReadMD Hermes Pet Adapter

This package is an **external, on-demand desktop plugin**.  Its renderer,
drag contract, pointer hit testing, zoom anchoring, and Electron IPC are
imported from the immutable snapshot in `third_party/hermes-agent-pet`.

ReadMD-owned files in this directory deliberately only implement the host
boundary: local state ingestion, window ownership, and safe drop/clipboard
commands.  Do not edit `third_party/hermes-agent-pet/apps`; refresh the pinned
upstream snapshot instead.

The adapter is never started during ReadMD startup.  A packaged plugin supplies
its own Electron runtime; the application starts it only after the user enables
the pet.

For a Windows plugin package, run `npm run build` followed by `npm run stage
-- <readmd-pet-output-directory>`. The staged directory contains `electron.exe`, its
runtime resources, `app/`, `NOTICE.md`, and a SHA-256 manifest. Package this
directory as a ZIP. ReadMD verifies the manifest and installs it below its own
user-data `pet/hermes-adapter/` directory; it never writes the application
directory. It is intentionally optional, not a dependency of normal startup.
