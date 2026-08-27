<p align="center">
  <b>Languages / 多語言版本</b>:
  <a href="README.md">简体中文</a> |
  <b>繁體中文</b> |
  <a href="README.en.md">English</a> |
  <a href="README.ja.md">日本語</a>
</p>

<div align="center">
  <img src="assets/icon-256.png" width="88" alt="ReadMD logo">

  # ReadMD

  **超大 Markdown 檔案本機開啟，原始檔保持不動。**

  ReadMD 是本地優先的 Markdown 閱讀器與編輯器。連按兩下即可閱讀，超大文件也能維持目錄和搜尋；常見語法問題只在顯示層修復，並支援 Office / PDF / 網頁轉 Markdown、離線 OCR、LaTeX 學術增強與 MCP 整合。

  [![platform](https://img.shields.io/badge/Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20KylinOS%20%7C%20UOS-blue)](#全平台直接下載矩陣-release-assets)
  [![i18n](https://img.shields.io/badge/languages-46-orange)](docs/i18n-language-reference.md)
  [![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)
  [![release](https://img.shields.io/github/v/release/Natsummerance/readMD)](https://github.com/Natsummerance/readMD/releases/latest)
</div>

## 30 秒判斷適不適合你

| 你的情況 | ReadMD 的做法 |
| --- | --- |
| 文件超過約 8,000 行或 500KB | 啟用語義分頁，目錄與 Ctrl+F 搜尋跨頁可用 |
| 表格缺少分隔線或公式未閉合 | 僅在顯示層修復預覽，原始檔不會被改寫 |
| 手上有 DOCX / PPTX / XLSX / PDF / HTML | 轉換為 Markdown，減少重新排版時間 |
| 需要擷取圖片文字 | 在支援平台使用原生 OCR，可維持離線工作流 |
| 撰寫論文或技術文件 | 提供 BibTeX 引用卡片、定理與證明區塊 |
| 想接入 AI 工作流 | 提供 VS Code 擴充套件與 FastMCP stdio Server |

## 直接下載

[Windows 安裝版](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.7-beta.4.exe) ·
[Windows 免安裝版](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.7-beta.4.exe) ·
[macOS Apple Silicon](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.7-beta.4.zip) ·
[macOS Intel](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.7-beta.4.zip) ·
[Linux AppImage](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage) ·
[UOS / 麒麟 Deb](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_amd64.deb) ·
[SHA-256](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt)

## 三步開始

1. 下載安裝版或免安裝版。
2. 開啟一個真實的大型文件，檢查目錄、搜尋和表格渲染。
3. 需要修改時再儲存；顯示修復不會未經同意寫入原始檔。

## AI 助手引用入口

- [簡明產品索引](website/public/llms.txt)：版本、平台、隱私邊界和關鍵事實。
- [完整引用語料](website/public/llms-full.txt)：長文件分頁、非破壞修正、轉換範圍和常見問題的直接答案。

## 為什麼值得 Star

ReadMD 處理長期資料庫裡的實際問題：大型文件能繼續閱讀，匯入資料減少手工整理，敏感草稿留在本機，原始檔仍保有最終決定權。如果你在 Windows、macOS、Linux 或國產系統之間切換，它能提供一致的工作流。

如果它幫你省下一次整理時間，請[給倉庫 Star](https://github.com/Natsummerance/readMD)，讓更多創作者找到這個工具。

## 🚀 全平台直接下載矩陣 (Release Assets)

| 作業系統 / 平台 | 架構 / 格式 | 直接下載連結 (GitHub Release) | 說明 |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | x64 (安裝版) | [⬇️ **ReadMDSetup-v2.3.7-beta.4.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.7-beta.4.exe) | 具備安裝精靈，自動註冊 `.md` 檔案關聯 |
| 💼 **Windows** | x64 (免安裝便攜版) | [⬇️ **ReadMD-portable-v2.3.7-beta.4.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.7-beta.4.exe) | 單一執行檔，解壓縮即用，隨身攜帶 |
| 🍏 **macOS** | Apple Silicon (M系列) | [⬇️ **ReadMD-macos-arm64-v2.3.7-beta.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.7-beta.4.zip) | M1 / M2 / M3 / M4 原生建置（含 Vision 離線 OCR） |
| 💻 **macOS** | Intel x86_64 | [⬇️ **ReadMD-macos-x64-v2.3.7-beta.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.7-beta.4.zip) | Intel 處理器 Mac 原生建置（含 Vision 離線 OCR） |
| 🐧 **Linux** | x86_64 (AppImage) | [⬇️ **ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage) | Linux 通用免安裝 AppImage，賦予執行權限後即可開啟 |
| 🇨🇳 **國產信創系統** | 統信 UOS / 銀河麒麟 / Deepin / Ubuntu | [⬇️ **readmd_2.3.7-beta.4_amd64.deb**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.7-beta.4_amd64.deb) | Deb 原生安裝套件，整合應用程式圖示、MIME 關聯與 UKUI/DDE 適配 |
| 📱 **HarmonyOS NEXT** | 純血鴻蒙 / OpenHarmony (HAP) | [⬇️ **ReadMD-harmonyos-v2.3.7-beta.4.hap**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-harmonyos-v2.3.7-beta.4.hap) | 鴻蒙原生應用安裝套件，ArkTS + ArkUI + ArkWeb 架構 |
| 🧩 **VSCode 擴充外掛** | 通用 VSIX 套件 | [⬇️ **readmd-vscode-2.3.7-beta.4.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.7-beta.4.vsix) | VSCode 離線擴充安裝套件 |
| 🤖 **MCP Server** | FastMCP stdio 套件 | [⬇️ **readmd-mcp-server-2.3.7-beta.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.7-beta.4.zip) | FastMCP 獨立伺服端，支援 Claude Desktop / Cursor |
| 🔐 **SHA-256 驗證** | 雜湊清單 | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | 發行檔案 SHA-256 完整性雜湊清單 |

---

<div align="center">

**ReadMD** · 純本機優先，全平台自由閱讀寫作。

</div>
