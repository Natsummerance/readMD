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

[Windows Setup](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMDSetup-v2.3.8.exe) ·
[Windows Portable](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-portable-v2.3.8.exe) ·
[macOS Apple Silicon](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-arm64-v2.3.8.zip) ·
[macOS Intel](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-x64-v2.3.8.zip) ·
[Linux AppImage](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-x86_64-v2.3.8.AppImage) ·
[Linux ARM64](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-aarch64-v2.3.8.AppImage) ·
[Deb for UOS/Kylin](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_amd64.deb) ·
[Kylin V10 ARM64](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_arm64.deb) ·
[SHA-256](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/SHA256SUMS.txt)

## Start in three steps

1. Download the installer or portable build for your system.
2. Open a real large file and check outline, search and table rendering.
3. Edit only when you are ready; rendering repairs do not silently write to your source.

## AI assistant citations

- [Concise product index](https://readmd.asia/llms.txt): version, platforms, privacy boundary, and key facts.
- [Full citation corpus](https://readmd.asia/llms-full.txt): direct answers about long-document pagination, non-destructive repair, conversion, and frequent questions.

## Why star ReadMD?

ReadMD solves the unglamorous problems in a long-lived document library: large files remain readable, imported material needs less cleanup, sensitive drafts stay local, and the original file retains final authority. If it saves you one cleanup session, please [star the repository](https://github.com/Natsummerance/readMD) so other writers can find it.


## Official Downloads & Platforms Matrix (Release Assets)

Only platforms with release evidence belong in this matrix. Windows 7 is a separate legacy-runtime build and is not bundled with the Windows 10/11 package; HarmonyOS/OpenHarmony and unevidenced architectures are outside the V2.3.8 support promise.

| OS / Platform | Architecture / Format | Direct Download Link (GitHub Release) | Description |
| :--- | :--- | :--- | :--- |
| **Windows** | x64 (Installer) | [⬇️ **ReadMDSetup-v2.3.8.exe**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMDSetup-v2.3.8.exe) | Setup wizard with automatic `.md` file associations |
| **Windows** | x64 (Portable) | [⬇️ **ReadMD-portable-v2.3.8.exe**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-portable-v2.3.8.exe) | Standalone single executable, no installation needed |
| **macOS** | Apple Silicon (M-Series) | [⬇️ **ReadMD-macos-arm64-v2.3.8.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-arm64-v2.3.8.zip) | Native build for Apple Silicon Macs with Vision OCR |
| **macOS** | Intel x86_64 | [⬇️ **ReadMD-macos-x64-v2.3.8.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-x64-v2.3.8.zip) | Native build for Intel Macs with Vision OCR |
| **Linux** | x86_64 (AppImage) | [⬇️ **ReadMD-linux-x86_64-v2.3.8.AppImage**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-x86_64-v2.3.8.AppImage) | Portable Linux AppImage for the tested Ubuntu/Debian matrix |
| **Linux** | ARM64 (AppImage) | [⬇️ **ReadMD-linux-aarch64-v2.3.8.AppImage**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-aarch64-v2.3.8.AppImage) | Portable build for Phytium, Kunpeng, and other ARM64 devices |
| **Domestic OS / Linux** | UOS / Kylin / Deepin / Debian / Ubuntu | [⬇️ **readmd_2.3.8_amd64.deb**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_amd64.deb) | Native Deb package with desktop entry & MIME association |
| ️ **Kylin V10 / Phytium** | ARM64 (aarch64) | [⬇️ **readmd_2.3.8_arm64.deb**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_arm64.deb) | Native build for D2000/E2000 boards with UKUI/X11 software-render fallback |
| **VSCode Extension** | Universal VSIX | [⬇️ **readmd-vscode-2.3.8.vsix**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd-vscode-2.3.8.vsix) | Offline VSIX extension with sync preview & auto-repair |
| **MCP Server** | FastMCP stdio Package | [⬇️ **readmd-mcp-server-2.3.8.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd-mcp-server-2.3.8.zip) | Standalone FastMCP server for Claude Desktop / Cursor |
| **SHA-256 Hashes** | Checksum List | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/SHA256SUMS.txt) | Complete SHA-256 integrity verification list |

---

## Multi-System & Native OS Integration

### 1. Linux & Chinese Domestic OS (KylinOS / UOS / Deepin)
- **Direct Installation**: Download [`readmd_2.3.8_amd64.deb`](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_amd64.deb) to install directly, or run [`ReadMD-linux-x86_64-v2.3.8.AppImage`](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-x86_64-v2.3.8.AppImage).
- **Environment Detection**: `src/readmd_modules/linux_native.py` detects OS distributions and dynamically adapts Wayland / X11 display backends.
- **Desktop Themes**: Probes DDE, UKUI, GNOME, and KDE dark mode settings via `gsettings`.
- **Desktop Entry**: Includes FreeDesktop launcher and MIME XML declaration.
- **Support boundary**: openEuler, Linglong and other unevidenced distributions are not claimed as fully supported in this release.

### 2. Windows & macOS
- **Windows**: Native WinRT OCR, Edge WebView2 hardware-accelerated rendering, single-instance tray daemon.
- **macOS**: Apple Vision offline OCR framework, native WebKit window, Touch Bar shortcuts.

---

<div align="center">

**ReadMD** · Pure local-first, distraction-free Markdown on tested supported platforms.

</div>
