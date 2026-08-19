<p align="center">
  🌐 <b>Languages / 多語言切換</b>: 
  <a href="README.md">简体中文</a> | 
  <b>繁體中文</b> | 
  <a href="README.en.md">English</a> | 
  <a href="README.ja.md">日本語</a>
</p>

<div align="center">

<img src="assets/icon-256.png" width="96" alt="ReadMD logo">

# 📖 ReadMD · 輕量級 Markdown 閱讀與編輯器

**純本機 · 秒開 · 離線可用** 的 Windows / macOS Markdown 閱讀與編輯軟體。

連按兩下 `.md` 檔案立即閱讀。ReadMD 在渲染前自動修正常見 Markdown 語法錯誤（表格、粗體、LaTeX 公式、標題缺少空格等），**僅優化視覺預覽呈現，絕不擅自竄改原檔案內容**；內建 AI 助手、萬物轉 MD、原生 OCR 辨識、網頁轉 MD、雙向編輯與區域網路行動端共用。

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-0078d6)
![version](https://img.shields.io/badge/version-v2.3.0-3b6ef5)
![webview2](https://img.shields.io/badge/runtime-WebView2-4fc08d)
![license](https://img.shields.io/badge/license-MIT-green)
![i18n](https://img.shields.io/badge/i18n-46%20Languages-orange)

<br>

<p align="center">
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_安裝版下載-v2.3.0-2563eb?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 安裝版下載">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_免安裝便攜版-v2.3.0-0284c7?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 便攜版下載">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(M系列晶片)-v2.3.0-111827?style=for-the-badge&logo=apple&logoColor=white" alt="macOS ARM64 下載">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Intel晶片)-v2.3.0-4b5563?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Intel 下載">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix">
    <img src="https://img.shields.io/badge/🧩_VSCode_外掛套件下載-v2.3.0-6366f1?style=for-the-badge&logo=visualstudiocode&logoColor=white" alt="VSCode VSIX 下載">
  </a>
</p>

</div>

---

## ✨ 核心特性

- ⚡ **極速秒開**：安裝版採用 onedir 目錄架構，冷啟動視窗載入 ≤1.5s（低階硬體或機械硬碟 ≤2s）；縮小或關閉視窗常駐系統匣，再次連按 `.md` 檔案 <0.3s 瞬間喚醒。
- 🌍 **全球 46+ 種語言支援 (v2.3.0)**：首次啟動自動依據作業系統語言完成在地化設定。完整支援繁體中文（台灣/香港）、簡體中文、英語、日語、韓語、歐洲諸語系以及阿拉伯語/希伯來語雙向排版 (RTL)。
- 📐 **LaTeX PRO 學術研究套件 (v2.3.0)**：免設定自動掃描同目錄下的 `.bib` 參考文獻庫；文中引用徽章支援懸浮預覽卡片（直接查看論文資訊、跳轉 DOI 與一鍵複製 BibTeX），並內建定理 (Theorem)、引理 (Lemma)、證明 (Proof 附 Q.E.D. ■ 符號) 及定義 (Definition) 等專業學術 Callout 區塊。
- 🧘 **Editor Studio PRO 沉浸式編輯 (v2.3.0)**：Zen Mode 禪模式（<kbd>F11</kbd> / <kbd>Esc</kbd> 隨時切換）、10x10 可視化表格繪製器、Excel / CSV 剪貼簿智慧轉換 Markdown 表格、以及即時中英文字數與閱讀時間儀表板。
- 🔌 **VSCode 官方擴充套件與 MCP 伺服器 (v2.3.0)**：提供 Visual Studio Code 雙向同步預覽與一鍵自動修復擴充套件；內建標準 FastMCP (stdio) 服務，完美串接 Claude Desktop、Cursor、Antigravity 與 Cline。
- 🛡️ **內建升級器平滑運作**：修復升級安裝程式呼叫時的檔案鎖定問題，啟動時自動清理暫存安裝檔，支援多節點鏡像來源降級。
- 🎨 **極簡清爽介面**：44px 精巧工具列、淺色 / 暗色 / 復古羊皮紙 (Sepia) 三款主題 Token、超大文件骨架屏、動畫遵循系統「減少動態效果」偏好設定。
- 🤖 **AI 智慧助理**：內建 OpenAI、DeepSeek、Kimi、Anthropic、Ollama 本地模型等多款公開預設；API 金鑰僅加密儲存於本機。支援上下文對話、文章潤色、程式碼審查與自訂提示詞範本。
- 🔄 **萬物轉換為 Markdown**：支援將 Word (.docx)、PowerPoint (.pptx)、Excel (.xlsx)、PDF、HTML、LaTeX (.tex) 等檔案一鍵轉換為純淨 Markdown 並於新分頁開啟。
- 🔍 **系統原生離線 OCR**：Windows 使用 WinRT、macOS 使用 Vision，皆為系統本機離線文字辨識，安全無隱私外洩風險。
- 🌐 **網頁擷取轉 MD**：結合 Trafilatura 與無頭 WebView 引擎（內建 Defuddle 與 Readability），即便是動態單頁應用程式 (SPA) 亦可精準擷取正文。
- 📱 **區域網路行動端共用**：開啟共用後手機掃描 QR Code 即可在同 Wi-Fi 內遠端閱讀與編輯（採加密隨機權杖安全驗證）。
- 📤 **多格式專業匯出**：支援匯出為 PDF、Word (DOCX)、單一自包含 HTML 及完整 LaTeX 論文原始碼。

---

## 🚀 下載矩陣

| 平台 / 版本類型 | 直接下載連結 | 說明 |
| :--- | :--- | :--- |
| 🪟 **Windows 安裝版** | [⬇️ **ReadMDSetup-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe) | 具備安裝精靈，自動註冊 `.md` 檔案關聯；重複執行即可無縫覆蓋升級 |
| 💼 **Windows 免安裝便攜版** | [⬇️ **ReadMD-portable-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe) | 單一執行檔，解壓縮即用，適合放在隨身碟 |
| 🍏 **macOS Apple Silicon** | [⬇️ **ReadMD-macos-arm64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip) | 適用於 Apple Silicon (M1 / M2 / M3 / M4) 系列晶片 Mac 原生建置 |
| 💻 **macOS Intel** | [⬇️ **ReadMD-macos-x64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip) | 適用於 Intel 處理器 Mac 原生建置 |
| 🧩 **VSCode 擴充外掛** | [⬇️ **readmd-vscode-2.3.0.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix) | Visual Studio Code 離線擴充安裝套件 |
| 🔐 **SHA-256 驗證清單** | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | 發行檔案 SHA-256 完整性雜湊清單 |

---

## 🧩 VSCode 擴充套件安裝指南

ReadMD 官方擴充套件讓您在 Visual Studio Code 中亦能享有與 ReadMD 完全一致的高品質預覽與語法自動修復能力。

### 方式一：於 VSCode 介面直接安裝（最推薦）
1. 下載 [`readmd-vscode-2.3.0.vsix`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix) 擴充套件；
2. 在 VSCode 中按下快捷鍵 <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd>（Mac 上為 <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd>）開啟「延伸模組 (Extensions)」面板；
3. 點選延伸模組面板右上角的 **`...` 更多動作** 按鈕；
4. 選擇 **`從 VSIX 安裝... (Install from VSIX...)`**；
5. 選取剛剛下載的 `.vsix` 檔案，數秒內即可完成載入！

### 方式二：透過終端機指令一鍵安裝
開啟命令提示字元或終端機，執行以下指令：
```bash
code --install-extension readmd-vscode-2.3.0.vsix
```

---

## 🤖 MCP (Model Context Protocol) 伺服器設定

ReadMD 提供符合 FastMCP (stdio) 標準的協定伺服器，讓 Claude Desktop、Cursor、Antigravity 等 AI 程式設計助理直接調用 ReadMD 核心的文件轉換與自動修復工具。

### 設定設定範例

#### 1. Claude Desktop
編輯 `claude_desktop_config.json` 加入以下內容：
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
於 MCP 設定介面中新增：
- **Name**: `readmd`
- **Command**: `python`
- **Args**: `["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]`

#### 🛠️ 提供之核心 MCP 工具：
- `readmd_fix_markdown`: 自動修復 Markdown 語法錯位與排版缺陷；
- `readmd_convert_to_markdown`: 將本機 Office/PDF/LaTeX 文件轉換為純淨 Markdown；
- `readmd_latex_to_md`: 精確轉換 LaTeX 論文與數學公式；
- `readmd_md_to_latex`: 將 Markdown 一鍵編譯為標準學術 LaTeX 原始碼；
- `readmd_parse_bibtex`: 解析 `.bib` 參考文獻資料庫並回傳結構化論文資訊。

---

## ⌨️ 常用快速鍵

| 快速鍵組合 | 功能說明 |
| :--- | :--- |
| <kbd>F11</kbd> / <kbd>Esc</kbd> | 切換 / 退出 Zen Mode 沉浸禪模式 |
| <kbd>F2</kbd> | 重新命名目前檔案（副檔名鎖定保護） |
| <kbd>Ctrl</kbd> + <kbd>O</kbd> | 開啟本機檔案 |
| <kbd>Ctrl</kbd> + <kbd>E</kbd> | 切換閱讀模式與編輯模式 |
| <kbd>Ctrl</kbd> + <kbd>S</kbd> | 儲存檔案 |
| <kbd>Ctrl</kbd> + <kbd>F</kbd> | 全文搜尋與取代 |
| <kbd>Ctrl</kbd> + <kbd>P</kbd> | 匯出 PDF / DOCX / HTML / LaTeX |
| <kbd>Ctrl</kbd> + <kbd>D</kbd> | 快速切換主題風格 (淺色 / 暗色 / 羊皮紙) |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>A</kbd> | 開啟 AI 智慧助理面板 |
| <kbd>Ctrl</kbd> + <kbd>=</kbd> / <kbd>-</kbd> | 放大 / 縮小字體尺寸 |

---

<p align="center">
  <b>ReadMD</b> 堅持完全免費、開源與離線本機優先。<br>
  由 <a href="https://github.com/Natsummerance">Natsummerance</a> 用心打造與維護。
</p>
