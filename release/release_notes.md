# ReadMD v2.3.7-beta.2 (修复与体验精细化发布)

ReadMD 是免费、开源的本地 Markdown 智能阅读、编辑与全格式转换排版套件；纯本地离线可用、秒级极速渲染，绝不改写原文件。

## 📦 全平台发布资产 (Release Assets)

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.7-beta.2.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.7-beta.2.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.2.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.2.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.2.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包 (UOS / 银河麒麟 / Deepin / Ubuntu / Debian)：`readmd_2.3.7-beta.2_amd64.deb`
- 📱 HarmonyOS NEXT 纯血鸿蒙安装包：`ReadMD-harmonyos-v2.3.7-beta.2.hap`
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.2.vsix`
- 🤖 FastMCP Server 独立包：`readmd-mcp-server-2.3.7-beta.2.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

---

## 🌟 本次版本核心修复与优化 (v2.3.7-beta.2)

### 1. 禅模式（Zen Mode）关闭交互重构（仿 Windows 自动隐藏任务栏）
- **彻底告别视觉干扰**：彻底移除了右下角/右上角突兀的实体浮动退出按钮（`.zen-exit-btn`），呈现 100% 极简纯净的沉浸式全屏文档阅读与编辑空间；
- **智能顶部感应滑出**：在窗口顶部注入悬停感应区 `#zen-hover-trigger`。当鼠标移至屏幕最顶端（`clientY <= 10px`）或悬停顶栏时，顶栏平滑滑出（含退出禅模式按钮、文档标题、字号缩放、主题切换）；鼠标移开（`clientY > 54px`）自动平滑滑回隐藏；
- **全局快捷键随时退出**：支持全局 `Esc` 和 `F11` 键随时一键退出禅模式，并带有精准的 Toast 引导提示与输入焦点自动恢复。

### 2. 样式定制（Custom Styles Modal）前端风格与功能重构
- **设计系统标准化**：移除底层技术黑话（如 `style.less / head.html`），全面重构为标准系统弹窗规范（`.modal-dialog`, `.style-modal-dialog`, `.style-modal-desc`, `.form-group`）；
- **常用排版模板快捷按钮栏**（`.style-presets-bar`）：
  - **首行缩进**：一键注入中文段落首行缩进 2 字符 CSS；
  - **精美表格**：一键注入现代圆角、斑马纹与高亮表头 CSS；
  - **等宽代码字体**：一键注入 Fira Code / Cascadia Code / Consolas 优先字体 CSS；
  - **打印分页优化**：一键注入 `@media print` 避免标题/表格跨页截断 CSS；
- **代码编辑体验增强**：代码输入框支持 `Tab` 键缩进插入 2 个空格以及 `Ctrl+Enter` / `Cmd+Enter` 快捷保存并即时生效。

### 3. 全球 46 国语言 i18n 与前端自动化测试 100% 覆盖
- **46 种语言 100% 同步**：新增的排版模板、禅模式提示文案均已同步至所有 46 国语言 JSON 字典文件（`assets/i18n/*.json`），实现 1,008 词条全量对齐（0 缺失）；
- **端到端测试全覆盖**：23 项 Playwright UI 端到端测试与 339 项单元/压力测试 100% 通过。

---

## 🔒 隐私与安全

- **纯本地运算**：Markdown 自愈、图表解析、Code Chunk 执行、EPUB 打包均在本地沙箱完成；
- **安全沙箱**：@import 包含越权路径防御与死循环防护，Code Chunk 具备进程隔离与超时自动终止；
- **零凭证泄露**：全仓经过严苛自动化隐私扫描，无任何硬编码密钥或外部未经授权的网络回传。

---

## 🛠️ SHA-256 完整性校验

下载对应文件后，核对文件名对应的一行：

Windows PowerShell：
```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.1.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.1.zip
```

