# ReadMD GNOME companion

The native host uses this companion only for GNOME sessions. It communicates
over the per-user Unix socket `readmd-pet-gnome.sock` and authenticates every
message with the application id, peer PID, and session token. GNOME is never
given the gtk-layer-shell backend; the extension owns panel/workspace and
window-list placement while the WRY surface remains the renderer.

Install `legacy/extension.js` for GNOME Shell 40–44 and `esm/extension.js` for
GNOME Shell 45 and newer. Both extensions expose the same socket protocol.
