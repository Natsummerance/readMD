# Markdown Preview Enhanced (MPE) 深度竞品分析与功能全景细则报告

> **调研对象**：[Markdown Preview Enhanced (MPE)](https://shd101wyy.github.io/markdown-preview-enhanced/#/zh-cn/)  
> **核心引擎**：[Crossnote (TypeScript)](https://github.com/shd101wyy/crossnote)  
> **作者/维护者**：Yiyi Wang (shd101wyy)  
> **平台覆盖**：VS Code 扩展 (350万+ 安装)、Atom 扩展 (历史维护)、Crossnote CLI / Note-taking app  
> **调研基准版本**：MPE v0.8.x ~ Crossnote v0.9.x  
> **对比基准**：ReadMD (v2.3.4)

---

## 1. 竞品画像与核心设计哲学

### 1.1 产品定位与目标受众
- **定位**：VS Code 生态中功能最全面的**增强型 Markdown 预览与多格式编译系统**。
- **哲学**：“飘逸的 Markdown 写作体验”——将纯文本 Markdown 扩展为**“可执行、可嵌入、可绘制复杂图表、可多管道导出”的综合文档工作台**。
- **目标受众**：重度技术文档作者、科研工作者、数据分析师、程序员、大学师生与书籍编撰者。

### 1.2 核心技术架构 (Crossnote 体系)
- **底层渲染内核**：基于 `Crossnote` 核心库，采用 `markdown-it` / `remarkable` 解析器，构建在 VS Code Webview 沙盒内。
- **模块化管道**：
  - 数学排版：KaTeX（高性能轻量）+ MathJax v3（高兼容性学术符号）；
  - 图形渲染：内置纯前端 JS 库（Mermaid, Viz.js, WaveDrom, Vega）与外部 CLI 守护（PlantUML/Java, D2, Graphviz）；
  - 代码执行引擎：Code Chunk 子进程调度器（调用系统 Python, R, Bash, Node.js 等）；
  - 多格式导出后端：Puppeteer (Chrome Headless), Prince XML, Pandoc (Haskell), Calibre / eBook-convert。

---

## 2. MPE 全功能清单与具体实现细则

### 2.1 基础与实时预览交互 (Core Preview & Interaction)
| 功能模块 | 具体特性细则 | 实现机制与配置 |
| :--- | :--- | :--- |
| **双向滑动同步** | 编辑器光标与预览视图毫秒级平滑定位；支持单向/双向开关联动。 | 基于 Source Map 与元素行号绑定，支持 `ctrl-shift-s` / `cmd-shift-s` 立即对齐。 |
| **实时更新控制** | 支持实时热重载（Live Update）与仅保存时更新（Save-only Update）切换。 | 针对 5000+ 行超大文档减少 Webview 重绘开销。 |
| **Zen Mode 无干扰写作** | 一键隐藏 VS Code 侧边栏与状态栏，全屏分屏沉浸式写作。 | 命令 `Markdown Preview Enhanced: Toggle Zen Mode`。 |
| **回车换行策略** | 兼容 CommonMark 严格换行（末尾双空格）与 GFM 宽松换行（单次回车即换行）。 | 设置项 `breakOnSingleNewline: true/false`。 |
| **自定义 CSS/JS Head** | 允许用户修改全局 `style.less` 与 `head.html`，注入自定义字体、JS 脚本与图标包。 | 命令 `Markdown Preview Enhanced: Customize Css` 与 `Customize Preview Html Head`。 |

---

### 2.2 数学排版引擎 (LaTeX Math Support)
| 特性 | 具体细则 | 语法范例与行为 |
| :--- | :--- | :--- |
| **双引擎切换** | 支持在 **KaTeX** 与 **MathJax (v2/v3)** 之间一键热切换。 | 在扩展设置 `mathRenderingOption` 中配置。 |
| **行内公式定界符** | 支持 `$...$` 与 `\(...\)`。 | 自动忽略货币符号 `$100` 等常见误判。 |
| **块级公式定界符** | 支持 `$$...$$`、`\[...\]` 以及 <code>```math</code> 专属代码块。 | 居中显示，支持溢出自动横向滚动。 |
| **宏定义扩展** | 支持在文档头部或 `MathJax Config` 中预置 `\newcommand` 与 `\def` 学术简写宏。 | 通过 `Markdown Preview Enhanced: Open Mathjax Config` 配置。 |

---

### 2.3 综合图表与可视化矩阵 (Diagrams & Data Visualization)
MPE 原生支持近 10 种工业级与学术级图表标记语法：

```mermaid
graph LR
    subgraph Frontend_PureJS[纯前端无需外部环境]
        Mermaid[Mermaid 流程/时序/甘特/状态图]
        WaveDrom[WaveDrom 数字时序与总线图]
        Viz[Graphviz / Viz.js dot 有向图]
        Vega[Vega / Vega-Lite 交互式数据图]
        Bitfield[Bitfield 寄存器位域图]
    end

    subgraph External_CLI[需要安装外部环境]
        PlantUML[PlantUML UML 类图/架构图 (需 Java)]
        D2[D2 声明式现代架构图 (需 D2 CLI)]
        Ditaa[Ditaa ASCII 字符转图 (需 Java)]
        TikZ[TikZ 矢量学术排版 (通过 Code Chunk / TeX)]
    end
```

- **Mermaid**：内置 3 套主题（`default`, `dark`, `forest`），支持通过 `@iconify-json` 注册外部图标包；
- **PlantUML**：支持代码块缺省 `@startuml` 时自动补全闭合标签；支持通过 Java 本地调用或官方 PlantUML Web 服务器代理渲染；
- **WaveDrom & Bitfield**：专为硬件工程师设计的数字逻辑电平波形与芯片寄存器位图；
- **Vega & Vega-Lite**：声明式 JSON 数据驱动图表，支持 `{interactive=true}` 开启前端悬浮、缩放与过滤交互；
- **D2**：支持通过代码块属性 `{layout="elk", theme=1, sketch=true}` 自定义现代手绘/架构风格。

---

### 2.4 文档模块化：文件导入系统 (`@import` File Imports)
这是 MPE 最具特色的功能之一，允许将项目中的离散素材组合进主文档：

1. **导入普通 Markdown 文件**：
   ```markdown
   @import "sub_chapter.md"
   ```
   自动递归内联编译子章节，解决长篇书籍分章协作问题。
2. **导入数据表格 (CSV)**：
   ```markdown
   @import "dataset.csv"
   ```
   自动将 CSV / TSV 格式化为标准 Markdown 渲染表格。
3. **导入源码片段并高亮行范围**：
   ```markdown
   @import "main.py" {line_begin=10 line_end=25 highlight=[15, 18]}
   ```
4. **导入图表文件与 PDF 页面**：
   ```markdown
   @import "architecture.puml"
   @import "paper.pdf" {page_no=1}
   ```

---

### 2.5 交互式代码块执行 (Code Chunk)
允许在 Markdown 预览时直接运行嵌入的代码，并将终端输出或图像就地回填进文档：

- **多语言支持**：Python, R, JavaScript, Bash, PowerShell, C++, Go, Julia, Rust, PHP 等数十种解释器。
- **语法属性控制**：
  ```markdown
  ```python {cmd=true id="plot1" args=["--fast"] matplotlib=true}
  import matplotlib.pyplot as plt
  plt.plot([1, 2, 3], [4, 5, 6])
  plt.show()
  ```
  ```
- **核心控制属性**：
  - `cmd=true`：标记该代码块可执行；
  - `hide=true`：仅显示执行结果，隐藏源代码；
  - `output="html"` / `output="markdown"` / `output="png"`：指定输出渲染管道；
  - `continue="id"`：支持上下文会话延续（跨代码块共享变量与环境，类似于 Jupyter Notebook）；
  - `element="<div id='viz'></div>"`：将输出绑定至自定义 DOM 节点。

---

### 2.6 幻灯片演说模式 (Presentation with Reveal.js)
MPE 内置完整的 **Reveal.js** 幻灯片制作与演讲引擎：

- **分页分隔符**：
  - `<!-- slide -->`：横向切换页面；
  - `<!-- subslide -->`：垂直下钻页面；
- **页面配置与转场**：
  ```yaml
  ---
  presentation:
    theme: league.css
    transition: slide
    slideNumber: true
    enableSpeakerNotes: true
  ---
  ```
- **演讲者视图 (Speaker Notes)**：支持 `<!-- note -->` 区域，按 <kbd>S</kbd> 键唤起专属计时器、下一页预览与演讲者备注面板。

---

### 2.7 导出全管道体系 (Export Pipelines)
MPE 提供了业界最丰富的导出后端选项：

1. **HTML 导出**：
   - 离线独立包（嵌入 Base64 图片与离线样式）；
   - CDN 网页（体积轻巧，适合挂载 Web 服务器）。
2. **Puppeteer 打印引擎 (Headless Chrome)**：
   - 导出像素级高保真 **PDF**、长图 **PNG** 与 **JPEG**；
   - 支持通过 Front-matter 自定义 `margin`, `format` (A4/Letter), `printBackground`, `headerTemplate` 与 `footerTemplate`。
3. **Prince XML 后端**：
   - 针对出版业的超高质量 PDF 排版（支持 CSS Paged Media 规范：书籍对称内外边距、脚注与双栏排版）。
4. **eBook 制作引擎**：
   - 借助 `calibre` 的 `ebook-convert` 工具，一键编译生成 **EPUB**、**MOBI**、**PDF** 电子书。
5. **Pandoc 深度集成**：
   - 支持调用本地 Pandoc 引擎将 Markdown 编译为 PDF、Word (`.docx`)、LaTeX、RTF、MediaWiki 等。

---

### 2.8 辅助实用工具
- **TOC 目录生成**：输入 `[TOC]` 或 `<!-- @import "[TOC]" -->`，自动抓取标题生成带层级跳转链接的目录树；
- **Image Helper 图床工具**：支持拖拽图片自动上传至 Imgur、SM.MS、七牛云或复制到本地相对路径 assets 目录；
- **GFM 预编译**：支持将含有 `@import`、Code Chunk 与特殊语法的 Markdown 展平编译为纯净的标准 GitHub Flavored Markdown。

---

## 3. ReadMD (v2.3.4) 与 MPE (Crossnote) 深度对比矩阵

| 评估维度 | Markdown Preview Enhanced (MPE) | ReadMD (v2.3.4) | 对比研判与优势归属 |
| :--- | :--- | :--- | :--- |
| **产品形态与生态** | 仅作为 VS Code / Atom 编辑器插件或前端 npm 包 | **三位一体**：跨平台独立桌面客户端 + MCP Server (AI Agents) + VS Code 插件 | **ReadMD 胜**：覆盖独立桌面 GUI、AI 协同 (Cursor/Claude) 与 IDE 插件 |
| **语法自愈与盘古排版** | ❌ 无语法修复功能；公式断裂或表格错位时直接渲染失败 | ✅ **核心特色**：内置智能自愈引擎，自动修复断裂公式、错位表格、中英文盘古空格与标点规范 | **ReadMD 绝胜**：业内独家自愈能力 |
| **全格式文档双向互转** | 需依赖外部安装的 Pandoc 命令行工具（未内置） | ✅ **原生内置**：支持 Word/PDF/PPT/Excel/TXT/TeX ⇄ Markdown 零配置秒级互转 | **ReadMD 胜**：零外部依赖，开箱即用 |
| **网页正文深度抽取** | ❌ 无内置网页抓取功能 | ✅ **内置原生**：支持输入任意 URL 自动清洗并深度抽取干净 Markdown | **ReadMD 胜** |
| **离线 OCR 文字识别** | ❌ 无 OCR 功能 | ✅ **内置原生**：支持 WinRT / macOS Vision / Tesseract 图片与扫描 PDF 本地 OCR | **ReadMD 胜** |
| **排版级多格式导出** | 依赖外部 Puppeteer (Node) / Prince / Pandoc 安装 | ✅ **原生内置**：PDF / Word (原生 OMML 公式) / HTML / LaTeX 导出，内置 15 种专业风格预设 | **ReadMD 胜**：免配 Node/Pandoc/Puppeteer |
| **AI 提示词流与 Agent 集成** | ❌ 无 AI 协议集成 | ✅ **深度集成**：完整暴露 10 项标准 MCP 工具 + 12 种专业文档 AI 提示词流程 | **ReadMD 胜** |
| **图表可视化多样性** | ✅ 支持 Mermaid, PlantUML, WaveDrom, Graphviz, Vega, D2, Ditaa | 仅内置原生 Mermaid 流程图与时序图 | **MPE 胜**：可视化库丰富度高 |
| **交互式代码块 (Code Chunk)** | ✅ 支持 Python, R, Bash, Node.js 等就地运行与会话延续 | ❌ 仅代码块语法高亮，未开启就地命令执行 | **MPE 胜**：数据分析与交互演示能力强 |
| **幻灯片演说制作** | ✅ 内置 Reveal.js 幻灯片与演讲者模式 | 需通过外部导出工具或基础排版呈现 | **MPE 胜**：适合制作技术演讲 PPT |
| **文档模块化 `@import`** | ✅ 强大的 `@import` 语法，支持分章嵌套与 CSV 转换 | 基础链接与嵌入 | **MPE 胜**：长篇书籍/文档模块化管理优秀 |

---

## 4. ReadMD 演进建议与吸取路线图

基于对 MPE 官方文档与架构的深度解构，建议 ReadMD 在保持**“自愈排版、零依赖全格式互转、离线安全、AI MCP 协同”**核心护城河的前提下，逐步吸收 MPE 的以下亮点功能：

### 阶段一：高价值低成本功能引入 (v2.4)
1. **文档模块化导入 (`@import` 基础版)**：
   - 在语法自愈与预览引擎中支持 `@import "filename.md"` 与 `@import "data.csv"`，让用户在 ReadMD 中能够组织多文件大项目或电子书。
2. **多图表渲染扩展 (纯 JS 零依赖栈)**：
   - 在 Webview 预览层引入 `WaveDrom`（数字波形）与 `Viz.js`（Graphviz 有向图），无需用户配置外部 Java/Node 环境即可扩展渲染能力。
3. **快速插入 Markdown 辅助组件 (Snippets & Helpers)**：
   - 引入可视化快捷插入表格对话框、一键生成 `[TOC]` 目录等实用交互。

### 阶段二：专业演说与多媒体制作 (v2.5)
1. **内置 Reveal.js 幻灯片全屏放映模式**：
   - 支持 `<!-- slide -->` 分隔符，在 ReadMD 桌面端与 VS Code 插件中提供「一键进入幻灯片演讲模式」与双屏演讲者视图。
2. **长篇电子书 (eBook) 批量打包**：
   - 结合已有的全格式转换与排版引擎，支持将工作区多个 Markdown 文件一键打包为带完整目录与元数据的 EPUB / PDF 电子书。

### 阶段三：安全沙箱化的代码块执行 (Code Chunk v3.0)
1. **安全隔离的本地代码运行器**：
   - 汲取 MPE 历史上 CVE-2026-49492 命令注入的教训，基于 ReadMD 现有的严格命令白名单（`validate_command`）与沙箱环境，为 Python/Matplotlib/Bash 提供安全可审计的就地代码运行与图表回填能力。
