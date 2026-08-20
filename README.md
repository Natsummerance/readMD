<p align="center">
  🌐 <b>Languages / 多语言版本</b>: 
  <b>简体中文</b> | 
  <a href="README.zh-TW.md">繁體中文</a> | 
  <a href="README.en.md">English</a> | 
  <a href="README.ja.md">日本語</a>
</p>

<div align="center">

<img src="assets/icon-256.png" width="96" alt="ReadMD logo">

# 📖 ReadMD · 轻量级全平台 Markdown 阅读与编辑器

**纯本地 · 极速秒开 · 离线可用 · 跨平台原生体验**

双击 `.md` 即读，渲染前自动修正常见语法错误（表格 / 加粗 / 公式 / 标题），**只影响显示，绝不改写原文件**；集成 AI 助手、万物转 MD、扫描 OCR、网页转 MD、LaTeX PRO 学术增强、Zen 禅模式与局域网移动端共享。

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Kylin%20%7C%20UOS%20%7C%20HarmonyOS-0078d6)
![version](https://img.shields.io/github/v/release/Natsummerance/readMD?color=3b6ef5)
![webview2](https://img.shields.io/badge/runtime-WebView2%20%7C%20WebKit%20%7C%20ArkWeb-4fc08d)
![repo size](https://img.shields.io/github/repo-size/Natsummerance/readMD)
![i18n](https://img.shields.io/badge/i18n-46%20Languages-orange)
![license](https://img.shields.io/badge/license-MIT-green)

<br>

<p align="center">
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.4.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_安装包-v2.3.4-2563eb?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 安装包">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.4.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_便携版-v2.3.4-0284c7?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 便携版">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.4.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(M芯片)-v2.3.4-111827?style=for-the-badge&logo=apple&logoColor=white" alt="macOS ARM64">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.4.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Intel)-v2.3.4-4b5563?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Intel">
  </a>
  <br>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.4.AppImage">
    <img src="https://img.shields.io/badge/⬇️_Linux_AppImage-v2.3.4-ea580c?style=for-the-badge&logo=linux&logoColor=white" alt="Linux AppImage">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.4_amd64.deb">
    <img src="https://img.shields.io/badge/⬇️_信创·统信UOS·麒麟_Deb-v2.3.4-b91c1c?style=for-the-badge&logo=debian&logoColor=white" alt="Deb 安装包">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-harmonyos-v2.3.4.hap">
    <img src="https://img.shields.io/badge/📱_鸿蒙_HarmonyOS_HAP-v2.3.4-059669?style=for-the-badge&logo=huawei&logoColor=white" alt="HarmonyOS HAP">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.4.vsix">
    <img src="https://img.shields.io/badge/🧩_VSCode_插件-v2.3.4-6366f1?style=for-the-badge&logo=visualstudiocode&logoColor=white" alt="VSCode VSIX">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.4.zip">
    <img src="https://img.shields.io/badge/🤖_MCP_Server-v2.3.4-0d9488?style=for-the-badge&logo=fastapi&logoColor=white" alt="MCP Server">
  </a>
</p>

</div>

---

## ✨ 核心特性

- 📖 **超长文档智能语义分页**：面对 >10,000 行超长文档自动激活智能分页，纯 SVG 极简矢量控制栏，大纲 (TOC) 与全文搜索 (Ctrl+F) 跨页联动，视口公式按需排版，杜绝假死。
- ⚡ **极速秒开**：安装版采用 onedir 目录结构，冷启动 ≤1.5s；关闭窗口常驻托盘，双击 `.md` 瞬时唤起（<0.3s）。
- 💻 **全操作系统原生覆盖**：Windows (Win 7 ~ 11)、macOS (Apple Silicon / Intel)、Linux (Ubuntu / Debian / Arch / Fedora)、国产信创 (银河麒麟 KylinOS / 统信 UOS / 深度 Deepin / openEuler) 与纯血鸿蒙 (HarmonyOS NEXT / OpenHarmony)。
- 🌍 **全球 46 语种 i18n 体系**：自适应操作系统语言初始化，支持 LTR 与 RTL 双向排版（阿拉伯语 / 希伯来语 / 维吾尔语），100% 纯正母语化覆盖。
- 📐 **LaTeX PRO 学术增强**：零配置自动扫描同目录 `.bib` 参考文献库生成悬浮引用卡片；内置 Theorem、Lemma、Proof (带 Q.E.D. ■ 徽标)、Definition 等学术 Callout 盒子。
- 🧘 **Editor Studio PRO 沉浸编辑**：Zen 禅模式（F11 / Esc 全屏专注）、10×10 可视化表格网格设计器、Excel / CSV 智能粘贴转 Markdown、实时字数与阅读时长看板。
- 🔌 **VSCode 插件与 MCP Server**：独立 VSCode 扩展支持双向同步预览与一键语法修复；标准 FastMCP (stdio) 赋能 Claude Desktop、Cursor、Antigravity 与 AI 编程助手。
- 🔄 **万物转 MD 与原生离线 OCR**：Word (.docx)、PowerPoint (.pptx)、Excel (.xlsx)、PDF、HTML、LaTeX (.tex) 一键转 MD；系统原生 OCR 引擎提取图片文字。
- 🌐 **网页转 MD 与局域网共享**：双级智能抽取正文（Trafilatura + 无头 WebView）；开启共享后手机扫码同一 Wi-Fi 下阅读与编辑。
- 🛠 **非破坏性自动修正**：表格缺分隔行 / 对齐错位、未闭合符号、公式语法缺失等仅在内存渲染阶段自动修正，原文件零修改。

---

## 🚀 全平台直接下载矩阵 (Release Assets)

| 操作系统 / 平台 | 架构 / 格式 | 直接下载链接 (GitHub Release) | 说明 |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | x64 (安装版) | [⬇️ **ReadMDSetup-v2.3.4.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.4.exe) | 动画安装向导，自动关联 `.md` 文件为默认打开方式 |
| 💼 **Windows** | x64 (绿色便携版) | [⬇️ **ReadMD-portable-v2.3.4.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.4.exe) | 单文件绿色版，免安装解压即用 |
| 🍏 **macOS** | Apple Silicon (M系列) | [⬇️ **ReadMD-macos-arm64-v2.3.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.4.zip) | M1 / M2 / M3 / M4 原生构建（含 Vision 离线 OCR） |
| 💻 **macOS** | Intel x86_64 | [⬇️ **ReadMD-macos-x64-v2.3.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.4.zip) | Intel 芯片 Mac 原生构建（含 Vision 离线 OCR） |
| 🐧 **Linux** | x86_64 (AppImage) | [⬇️ **ReadMD-linux-x86_64-v2.3.4.AppImage**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.4.AppImage) | 通用 Linux 免安装 AppImage，赋予执行权限后直接运行 |
| 🇨🇳 **国产信创系统** | 统信 UOS / 银河麒麟 / Deepin / Ubuntu | [⬇️ **readmd_2.3.4_amd64.deb**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.4_amd64.deb) | Deb 原生安装包，集成应用菜单图标、MIME 关联与 UKUI/DDE 主题适配 |
| 📱 **HarmonyOS NEXT** | 纯血鸿蒙 / OpenHarmony (HAP) | [⬇️ **ReadMD-harmonyos-v2.3.4.hap**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-harmonyos-v2.3.4.hap) | 鸿蒙原生应用安装包，ArkTS + ArkUI + ArkWeb 架构 |
| 🧩 **VSCode 插件** | 通用 VSIX 扩展包 | [⬇️ **readmd-vscode-2.3.4.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.4.vsix) | VSCode 离线安装包，支持双向同步预览与格式自愈 |
| 🤖 **MCP Server** | FastMCP stdio 独立包 | [⬇️ **readmd-mcp-server-2.3.4.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.4.zip) | 独立 FastMCP 服务端，供 Claude Desktop / Cursor 一键接入 |
| 🔐 **SHA-256 校验** | 完整性清单 | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | 全量资产 SHA-256 哈希校验清单 |

---

## 💻 多系统与信创国产 / 鸿蒙深度适配

### 1. Linux 与国产操作系统适配（统信 UOS / 银河麒麟 / 深度 / openEuler）
- **直接安装使用**：下载 [`readmd_2.3.4_amd64.deb`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd_2.3.4_amd64.deb) 双击安装，或直接运行 [`ReadMD-linux-x86_64-v2.3.4.AppImage`](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-linux-x86_64-v2.3.4.AppImage)。
- **系统与环境识别**：`src/readmd_modules/linux_native.py` 自动识别系统发行版，自适应配置 Wayland / X11 显示后端。
- **桌面与深色模式**：自动侦测 DDE（统信/Deepin）、UKUI（银河麒麟）与 GNOME/KDE 的外观主题，实时同步深色/浅色配色。
- **桌面集成与关联**：内置 FreeDesktop 桌面入口与 MIME 类型声明，支持双击 `.md` 默认打开。
- **玲珑分发格式**：提供 `packages/linglong/linglong.yaml`，适配统信应用商店分发。

### 2. HarmonyOS NEXT (纯血鸿蒙) 与 OpenHarmony 原生应用
- **应用包安装**：直接下载 [`ReadMD-harmonyos-v2.3.4.hap`](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-harmonyos-v2.3.4.hap) 或使用 DevEco Studio NEXT 打开 `packages/harmonyos-app/` 编译。
- **ArkUI + ArkWeb 架构**：通过 ArkWeb 容器完整复用 ReadMD 离线渲染核心（Marked + KaTeX + 46 语种 i18n + LaTeX PRO）。
- **系统能力桥接 (`ReadMDBridge.ets`)**：
  - 系统剪贴板交互 (`@ohos.pasteboard`)；
  - 原生文件选择与保存 (`@ohos.file.picker` / `@ohos.file.fs`)；
  - 系统区域语言自适应检测 (`@ohos.i18n`)；
  - 鸿蒙原生离线文字识别 (`@ohos.ai.OCR`)。

### 3. Windows & macOS 平台特性
- **Windows**：内置 WinRT 原生 OCR、WebView2 硬件加速渲染、系统托盘常驻与智能单实例通信。
- **macOS**：原生 Apple Vision 离线文字识别、WebKit 视窗引擎、跟随系统外观与 Touch Bar 快捷操作。

---

## 🧩 VSCode 扩展与 MCP Server 生态

### 1. VSCode 插件安装与使用
- **界面安装**：下载 [`readmd-vscode-2.3.4.vsix`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.4.vsix) -> 在 VSCode 扩展面板选择 `... -> 从 VSIX 安装...`。
- **命令行安装**：
  ```bash
  code --install-extension readmd-vscode-2.3.4.vsix
  ```
- **核心功能**：打开 Markdown 文件点击右上角书本图标开启同步预览；右键菜单支持一键自动修复语法错误与转换为 LaTeX 源码。

### 2. MCP (Model Context Protocol) Server 配置
直接下载解压 [`readmd-mcp-server-2.3.4.zip`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-mcp-server-2.3.4.zip)，在 Claude Desktop、Cursor、Antigravity 与 Cline 中配置：

```json
{
  "mcpServers": {
    "readmd": {
      "command": "python",
      "args": ["packages/mcp-server/readmd_mcp_server.py"]
    }
  }
}
```

---

## 📐 LaTeX PRO 学术增强与 BibTeX 引用

1. **BibTeX 自动扫描与引用卡片**：
   - 自动扫描同目录下 `.bib` 参考文献文件；
   - 文中书写 `[@vaswani2017attention]` 或 `@author2024` 自动渲染为引用徽章；
   - 悬停徽章展示论文标题、作者、年份、DOI 链接并支持一键复制 BibTeX；文末自动汇总生成参考文献表。
2. **学术 Callout 盒子**：
   - `::: theorem [定理名称]` / `::: lemma [引理名称]` / `::: definition [概念定义]`
   - `::: proof` 证明块自动添加文末 Q.E.D. ■ 符号。

---

## 🧘 Editor Studio PRO 沉浸编辑

- **Zen Mode 禅模式**：按 <kbd>F11</kbd> 或点击编辑栏「🧘 禅模式」全屏专注写作，按 <kbd>Esc</kbd> 退出。
- **10×10 可视化表格设计器**：鼠标自由滑选行列生成工整 Markdown 表格。
- **智能表格粘贴**：复制 Excel、WPS、Numbers 或网页表格，在编辑器中直接粘贴为 Markdown 表格。
- **实时文档统计**：实时显示中文字数、西文词数与预计阅读时间。

---

## 🌍 全球 46 语种 i18n 全量母语化

ReadMD 完整支持 46 种语言，初次启动自适应系统语言，通过独立真伪审计（≥97% 真实母语率），零英文 fallback。

| 分区 | 覆盖语言 | 详细语言列表 |
| :--- | :--- | :--- |
| 🌏 **东亚** (6语) | 中文/日/韩/蒙 | 简体中文 (`zh-CN`)、繁体中文·香港 (`zh-HK`)、繁体中文·台湾 (`zh-TW`)、日本語 (`ja`)、한국어 (`ko`)、Монгол хэл (`mn`) |
| 🌍 **欧洲主流** (19语) | 西欧/中欧/东欧/北欧 | English (`en`)、Français (`fr`)、Deutsch (`de`)、Español (`es`)、Português (`pt`)、Русский (`ru`)、Italiano (`it`)、Nederlands (`nl`)、Türkçe (`tr`)、Ελληνικά (`el`)、Magyar (`hu`)、Українська (`uk`)、Hrvatski (`hr`)、Slovenščina (`sl`)、Română (`ro`)、Dansk (`da`)、Norsk (`no`)、Svenska (`sv`)、Suomi (`fi`) |
| 🕌 **中东与南亚** (7语) | 阿/希/维/藏/印/孟/尼 | العربية (`ar`, RTL)、עברית (`he`, RTL)、ئۇيغۇرچە (`ug`, RTL)、བོད་སྐད། (`bo`)、हिन्दी (`hi`)、বাংলা (`bn`)、नेपाली (`ne`) |
| 🌴 **东南亚** (8语) | 泰/越/印尼/马/缅/老/高棉/菲 | ภาษาไทย (`th`)、Tiếng Việt (`vi`)、Bahasa Indonesia (`id`)、Bahasa Melayu (`ms`)、မြန်မာဘာသာ (`my`)、ພາສາລາວ (`lo`)、ភាសាខ្មែរ (`km`)、Tagalog (`tl`) |
| 🌐 **特色与区域** (6语) | 爱/马耳他/格陵兰/世界/非 | Gaeilge (`ga`)、Malti (`mt`)、Kalaallisut (`kl`)、Esperanto (`eo`)、Ikinyarwanda (`rw`)、Kikongo (`kg`) |

> 📖 详细规范与词条清单：[`docs/i18n-language-reference.md`](docs/i18n-language-reference.md)

---

## 🧠 核心功能与使用指南

- **自动修正**：表格缺线对齐补全、加粗与公式未闭合自动闭合、`#标题` 补空格，修正详情在「🛠 修复」面板中清晰展示。
- **万物转 MD**：支持 Word、PowerPoint、Excel、PDF、HTML、LaTeX 等批量或拖拽转换，转换后自动在新标签页打开。
- **扫描 OCR**：Windows (WinRT) 与 macOS (Vision) 本地离线识别，剪贴板截图一键提取文字。
- **网页转 MD**：Trafilatura 智能抽取正文，动态单页应用自动通过无头 WebView 渲染提取。
- **局域网共享**：点击工具栏「📱」生成二维码与随机密钥，手机扫码在同 Wi-Fi 下阅读与编辑。

---

## ⌨️ 常用快捷键

| 快捷键 | 功能操作 | 快捷键 | 功能操作 |
| :--- | :--- | :--- | :--- |
| <kbd>Ctrl</kbd>+<kbd>O</kbd> | 打开 Markdown 文件 | <kbd>Ctrl</kbd>+<kbd>E</kbd> | 进入/退出编辑模式 |
| <kbd>Ctrl</kbd>+<kbd>S</kbd> | 保存当前文档（首存自动备份） | <kbd>Ctrl</kbd>+<kbd>P</kbd> | 打开导出面板 (PDF/Word/HTML) |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | 全文搜索 (Enter 下一个 / Esc 退出) | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd> | 展开/收起目录大纲侧栏 |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> | 呼出 AI 助手面板 | <kbd>Ctrl</kbd>+<kbd>U</kbd> | 打开网页转 MD 弹窗 |
| <kbd>Ctrl</kbd>+<kbd>D</kbd> | 切换主题 (浅色/暗色/复古) | <kbd>F11</kbd> / <kbd>Esc</kbd> | 进入 / 退出 Zen 禅模式 |

---

## 📦 架构与源码构建

```
readmd/
├─ readmd.py                     # 主程序入口（本地 Web 服务 + 视窗 + 托盘常驻）
├─ src/                          # 核心算法与原生适配层
│  ├─ readmd_fix.py              #   Markdown 自动修正引擎
│  └─ readmd_modules/            #   原生平台适配 (linux_native.py) / OCR / 转换 / AI
├─ packages/                     # 多端生态分包
│  ├─ vscode-extension/          #   VSCode 官方扩展
│  ├─ mcp-server/                #   FastMCP (stdio) 服务端
│  ├─ harmonyos-app/             #   HarmonyOS NEXT (ArkTS) 鸿蒙原生应用
│  └─ linglong/                  #   统信 UOS 玲珑容器打包配置
├─ scripts/                      # 打包与构建脚本 (windows / linux / unix)
├─ assets/                       # 前端渲染、主题 token 与 46 语种 i18n 资源
├─ docs/                         # 语言对照清单与开发文档
└─ tests/                        # 单元测试与回归套件 (151 项全通过)
```

**本地构建与运行**：
- Windows：运行 `scripts\windows\install.bat` 安装依赖，`scripts\windows\run.bat` 运行；`scripts\windows\package.bat` 打包。
- Linux：运行 `bash scripts/linux/build_linux.sh` 打包 AppImage 与 Deb 安装包。
- macOS：运行 `./install.sh` 安装依赖，`./setup.sh` 构建 ReadMD.app。
- 鸿蒙：使用 DevEco Studio NEXT 打开 `packages/harmonyos-app` 进行编译。

---

## 📝 更新日志 (v2.3.4)

- 🌍 **46 语种全量母语化与机翻清洗**：深入审核 42 个非中英文语言字典，消除波兰语混淆、西部之塔、农作物裁剪等全部系统性翻译错误，实现 100% 完整覆盖。
- 🐧 **Linux 全架构与信创国产系统支持**：新增 `linux_native.py` 原生模块，适配银河麒麟 KylinOS V10 SP2、统信 UOS 20、深度 Deepin、openEuler；提供 FreeDesktop 桌面集成与玲珑容器声明。
- 📱 **HarmonyOS NEXT 鸿蒙原生应用分包**：基于 ArkTS + ArkUI + ArkWeb 构建，系统级桥接剪贴板、文件选择器与原生 OCR。
- 📐 **LaTeX PRO 学术论文套件**：零配置 `.bib` 参考文献浮动卡片、学术 Callout 盒子与 LaTeX 导出。
- 🧘 **Editor Studio PRO 体验重塑**：Zen 禅模式全屏专注、10×10 表格设计器、Excel 智能转换。
- 🔌 **VSCode 插件与 MCP Server 生态完善**：升级至 v2.3.4，提供稳定的多端 AI 写作协作流。

---

## ❓ 常见问题

- **Q: 为什么双击打开速度极快？**
  - A: 安装版为 onedir 目录部署，启动无需临时解压；关闭后常驻系统托盘，二次打开通过轻量本地通信在 0.3s 内瞬间唤醒。
- **Q: 打开未签名安装包被系统拦截怎么办？**
  - A: Windows 在 SmartScreen 提示点击「更多信息 → 仍要运行」；macOS 首次打开在 Finder 中右键 `ReadMD.app` 选择「打开」。请核对 `SHA256SUMS.txt` 校验码。
- **Q: 文档格式自动修正常见错误会改写我的原始文件吗？**
  - A: **绝不会**。自动修正仅发生在内存渲染阶段，原 `.md` 源码保持 100% 完好无损。

---

<div align="center">

**ReadMD** · 纯本地优先，全平台自由阅读写作。

</div>
