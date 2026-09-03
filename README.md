<p align="center">
 <b>Languages</b>:
 <b>简体中文</b> |
 <a href="README.zh-TW.md">繁體中文</a> |
 <a href="README.en.md">English</a> |
 <a href="README.ja.md">日本語</a>
</p>

<div align="center">
 <img src="assets/icon-256.png" width="88" alt="ReadMD logo">

 # ReadMD

 **Open huge Markdown files locally. Keep the original untouched.**

 ReadMD 是本地优先的 Markdown 阅读器和编辑器：双击即读，超长文档不假死，常见语法错误只在显示层修复；支持 Office / PDF / 网页转 MD、离线 OCR、LaTeX 学术增强和 MCP 接入。

 [![platform](https://img.shields.io/badge/Windows%2010%2F11%20%7C%20macOS%2013%2B%20%7C%20Linux%20%7C%20UOS%20%7C%20Kylin-blue)](#正式下载矩阵-release-assets)
 [![i18n](https://img.shields.io/badge/languages-46-orange)](#全球-46-语种-i18n-全量母语化)
 [![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)
 [![release](https://img.shields.io/github/v/release/Natsummerance/readMD)](https://github.com/Natsummerance/readMD/releases/latest)
 [![website](https://img.shields.io/badge/site-readmd.asia-black)](https://readmd.asia)
</div>

## 30 秒判断它适不适合你

| 你的情况 | ReadMD 的做法 |
| --- | --- |
| 文件超过 8,000 行或 500KB | 自动启用语义分页；目录和 Ctrl+F 跨页联动 |
| 表格缺分隔线或公式未闭合 | 渲染前在内存修正；原文件仍保持不变 |
| 手里有 DOCX / PPTX / XLSX / PDF / HTML | 一键转入 Markdown，减少手工搬运 |
| 需要提取扫描件文字 | 调用系统原生 OCR，支持离线工作流 |
| 写论文或技术文档 | BibTeX 引用卡片、定理/证明盒子与 LaTeX 导出 |
| 想接入 AI 编程助手 | 提供 VS Code 扩展和 FastMCP stdio Server |

## 直接下载

[Windows 安装包](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMDSetup-v2.3.8.exe) ·
[Windows 便携版](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-portable-v2.3.8.exe) ·
[macOS Apple Silicon](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-arm64-v2.3.8.zip) ·
[macOS Intel](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-x64-v2.3.8.zip) ·
[Linux AppImage](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-x86_64-v2.3.8.AppImage) ·
[Linux ARM64](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-aarch64-v2.3.8.AppImage) ·
[UOS / 麒麟 Deb](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_amd64.deb) ·
[麒麟 V10 ARM64](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_arm64.deb) ·
[SHA-256](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/SHA256SUMS.txt)

## 三步开始

1. 按系统下载安装包，或选择免安装便携版。
2. 打开一个真实的大文件；先检查目录、搜索和表格渲染。
3. 需要修改时再编辑保存；自动修复不会在你不知情时改写源文件。

## AI 助手引用入口

- [简明产品索引](https://readmd.asia/llms.txt)：版本、平台、隐私边界和关键事实。
- [完整引用语料](https://readmd.asia/llms-full.txt)：长文档分页、非破坏修正、转换范围和常见问题的直接答案。

## 为什么值得 Star

ReadMD 解决的是长期资料库里的实际问题：大文件能继续读，导入稿不用重排，敏感内容可以留在本机，原文始终保留最终决定权。如果你在 Windows、macOS、Linux 或信创环境之间切换，它提供一致的工作流。

如果它帮你省下一次整理时间，请 [点 Star](https://github.com/Natsummerance/readMD)，让更多写作者找到这个项目。

## 候选下载矩阵 (Preview Release Assets)

本矩阵只列当前候选的交付目标，不等同于正式支持承诺。Windows 7 仅提供单独的遗留运行时构建，不与 Windows 10/11 安装包混装；HarmonyOS/OpenHarmony 及未完成实机验收的架构不属于 V2.3.8 支持范围。

| 操作系统 / 平台 | 架构 / 格式 | 直接下载链接 (GitHub Release) | 说明 |
| :--- | :--- | :--- | :--- |
| **Windows** | x64 (安装版) | [⬇️ **ReadMDSetup-v2.3.8.exe**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMDSetup-v2.3.8.exe) | 动画安装向导，自动关联 `.md` 文件为默认打开方式 |
| **Windows** | x64 (绿色便携版) | [⬇️ **ReadMD-portable-v2.3.8.exe**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-portable-v2.3.8.exe) | 单文件绿色版，免安装解压即用 |
| **macOS** | Apple Silicon (M系列) | [⬇️ **ReadMD-macos-arm64-v2.3.8.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-arm64-v2.3.8.zip) | M1 / M2 / M3 / M4 原生构建（含 Vision 离线 OCR） |
| **macOS** | Intel x86_64 | [⬇️ **ReadMD-macos-x64-v2.3.8.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-macos-x64-v2.3.8.zip) | Intel 芯片 Mac 原生构建（含 Vision 离线 OCR） |
| **Linux** | x86_64 (AppImage) | [⬇️ **ReadMD-linux-x86_64-v2.3.8.AppImage**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-x86_64-v2.3.8.AppImage) | 通用 Linux 免安装 AppImage，赋予执行权限后直接运行 |
| **Linux** | ARM64 (AppImage) | [⬇️ **ReadMD-linux-aarch64-v2.3.8.AppImage**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-aarch64-v2.3.8.AppImage) | 飞腾 / 鲲鹏等 ARM64 设备免安装运行 |
| **国产信创系统** | 统信 UOS / 银河麒麟 / Deepin / Ubuntu | [⬇️ **readmd_2.3.8_amd64.deb**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_amd64.deb) | Deb 原生安装包；需以对应系统的实机验收记录为准 |
| ️ **麒麟 V10 / 飞腾** | ARM64 (aarch64) | [⬇️ **readmd_2.3.8_arm64.deb**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_arm64.deb) | 飞腾 D2000/E2000 原生构建，内置 UKUI/X11 与软件渲染安全回退 |
| **VSCode 插件** | 通用 VSIX 扩展包 | [⬇️ **readmd-vscode-2.3.8.vsix**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd-vscode-2.3.8.vsix) | VSCode 离线安装包，支持双向同步预览与格式自愈 |
| **MCP Server** | FastMCP stdio 独立包 | [⬇️ **readmd-mcp-server-2.3.8.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd-mcp-server-2.3.8.zip) | 独立 FastMCP 服务端，供 Claude Desktop / Cursor 一键接入 |
| **SHA-256 校验** | 完整性清单 | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/SHA256SUMS.txt) | 全量资产 SHA-256 哈希校验清单 |

---

## 多系统与信创国产适配

### 1. Linux 与国产操作系统适配（统信 UOS / 银河麒麟 / 深度）
- **直接安装使用**：下载 [`readmd_2.3.8_amd64.deb`](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_amd64.deb) 双击安装，或直接运行 [`ReadMD-linux-x86_64-v2.3.8.AppImage`](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/ReadMD-linux-x86_64-v2.3.8.AppImage)。银河麒麟 V10 + 飞腾 ARM64 设备使用 [`readmd_2.3.8_arm64.deb`](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd_2.3.8_arm64.deb)。
- **飞腾安全回退**：自动识别 Phytium D2000/E2000/FT 系列；WebKitGTK 启动时优先选择 UKUI/X11、禁用 DMABUF 合成并回退 llvmpipe，避免旧 GPU 驱动白屏或崩溃。
- **系统与环境识别**：`src/readmd_modules/linux_native.py` 自动识别系统发行版，自适应配置 Wayland / X11 显示后端。
- **桌面与深色模式**：自动侦测 DDE（统信/Deepin）、UKUI（银河麒麟）与 GNOME/KDE 的外观主题，实时同步深色/浅色配色。
- **桌面集成与关联**：内置 FreeDesktop 桌面入口与 MIME 类型声明，支持双击 `.md` 默认打开。
- **支持边界**：本版本不宣称 openEuler、玲珑或其他未列出的发行版具备完整原生支持。

### 2. Windows & macOS 平台特性
- **Windows**：内置 WinRT 原生 OCR、WebView2 硬件加速渲染、系统托盘常驻与智能单实例通信。
- **macOS**：原生 Apple Vision 离线文字识别、WebKit 视窗引擎、跟随系统外观与 Touch Bar 快捷操作。

---

## VSCode 扩展与 MCP Server 生态

### 1. VSCode 插件安装与使用
- **界面安装**：下载 [`readmd-vscode-2.3.8.vsix`](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd-vscode-2.3.8.vsix) -> 在 VSCode 扩展面板选择 `... -> 从 VSIX 安装...`。
- **命令行安装**：
 ```bash
 code --install-extension readmd-vscode-2.3.8.vsix
 ```
- **核心功能**：打开 Markdown 文件点击右上角书本图标开启同步预览；右键菜单支持一键自动修复语法错误与转换为 LaTeX 源码。

### 2. MCP (Model Context Protocol) Server 配置
直接下载解压 [`readmd-mcp-server-2.3.8.zip`](https://github.com/Natsummerance/readMD/releases/download/v2.3.8/readmd-mcp-server-2.3.8.zip)，在 Claude Desktop、Cursor、Antigravity 与 Cline 中配置：

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

## LaTeX PRO 学术增强与 BibTeX 引用

1. **BibTeX 自动扫描与引用卡片**：
 - 自动扫描同目录下 `.bib` 参考文献文件；
 - 文中书写 `[@vaswani2017attention]` 或 `@author2024` 自动渲染为引用徽章；
 - 悬停徽章展示论文标题、作者、年份、DOI 链接并支持一键复制 BibTeX；文末自动汇总生成参考文献表。
2. **学术 Callout 盒子**：
 - `::: theorem [定理名称]` / `::: lemma [引理名称]` / `::: definition [概念定义]`
 - `::: proof` 证明块自动添加文末 Q.E.D. ■ 符号。

---

## Editor Studio PRO 沉浸编辑

- **Zen Mode 禅模式**：按 <kbd>F11</kbd> 或点击编辑栏「 禅模式」全屏专注写作，按 <kbd>Esc</kbd> 退出。
- **10×10 可视化表格设计器**：鼠标自由滑选行列生成工整 Markdown 表格。
- **智能表格粘贴**：复制 Excel、WPS、Numbers 或网页表格，在编辑器中直接粘贴为 Markdown 表格。
- **实时文档统计**：实时显示中文字数、西文词数与预计阅读时间。

---

## 全球 46 语种 i18n 全量母语化

ReadMD 提供 46 个 locale；每次发布均校验键、占位符与方向一致性。候选版的语义翻译质量和原生平台可用性以对应验收证据为准，不以静态字典数量替代实测。

| 分区 | 覆盖语言 | 详细语言列表 |
| :--- | :--- | :--- |
| **东亚** (6语) | 中文/日/韩/蒙 | 简体中文 (`zh-CN`)、繁体中文·香港 (`zh-HK`)、繁体中文·台湾 (`zh-TW`)、日本語 (`ja`)、한국어 (`ko`)、Монгол хэл (`mn`) |
| **欧洲主流** (19语) | 西欧/中欧/东欧/北欧 | English (`en`)、Français (`fr`)、Deutsch (`de`)、Español (`es`)、Português (`pt`)、Русский (`ru`)、Italiano (`it`)、Nederlands (`nl`)、Türkçe (`tr`)、Ελληνικά (`el`)、Magyar (`hu`)、Українська (`uk`)、Hrvatski (`hr`)、Slovenščina (`sl`)、Română (`ro`)、Dansk (`da`)、Norsk (`no`)、Svenska (`sv`)、Suomi (`fi`) |
| **中东与南亚** (7语) | 阿/希/维/藏/印/孟/尼 | العربية (`ar`, RTL)、עברית (`he`, RTL)、ئۇيغۇرچە (`ug`, RTL)、བོད་སྐད། (`bo`)、हिन्दी (`hi`)、বাংলা (`bn`)、नेपाली (`ne`) |
| **东南亚** (8语) | 泰/越/印尼/马/缅/老/高棉/菲 | ภาษาไทย (`th`)、Tiếng Việt (`vi`)、Bahasa Indonesia (`id`)、Bahasa Melayu (`ms`)、မြန်မာဘာသာ (`my`)、ພາສາລາວ (`lo`)、ភាសាខ្មែរ (`km`)、Tagalog (`tl`) |
| **特色与区域** (6语) | 爱/马耳他/格陵兰/世界/非 | Gaeilge (`ga`)、Malti (`mt`)、Kalaallisut (`kl`)、Esperanto (`eo`)、Ikinyarwanda (`rw`)、Kikongo (`kg`) |

> 详细规范与词条清单：[`docs/i18n-language-reference.md`](docs/i18n-language-reference.md)

---

## 核心功能与使用指南

- **自动修正**：表格缺线对齐补全、加粗与公式未闭合自动闭合、`#标题` 补空格，修正详情在「 修复」面板中清晰展示。
- **万物转 MD**：支持 Word、PowerPoint、Excel、PDF、HTML、LaTeX 等批量或拖拽转换，转换后自动在新标签页打开。
- **扫描 OCR**：Windows (WinRT) 与 macOS (Vision) 本地离线识别，剪贴板截图一键提取文字。
- **网页转 MD**：Trafilatura 智能抽取正文，动态单页应用自动通过无头 WebView 渲染提取。
- **局域网共享**：点击工具栏「」生成二维码与随机密钥，手机扫码在同 Wi-Fi 下阅读与编辑。

---

## ️ 常用快捷键

| 快捷键 | 功能操作 | 快捷键 | 功能操作 |
| :--- | :--- | :--- | :--- |
| <kbd>Ctrl</kbd>+<kbd>O</kbd> | 打开 Markdown 文件 | <kbd>Ctrl</kbd>+<kbd>E</kbd> | 进入/退出编辑模式 |
| <kbd>Ctrl</kbd>+<kbd>S</kbd> | 保存当前文档（首存自动备份） | <kbd>Ctrl</kbd>+<kbd>P</kbd> | 打开导出面板 (PDF/Word/HTML) |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | 全文搜索 (Enter 下一个 / Esc 退出) | <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd> | 展开/收起目录大纲侧栏 |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> | 呼出 AI 助手面板 | <kbd>Ctrl</kbd>+<kbd>U</kbd> | 打开网页转 MD 弹窗 |
| <kbd>Ctrl</kbd>+<kbd>D</kbd> | 切换主题 (浅色/暗色/复古) | <kbd>F11</kbd> / <kbd>Esc</kbd> | 进入 / 退出 Zen 禅模式 |

---

## 架构与源码构建

```
readmd/
├─ readmd.py # 主程序入口（本地 Web 服务 + 视窗 + 托盘常驻）
├─ src/ # 核心算法与原生适配层
│ ├─ readmd_fix.py # Markdown 自动修正引擎
│ └─ readmd_modules/ # 原生平台适配 (linux_native.py) / OCR / 转换 / AI
├─ packages/ # 多端生态分包
│ ├─ vscode-extension/ # VSCode 官方扩展
│ ├─ mcp-server/ # FastMCP (stdio) 服务端
│ ├─ harmonyos-app/ # HarmonyOS/OpenHarmony 实验性源码预览（未支持）
│ └─ linglong/ # 统信 UOS 玲珑容器打包配置
├─ scripts/ # 打包与构建脚本 (windows / linux / unix)
├─ assets/ # 前端渲染、主题 token 与 46 语种 i18n 资源
├─ docs/ # 语言对照清单与开发文档
└─ tests/ # 单元测试与回归套件 (151 项全通过)
```

**本地构建与运行**：
- Windows：运行 `scripts\windows\install.bat` 安装依赖，`scripts\windows\run.bat` 运行；`scripts\windows\package.bat` 打包。
- Linux：运行 `bash scripts/linux/build_linux.sh` 打包 AppImage 与 Deb 安装包。
- macOS：运行 `./install.sh` 安装依赖，`./setup.sh` 构建 ReadMD.app。
- HarmonyOS/OpenHarmony：本版本不支持，目录仅保留实验性源码预览。

---

## 更新日志 (v2.3.8-preview.1)

- **46 语种全量母语化与机翻清洗**：深入审核 42 个非中英文语言字典，消除波兰语混淆、西部之塔、农作物裁剪等全部系统性翻译错误，实现 100% 完整覆盖。
- **Linux 与信创国产系统适配**：新增 `linux_native.py` 原生模块；正式支持只覆盖矩阵中有实机证据的 Ubuntu、Debian、UOS、银河麒麟和 Deepin 架构。
- **离线来源与 Skills**：上游原文、许可证和适配元数据随包提供，AI 指令统一从 Skills 加载。
- **LaTeX PRO 学术论文套件**：零配置 `.bib` 参考文献浮动卡片、学术 Callout 盒子与 LaTeX 导出。
- **Editor Studio PRO 体验重塑**：Zen 禅模式全屏专注、10×10 表格设计器、Excel 智能转换。
- **VSCode 插件与 MCP Server 生态完善**：升级至 v2.3.8，提供多端 AI 写作协作流。

---

## 常见问题

- **Q: 为什么双击打开速度极快？**
 - A: 安装版为 onedir 目录部署，启动无需临时解压；关闭后常驻系统托盘，二次打开通过轻量本地通信在 0.3s 内瞬间唤醒。
- **Q: 打开未签名安装包被系统拦截怎么办？**
 - A: Windows 在 SmartScreen 提示点击「更多信息 → 仍要运行」；macOS 首次打开在 Finder 中右键 `ReadMD.app` 选择「打开」。请核对 `SHA256SUMS.txt` 校验码。
- **Q: 文档格式自动修正常见错误会改写我的原始文件吗？**
 - A: **绝不会**。自动修正仅发生在内存渲染阶段，原 `.md` 源码保持 100% 完好无损。

---

<div align="center">

**ReadMD** · 纯本地优先，在已验收平台上自由阅读写作。

</div>
