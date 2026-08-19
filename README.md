<p align="center">
  🌐 <b>Languages / 多语言版本</b>: 
  <b>简体中文</b> | 
  <a href="README.zh-TW.md">繁體中文</a> | 
  <a href="README.en.md">English</a> | 
  <a href="README.ja.md">日本語</a>
</p>


<div align="center">

<img src="assets/icon-256.png" width="96" alt="ReadMD logo">

# 📖 ReadMD · 轻量级 Markdown 阅读器

**纯本地 · 秒开 · 离线可用** 的 Windows / macOS Markdown 阅读器。

双击 `.md` 即读，渲染前自动修正常见 Markdown 错误（表格 / 加粗 / 公式 / 标题），**只影响显示，绝不改写原文件**；集成 AI 助手、万物转 MD、扫描 OCR、网页转 MD、主动编辑与移动端共享。

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-0078d6)
![version](https://img.shields.io/github/v/release/Natsummerance/readMD?color=3b6ef5)
![webview2](https://img.shields.io/badge/runtime-WebView2-4fc08d)
![repo size](https://img.shields.io/github/repo-size/Natsummerance/readMD)
![i18n](https://img.shields.io/badge/i18n-46%20Languages-orange)

<br>

<p align="center">
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_安装包下载-v2.3.0-2563eb?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 安装包下载">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_便携版下载-v2.3.0-0284c7?style=for-the-badge&logo=windows&logoColor=white" alt="Windows 便携版下载">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(M系列芯片)-v2.3.0-111827?style=for-the-badge&logo=apple&logoColor=white" alt="macOS ARM64 下载">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Intel芯片)-v2.3.0-4b5563?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Intel 下载">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix">
    <img src="https://img.shields.io/badge/🧩_VSCode_插件下载-v2.3.0-6366f1?style=for-the-badge&logo=visualstudiocode&logoColor=white" alt="VSCode VSIX 下载">
  </a>
</p>

</div>


---

## ✨ 特性

- ⚡ **秒开**：安装版为 onedir 目录安装，冷启动窗口可用 ≤1.5s（低配机 / 机械硬盘 ≤2s）；关闭窗口隐藏到系统托盘常驻，再双击 `.md` 瞬时唤起（<0.3s）
- 🌍 **全球 45+ 语种 i18n 体系（v2.3.0）**：初次启动自动根据系统语言初始化，包含简体中文、繁中（台/港）、英语、西/法/德/意/日/韩/俄、阿拉伯语/希伯来语（RTL 双向排版）以及藏语、维吾尔语、蒙古语等；配套多模型（DeepSeek / Qwen / Mimo / Google）自动翻译维护工具链
- 📐 **LaTeX PRO 学术增强（v2.3.0）**：零配置自动扫描同目录 `.bib` 参考文献数据库并生成悬浮交互卡片；提供定理 (Theorem)、引理 (Lemma)、证明 (Proof with Q.E.D.)、定义 (Definition) 等学术 Callout 盒子
- 🧘 **Editor Studio PRO 极致体验（v2.3.0）**：Zen Mode 沉浸禅模式（F11 / Esc 切换）；10x10 可视化表格网格设计器；Excel / CSV 智能粘贴直接转 Markdown 表格；实时字数、词数与阅读时长看板
- 🔌 **VSCode 插件与 MCP Server（v2.3.0）**：采用 Monorepo 统一分包架构（`packages/mcp-server` & `packages/vscode-extension`），让 Claude Desktop、Cursor 及 VSCode 具备 ReadMD 核心自愈修复与学术排版能力，客户端安装包零冗余
- 🛡️ **软件内更新器排查与修复（v2.3.0）**：修复调起安装包后旧进程文件句柄锁死问题，启动自动清理 `%TEMP%` 残留安装包，支持多镜像源毫秒级降级与优雅手动检查反馈
- 🎨 **界面清爽**：44px 工具条 + 内联 SVG 图标、欢迎页最近文件网格、浅色 / 暗色 / sepia 三主题全套设计 token、大文档骨架屏、动画遵循系统「减弱动态效果」
- 🤖 **AI 助手与对话导入**：官方预设 + 可增删改的自定义连接；API Key 仅本机保存且配置接口不回传明文。可从一次性授权剪贴板、用户选择的导出文件或公开网页地址预览并导入对话
- 🔄 **万物转 MD**：docx / pptx / xlsx / pdf / html / csv / json / tex 等转为 Markdown；单文件或批量（多选 / 整文件夹）一键转换，结果自动保存到源目录同名 `.md` 并直接以新标签页打开
- 🔍 **扫描转 MD（OCR）**：Windows 使用 WinRT、macOS 使用 Vision，均为系统原生离线识别；剪贴板截图一键启动 OCR 提取排版文字
- 🌐 **网页转 MD**：Trafilatura 双级抽取，静态正文不足时自动使用系统 WebView 内核 + 离线 Defuddle / Mozilla Readability 动态渲染
- 📱 **移动端共享**：开启局域网共享后，手机扫码在同一 Wi-Fi 下阅读 / 转 MD / OCR / AI（随机令牌鉴权）
- 📑 **阅读体验**：多标签页系统、双击重命名固定扩展名防误触、目录侧栏（滚动高亮）、全文搜索、三主题、字号缩放、打印 / 导出 PDF、文件夹浏览、大文档增量渲染、文件外部修改自动刷新
- 📤 **导出 PDF / DOCX / HTML / LaTeX**：统一兼容 Windows/macOS 保存对话框路径，支持一键导出排版完备的 LaTeX (.tex) 论文源码
- 🛠 **自动修正**：表格缺分隔行 / 列数不齐、未闭合 `**` `__` `*`、未闭合 `$` `$$`、`#标题` 缺空格、BOM、CRLF 等，逐处列出修改

## 🚀 快速开始

**方式一：一键直接下载（推荐 · 点击即可直接下载）**

点击下方对应平台链接，无需跳转查找 Release 页面即可直接高速下载：

| 平台 / 类型 | 一键直接下载 | 说明 |
| :--- | :--- | :--- |
| 🪟 **Windows 安装版** | [⬇️ **ReadMDSetup-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe) | 动画安装向导，可自动关联 `.md` 为默认打开方式；已安装时运行即平滑升级（未签名） |
| 💼 **Windows 便携版** | [⬇️ **ReadMD-portable-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe) | 免安装绿色版，解压即用，随身携带（未签名） |
| 🍏 **macOS Apple Silicon** | [⬇️ **ReadMD-macos-arm64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip) | 适用于 M1 / M2 / M3 / M4 系列芯片 Mac 原生构建（未签名） |
| 💻 **macOS Intel** | [⬇️ **ReadMD-macos-x64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip) | 适用于 Intel 处理器 Mac 原生构建（未签名） |
| 🧩 **VSCode 插件离线包** | [⬇️ **readmd-vscode-2.3.0.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix) | VSCode 扩展安装包，支持双向同步预览与一键格式自愈修复 |
| 🔐 **SHA-256 清单** | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | 全量发布文件的 SHA-256 完整性校验清单 |

> 安装包自带 `ReadMDUninstall.exe` 卸载器，卸载时仅移除安装器创建的关联与文件，不动你的文档与配置。
>
> v2.3.0 在同一个 Release 中提供 Windows 安装版/便携版及 Intel/Apple Silicon macOS 包。所有发布包均未签名：Windows 如出现 SmartScreen，请先核验 SHA-256 后通过“更多信息 → 仍要运行”；macOS 包不包含 WinRT 或 Windows 安装器依赖，首次启动请在 Finder 中右键 `ReadMD.app` →“打开”。

---

## 🧩 VSCode 插件极速安装与使用指南

ReadMD 官方 VSCode 扩展为 Visual Studio Code 带来原汁原味的 ReadMD 极简双向预览、格式自愈修复与 LaTeX 转换能力。

### 安装方式一：VSCode 界面一键安装（推荐）
1. 下载 [`readmd-vscode-2.3.0.vsix`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix)；
2. 在 VSCode 中按快捷键 <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd>（Mac 上为 <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd>）打开扩展面板；
3. 点击扩展面板右上角的 **`...` 更多操作** 按钮，选择 **从 VSIX 安装... (Install from VSIX...)**；
4. 选中下载的 `.vsix` 文件即可秒级完成安装！

### 安装方式二：命令行一键安装
直接在终端中运行：
```bash
code --install-extension readmd-vscode-2.3.0.vsix
```

### 核心功能与使用：
- **📖 实时同步预览**：打开任一 Markdown 文档，点击右上角书本图标或按命令面板 `ReadMD: Open Custom Preview`，在侧边栏开启与 ReadMD 样式完全一致的高清预览；
- **🛠️ 一键格式自愈修复**：在 Markdown 编辑器中右键选择 `ReadMD: Auto-Fix Markdown Formatting Errors`，自动诊断并修复公式断裂、表格错位、代码块未闭合等格式问题；
- **📐 转换为 LaTeX 论文**：右键选择 `ReadMD: Convert Current Markdown to LaTeX`，将 Markdown 一键生成标准学术 LaTeX 源码。

---

## 🤖 MCP (Model Context Protocol) Server 接入指南

ReadMD 内置标准 FastMCP (stdio) 服务，赋能 Claude Desktop、Cursor、Antigravity、VSCode (Cline / Continue / Roo Code) 等 AI 编程助手直接调用 ReadMD 核心文档处理与自愈能力。

### 客户端配置示例

#### 1. Claude Desktop 配置
编辑 `%APPDATA%\Claude\claude_desktop_config.json`（Windows）或 `~/Library/Application Support/Claude/claude_desktop_config.json`（macOS）：

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

#### 2. Cursor / VSCode Cline 配置
在 Cursor MCP 设置或 Cline MCP Settings 中添加：
- **Name**: `readmd`
- **Type**: `command`
- **Command**: `python`
- **Args**: `["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]`

#### 🛠️ 提供的核心 MCP Tools 清单：
| 工具名称 | 功能描述 |
| :--- | :--- |
| **`readmd_fix_markdown`** | 自动修复 Markdown 文本语法与格式错误（公式、表格、缩进、转义等） |
| **`readmd_convert_to_markdown`** | 本地各种格式（Word/PDF/PPT/Excel/LaTeX/HTML/EPUB）转为干净 Markdown |
| **`readmd_latex_to_md`** | 将 LaTeX 论文或公式源码精确转为标准 Markdown |
| **`readmd_md_to_latex`** | 将 Markdown 编译为排版标准的独立学术 LaTeX 源码（含 booktabs 表格） |
| **`readmd_parse_bibtex`** | 扫描并解析 BibTeX (`.bib`) 参考文献库，提取结构化论文引用元数据 |

---

## 📐 LaTeX PRO 学术增强与 BibTeX 引用

ReadMD v2.3.0 专为科研与学术论文写作深度优化：
1. **BibTeX 零配置自动扫描**：
   - 当打开任一 Markdown 文档时，ReadMD 会自动扫描同目录下的 `.bib` 参考文献数据库；
   - 文中书写 `[@vaswani2017attention]` 或 `@knuth1984texbook` 会自动渲染为学术引用徽章；
   - **浮动卡片交互**：鼠标悬停引用徽章，弹出包含论文标题、作者、年份、期刊、DOI 链接及一键复制 BibTeX 的浮动交互卡片；
   - 文档末尾自动汇总生成排版规范的参考文献列表 (References)。
2. **学术定理 Callout 盒子**：
   - `::: theorem [柯西-施瓦茨不等式]` -> 定理高亮块
   - `::: lemma [引理名称]` -> 引理高亮块
   - `::: proof` -> 证明块（自动附带文末 Q.E.D. ■ 徽标）
   - `::: definition [流形]` -> 数学概念定义块

---

## 🧘 Editor Studio PRO 极致编辑体验

- **Zen Mode 沉浸禅模式**：按 <kbd>F11</kbd> 或点击编辑栏「🧘 禅模式」，全屏隐藏所有工具栏与干扰元素，支持按 <kbd>Esc</kbd> 或右上角退出按钮随时平滑复原；
- **10x10 可视化表格网格设计器**：点击「插入表格」，鼠标在 10×10 网格上自由滑选行列，一键生成对齐工整的 Markdown 表格骨架；
- **智能 Excel / CSV 粘贴转换**：从 Excel、WPS、Numbers 或网页直接复制多行多列数据，在编辑器中粘贴时自动识别并转为标准 Markdown 表格；
- **实时文档统计看板**：编辑栏实时计算中文字数、西文词数与预计阅读时间。

---

## 🌍 全球 46+ 语种 i18n 体系与多模型自动翻译

ReadMD 支持全球 46 种语言，初次启动自动根据操作系统区域语言初始化。

### 自动化翻译工具链 (`tools/i18n_sync.py`)
无需人工繁琐维护多语言，支持调用 Google Translate 或高性价比 AI 小模型（DeepSeek-V3 / Qwen-Max / Mimo / GLM）进行增量词条自动翻译与校验：

```bash
# 仅校验 46 语种字典完整性
python tools/i18n_sync.py --validate-only

# 使用免费 Google Translate 自动补齐缺失词条
python tools/i18n_sync.py --provider google

# 使用 DeepSeek / Qwen / Mimo 自动翻译
python tools/i18n_sync.py --provider openai --api-key YOUR_KEY --model deepseek-chat
```







> ReadMD 免费使用，不要求订阅或内置账号。下载文件和 `SHA256SUMS.txt` 后，Windows 在 PowerShell 运行 `Get-FileHash .\文件名 -Algorithm SHA256`，macOS 在终端运行 `shasum -a 256 文件名`，把输出与同名清单行完全比对。校验通过代表下载完整，不代表代码签名。

**方式二：源码运行（开发 / 自定义）**

环境要求：Windows 10/11（WebView2）或 macOS 12+，Python 3.9+。

```bat
双击 install.bat
```

macOS 源码运行或打包使用独立依赖：

```bash
./install.sh             # 安装 config/requirements-macos.txt
./setup.sh               # 构建未签名 ReadMD.app
```

脚本会创建 `.venv`、安装依赖并注册 `.md / .markdown / .mdown / .mkd` 文件关联（HKCU，无需管理员）。文件关联使用白色文档页 + 蓝色 MD 标识的独立图标，不会复用应用 Logo。之后直接双击任意 `.md` 文件即可用 ReadMD 打开；或：

```bat
run.bat                              rem 一键运行
.venv\Scripts\pythonw.exe readmd.py  rem 打开文件 / 空启动
python readmd.py --browser "文件.md"  rem 无 pywebview 时用浏览器兜底
```

> 若 Windows 仍用其他程序打开：右键 `.md` → 打开方式 → 选择 ReadMD → 始终使用；或点击阅读器工具栏「设为默认」。程序化修改默认应用受 Windows UserChoice 哈希保护，此路径已是最佳实践。

## 🪟 Win7 兼容版（v2.1.1 Beta）

> 正式版（v2.1.1）基于 Python 3.10 + pywebview 6.x，**不支持 Windows 7**。为仍在使用 Win7 的机器提供独立的 **v2.1.1 Beta** 兼容版，独立发布（pre-release tag `v2.1.1-beta`），不影响正式版。

**适用环境**：Windows 7 SP1 x64（需 .NET Framework 4.8 与 VC++ 运行库，详见 Release 说明）；安装包内嵌 **固定版 WebView2 109 运行时**（Win7 最后支持线），安装时自动放入安装目录，无需联网安装系统级运行时。

**与正式版一致的能力**：秒开（onedir 目录安装 + 单实例托盘）、浅色 / 暗色 / sepia 三主题阅读、自动修正、目录 / 搜索 / 编辑 / 导出 PDF·DOCX·HTML / 打印、docx / pdf 转 Markdown（含自动保存与严格校验）。

**Win7 版暂不支持**（入口会明确提示）：OCR（依赖 WinRT，仅 Win10+）、AI 助手、网页转 MD、以及 docx / pdf 以外的格式转换。

**构建**：独立 Python 3.9.13 构建链（`.venv-win7` + `win7-reqs.txt` + `build_win7.bat`），不污染正式版发布链。

## 🖱️ 使用

### 快捷键

| 快捷键 | 功能 |
| --- | --- |
| Ctrl+O | 打开文件 |
| Ctrl+U | 网页转 MD |
| Ctrl+E | 编辑当前 MD（Ctrl+S 保存） |
| Ctrl+F | 搜索（Enter 下一个 / Shift+Enter 上一个 / Esc 关闭） |
| Ctrl+Shift+F | 目录侧栏 |
| Ctrl+Shift+A | AI 助手面板 |
| Ctrl+Shift+P | 编辑模式下打开 Markdown 命令面板 |
| Ctrl+D | 切换主题 |
| Ctrl+= / Ctrl+- | 增大 / 减小字号 |
| Ctrl+R | 重新加载 |
| Ctrl+P | 打开导出面板（PDF / DOCX / HTML / 打印，含样式定制） |
| Ctrl+← / Ctrl+→ | 历史后退 / 前进 |

### 常用操作

- **打开**：工具栏「打开」或 Ctrl+O；「文件夹」浏览整个目录逐个阅读
- **最近文件**：欢迎页最近文件网格，一键回到上次阅读的位置
- **转换 / OCR / 网页**：可在「更多」菜单中找到；结果为虚拟文档，可「另存」为 `.md`
- **导出**：工具栏「导出」按钮或 Ctrl+P 打开导出面板，选格式与样式预设，一键导出 PDF / DOCX / HTML；导出后可「打开」或「所在文件夹」
- **修复**：渲染时自动修正会在「🛠 修复」面板中列出每一处修改
- **托盘**：关闭窗口即隐藏到系统托盘（再次打开瞬时）；托盘菜单「显示 / 打开文件 / 退出」

## 🧠 核心能力详解

### 自动修正

修正均为**保守启发式**，只发生在内存渲染阶段，并在「🛠 修复」面板中列出每一处修改：

- **表格**：检测连续的竖线行，缺少 `|---|` 表头分隔行时自动补全；各列不足时补齐空单元格；分隔行不足 3 个连字符时补足
- **加粗**：`**文字` 补全为 `**文字**`；游离的结束符转义为字面量；`2 * 3` 这类乘号转义；列表 `* 项` 与分隔线 `***` 不受影响
- **公式**：`$x^2$ 和 $y` 补全为 `$y$`；`$$` 块级公式未闭合时补 `$$`；`价格 $5` 这类货币不会被误判；代码块与行内代码内的内容一律跳过
- **标题**：`#标题` → `# 标题`

### 万物转 MD / 扫描 OCR / 网页转 MD

- **文件转换**：工具栏「转换」选择任意文件（或直接 `python readmd.py 文件.docx`），MarkItDown 转成 Markdown 后自动过修正器并渲染；未提取到文字时提示改用 OCR
- **扫描 / 图片**：工具栏「OCR」选择图片或 PDF；Windows 内置 OCR 离线识别（需系统已安装对应语言包，中文一般自带）；扫描版 PDF 逐页渲染后识别
- **网页**：工具栏「网页」输入 URL；默认智能提取正文，静态页面内容不足时桌面版会自动通过系统 WebView 动态渲染。可切换“保留完整页面”、合并同站最多 10 页，或选择把安全的远程图片下载到本地资源目录
- **网页限制**：仅处理公开 HTTP/HTTPS 页面，不绕过登录、验证码和付费墙；本机、局域网及云元数据地址会被安全策略阻止。局域网共享页面可使用静态抓取，动态渲染需在桌面应用中执行
- 转换 / OCR / 网页模块均为**首次渲染完成后的后台懒加载**，不影响 Markdown 阅读的启动速度

### 大文档增量渲染

- 超过 **300KB 或 6000 行** 的文档自动进入**增量分块渲染**：正文按围栏代码块 / 空行切成小块逐帧渲染，顶部显示「渲染中… N%」，可边渲染边滚动阅读
- 代码块与公式跨块保护不拆分，渲染完成后统一生成目录 / 搜索索引 / 公式排版，滚动位置自动恢复
- 小文档仍为一次性整篇渲染，启动速度不受影响

### 主动编辑（CodeMirror 6）

- 行号、括号配对、自动缩进、代码折叠、语法高亮（Markdown + 内嵌代码语言）、亮 / 暗主题跟随
- 输入 `#` `*` `` ` `` `[` `>` `|` `~` 等触发 18 种 Markdown 语法补全，选中即插入并定位光标
- 工具栏一键插入：加粗 / 斜体 / 删除线 / 标题 / 引用 / 列表 / 任务 / 链接 / 图片 / 行内代码 / 代码块 / 公式 / 表格 / 分隔线
- **插入图片**：本地图片可在画布上裁剪（自由 / 1:1 / 4:3 / 16:9）、旋转（90° / 任意角度）、缩放（10%~300%），导出 PNG 保存到文档同目录 `images/` 并以相对路径插入
- CodeMirror 已离线打包（`assets/vendor/codemirror.bundle.js`），仅首次进入编辑模式时加载，不影响阅读秒开

### AI 助手

- **提供商**：OpenAI / DeepSeek / Kimi / 智谱 GLM / 通义千问 / 硅基流动 / OpenRouter / Groq / xAI / Mistral / Gemini / 火山方舟 / 腾讯混元 / Ollama（本地）/ Anthropic 等公开预设，并支持完全自定义连接
- **API Key**：面板中填写即保存到本机（`%APPDATA%\ReadMD\ai.json`）；留空时自动读取环境变量（如 `DEEPSEEK_API_KEY`），无需重复填写
- **自定义**：可直接修改任意预设的地址 / 模型 / Key；兼容 OpenAI Chat Completions 与 Anthropic Messages 双协议，绝大多数聚合网关 / NewAPI / One API 可直接使用
- **动作**：快速阅读、润色、修改、扩充、续写、翻译、提问；默认处理全文，可勾选「仅处理选中文字」
- **模板**：内置 14 个常用模板（总结要点 / 生成周报 / 生成大纲 / 代码审查 / 修正格式等），支持新建 / 编辑 / 删除与 `{doc}` `{prompt}` 占位符
- **历史会话**：多轮上下文自动累积，可保存 / 恢复（最多 50 个会话 / 60 条消息）
- **落地**：流式渲染结果可「应用到文档」（进入编辑审阅后 Ctrl+S 保存）、「复制」或「另存为」

### 移动端共享

- 点工具栏「📱」开启共享，弹出二维码；手机连同一 Wi-Fi 扫码即可阅读当前文档，并使用转 MD / OCR / 网页 / AI 全部功能
- 每次开启生成随机访问令牌；局域网 API 请求（除页面与静态资源外）均需携带令牌，关闭共享即失效
- 命令行方式：`python readmd.py --share` 启动时即开启共享


### 导出 PDF / DOCX / HTML（v2.1.0）

- **入口**：工具栏「导出」按钮或 `Ctrl+P`，打开导出面板；面板内也可直接「打印当前文档」
- **三种格式**：PDF（reportlab，中文微软雅黑 + 表格 / 代码块 / 封面 / 目录 / 页码）、DOCX（python-docx，标题用 Word 内置 Heading 样式，导航窗格可直接定位）、HTML（单文件自包含，内联 marked + MathJax，任何浏览器离线打开）
- **样式定制**：内置「简约 / 经典 / 商务」预设，也可全量可视化调整——纸张大小 / 方向 / 页边距、封面与目录、正文与各级标题（颜色 / 字号 / 加粗 / 对齐）、表格（表头颜色 / 边框 / 斑马纹 / 单元格字号）、代码块、引用、链接、页脚页码、PDF 元数据、HTML 亮 / 暗 / 米色主题；「存为预设」后可在下拉中复用
- **公式与图片**：`$...$` / `$$...$$` 公式在 PDF / DOCX 中由本地 matplotlib 渲染为图片（离线、无需 LaTeX）；HTML 导出保留 MathJax 完整渲染；本地图片按文档目录解析并嵌入（缺失自动跳过并提示）
- **说明**：导出使用当前文档内容（文件模式含未保存的编辑；转换 / OCR / 网页结果同样可导出）；样式参数自动记忆
## 📦 目录结构

```
readmd/
├─ readmd.py            # 主程序（本地服务 + 窗口 + 单实例托盘 + 里程碑打点）
├─ src/readmd_fix.py        # 自动修正器（纯标准库）
├─ tests/test_fix_test.py   # 修正器测试（37 项，python tests/test_fix_test.py）
├─ src/readmd_modules/      # 懒加载扩展模块
│  ├─ convert.py        #   万物转 MD（MarkItDown）
│  ├─ ocr.py            #   扫描转 MD（WinRT OCR + PyMuPDF）
│  ├─ web.py            #   网页转 MD（安全下载 + Trafilatura / Readability / 图片本地化）
│  └─ ai.py             #   AI 助手（双协议 + 提供商注册表）
├─ DESIGN.md            # 设计规范（色盘 / 字体 / 间距 / 圆角 token）
├─ installer/           # 安装器（动画 UI + onedir 目录安装）
│  ├─ setup_app.py      #   安装 / 卸载 / 静默模式主程序
│  ├─ setup.html        #   动画界面（毛玻璃 / 弹簧动效 / 极光背景）
│  └─ build_setup.bat   #   构建 ReadMDSetup.exe + ReadMDUninstall.exe
├─ assets/
│  ├─ index.html        # 界面骨架
│  ├─ style.css         # 阅读主题 + 移动端响应式
│  ├─ app.js            # 渲染 / 目录 / 搜索 / 公式 / AI / 转换 / 编辑
│  ├─ readmd.ico        # 多尺寸应用图标（16~256）
│  └─ vendor/           # marked + MathJax + qrcode + codemirror.bundle（全部离线）
├─ package.bat          # 一键打包（onedir 安装版 + 便携单文件）
├─ setup.bat            # 一键：打包 + 注册默认打开 + 启动
├─ install.bat          # 一键安装依赖 + 注册文件关联
├─ run.bat              # 一键运行（venv pythonw）
├─ uninstall.bat        # 移除文件关联（保留备份）
├─ deploy.bat           # ★一键部署：测试 → 推送 main/tag → 等待 CI 发布
├─ release.py           # 既有 Release 校验/文案维护（不会创建或上传）
└─ release_notes.md     # GitHub Actions 使用的 Release 发布说明
```

## 🔨 打包 / 一键安装

| 脚本 | 作用 |
| --- | --- |
| `run.bat` | 一键运行（venv pythonw，秒开） |
| `install.bat` | 安装依赖 + 生成图标 + 注册 `.md` 关联 |
| `package.bat` | 一键打包：onedir 安装版 `dist\ReadMD\ReadMD.exe` + 便携单文件 `dist\ReadMD-portable.exe` |
| `setup.bat` | 一键完成：装依赖 → 打包 onedir exe → 注册默认打开方式 → 启动 |
| `uninstall.bat` | 移除关联并尝试恢复安装前备份 |
| `installer\build_setup.bat` | 构建 `dist\ReadMDSetup.exe`（内嵌 onedir 目录）与 `dist\ReadMDUninstall.exe` |
| `release.py` | 校验既有 Release 的五个资产或更新其文案；不会创建 Release 或上传资产 |

> 安装版为目录安装（约 200MB，含 OCR 与转换依赖，冷启动无需解压、秒开）；便携版为单文件（首次启动需解压、稍慢）。日常开发推荐源码版（`install.bat` + `run.bat`）。

## 🌍 一键部署与发布

环境要求：已安装 Git 与已登录的 [GitHub CLI](https://cli.github.com/)。`deploy.bat` 只推送 `main` 和不可移动的版本标签，并等待 CI；CI 是唯一的 Release 发布方。

```bat
deploy.bat                 rem 完整流程：测试 → 推送 main/tag → 等待 CI 发布
deploy.bat --skip-tests    rem 跳过自测
deploy.bat --tag v2.2.4    rem 指定发布标签（默认 v2.2.4）
```

也可以单独使用 `release.py`：

```bat
python release.py --verify             rem 校验既有 Release 的四资产 + SHA256SUMS.txt
python release.py --update             rem 更新已存在 Release 的标题与说明（读 release_notes.md）
```

## 🗺️ 路线图与生态规划 (Roadmap)

- 🧩 **v2.3.0 规划（开发者生态与 AI 协议）**：
  - 🔌 **VSCode 深度集成插件**：推出官方 ReadMD VSCode 扩展，无缝集成双向编辑、实时高保真预览、公式自动修复与一键导出。
  - 🤖 **MCP (Model Context Protocol) Server**：接入标准 MCP 协议，支持 Claude Code / Cursor / Windsurf / Codex 等 AI 助手直接读取、转换、搜索本地 Markdown 与知识库。
- 🐧 **v2.3.1 规划（全平台与国产化生态）**：
  - 💻 **Linux 原生发行版支持**：提供针对 Ubuntu / Debian (`.deb`)、Fedora / RHEL (`.rpm`) 及通用 `AppImage` 的独立发布包。
  - 🇨🇳 **国产操作系统深度适配**：全面适配银河麒麟 (KylinOS)、统信 UOS 等国产操作系统及龙芯 (LoongArch) / 飞腾 (ARM64) 硬件平台。

## 📝 更新日志

- **v2.2.9**：**全能剪贴板智能建档 + 未保存关闭安全弹窗 + 状态深度清理 + 网页抓取现代重构 + LaTeX 双向互转**
  - 📋 **全能智能剪贴板自适应分流**：不管剪贴板是什么内容，一键自适应识别建档——富文本自动经 Turndown 转为 Markdown，图片/截图自动调起本地 OCR 引擎提取排版文字，单个 URL 自动填入网页转 MD 弹窗，纯文本/公式秒级新建标签页。
  - 💾 **未保存修改自定义确认模态弹窗**：关闭未保存标签页时，弹出优雅现代化对话框，提供「保存 / 不保存 / 取消」三态操作；批量关闭与关闭其他标签时安全异步遍历，点击取消即刻中断，彻底防止数据误失。
  - 🧹 **标签关闭与返回主页全局状态深度清理**：关闭全部标签或返回主页时，彻底清空大纲与目录缓存、搜索高亮、侧边栏激活状态与文档标题，杜绝任何视觉与数据残留。
  - 🌐 **网页抓取现代极简界面重构**：去除暗色模式下白底白字的模式下拉框，改用双核心动作卡片（【⚡ 智能提取正文】为主操作 / 【🖥️ 完整动态渲染】为辅助操作）；合并同站抓取为数字步进器（默认1页，支持加减按钮、滚轮增减、1~30页限制），局域网授权协议边缘化放置，支持剪贴板一键粘贴网址。
  - 🧮 **轻量级 LaTeX ⇄ Markdown 双向互转**：内置纯 Python 互转引擎（零 TeXLive/Pandoc 外部依赖），导出面板新增 LaTeX (.tex) 格式支持（生成包含宏包与 booktabs 表格的标准学术论文源码）；万物转 MD 与全局拖拽原生支持 `.tex` / `.latex` 文件一键转换为 Markdown。
  - 🪟 **Win7 兼容版同步升级**：打包构建链同步升级至 v2.2.9。

- **v2.2.8**：**软件内自动更新 + LaTeX 全量自修复 + OCR 智能排版规范化 + 导出实时高保真渲染**

  - 🚀 **软件内自动检查与本地更新系统**：启动静默检查 GitHub Releases，状态栏小圆点与更多菜单提示，弹窗完整渲染 Markdown 更新日志；智能匹配 Windows 安装版/便携版/macOS 包并校验 SHA256；支持一键热更重启与国内加速镜像
  - 🧮 **LaTeX 全量兼容与公式自修复算法**：支持 `\begin{cases}`, `\begin{align}`, `\begin{matrix}`, `\begin{equation}` 等多行 TeX 原生环境；智能配平花括号 `{}`、修复转义反斜杠、HTML 实体还原、Unicode 符号自动转 LaTeX，渲染失败优雅降级源码卡片
  - 📥 **万物转 MD 拖拽与批量转换自动开标签**：拖入 Word、PDF、PPT、Excel、EPUB、TXT 或在弹窗中批量转换后，自动将转换生成的 Markdown 文档打开至新标签页
  - 📑 **导出排版动态高保真预览**：根据预设（简约/经典/商务）与个性化配置实时生成样式表，精准映射正文字体、段距、标题阶梯、表格边框斑马纹、代码块与引用，彻底修复暗色模式及主题切换下的白底白字/黑底黑字问题
  - 🖼️ **OCR 智能排版与格式规范化**：清除 CJK 汉字间虚假空格、修复跨行断字连字符、智能聚合句内硬断行、自动提升章节标题与有序/无序列表
  - 🔍 **空文档状态搜索保护**：主页与无文档状态下，顶栏搜索按钮设为 disabled 并拦截 `Ctrl+F` 快捷键，关闭所有标签自动清理搜索栏
  - 🤖 **欢迎页 AI 助手按钮修复**：解决首屏欢迎页点击 AI 助手按钮因双重绑定被立即关闭的问题，单次点击稳定唤出
  - 🪟 **安装程序窗口化界面去重**：清除 HTML 内部冗余关闭与最小化按钮，统一使用操作系统原生窗口外框控制
  - 🪟 **Win7 兼容版同步升级**：打包构建链同步升级至 v2.2.8


- **v2.2.7**：**多标签页窗口模式 + 全格式拖拽 + 导出高保真预览**

  - 🗂️ **多标签页系统**：支持打开多个页面，标签拖拽排序、双击重命名（联动磁盘文件/虚拟文档）、溢出自动折叠（Hover 查看/Click 锁定）、右键菜单，全部关闭后自动返回主页
  - 📥 **全格式拖拽支持与悬浮动效**：全局拦截拖拽并智能分流（Markdown 标签页打开 / Office & PDF 批量转换 / URL 网页抓取 / 纯文本建档），配备毛玻璃光晕动效
  - 📋 **剪贴板极速建档**：任意界面 `Ctrl+V` 生成虚拟 MD 文档，`Ctrl+S` 可指定位置保存
  - 📑 **导出文档实时微缩预览**：配置参数默认折叠收拢，左侧下方嵌入高保真排版小窗并支持点击放大预览
  - 🔙 **返回主页移至底部栏**：移入底部状态栏右侧模块区旁，视觉更统一
  - 🛠️ **修复侧边栏与预览按键**：修复展开文件树时点击目录按钮直接收回侧边栏；修复编辑界面顶部预览方向按键位置
  - 🌐 **网页抓取本地网络放行**：默认允许局域网与本地网络页面解析
  - 🪟 **Win7 兼容版同步升级**：升级 Win7 打包构建链至 v2.2.7

- **v2.2.6**：**UX 全面优化 + 项目结构重构**

  - ✨ **智能按钮状态管理**：未打开文件时，导出 / 字号调整按钮自动禁用，避免误操作
  - 🏠 **主页六模块布局**：固定展示「打开 Markdown」「打开文件夹」「AI 助手」「万物转 MD」「网页转 MD」「扫描转 MD」六大核心功能，界面更清晰
  - 📂 **VSCode 风格目录树**：打开文件夹后以树形结构展示，支持折叠/展开，自动过滤上级目录名称，浏览体验大幅提升
  - 🔗 **目录锚点跳转**：点击目录项平滑滚动到对应章节，长文档导航更高效
  - 🔙 **一键返回主页**：右下角「返回主页」按钮，随时回到欢迎页
  - 📍 **目录按钮最左侧**：工具栏目录按钮移至最左侧，符合阅读习惯
  - 📋 **剪贴板导入优化**：从主页移除「从剪贴板新建」，改为右上角下拉菜单中的快捷入口
  - 🗂️ **根目录重构整理**：源代码归档到 `src/`，测试归档到 `tests/`，脚本分类到 `scripts/windows/` 和 `scripts/unix/`，配置文件移至 `config/`，发布文件移至 `release/`，文档移至 `docs/`。根目录仅保留 `readmd.py`、`README.md` 和 `.gitignore`，大幅提升项目结构清晰度
  - 🛠️ **路径引用全面更新**：所有 Python 导入、脚本路径、文档引用均已同步更新，确保功能完全正常

- **v2.2.4**：可选模块按功能按需加载；新增不包含文档数据的启动探针；AI 面板支持预览后导入一次性授权剪贴板、用户选择的导出文件和公开网页对话，并对大小、压缩包与危险链接设限。

- **v2.1.0**：新增「导出」——PDF / DOCX / HTML 一键导出；导出面板（打印按钮升级）：内置「简约 / 经典 / 商务」样式预设 + 全量可视化定制（页面 / 标题 / 表格 / 代码块 / 引用 / 页码等），可保存自定义预设；公式在 PDF / DOCX 中本地渲染为图片，HTML 为单文件离线可开；图片按文档目录嵌入
- **v2.0.1（安装器修复）**：移除安装包 / 卸载器的 PyInstaller 启动画面，修复低配机黑屏置顶弹窗卡死安装流程的问题；安装版本号统一为 2.0.1
- **v2.0.0**：秒开（onedir 目录安装 + 单实例托盘常驻）；界面全面改版（44px 工具条 / SVG 图标 / 欢迎页最近文件网格 / 三主题设计 token）；大文档骨架屏与无障碍优化
- **v1.4.0**：插入图片（裁剪 / 缩放 / 旋转）；苹果风动画安装器

## 🗑️ 卸载

- **安装版**：「设置 → 应用」中找到 ReadMD 卸载；或运行安装目录中的 `ReadMDUninstall.exe`
- **源码版**：双击 `uninstall.bat` 移除文件关联（并尝试恢复安装前的 `.md` 关联备份），`.venv` 与 `readmd` 文件夹保留，可手动删除

## ❓ 常见问题

- **提示未安装 pywebview**：运行 `install.bat`，或在 PowerShell 执行 `python -m pip install pywebview`
- **打开时报 WebView2 相关错误**：系统缺少 Edge WebView2 运行时，下载安装 https://developer.microsoft.com/microsoft-edge/webview2/ 后重试
- **启动闪一下控制台**：双击 `.md` 走的是 `pythonw.exe`（无控制台）；手动用 `python readmd.py` 运行出现控制台属正常现象
- **为什么打开这么快**：安装版是 onedir 目录安装，启动无需解压；窗口创建约 0.1s，页面由常驻实例秒级唤起
- **安全性说明**：Markdown 中的原始 HTML（如 `<script>`）会按原样渲染，与大多数阅读器一致，仅建议打开可信文件

---

<div align="center">

**ReadMD** · 纯本地优先，你的文档不出本机。

</div>
