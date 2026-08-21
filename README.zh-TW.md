<p align="center">
  🌐 <b>Languages / 多語言版本</b>: 
  <a href="README.md">简体中文</a> | 
  <b>繁體中文</b> | 
  <a href="README.en.md">English</a> | 
  <a href="README.ja.md">日本語</a>
</p>

<div align="center">

<img src="assets/icon-256.png" width="96" alt="ReadMD logo">

# 📖 ReadMD · 輕量級全平台 Markdown 閱讀與編輯器

**純本機 · 極速秒開 · 離線可用 · 跨平台原生體驗**

連按兩下 `.md` 檔案立即閱讀。ReadMD 在渲染前自動修正常見語法錯誤（表格、粗體、LaTeX 公式、標題缺少空格等），**僅優化視覺預覽呈現，絕不竄改原檔案內容**；內建 AI 助手、萬物轉 MD、原生離線 OCR、網頁轉 MD、LaTeX PRO 學術增強、Zen 禪模式與區域網路行動端共用。

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Kylin%20%7C%20UOS%20%7C%20HarmonyOS-0078d6)
![version](https://img.shields.io/badge/version-v2.3.6-3b6ef5)
![webview2](https://img.shields.io/badge/runtime-WebView2%20%7C%20WebKit%20%7C%20ArkWeb-4fc08d)
![repo size](https://img.shields.io/github/repo-size/Natsummerance/readMD)
![i18n](https://img.shields.io/badge/i18n-46%20Languages-orange)
![license](https://img.shields.io/badge/license-MIT-green)

<br>

<p align="center">
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.6.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_安裝版-v2.3.6-2563eb?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 安裝版">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.6.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_免安裝版-v2.3.6-0284c7?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 免安裝版">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.6.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(M系列晶片)-v2.3.6-111827?style=for-the-badge&logo=apple&logoColor=white" alt="macOS ARM64">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.6.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Intel晶片)-v2.3.6-4b5563?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Intel">
  </a>
  <br>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.6.AppImage">
    <img src="https://img.shields.io/badge/⬇️_Linux_AppImage-v2.3.6-ea580c?style=for-the-badge&logo=linux&logoColor=white" alt="Linux AppImage">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.6_amd64.deb">
    <img src="https://img.shields.io/badge/⬇️_信創·統信UOS·麒麟_Deb-v2.3.6-b91c1c?style=for-the-badge&logo=debian&logoColor=white" alt="Deb 安裝套件">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-harmonyos-v2.3.6.hap">
    <img src="https://img.shields.io/badge/📱_鴻蒙_HarmonyOS_HAP-v2.3.6-059669?style=for-the-badge&logo=huawei&logoColor=white" alt="HarmonyOS HAP">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.6.vsix">
    <img src="https://img.shields.io/badge/🧩_VSCode_擴充套件-v2.3.6-6366f1?style=for-the-badge&logo=visualstudiocode&logoColor=white" alt="VSCode VSIX">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.6.zip">
    <img src="https://img.shields.io/badge/🤖_MCP_Server-v2.3.6-0d9488?style=for-the-badge&logo=fastapi&logoColor=white" alt="MCP Server">
  </a>
</p>

</div>

---

## ✨ 核心特性

- 📖 **超長文件智慧語義分頁**：面對 >10,000 行超長文件自動啟用智慧分頁，純 SVG 極簡向量控制列，大綱 (TOC) 與全文搜尋 (Ctrl+F) 跨頁聯動，公式按需排版，杜絕卡頓。
- ⚡ **極速秒開**：安裝版採用 onedir 目錄架構，冷啟動 ≤1.5s；縮小至系統匣常駐，再次開啟 <0.3s 瞬間喚醒。
- 💻 **全作業系統覆蓋**：Windows (Win 7 ~ 11)、macOS (Apple Silicon / Intel)、Linux (Ubuntu / Debian / Fedora / Arch)、國產信創 (銀河麒麟 KylinOS / 統信 UOS / 深度 Deepin / openEuler) 與純血鴻蒙 (HarmonyOS NEXT / OpenHarmony)。
- 🌍 **全球 46 種語言在地化**：自動依據作業系統語言完成設定，支援 LTR 與 RTL 雙向排版（阿拉伯語 / 希伯來語 / 維吾爾語），100% 完整母語覆蓋。
- 📐 **LaTeX PRO 學術研究套件**：自動掃描 `.bib` 參考文獻庫並生成懸浮預覽卡片；內建 Theorem、Lemma、Proof (附 Q.E.D. ■ 符號)、Definition 等專業學術 Callout 區塊。
- 🧘 **Editor Studio PRO 沉浸式編輯**：Zen Mode 禪模式（<kbd>F11</kbd> / <kbd>Esc</kbd> 全螢幕專注）、10×10 視覺化表格繪製器、Excel / CSV 剪貼簿智慧轉換 Markdown 表格。
- 🔌 **VSCode 官方擴充外掛與 MCP 伺服器**：提供雙向同步預覽與語法自動修復外掛；內建 FastMCP (stdio) 服務支援 Claude Desktop、Cursor 與 AI 助理。
- 🔄 **萬物轉換為 Markdown 與原生離線 OCR**：Office / PDF / LaTeX 一鍵轉換；Windows (WinRT) 與 macOS (Vision) 本機原生 OCR 辨識。

---

## 🚀 全平台直接下載矩陣 (Release Assets)

| 作業系統 / 平台 | 架構 / 格式 | 直接下載連結 (GitHub Release) | 說明 |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | x64 (安裝版) | [⬇️ **ReadMDSetup-v2.3.6.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.6.exe) | 具備安裝精靈，自動註冊 `.md` 檔案關聯 |
| 💼 **Windows** | x64 (免安裝便攜版) | [⬇️ **ReadMD-portable-v2.3.6.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.6.exe) | 單一執行檔，解壓縮即用，隨身攜帶 |
| 🍏 **macOS** | Apple Silicon (M系列) | [⬇️ **ReadMD-macos-arm64-v2.3.6.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.6.zip) | M1 / M2 / M3 / M4 原生建置（含 Vision 離線 OCR） |
| 💻 **macOS** | Intel x86_64 | [⬇️ **ReadMD-macos-x64-v2.3.6.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.6.zip) | Intel 處理器 Mac 原生建置（含 Vision 離线 OCR） |
| 🐧 **Linux** | x86_64 (AppImage) | [⬇️ **ReadMD-linux-x86_64-v2.3.6.AppImage**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.6.AppImage) | Linux 通用免安裝 AppImage，賦予執行權限後即可開啟 |
| 🇨🇳 **國產信創系統** | 統信 UOS / 銀河麒麟 / Deepin / Ubuntu | [⬇️ **readmd_2.3.6_amd64.deb**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.6_amd64.deb) | Deb 原生安裝套件，整合應用程式圖示、MIME 關聯與 UKUI/DDE 適配 |
| 📱 **HarmonyOS NEXT** | 純血鴻蒙 / OpenHarmony (HAP) | [⬇️ **ReadMD-harmonyos-v2.3.6.hap**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-harmonyos-v2.3.6.hap) | 鴻蒙原生應用安裝套件，ArkTS + ArkUI + ArkWeb 架構 |
| 🧩 **VSCode 擴充外掛** | 通用 VSIX 套件 | [⬇️ **readmd-vscode-2.3.6.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.6.vsix) | VSCode 離線擴充安裝套件 |
| 🤖 **MCP Server** | FastMCP stdio 套件 | [⬇️ **readmd-mcp-server-2.3.6.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.6.zip) | FastMCP 獨立伺服端，支援 Claude Desktop / Cursor |
| 🔐 **SHA-256 驗證** | 雜湊清單 | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | 發行檔案 SHA-256 完整性雜湊清單 |

---

<div align="center">

**ReadMD** · 純本機優先，全平台自由閱讀寫作。

</div>
