# ReadMD V0.0.2 - 纯 Rust 原生全平台架构版 (Pure Rust Edition) 🦀

> **100% 纯 Rust 原生全平台 Markdown 阅读器与编辑器**：长文档语义分页、显示层非破坏纠错、Office/PDF/网页转 MD、离线 OCR、LaTeX 学术增强与 MCP 接入。
> 官网地址：[https://rust.readmd.asia](https://rust.readmd.asia) ｜ 爱发电支持：[https://ifdian.net/a/natsummerance](https://ifdian.net/a/natsummerance)

V0.0.2 把编辑、渲染与文件转换补齐到可用，移除了仓库里最后的 Python 依赖（含 MCP 服务器、VS Code 扩展、构建与发布脚本），并全面整修了界面交互。

## 🆕 V0.0.2 更新内容

### 编辑、渲染与导出
- **保存保留原编码**：GBK / Shift-JIS / UTF-16 等文件打开后原样保存；无法表示的字符会提示，而不是静默丢失。另存为改为原子写入。
- **`.txt` 结构化**：纯文本按段落与标题渲染，编辑后仍写回纯文本。
- **DOCX 导出**：基于共享 Markdown AST，保留标题样式、嵌套列表、表格对齐、代码块与本地图片。
- **PDF 导出**：嵌入字体子集（中日韩字符不再缺字），公式按矢量排版，代码块语法着色，表格对齐与 `lang` 保留。
- **LaTeX 导出**：修复中文后接加粗时的崩溃，自动配置 CJK 支持，本地图片复制到 `<文件名>.assets/`。
- **EPUB 与演示文稿**：EPUB 打包图片并使用保存对话框；演示文稿可导出为离线可用的单个 HTML 文件。
- **导出预设与警告列表**：常用导出配置可保存为预设；导出后的提示合并展示、可展开查看。
- **编辑器工具栏**：加粗、列表、引用等变换支持一次撤销，主题切换即时生效。

### 文件转换
- 新增 **HTML、XLS（BIFF）、PPT（旧版二进制）、MOBI** 转 Markdown；旧版 Office 文件解析失败时给出明确原因。
- 转换输出冲突可选 **跳过 / 覆盖 / 自动重命名**；支持打开输出目录，ZIP 失败会提示原因。
- **Windows 原生 OCR**（WinRT，无需额外安装）；其他平台检测系统 OCR 引擎，缺失时给出安装指引。
- 插件中心改为展示内核**原生内置能力**，不再出现 pip 安装入口。

### 不再依赖 Python
- **`readmd --mcp`**：MCP 服务器由 Rust 内核直接提供（stdio，19 项工具，支持 2025-06-18 / 2025-03-26 / 2024-11-05 协议）。有副作用的工具需要显式 `confirm`，导出不会覆盖已有文件，除非传入 `overwrite`。配置示例：`{"command": "readmd", "args": ["--mcp"]}`。
- **VS Code 扩展**直接调用 ReadMD 可执行文件（可在设置 `readmd.executablePath` 中指定），不再需要 Python 环境。
- 构建、版本同步、发布资产、桌宠打包全部改为 `cargo xtask`；仓库校验改为零依赖 Node 脚本；删除 90 个遗留 `.py` 文件，并新增 CI 检查防止 Python 调用回流。
- 新增 `desktop` 特性开关：`--no-default-features` 可构建无窗口依赖的服务器版本。

### 界面与交互
- **统一的弹窗焦点管理**：28 个弹窗打开时焦点留在弹窗内、Tab 循环、Esc 逐层关闭并把焦点还给触发按钮；输入法组字时按 Esc 不会误关弹窗。
- **长任务反馈**：导出、转换、OCR、批量转换显示进行中状态和耗时，防止重复提交；导出可**取消**（取消后不会留下半成品文件），超过 2 分钟无进展会提示。
- **设计令牌**：颜色、间距、字号、圆角与动效统一到 `tokens.css`，三套主题通过 WCAG AA 对比度检查；键盘焦点环清晰可见，支持高对比度模式与减少动态效果。
- **窄屏适配**：360px 宽度下工具栏、状态栏与各弹窗不再出现横向滚动。
- 修复多处按钮缺少无障碍名称、控件未接线的问题，并加入 CI 检查。

### 稳定性
- HTTP 处理器与批量转换的每一项都有 panic 兜底，单个文件出错不会拖垮整个进程。
- 打开 / 在文件夹中显示不再"假成功"，失败时给出本地化原因；全部错误码在 46 种语言中均有翻译。

---

## V0.0.1 基础能力回顾

V0.0.1 完成了端到端纯 Rust 原生架构重构（`readmd-kernel`），实现零 Python 运行时依赖、毫秒级冷启动、极致内存优化以及全平台原生集成。

---

## 🌟 核心特性与功能亮点 (Features & Highlights)

### 1. 🦀 100% 纯 Rust 原生内核（零 Python 运行时依赖）
- **单二进制便携原生可用**：彻底脱离历史 Python/PyInstaller 运行时，摆脱虚拟环境与动态库束缚，双击即开，原生毫秒级响应。
- **内存优化超 80%**：待机内存降至极致低位（30MB~50MB），彻底解决传统桌面框架高资源占用的痛点。
- **强类型与内存安全**：全量 1,780+ 项 Rust 单元与集成测试 100% 通过，坚如磐石。

### 2. 🖥️ 全平台原生桌面适配 (Windows / macOS / Linux / 国产系统)
- **Windows (10/11)**：基于 Microsoft Edge WebView2 与 Win32/COM 原生集成，绿色免安装单文件即可流畅运行。
- **macOS (Apple Silicon & Intel)**：基于 WKWebView 与 Cocoa 原生打包，提供原生 `.app` 应用程序捆绑包。
- **Linux & 国产操作系统**：原生集成 WebKitGTK 与 X11/Wayland 桌面环境，原生支持 Ubuntu、Debian、统信 UOS 与银河麒麟 Kylin。

### 3. 📊 8 大科学与学术图表原生渲染引擎
- **Mermaid**：流程图、时序图、甘特图、类图、状态图、Git 提交图。
- **Graphviz (DOT)**：复杂有向图、状态拓扑图与层次图。
- **Vega & Vega-Lite**：声明式统计与数据分析可视化图表。
- **KaTeX**：高保真 LaTeX 数学公式极速排版与渲染。
- **PlantUML**：系统架构设计与专业 UML 图。
- **Markmap**：一键生成交互式思维导图。
- **ABCjs**：五线谱乐谱排版与实时交互发音。
- **科学图标与矢量图**：原生 SVG 与高分辨率科学图解支持。

### 4. 🔄 通用文档跨格式转换引擎 (Universal Converter)
- **支持格式**：PDF、Word (.docx)、PowerPoint (.pptx)、Excel (.xlsx)、HTML、EPUB、Jupyter Notebook (.ipynb)、TXT 与 Markdown (.md) 互转。
- **版面保真还原**：精准提取标题、列表、复杂表格、公式、代码高亮与配图。
- **独立资源归档**：提取的文档配图自动组织至 `<文件名>.assets/` 干净目录中。

### 5. 🐱 原生交互 Live2D 桌面伴读宠物 (Desktop Companion)
- **透明窗口与点击穿透**：无任何边框遮挡，流畅浮动于桌面。
- **丰富角色阵容**：Arch-Chan (Live2D)、Hermes、Mochi、Moss、Amber、Niu-Lai、Cache-Capy 等。
- **多状态伴读**：打字伴奏、敲鼓、抚摸交互、挂机微呼吸动画。

---

## 📦 全平台发布资产下载 (Release Assets & Installers)

| 资产文件名 | 操作系统 / 架构 | 类型 | 格式说明 |
| :--- | :--- | :--- | :--- |
| **`ReadMDSetup-windows-x64.exe`** | Windows 10/11 (x64) | 🎯 **安装包** | 官方安装向导，自动配置开始菜单、桌面图标与 .md 文件关联 |
| **`ReadMD-windows-x64.zip`** | Windows 10/11 (x64) | 便携版 | 免安装便携 ZIP，解压即用 |
| **`ReadMD-macos-arm64.dmg`** | macOS 11+ (Apple Silicon) | 🎯 **安装包** | 原生 macOS 磁盘映象（DMG 拖拽安装） |
| **`ReadMD-macos-arm64.zip`** | macOS 11+ (Apple Silicon) | 便携版 | 原生 `ReadMD.app` 压缩归档 |
| **`ReadMD-macos-x64.dmg`** | macOS 10.15+ (Intel x64) | 🎯 **安装包** | 原生 macOS 磁盘映象（DMG 拖拽安装） |
| **`ReadMD-macos-x64.zip`** | macOS 10.15+ (Intel x64) | 便携版 | 原生 `ReadMD.app` 压缩归档 |
| **`ReadMD-linux-x86_64.deb`** | Linux (Ubuntu/Debian/UOS/麒麟) | 🎯 **安装包** | 原生 Debian/Ubuntu/统信UOS/银河麒麟 安装包（带桌面图标与菜单） |
| **`ReadMD-linux-x86_64.tar.gz`** | Linux x64 全发行版 | 便携版 | 原生 WebKitGTK Linux 二进制与静态资源便携包 |
| **`SHA256SUMS.txt`** | 全平台通用 | 校验单 | 全量发行包 SHA-256 完整性校验哈希值清单 |

> **完整性校验**：所有安装包与便携包的 SHA-256 校验哈希清单请直接参阅随附发行的 `SHA256SUMS.txt`。

---

- 官方网站：[https://rust.readmd.asia](https://rust.readmd.asia)
- 源码仓库：[https://github.com/Natsummerance/rust-ReadMD](https://github.com/Natsummerance/rust-ReadMD)
- 赞助支持（爱发电）：[https://ifdian.net/a/natsummerance](https://ifdian.net/a/natsummerance)
