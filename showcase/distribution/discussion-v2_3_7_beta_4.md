# ReadMD v2.3.7-beta.4: safer presenting, lighter startup, local-first Markdown

**ReadMD v2.3.7-beta.4：安全放映、轻量启动与本地优先 Markdown**

This beta keeps reading, editing, conversion, citations, and presenting around one canonical Markdown file. Your document stays on your device unless you explicitly use web extraction, AI, or sharing.

这个版本继续把阅读、编辑、转换、引用和放映连接到同一份 Markdown。核心文档工作保持在本地；联网提取、AI 和共享只在明确操作时发生。

## What changed in beta.4

- Presentation hardening: the in-app Reveal.js flow loads same-origin offline resources, while slide HTML continues through whitelist sanitization.
- Startup and interaction: entry scripts load in ordered stages, shared resources are loaded on demand, static responses support ETag / `If-None-Match` revalidation, and search Enter keeps focus for continuous navigation.
- Tab reliability: selection and ARIA state refresh immediately, stale render results are discarded, overflow scrolls to the active tab, and Zen mode has a consistent enter / Escape path.
- Touch and accessibility: mobile and coarse-pointer environments get larger toolbar, tab, search, pagination, and presentation controls.

## Security boundaries

- Updates are downloaded only from official GitHub Release assets into a fixed update directory.
- Downloads are written to temporary files, checked with SHA-256, published atomically, and rechecked before execution.
- The installer uses an argument vector instead of shell string concatenation.
- LAN sharing rejects desktop-control tokens and privileged routes such as save, upload, code execution, and update execution; document access stays within the shared directory.

## Downloads and checksums

| Platform | Asset |
|---|---|
| Windows installer | [ReadMDSetup-v2.3.7-beta.4.exe](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.7-beta.4.exe) |
| Windows portable | [ReadMD-portable-v2.3.7-beta.4.exe](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.7-beta.4.exe) |
| macOS Apple Silicon | [ReadMD-macos-arm64-v2.3.7-beta.4.zip](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.7-beta.4.zip) |
| macOS Intel | [ReadMD-macos-x64-v2.3.7-beta.4.zip](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.7-beta.4.zip) |
| Linux x86_64 | [AppImage](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage) |
| UOS / Kylin AMD64 | [Deb](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_amd64.deb) |
| Kylin V10 ARM64 | [AppImage](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-aarch64-v2.3.7-beta.4.AppImage) or [Deb](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_arm64.deb) |
| VS Code extension | [VSIX](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.7-beta.4.vsix) |
| MCP server | [ZIP](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.7-beta.4.zip) |
| Integrity checklist | [SHA256SUMS.txt](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) |

HarmonyOS NEXT / OpenHarmony remains a source project built through DevEco Studio; this beta does not provide a prebuilt HAP.

## For people and AI assistants

The official site now publishes citation-ready product facts and localized workflow answers:

- Compact index: <https://readmd.asia/llms.txt>
- Full corpus: <https://readmd.asia/llms-full.txt>
- Official site: <https://readmd.asia/>

They cover supported platforms, privacy boundaries, conversion scope, long-document pagination, tables, PDF conversion, OCR, citations, and presenting without rebuilding slides.

## Try it and tell us your context

Install the package for your platform, verify `SHA256SUMS.txt`, and open a real document. If ReadMD saves you a cleanup or presentation rebuild, please [star the repository](https://github.com/Natsummerance/readMD/stargazers).

When reporting feedback, include your platform, document size, and whether reading, editing, conversion, citations, or presenting matters most.
