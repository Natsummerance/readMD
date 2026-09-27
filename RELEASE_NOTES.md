# ReadMD V0.0.1 - 纯 Rust 原生全平台架构版 (Pure Rust Edition) 🦀🚀

> **100% 纯 Rust 原生全平台 Markdown 阅读器与编辑器**：长文档语义分页、显示层非破坏纠错、Office/PDF/网页转 MD、离线 OCR、LaTeX 学术增强与 MCP 接入。
> 官网地址：[https://rust.readmd.asia](https://rust.readmd.asia) ｜ 爱发电支持：[https://ifdian.net/a/natsummerance](https://ifdian.net/a/natsummerance)

欢迎使用 **ReadMD 纯 Rust 原生版首个正式 Release (V0.0.1)**！本版本完成了端到端纯 Rust 原生架构重构（`readmd-kernel`），实现零 Python 运行时依赖、毫秒级冷启动、极致内存优化以及全平台原生集成。

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
