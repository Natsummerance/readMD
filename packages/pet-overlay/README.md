# ReadMD optional pet overlay

This small Electron overlay is directly adapted from Hermes Agent's MIT desktop
pet overlay, so the transparent native window, screen-coordinate dragging and
per-display clamping use its established architecture rather than pywebview.

It is not launched by ReadMD startup and has no bundled Electron runtime yet.
During development it can be run with an existing Electron executable:

```powershell
& 'T:\Programming\Project\Hermes\hermes-agent\apps\desktop\node_modules\electron\dist\electron.exe' 'T:\Programming\Project\codex\creator\readmd\packages\pet-overlay'
```

The included Hermes sprite is only an MIT-licensed fallback to prove the native
overlay. The selected Arch-chan Live2D model remains separately fail-closed
until the Cubism renderer and its publication conditions are approved.
