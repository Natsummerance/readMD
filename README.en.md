<p align="center">
  🌐 <b>Languages</b>: 
  <a href="README.md">简体中文</a> | 
  <a href="README.zh-TW.md">繁體中文</a> | 
  <b>English</b> | 
  <a href="README.ja.md">日本語</a>
</p>

<div align="center">

<img src="assets/icon-256.png" width="96" alt="ReadMD logo">

# 📖 ReadMD · Lightweight Markdown Viewer & Editor

**Pure Local · Instant Launch · Offline Ready** Markdown Viewer & Editor for Windows and macOS.

Double-click any `.md` file to read instantly. ReadMD automatically repairs common Markdown formatting errors (tables, bold, LaTeX math, unspaced headings) before rendering—**only modifying the visual preview, never altering your original source file**. Built-in AI assistant, universal file conversion, native OCR, web-to-Markdown extraction, live editing, and local network mobile sharing.

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-0078d6)
![version](https://img.shields.io/badge/version-v2.3.0-3b6ef5)
![webview2](https://img.shields.io/badge/runtime-WebView2-4fc08d)
![license](https://img.shields.io/badge/license-MIT-green)
![i18n](https://img.shields.io/badge/i18n-46%20Languages-orange)

<br>

<p align="center">
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_Setup-v2.3.0-2563eb?style=for-the-badge&logo=windows&logoColor=white" alt="Windows Setup Download">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_Portable-v2.3.0-0284c7?style=for-the-badge&logo=windows&logoColor=white" alt="Windows Portable Download">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Apple_Silicon)-v2.3.0-111827?style=for-the-badge&logo=apple&logoColor=white" alt="macOS ARM64 Download">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Intel)-v2.3.0-4b5563?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Intel Download">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix">
    <img src="https://img.shields.io/badge/🧩_VSCode_VSIX-v2.3.0-6366f1?style=for-the-badge&logo=visualstudiocode&logoColor=white" alt="VSCode VSIX Download">
  </a>
</p>

</div>

---

## ✨ Key Features

- ⚡ **Instant Launch**: Installed version uses directory deployment (onedir) with cold start ≤ 1.5s (≤ 2s on low-end machines). Window minimize to system tray enables instant wake-up in < 0.3s.
- 🌍 **Global 46+ Languages (v2.3.0)**: Automatically detects system language on first startup. Complete support for LTR & RTL layouts (Arabic, Hebrew, Uyghur) and Asian/European languages.
- 📐 **LaTeX PRO Academic Suite (v2.3.0)**: Zero-config auto-scanning of `.bib` bibliography files in the same directory. Interactive hover cards for citations (with DOI jump & BibTeX copying), plus academic callouts for Theorem, Lemma, Proof (with Q.E.D. symbol), and Definition.
- 🧘 **Editor Studio PRO (v2.3.0)**: Immersive Zen Mode (<kbd>F11</kbd> / <kbd>Esc</kbd>), 10x10 interactive table grid builder, smart Excel/CSV-to-Markdown paste conversion, and live word count & reading time stats.
- 🔌 **VSCode Extension & MCP Server (v2.3.0)**: Standalone VSCode extension with live sync preview and one-click auto-repair, plus standard FastMCP stdio server for Claude Desktop, Cursor, Antigravity, and Cline.
- 🛡️ **In-App Auto-Updater**: Smooth background check with mirror fallback and zero-lock file release mechanism.
- 🎨 **Minimalist Design**: 44px compact toolbar, Light / Dark / Sepia themes, skeleton screens for large files, and system "prefers-reduced-motion" compliance.
- 🤖 **AI Assistant & Chat Import**: Preset providers (OpenAI, DeepSeek, Kimi, Anthropic, Ollama, etc.) with local encrypted API key storage. Supports context-aware chat, polishing, code review, and prompt templates.
- 🔄 **Universal Conversion**: Converts Word (.docx), PowerPoint (.pptx), Excel (.xlsx), PDF, HTML, and LaTeX (.tex) into clean Markdown with automatic tab opening.
- 🔍 **Native OCR**: Uses Windows WinRT and macOS Vision offline OCR engines. One-click clipboard image text extraction.
- 🌐 **Web to Markdown**: Two-stage extraction with Trafilatura and fallback to headless WebView with Defuddle / Readability for dynamic single-page apps.
- 📱 **Mobile Sharing**: Scan QR code to read and edit on local Wi-Fi with cryptographically random token authentication.
- 📤 **Export to PDF / DOCX / HTML / LaTeX**: Custom paper size, margins, font colors, code block styling, and complete LaTeX paper source generation.
- 🛠 **Non-Destructive Auto-Repair**: Fixes table column misalignment, missing separator rows, unclosed formatting (`**`, `*`, `$`, `$$`), unspaced `#` headers, and BOM/CRLF issues.

---

## 🚀 Direct Downloads

| Platform / Type | Direct Download Link | Description |
| :--- | :--- | :--- |
| 🪟 **Windows Installer** | [⬇️ **ReadMDSetup-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe) | Smooth setup wizard with automatic `.md` file associations |
| 💼 **Windows Portable** | [⬇️ **ReadMD-portable-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe) | Single executable, green portable edition |
| 🍏 **macOS Apple Silicon** | [⬇️ **ReadMD-macos-arm64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip) | Native build for Apple Silicon Macs (M1 / M2 / M3 / M4) |
| 💻 **macOS Intel** | [⬇️ **ReadMD-macos-x64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip) | Native build for Intel Macs |
| 🧩 **VSCode Extension** | [⬇️ **readmd-vscode-2.3.0.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix) | Offline VSIX extension package for Visual Studio Code |
| 🔐 **SHA-256 Checksums** | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | Integrity verification checksum list |

---

## 🧩 VSCode Extension Installation Guide

ReadMD provides an official lightweight VSCode extension that brings ReadMD's signature rendering style, live synchronized preview, and non-destructive syntax repair into Visual Studio Code.

### Method 1: Install from VSIX in VSCode (Recommended)
1. Download [`readmd-vscode-2.3.0.vsix`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix);
2. In VSCode, press <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd> (or <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd> on macOS) to open the Extensions tab;
3. Click the **`...` (Views and More Actions)** button at the top right of the Extensions panel;
4. Select **`Install from VSIX...`**;
5. Select the downloaded `.vsix` file to install immediately!

### Method 2: Install via Command Line
Run the following in your terminal:
```bash
code --install-extension readmd-vscode-2.3.0.vsix
```

### Features:
- **📖 Live Synced Preview**: Click the book icon on the editor title bar or run `ReadMD: Open Custom Preview` for a high-fidelity side-by-side preview;
- **🛠️ One-Click Auto-Fix**: Right-click in any Markdown file and select `ReadMD: Auto-Fix Markdown Formatting Errors` to fix broken tables, formulas, and unclosed tags;
- **📐 Convert to LaTeX**: Right-click and choose `ReadMD: Convert Current Markdown to LaTeX` to generate ready-to-compile academic LaTeX source.

---

## 🤖 MCP (Model Context Protocol) Server Setup

ReadMD includes a standard FastMCP (stdio) server, enabling AI coding assistants (Claude Desktop, Cursor, Antigravity, Cline, Continue) to leverage ReadMD's parsing, conversion, and self-healing engine.

### Configuration

#### 1. Claude Desktop
Add this to your `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "readmd": {
      "command": "python",
      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]
    }
  }
}
```

#### 2. Cursor / Cline
In MCP Settings, configure:
- **Name**: `readmd`
- **Command**: `python`
- **Args**: `["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]`

#### 🛠️ Provided MCP Tools:
| Tool Name | Description |
| :--- | :--- |
| **`readmd_fix_markdown`** | Automatically repairs Markdown formatting errors without losing content |
| **`readmd_convert_to_markdown`** | Converts Word, PDF, PPT, Excel, LaTeX, or HTML to clean Markdown |
| **`readmd_latex_to_md`** | Accurately converts LaTeX math and documents to standard Markdown |
| **`readmd_md_to_latex`** | Compiles Markdown into academic standalone LaTeX with booktabs tables |
| **`readmd_parse_bibtex`** | Parses `.bib` files and returns structured citation metadata |

---

## 📐 LaTeX PRO & Academic Citations

- **BibTeX Auto-Scanning**: Open any Markdown document, and ReadMD will automatically discover adjacent `.bib` files. Citations like `[@vaswani2017attention]` or `@knuth1984texbook` are rendered as clickable badges.
- **Hover Citation Card**: Hover over any citation badge to view title, authors, year, journal, DOI link, and a one-click copy BibTeX button.
- **Academic Callout Blocks**:
  - `::: theorem [Cauchy-Schwarz Inequality]` -> Theorem callout box
  - `::: lemma [Lemma title]` -> Lemma callout box
  - `::: proof` -> Proof callout box with Q.E.D. ■ symbol
  - `::: definition [Manifold]` -> Definition callout box

---

## 🧘 Editor Studio PRO

- **Zen Mode**: Press <kbd>F11</kbd> or click the Zen button to enter distraction-free full-screen writing. Press <kbd>Esc</kbd> or click the top-right exit button to return.
- **10x10 Table Designer**: Click "Insert Table" and slide over the interactive grid to insert perfectly formatted tables.
- **Smart Excel / CSV Paste**: Copy tabular cells from Excel, Google Sheets, or WPS, and paste directly into the editor—ReadMD converts them into aligned Markdown tables automatically.
- **Real-Time Document Stats**: Live counters for character count, word count, and estimated reading time.

---

## ⌨️ Common Keyboard Shortcuts

| Shortcut | Description |
| :--- | :--- |
| <kbd>F11</kbd> / <kbd>Esc</kbd> | Toggle Zen Mode / Exit Fullscreen |
| <kbd>F2</kbd> | Rename current file (extension protected) |
| <kbd>Ctrl</kbd> + <kbd>O</kbd> | Open local file |
| <kbd>Ctrl</kbd> + <kbd>E</kbd> | Toggle between Reader and Editor |
| <kbd>Ctrl</kbd> + <kbd>S</kbd> | Save document |
| <kbd>Ctrl</kbd> + <kbd>F</kbd> | Full text search & replace |
| <kbd>Ctrl</kbd> + <kbd>P</kbd> | Export to PDF / DOCX / HTML / LaTeX |
| <kbd>Ctrl</kbd> + <kbd>D</kbd> | Cycle Themes (Light / Dark / Sepia) |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>A</kbd> | Open AI Assistant |
| <kbd>Ctrl</kbd> + <kbd>=</kbd> / <kbd>-</kbd> | Increase / Decrease Font Size |

---

<p align="center">
  <b>ReadMD</b> is free, open-source, and offline-first.<br>
  Crafted with ❤️ by <a href="https://github.com/Natsummerance">Natsummerance</a>.
</p>
