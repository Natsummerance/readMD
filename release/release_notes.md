# ReadMD v2.3.5

ReadMD 是免费的本地 Markdown 智能阅读与排版套件；纯本地、秒级极速渲染、离线可用，绝不改写原文件。

## 📦 全平台发布资产 (Release Assets)

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.5.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.5.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.5.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.5.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.5.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包 (UOS / 银河麒麟 / Deepin / Ubuntu / Debian)：`readmd_2.3.5_amd64.deb`
- 📱 HarmonyOS NEXT 纯血鸿蒙安装包：`ReadMD-harmonyos-v2.3.5.hap`
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.5.vsix`
- 🤖 FastMCP Server 独立包：`readmd-mcp-server-2.3.5.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

---

## 🌟 本次版本核心更新 (v2.3.5)

### 1. 渲染引擎与自愈算法底层加固
- **Marked AST 内联委托修复**：修复 `parseMarkdownWithSourceMap` 中标题和段落加粗 (`**...**`)、斜体和链接的行内解析委托；
- **表格渲染容错**：修复多列复杂表格 `[object Object]` 异常，严格保留标准 `<thead>` 与 `<tbody>` HTML 结构；
- **CJK 标点与括号智能边界适配**：扩展分类器支持全角括号 `（）`、书名号 `《》`、方括号 `【】` 及中文引号两端的加粗标记智能识别与平衡，杜绝粘连丢失；
- **全球 46 种语言国际化字典 100% 补全**：所有新增功能词条全部补全，实现 0 缺失键。

### 2. 竞品级功能可视化 UI 交互入口全面落地
- **交互式代码块配置弹窗 (Interactive Code Chunk Modal)**：提供语言快速选择 (Python / JS / Bash / R / PHP / Go / Ruby)、Matplotlib 绘图开关与代码折叠选项，一键插入；
- **科学与工程图表选择器 (Diagrams Modal)**：内置 PlantUML、TikZ / PGFPlots、WaveDrom、Vega-Lite、Graphviz DOT、D2、BitField 标准预设模版，支持即时选择插入；
- **子文档引用插入助手 (@import Modal)**：图形化输入相对路径，轻松实现模块化拼装；
- **文档样式与 Frontmatter 元数据模板**：一键注入标题、Reveal.js 演说主题与全局 CSS 样式；
- **编辑器 `/` 命令面板与插入菜单全覆盖**：所有高级组件均已深度整合至编辑器插入菜单与 `/` 快捷指令中。

### 3. 阅读器与演说体验深度提升
- **批量运行所有代码块 (Run All Code Chunks)**：在更多菜单中提供一键执行文档中全部代码块的能力，顺次捕获控制台输出与生成图表；
- **F5 一键全屏演说放映**：支持 `F5` 快捷键极速启动 Reveal.js 全屏放映演说模式；
- **禅模式 (F11)** 与 **自定义样式 (CSS/Head)** 快捷入口体验升级。

### 4. VSCode 扩展与 MCP 智能体生态全功能对齐
- **VSCode 新增命令与 Snippets**：注册 `readmd.insertCodeChunk`、`readmd.insertDiagram`、`readmd.insertDocImport`、`readmd.insertFrontmatter` 交互命令，并提供完整 Markdown 代码片段与侧边栏工具箱直达入口；
- **MCP Server 全面升级**：无缝支持代码块就地沙箱执行、科学图表渲染与子文档展平，与 Claude Desktop / Cursor / Antigravity 深度协同。

### 1. `@import` 工程化模块化编译体系
- **子 Markdown 嵌套**：支持 `@import "chapter.md"` 递归嵌入与展平，内置 8 层嵌套深度限制与环形循环引用拦截；
- **表格数据导入**：支持 `@import "data.csv"` / `@import "data.tsv"` 原生编译为高可读 Markdown 表格；
- **源码行号精准切片**：支持 `@import "app.py" {line_begin=10 line_end=40 highlight=[15, 20]}` 局部代码片段提取；
- **PDF 指定页码抽取**：支持 `@import "spec.pdf" {page_no=3}` 自动抽取目标页面并高清渲染为矢量图像；
- **LESS 样式与 TikZ 嵌入**：支持 `@import "theme.less"` 动态作用域渲染与 `.tikz` 物理/几何矢量图形。

### 2. 原生 `[TOC]` 嵌入式智能目录引擎
- 在文档任意位置输入 `[TOC]`，自动遍历提取全文档标题 AST 语法树；
- 支持深度区间过滤（如 `<!-- @import "[TOC]" {depth_from=2 depth_to=4} -->`），自动生成符合 GitHub 规范的短锚点 Slug；
- 保存或导出时支持原地自愈替换，保持目录与正文 100% 同步。

### 3. 全景科学与专业图表渲染矩阵
- **时序与硬件波形**：原生支持 ````wavedrom```` 与 ````bitfield````；
- **Graphviz 拓扑结构**：集成 ````dot```` / ````viz```` 原生 Graphviz 语法；
- **PlantUML 双通道渲染**：支持内网/本地轻量离线与远端双通道解析；
- **统计图表**：原生支持 ````vega```` / ````vega-lite```` 数据可视化；
- **TikZ 科学几何绘图**：基于纯前端 WebAssembly TikZjax 引擎，原生生成平滑矢量 `<svg>`，无需安装庞大的 TeX 环境。

### 4. Reveal.js 专业演说与幻灯片模式
- 使用 `<!-- slide -->` 与 `<!-- subslide -->` 智能切分水平/垂直幻灯片；
- 支持 `<!-- note: ... -->` 演讲者私密备注视窗与双屏计时同步；
- 支持内置 9 款经典演讲主题与转场动画，支持一键导出为零外部依赖的独立离线 HTML 演示文件。

### 5. 多语言安全 Code Chunk 执行器
- 支持光标处/选中代码就地安全沙箱执行；
- 支持 **Python**（内置 Matplotlib 图表自动拦截并转为 Base64 预览）、**JavaScript / Node.js**、**Shell (Bash/PowerShell/Cmd)**、**R 语言**与 **Rust**；
- 具备严格的超时守护（Timeout Killer）与进程树清理，杜绝孤儿后台进程。

### 6. 原生 EPUB 3.0 电子书打包导出引擎
- 纯 Python 标准库原生构建符合 IDPF EPUB 3.2 规范的 OCF ZIP 容器（零外部命令行依赖）；
- 自动生成符合规范的 `mimetype`（无压缩首位存储）、`container.xml`、`content.opf`、`nav.xhtml`、`toc.ncx` 与精美电子书排版 CSS；
- 支持在 Apple Books、微信读书、Kindle 等各平台电子书阅读器中完美阅读。

### 7. AST 像素级双向滚动同步锚定
- 针对块级元素动态注入 `data-source-line` 行号标签；
- 配合前端基于双锚点的线性插值算法（Linear Interpolation），彻底消除公式折叠与多行复杂表格引起的滚动偏移与抖动。

### 8. VSCode 插件与 MCP Server 全面进化
- VSCode 扩展升级至 v2.3.4，新增侧边栏快速工具箱与所有新增特性命令；
- MCP Server 扩展至 15 项标准工具，支持 Claude Desktop / Cursor / Antigravity 智能体全链路调度。

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
Get-FileHash .\ReadMDSetup-v2.3.4.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.4.zip
```
