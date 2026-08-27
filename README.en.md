<p align="center">
  <b>Languages</b>:
  <a href="README.md">简体中文</a> |
  <a href="README.zh-TW.md">繁體中文</a> |
  <b>English</b> |
  <a href="README.ja.md">日本語</a>
</p>

<div align="center">
  <img src="assets/icon-256.png" width="88" alt="ReadMD logo">

  # ReadMD

  **Open huge Markdown files locally. Keep the original untouched.**

  ReadMD is a local-first Markdown reader and editor for desktop. Double-click to read, navigate oversized documents without freezing, repair display-only syntax issues, convert Office/PDF/web files, extract image text offline, and connect AI agents through MCP.

  [![platform](https://img.shields.io/badge/Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20KylinOS%20%7C%20UOS-blue)](#direct-downloads-platforms-matrix-release-assets)
  [![i18n](https://img.shields.io/badge/languages-46-orange)](docs/i18n-language-reference.md)
  [![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)
  [![release](https://img.shields.io/github/v/release/Natsummerance/readMD)](https://github.com/Natsummerance/readMD/releases/latest)
  [![website](https://img.shields.io/badge/site-readmd.asia-black)](https://readmd.asia)
</div>

## Is it right for you?

| Your situation | What ReadMD does |
| --- | --- |
| A file exceeds 8,000 lines or 500 KB | Semantic pagination keeps the outline and Ctrl+F search usable |
| A table lacks separators or math is unclosed | Rendering repairs the display in memory while the original stays unchanged |
| You have DOCX / PPTX / XLSX / PDF / HTML | Convert it to Markdown instead of rebuilding formatting by hand |
| An image contains text you need | Use native OS OCR in an offline workflow where supported |
| You write papers or technical documents | Use BibTeX cards, theorem/proof boxes and LaTeX export |
| You want an AI-assisted workflow | Use the VS Code extension and FastMCP stdio server |

## Direct downloads

[Windows Setup](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.7-beta.4.exe) ·
[Windows Portable](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.7-beta.4.exe) ·
[macOS Apple Silicon](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.7-beta.4.zip) ·
[macOS Intel](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.7-beta.4.zip) ·
[Linux AppImage](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage) ·
[Deb for UOS/Kylin](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_amd64.deb) ·
[Kylin V10 ARM64](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_arm64.deb) ·
[SHA-256](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt)

## Start in three steps

1. Download the installer or portable build for your system.
2. Open a real large file and check outline, search and table rendering.
3. Edit only when you are ready; rendering repairs do not silently write to your source.

## AI assistant citations

- [Concise product index](https://readmd.asia/llms.txt): version, platforms, privacy boundary, and key facts.
- [Full citation corpus](https://readmd.asia/llms-full.txt): direct answers about long-document pagination, non-destructive repair, conversion, and frequent questions.

## Why star ReadMD?

ReadMD solves the unglamorous problems in a long-lived document library: large files remain readable, imported material needs less cleanup, sensitive drafts stay local, and the original file retains final authority. If it saves you one cleanup session, please [star the repository](https://github.com/Natsummerance/readMD) so other writers can find it.


## 🚀 Direct Downloads & Platforms Matrix (Release Assets)

| OS / Platform | Architecture / Format | Direct Download Link (GitHub Release) | Description |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | x64 (Installer) | [⬇️ **ReadMDSetup-v2.3.7-beta.4.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.7-beta.4.exe) | Setup wizard with automatic `.md` file associations |
| 💼 **Windows** | x64 (Portable) | [⬇️ **ReadMD-portable-v2.3.7-beta.4.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.7-beta.4.exe) | Standalone single executable, no installation needed |
| 🍏 **macOS** | Apple Silicon (M-Series) | [⬇️ **ReadMD-macos-arm64-v2.3.7-beta.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.7-beta.4.zip) | Native build for Apple Silicon Macs with Vision OCR |
| 💻 **macOS** | Intel x86_64 | [⬇️ **ReadMD-macos-x64-v2.3.7-beta.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.7-beta.4.zip) | Native build for Intel Macs with Vision OCR |
| 🐧 **Linux** | x86_64 (AppImage) | [⬇️ **ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage) | Portable Linux AppImage for Ubuntu, Debian, Fedora, Arch |
| 🇨🇳 **Domestic OS / Linux** | UOS / Kylin / Deepin / Debian / Ubuntu | [⬇️ **readmd_2.3.7-beta.4_amd64.deb**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_amd64.deb) | Native Deb package with desktop entry & MIME association |
| 🖥️ **Kylin V10 / Phytium** | ARM64 (aarch64) | [⬇️ **readmd_2.3.7-beta.4_arm64.deb**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_arm64.deb) | Native build for D2000/E2000 boards with UKUI/X11 software-render fallback |
| 📱 **HarmonyOS NEXT** | Source project (DevEco build) | [🧩 **packages/harmonyos-app**](https://github.com/Natsummerance/readMD/tree/main/packages/harmonyos-app) | ArkTS + ArkUI + ArkWeb source project; no prebuilt HAP is provided |
| 🧩 **VSCode Extension** | Universal VSIX | [⬇️ **readmd-vscode-2.3.7-beta.4.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.7-beta.4.vsix) | Offline VSIX extension with sync preview & auto-repair |
| 🤖 **MCP Server** | FastMCP stdio Package | [⬇️ **readmd-mcp-server-2.3.7-beta.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.7-beta.4.zip) | Standalone FastMCP server for Claude Desktop / Cursor |
| 🔐 **SHA-256 Hashes** | Checksum List | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | Complete SHA-256 integrity verification list |

---

## 💻 Multi-System & Native OS Integration

### 1. Linux & Chinese Domestic OS (KylinOS / UOS / Deepin / openEuler)
- **Direct Installation**: Download [`readmd_2.3.7-beta.4_amd64.deb`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_amd64.deb) to install directly, or run [`ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage`](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage).
- **Environment Detection**: `src/readmd_modules/linux_native.py` detects OS distributions and dynamically adapts Wayland / X11 display backends.
- **Desktop Themes**: Probes DDE, UKUI, GNOME, and KDE dark mode settings via `gsettings`.
- **Desktop Entry**: Includes FreeDesktop launcher and MIME XML declaration.
- **Linglong Format**: Declarative `packages/linglong/linglong.yaml` for UOS AppStore distribution.

### 2. HarmonyOS NEXT (Pure Harmony) & OpenHarmony
- **Source Build**: Open [`packages/harmonyos-app/`](https://github.com/Natsummerance/readMD/tree/main/packages/harmonyos-app) in DevEco Studio NEXT and compile it; no prebuilt HAP is provided.
- **ArkUI + ArkWeb**: Reuses ReadMD offline rendering engine inside ArkWeb containers.
- **ReadMDBridge (`ReadMDBridge.ets`)**:
  - Clipboard integration (`@ohos.pasteboard`);
  - Native file picker and file system (`@ohos.file.picker` / `@ohos.file.fs`);
  - System locale detection (`@ohos.i18n`);
  - Native offline OCR (`@ohos.ai.OCR`).

### 3. Windows & macOS
- **Windows**: Native WinRT OCR, Edge WebView2 hardware-accelerated rendering, single-instance tray daemon.
- **macOS**: Apple Vision offline OCR framework, native WebKit window, Touch Bar shortcuts.

---

<div align="center">

**ReadMD** · Pure local-first, distraction-free Markdown across all platforms.

</div>
