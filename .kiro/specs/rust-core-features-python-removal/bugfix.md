# Bugfix 需求文档：Rust 核心功能补全与 Python 依赖移除

## Introduction

ReadMD 已经从旧的 Python 宿主（`readmd.py` + `src/readmd_core` + `src/readmd_modules`）迁移到 Rust 内核（`rust/readmd-kernel`，Tao/WRY 窗口 + 本地 HTTP API + `assets/` 前端）。仓库根目录下的 `readmd.py`、`src/`、`config/` 已不存在，Rust 内核可以正常编译，`cargo test -p readmd-kernel --lib` 目前 1670 项全部通过，前端调用的 70 个 `/api/*` 路由和注入的 `pywebview` 兼容层方法基本都已接到 Rust 实现上。

但审查发现三类问题，导致"功能真正可用"、"去除 Python 依赖"和"整体质量达标"这三个目标都没有达成：

1. **核心功能在 Rust 端被降级实现或逐字复刻了 Python 的缺陷**。编辑保存会丢失原文件编码；DOCX / LaTeX 导出退化为逐行字符串拼接，忽略样式和图片，LaTeX 导出遇到中文加粗会让处理线程 panic；PDF 导出使用非嵌入的 CJK 字体，公式以 LaTeX 源码输出；EPUB 导出不打包本地图片；HTML 文件"转换"只返回原始 HTML 源码；`.xls` / `.ppt` / OCR 永远失败并提示安装 Python 包；macOS / Linux 上没有文件对话框，且安装包找不到 `assets` 目录。
2. **Python 仍然处在运行和构建的必经路径上**。前端实际加载的 `assets/readmd.boot.js` 只能用 `python tools/sync_version.py` 生成；MCP 服务器和 VS Code 扩展运行时要启动 Python 解释器，并且依赖已经被删除的 `src/`；Dockerfile、发布同步 CI、官网校验、Playwright UI 测试和插件中心也都依赖 Python。
3. **前端和桌面集成还有一批功能缺陷，核心界面的 UI/UX 质量不达标**。复审前端（`assets/app.js`、`assets/js/**`、CSS）和内核路由后确认：导出面板的预设接口是空桩，自定义预设不会持久化；单文件转换和拖放转换会静默覆盖已有的同名 `.md`；桌面版拖放进来的文件被当成临时副本打开，编辑保存写不回原文件；批量 OCR 的识别结果被丢弃；演示文稿无法导出为文件；"打开结果目录"按钮永远不显示；编辑器的块级语法按钮会插入到行中间；主题"跟随系统"失效；多处用户可见文案绕过了 i18n。界面层面，样式没有收敛为设计令牌（`style.css` 中有 98 种硬编码颜色、129 处 `!important`、17 种像素字号、14 组断点），多数模态框没有焦点管理，长时间的导出/转换只有一个全局"处理中…"提示，还存在未接线的按钮和死样式。

用户已授权对项目做全面升级：只要所有功能都没有 bug、都能正常使用，可以自由调整界面设计和实现方式，并删除只服务于已退役 Python 宿主的历史脚本。

本次修复的目标：在保持 Rust 架构（单一 Rust 可执行文件 + 本地 HTTP API + Web 前端）的前提下，让编辑、渲染、文件转换/导出端到端可用，并达到该架构能做到的最好效果；移除所有运行时和构建/发布/CI 时对 Python 解释器的依赖；修复复审发现的前端与桌面集成缺陷；把编辑器、预览/阅读器、导出面板和转换流程这几个核心界面升级到统一、可访问、反馈清晰的设计水平。之前"与 Python 逐字节对齐（parity）"的约束，只要和正确行为冲突，就以本文件的"期望行为"为准；界面外观不要求与现状一致，但功能覆盖、路由契约和安全语义必须保持（见"不变行为"）。

## Bug Analysis

### Current Behavior (Defect)

**编辑与保存**

1.1 WHEN 用户打开一个非 UTF-8 编码的文本文档（如 GBK / GB18030 / UTF-16），编辑后保存 THEN 系统忽略前端传来的 `encoding` 参数，总是以 UTF-8 写回，悄悄改变原文件编码，其他依赖原编码的程序因此读出乱码

1.2 WHEN 用户在阅读器中直接打开 `.txt` 文件 THEN 系统返回原始文本、`structured` 为 false（该能力被标记为 `PENDING` 的 `/api/file.txt-md-structuring`），不做 Markdown 结构化，而转换入口里对同一文件已经有可用的 `txt_to_markdown` 结构化实现

**渲染与导出**

1.3 WHEN 用户导出 DOCX THEN 系统使用逐行字符串匹配生成文档：忽略导出面板的全部样式选项（页面尺寸、方向、边距、字体、字号、颜色、标题/表格样式），不嵌入任何本地图片，丢失有序列表、嵌套列表、任务列表、斜体、删除线、超链接、H5/H6、分隔线和表格对齐，导出结果明显比预览差

1.4 WHEN 用户导出 LaTeX THEN 系统使用简化的逐行转换器，而不是内核中已有的 `texmd::md_to_latex`；生成的文档只有 `\usepackage[utf8]{inputenc}`，没有任何 CJK 支持，中文文档无法用常规 TeX 引擎编译；图片、有序列表、表格、链接也会丢失

1.5 WHEN 用户导出的 LaTeX 文档中，非 ASCII 字符（如中文）之后出现 `**加粗**` 或 `` `行内代码` `` THEN 系统按字符下标切 UTF-8 字节字符串（`format_inline_latex`），触发 "byte index is not a char boundary" panic，连接线程直接终止，前端只收到网络错误，导出失败

1.6 WHEN 用户导出 EPUB 且文档引用了本地图片 THEN 系统不打包这些图片，EPUB 里的 `<img>` 指向不存在的相对路径，阅读器中图片全部缺失

1.7 WHEN 用户导出包含中文、日文、韩文或其他非 WinAnsi 字符的 PDF THEN 系统只写入一个不嵌入字形的 `STSong-Light` / `UniGB-UCS2-H` 引用，文字能否显示完全取决于阅读器是否装有 Adobe 亚洲字体包，GB 字符集以外的字符（如韩文、部分日文、符号）显示为空白或乱码；用户在样式中选择的字体也不会嵌入

1.8 WHEN 用户导出包含数学公式、带语言标识的代码块或设置了列对齐的表格的 PDF THEN 系统把公式按 LaTeX 源码原样输出（并为每个公式追加"公式无法渲染"警告），丢弃代码块的语言信息，也丢弃表格列对齐

**文件转换（导入为 Markdown）**

1.9 WHEN 用户转换 `.html` / `.htm` 文件 THEN 系统走 `markitdown_text` 这个"按文本读取"的替身，把原始 HTML 源码当作 Markdown 返回（engine 标为 `markitdown`），不做任何 HTML→Markdown 转换

1.10 WHEN 用户转换 `.xls` 或 `.ppt`（旧版 OLE2 二进制 Office）文件 THEN 系统总是失败，错误信息要求"安装 MarkItDown"（一个 Python 包），而内核中已有可复用的 OLE2 读取器（`ole2.rs`，`.doc` 已经在用）

1.11 WHEN 用户转换 `.mobi` 文件（前端 `CONVERT_EXTS` 和 ZIP 批量转换都宣称支持）THEN 系统判定为二进制并返回 `unsupported_format`

1.12 WHEN 用户对图片或扫描版 PDF 使用 OCR（包括 PDF 文本层提取失败后的 OCR 兜底）THEN 系统永远没有可用引擎（`pick_engine()` 恒为 None），返回 `ocr-no-engine`，错误文案还要求安装 PyObjC、RapidOCR 等 Python 组件，即使在自带系统 OCR 的 Windows 10/11 上也是如此

1.13 WHEN 用户转换音视频文件而本机没有转写能力 THEN 系统返回的安装指引要求执行 `pip install openai-whisper` 或在插件中心安装 Python 插件

**跨平台桌面**

1.14 WHEN 用户在 macOS 或 Linux 桌面版中点击打开文件、打开文件夹、另存为或导出 THEN 系统的所有原生对话框路由（`/api/dialog/*`、`/api/export` 的保存对话框）只有 Windows PowerShell 实现，其他平台直接返回空或 `canceled`，用户无法打开文档、另存或导出

1.15 WHEN 用户安装 Linux `.deb` 包（可执行文件位于 `/usr/bin/readmd`，资源位于 `/usr/share/readmd/assets`）或 macOS `.app`（资源位于 `Contents/Resources/assets`）并启动 THEN 系统的 `assets_dir()` 候选路径不包含这两个位置，找不到前端资源，窗口空白或无法加载界面

**Python 依赖（运行时与构建时）**

1.16 WHEN 开发者修改 `assets/js/**` 或 `assets/app.js` 后需要重新生成前端实际加载的 `assets/readmd.boot.js`，或同步版本号 THEN 系统只能通过 `python tools/sync_version.py` 完成，没有 Python 就无法产出可发布的前端和版本号

1.17 WHEN 用户按文档配置 ReadMD MCP 服务器（`packages/mcp-server`）THEN 系统要求用 `python` 启动 `readmd_mcp_server.py`，该脚本 `import src.readmd_core ...`，而仓库中的 `src/` 已被删除，从仓库或按 README 部署时直接导入失败；Rust 可执行文件没有提供 MCP 能力

1.18 WHEN 用户使用 VS Code 扩展，或开发者构建 VSIX THEN 扩展运行时通过 `pythonFinder` 查找并启动 Python 解释器运行 MCP 脚本，并依赖打包进扩展的旧版 Python 核心副本（`packages/vscode-extension/core/src/**`）；`scripts/stage-core.mjs` 从已不存在的仓库根 `src/` 复制文件，VSIX 构建失败

1.19 WHEN 执行 `docker build` / `docker compose up` THEN 系统基于 `python:3.11-alpine` 镜像，执行 `pip install`，并复制已不存在的 `config/`、`src/`、`readmd.py`，镜像构建失败

1.20 WHEN 发布同步工作流（`release-sync.yml`）或官网工作流/`npm run verify`、`verify:release` 运行 THEN 系统调用 `python tools/release_asset_sync.py` 和 `python showcase/scripts/validate_website.py`，发布和官网部署都依赖 Python

1.21 WHEN 运行 Playwright UI 测试（`ui-tests/playwright.config.js`）或 UI 录制脚本 THEN 系统用 `python ../tools/ui_server.py` 启动测试服务器，而该文件不存在，UI 测试无法启动；录制脚本还通过 `python -c "from src.readmd_modules.convert import ..."` 调用已删除的 Python 模块

1.22 WHEN 用户在插件中心安装任意插件（easyocr、rapidocr、whisper、pypandoc、jieba、pygments 等）THEN 这些插件全部是 pip 包，系统总是返回 `pip_unavailable`，插件中心展示的是一整套永远装不上的 Python 能力

1.23 WHEN 开发者按文档构建桌宠运行时包或运行仓库的校验/测试门禁 THEN 需要执行 `python packages/readmd-pet-rust/scripts/build-package.py`、`rust/tools/*.py`、`tools/**/*.py`、`tests/**/*.py`，文档也仍把桌宠描述为由 "Python PetRuntimeOrchestrator" 启动；仓库中还留有大量只服务于已退役 Python 宿主的脚本和副本（`tools/**/*.py` 38 个，其中 `tools/migration` 8 个；`rust/tools/*.py` 2 个；`tests/**/*.py` 31 个；`showcase/scripts/*.py` 31 个；`docs/dev/i18n-build` 等 `docs/dev/**/*.py` 84 个；`packages/vscode-extension/core/src/**` 70 个；`packages/mcp-server` 与 `packages/readmd-pet-rust/scripts` 各 1 个）

**导出面板（复审新增）**

1.24 WHEN 用户在 Rust 桌面版打开导出面板 THEN 注入的兼容层 `get_export_presets` 固定返回 `{}`，`save_export_presets` 不做任何事就返回 `true`：内核 `export_styles` 中已有的默认样式和 `minimal` / `classic` / `business` 预设不会出现在预设下拉框里；数字字段显示为空（收集时成为 `NaN`/`null`），颜色字段显示为 `#000000`，复选框全部未勾选，与内核默认样式不一致；用户保存的自定义预设和"上次使用的选项"在重启后全部丢失

1.25 WHEN 导出成功但带有警告（如图片缺失、字体降级）THEN 系统只弹出"导出完成，N 条提示"的计数提示，警告的具体内容在界面上无处查看

1.26 WHEN 用户想把文档导出为演示文稿文件 THEN 导出面板没有"演示文稿"格式入口，`export.js` 中 `fmt === 'presentation'` 的分支永远走不到；即使走到，`/api/export/presentation` 也只返回 `standalone=false`（依赖同源 vendor 资源）的 `html` 字符串，不写任何文件，前端却会提示"导出完成"；用户只能在应用内放映，无法得到可离线分发的演示文稿

1.27 WHEN 用户导出 EPUB THEN 前端以空的 `out_path` 调用 `/api/export/epub`，系统不弹保存对话框，直接写入 `DATA_DIR/exports/readmd_export_<毫秒>.epub`，与 PDF/DOCX/HTML/LaTeX 的"选择保存位置"流程不一致，用户无法指定文件名和位置

**文件转换（复审新增）**

1.28 WHEN 用户转换单个文件（打开对话框选择、拖放单个文件）或一次拖放多个文档 THEN 前端分别以 `overwrite=1` / `overwrite: true` 调用转换接口，源文件旁已有的同名 `.md`（可能是用户手写的文档）被静默覆盖；`convertFile` 里"已存在同名 .md，跳过保存"的分支永远不会执行

1.29 WHEN 用户在批量工作台中对多张图片做 OCR THEN `/api/ocr` 只在响应里返回识别文本，批量流程把该行标记为"成功"后就丢弃了文本，既不保存为文件，也不设置 `dataset.out`，点击该行没有任何反应，识别结果全部丢失

1.30 WHEN 批量转换完成 THEN "打开结果目录"按钮（`#convert-open-dir`）在所有代码路径中都只会被加上 `hidden`，从不显示，该按钮及其点击处理实际不可用

1.31 WHEN 拖放的 ZIP 压缩包解压失败（损坏、过大、接口报错）THEN 错误只写入 `console.error`，用户界面没有任何提示，看起来像"拖放没有反应"

**桌面集成（复审新增）**

1.32 WHEN 用户在桌面版中把本地文件拖进窗口 THEN 内核没有注册 WRY 原生拖放处理，WebView 中的 `File` 对象没有 `path` 属性，前端只能把文件上传为临时副本：拖入的 `.md` 以"浏览器副本"打开，编辑保存写入的是副本而不是原文件，最近文件记录的是临时路径；拖入的文档转换后，输出写到上传目录而不是源文件旁边

1.33 WHEN 用户在 macOS 或 Linux 上点击"在文件夹中显示" THEN `/api/system/reveal-path` 只调用 Windows 实现，什么也不做却返回 `{ok: true}`；`/api/system/open-path` 在打开失败时同样返回 `{ok: true}`，前端无法提示失败

1.34 WHEN 用户使用"另存为"（`/api/dialog/save-as`）THEN 系统用非原子的 `std::fs::write` 直接写目标文件，覆盖已有文件时不生成备份；网页图片资源复制失败被 `let _ =` 静默忽略，保存后的文档引用了不存在的图片；对话框标题和文件类型过滤器是硬编码的中文

**编辑器（复审新增）**

1.35 WHEN 用户使用编辑器工具栏的块级语法按钮 THEN 标题、引用、无序/有序/任务列表的前缀插入在选区起点而不是行首（光标在行中时会得到 `abc## def`）；多行选区只有第一行加上前缀；在一段文字末尾插入分隔线得到 `文字\n---`，按 CommonMark 规则会把上一行变成二级标题；在行中插入代码块会让围栏标记紧跟在已有文字之后（例如 `` abc``` ``），形成无效围栏；对已加粗/斜体/删除线的选区再次点击会再包一层标记，而不是取消

**设置与主题（复审新增）**

1.36 WHEN 主题为默认的"跟随系统"（`auto`）且用户在运行期间切换系统深浅色 THEN 界面不跟随变化（没有监听 `prefers-color-scheme` 变化）；点击主题按钮只在 dark → sepia → light 之间循环，一旦点过就再也回不到 `auto`；按钮只有 ☀ / ☾ 两种图标（sepia 和 light 显示相同），也没有反映当前主题的 `aria-label`；编辑器（CodeMirror）只在 dark 下切换暗色主题，sepia 下仍使用默认浅色编辑器主题，与页面底色不协调

**国际化（复审新增）**

1.37 WHEN 界面显示后端返回的消息或部分前端提示 THEN 多处用户可见文案绕过了 i18n：后端直接返回中文（转换的 `note: "未提取到文字…"`、OCR 的 `error: "文件不存在"`、PowerShell 对话框的"另存为 / 导出文档 / Markdown 文件"），前端直接显示；前端硬编码中文（`editor.js` 的"图片保存失败"、`render.js` 的"正在打开文档"和双链 `title="双链跳转"`、`pet-batch.js` 的"正在下载更新"）；转换/OCR 失败时后端只返回 `error_code`（`conversion_failed`、`ocr_failed`、`file_not_found`），前端只读 `d.error`，于是只显示笼统的"转换失败 / OCR 失败"，没有原因

**核心界面 UI/UX 质量（复审新增）**

1.38 WHEN 渲染编辑器、预览/阅读器、导出面板、转换/批量工作台、设置和插件中心 THEN 样式没有收敛为设计系统：`assets/style.css` 有 98 种不同的硬编码十六进制颜色、129 处 `!important`、17 种不同的像素字号、14 组互不一致的媒体查询断点（560/599/600/640/720/760/900/1100px 等）；还有 JS 从不设置的主题选择器（`.theme-dark`、`.dark`）留下的死样式；同类控件（主按钮、次按钮、图标按钮、输入框）在不同界面中的尺寸、圆角、间距各不相同

1.39 WHEN 用户用键盘或辅助技术操作模态框（导出、转换/批量、插件中心、AI 设置、最近文件、图片编辑器、公式选择器等）THEN 这些弹窗虽然声明了 `aria-modal="true"`，但全项目只有关系图弹窗实现了焦点陷阱：其余弹窗打开时焦点不移入，Tab 会跳到背后的页面，除保存冲突弹窗外关闭后焦点也不回到触发按钮；导出面板的 Esc 只在焦点位于 `#export-box` 内部时才生效

1.40 WHEN 用户执行导出、单文件转换或 OCR 这类耗时操作 THEN 界面只显示一个全局的"处理中…"浮层：没有阶段或进度信息，不能取消，也没有超时；完成后警告只显示条数（见 1.25），失败时只显示笼统文案（见 1.37）；操作期间触发按钮仍可重复点击

1.41 WHEN 用户浏览核心界面的按钮、菜单项和输入控件 THEN 存在未接线或不可达的控件和代码：`#convert-open-dir` 永不显示（见 1.30）、导出面板的演示文稿分支没有入口（见 1.26）、隐藏的 `#batch-file-input` 没有任何脚本引用、插件中心列出永远装不上的 pip 插件（见 1.22），仓库也没有检查"每个控件都已接线"的自动化手段

### Expected Behavior (Correct)

**编辑与保存**

2.1 WHEN 用户打开一个非 UTF-8 编码的文本文档，编辑后保存 THEN 系统 SHALL 按打开时检测到并由前端回传的编码（至少覆盖 UTF-8、UTF-8 BOM、GBK/GB18030、UTF-16 LE/BE）写回；当内容含有该编码无法表示的字符时，SHALL 拒绝静默替换，返回明确的错误码，并让用户选择改用 UTF-8 保存

2.2 WHEN 用户在阅读器中直接打开 `.txt` 文件 THEN 系统 SHALL 使用内核已有的 Rust `txt_to_markdown` 进行结构化，返回 `structured: true` 和结构化后的 Markdown，与转换入口的结果一致；`PENDING` 列表中不再保留该项

**渲染与导出**

2.3 WHEN 用户导出 DOCX THEN 系统 SHALL 基于与 PDF 相同的 Markdown AST 和经过 `export_styles::sanitize_options` 处理的样式生成 DOCX：应用页面尺寸、方向、边距、字体、字号、颜色、标题和表格样式；嵌入本地图片（缺失或远程图片给出警告）；正确输出 H1–H6、有序/无序/嵌套/任务列表、引用、粗体、斜体、删除线、行内代码、超链接、分隔线、代码块、带对齐的表格，以及 OMML 行内/块级公式

2.4 WHEN 用户导出 LaTeX THEN 系统 SHALL 使用内核中的 `texmd::md_to_latex`（或同等完整的基于 AST 的转换器）生成独立文档，包含 CJK 支持（如 `ctex` / `xeCJK`，可用 XeLaTeX 编译），并正确转换图片、有序/无序列表、表格、链接、代码块和公式，同时应用导出选项中的标题和作者

2.5 WHEN 用户导出的 LaTeX 文档中，非 ASCII 字符之后出现 `**加粗**` 或 `` `行内代码` `` THEN 系统 SHALL 在字符边界上安全地处理字符串，不发生 panic，正常生成 `\textbf{}` / `\texttt{}`；并且 HTTP 处理器中任何未预期的 panic SHALL 被捕获，转换为结构化的错误响应，而不是让连接中断

2.6 WHEN 用户导出 EPUB 且文档引用了本地图片 THEN 系统 SHALL 把可解析的本地图片打包进 EPUB（写入 manifest，并把 `<img>` 改写为包内路径），无法解析的图片 SHALL 在 `warns` 中逐条列出

2.7 WHEN 用户导出包含 CJK 或其他非 WinAnsi 字符的 PDF THEN 系统 SHALL 从系统字体（或用户所选字体）中嵌入实际使用字形的 TrueType/OpenType 子集（CIDFontType2 + ToUnicode），使 PDF 在任何阅读器中都能正确显示，并且文本可复制、可搜索；找不到可用字体时 SHALL 在 `warns` 中说明降级情况

2.8 WHEN 用户导出包含数学公式、带语言标识的代码块或设置了列对齐的表格的 PDF THEN 系统 SHALL 把公式渲染为排版后的图形（如由纯 Rust 数学排版生成的矢量图，与 HTML 预览的公式效果接近）而不是 LaTeX 源码；SHALL 保留代码块语言并进行语法高亮着色；SHALL 按 Markdown 分隔行指定的对齐方式排版表格列；只有真正无法排版的公式才回退为文本并给出警告

**文件转换（导入为 Markdown）**

2.9 WHEN 用户转换 `.html` / `.htm` 文件 THEN 系统 SHALL 用 Rust 实现的 HTML→Markdown 转换器（可复用 `headless_renderer` / 网页提取已有的逻辑）输出结构化的 Markdown（标题、段落、列表、表格、链接、图片、代码块），engine 标注为 Rust 原生引擎，而不是返回原始 HTML 源码

2.10 WHEN 用户转换 `.xls` 或 `.ppt` 文件 THEN 系统 SHALL 基于现有 `ole2.rs` 用纯 Rust 解析 BIFF8 工作簿和 PowerPoint 97-2003 演示文稿，分别输出 GFM 表格（每个工作表一节）和按幻灯片分节的文本；解析失败时 SHALL 返回不提及任何 Python 包的明确错误

2.11 WHEN 用户转换 `.mobi` 文件 THEN 系统 SHALL 用纯 Rust 解析未加密的 MOBI/PalmDOC（含 PalmDOC/LZ77 解压）并输出 Markdown；遇到 DRM 或不支持的变体时 SHALL 返回明确的 `unsupported_format` 类错误和原因

2.12 WHEN 用户在 Windows 10/11 上对图片或扫描版 PDF 使用 OCR THEN 系统 SHALL 调用系统自带的 OCR 能力（Windows.Media.Ocr）完成识别，并按现有版面分析（XY-Cut）输出 Markdown；在没有系统 OCR 的平台上 SHALL 返回可本地化的"当前平台无可用 OCR 引擎"错误，文案中不出现任何 Python 组件

2.13 WHEN 用户转换音视频文件而本机没有转写能力 THEN 系统 SHALL 返回不包含 `pip` 或 Python 插件安装步骤的降级说明，只描述 ReadMD 自身可用的途径（如系统工具或后续提供的 Rust 原生能力）

**跨平台桌面**

2.14 WHEN 用户在 macOS 或 Linux 桌面版中点击打开文件、打开文件夹、另存为或导出 THEN 系统 SHALL 弹出该平台的原生文件对话框，返回值格式与 Windows 版一致，保证打开、另存、导出流程在三个平台上都能完成

2.15 WHEN 用户从 Linux `.deb` 包或 macOS `.app` 启动 ReadMD THEN 系统 SHALL 能定位到打包的前端资源（`<exe>/../share/readmd/assets`、`<exe>/../Resources/assets`，以及现有的候选路径），正常加载界面

**Python 依赖（运行时与构建时）**

2.16 WHEN 开发者需要重新生成 `assets/readmd.boot.js` 或同步版本号 THEN 系统 SHALL 提供不依赖 Python 的工具（如 Rust `xtask`/工具二进制或 Cargo 构建步骤），输出与当前拼接规则逐字节一致的 bundle，并提供 `--check` 模式供 CI 校验

2.17 WHEN 用户配置 ReadMD MCP 服务器 THEN 系统 SHALL 由 Rust 可执行文件提供 stdio MCP 模式（如 `readmd --mcp`），直接调用内核中已有的 Rust 实现，至少覆盖现有 Python 服务器公开的工具（修复 Markdown、生成目录、处理导入、代码块运行、转换、OCR、导出文档/EPUB/演示文稿、LaTeX↔Markdown、LaTeX→OMML、BibTeX、PDF 审阅/编辑/回滚、网页转 Markdown、AI 相关工具）；配置模板和 README SHALL 不再出现 `python`

2.18 WHEN 用户使用 VS Code 扩展，或开发者构建 VSIX THEN 扩展 SHALL 通过 Rust 可执行文件（`readmd --mcp`）提供全部功能，不再查找或启动 Python；VSIX SHALL 不再打包 Python 核心副本；`readmd.pythonPath` 配置项和 `pythonFinder` SHALL 被移除；VSIX 构建 SHALL 能在没有 Python 的环境中成功完成

2.19 WHEN 执行 `docker build` / `docker compose up` THEN 系统 SHALL 用多阶段构建（Rust 编译阶段 + 精简运行镜像）得到以浏览器/局域网模式运行 Rust 内核的镜像，不包含 Python，也不引用已删除的文件

2.20 WHEN 发布同步工作流、官网工作流或 `npm run verify` / `verify:release` 运行 THEN 这些步骤 SHALL 使用 Rust 或 Node（官网已依赖 Node）实现，不再调用 Python

2.21 WHEN 运行 Playwright UI 测试或 UI 录制脚本 THEN 测试服务器 SHALL 由 Rust 内核（如 `readmd --browser --port <p>` 或专用测试模式）提供，转换产物 SHALL 通过 Rust 内核的 HTTP API 获取，不再调用 Python

2.22 WHEN 用户打开插件中心 THEN 系统 SHALL 不再展示任何需要 pip 安装的 Python 插件；插件中心 SHALL 只列出 ReadMD 能实际提供的能力（Rust 原生能力或可检测的外部可选工具），并如实显示可用状态

2.23 WHEN 开发者构建桌宠运行时包或运行仓库质量门禁 THEN 所有构建、发布、CI 和运行时步骤 SHALL 不需要 Python 解释器；只服务于已退役 Python 宿主的历史脚本和副本（`tools/**/*.py` 含 `tools/migration`、`rust/tools/*.py`、`tests/**/*.py`、`showcase/scripts/*.py`、`docs/dev/i18n-build` 等 `docs/dev/**/*.py`、`packages/vscode-extension/core/src/**` Python 副本、`packages/mcp-server` 的 Python 服务器、`packages/readmd-pet-rust/scripts/build-package.py`）SHALL 从仓库删除，但每一项删除 SHALL 满足以下之一：其中仍有价值的校验或功能已移植为 Rust 工具、`cargo test` 用例或 Node 脚本，并在 CI 中执行；或者在设计文档的删除清单中注明"已过时"及理由。删除后 SHALL 满足：仓库中任何 workflow、`package.json` 脚本、Dockerfile、Cargo 构建脚本和运行时代码都不调用 `python` / `python3` / `pip`，并由一个 Node 或 Rust 检查在 CI 中持续断言这一点（3.12、3.13 中属于用户内容的 `.py` 不在检查范围内）；README、CONTRIBUTING 和开发文档 SHALL 更新为 Rust 架构的描述

**导出面板**

2.24 WHEN 用户在任意模式（桌面兼容层或浏览器/HTTP）打开导出面板 THEN 系统 SHALL 从 Rust 内核获取导出预设：`defaults` 等于 `export_styles::default_style()`，内置预设覆盖 `preset_names()` 中的全部项（`minimal` / `classic` / `business`，显示本地化名称）；面板初始化后，每个字段的显示值 SHALL 等于默认样式中对应的值（不再出现空数字、全黑颜色、全不勾选）；用户保存的自定义预设和"上次使用的选项" SHALL 持久化到数据目录，重启后仍能读取；与内置预设重名的保存 SHALL 被拒绝并提示

2.25 WHEN 导出成功但带有警告 THEN 系统 SHALL 在导出结果区逐条列出警告内容（本地化文本，可展开/收起），同时保留成功路径和"打开 / 在文件夹中显示"操作

2.26 WHEN 用户需要导出演示文稿文件 THEN 导出面板 SHALL 提供"演示文稿（HTML）"格式入口，沿用现有主题和切换效果选项；导出 SHALL 弹出保存对话框，生成 `standalone` 的单文件 HTML（内联 reveal.js 资源，本地图片以 data URI 嵌入），离线双击即可放映，并返回 `{ok, path, size, warns, error, canceled}`；应用内的演示模式保持不变

2.27 WHEN 用户导出 EPUB THEN 系统 SHALL 与其他格式一样弹出保存对话框（默认文件名为 `<文档名>.epub`），取消时返回 `canceled: true` 且不写文件，确认后写入用户选择的位置并继续应用 EPUB 元数据（标题、作者、语言、分章级别等）；显式传入 `out_path` 时现有的 `409 output_exists` / `overwrite` 语义不变

**文件转换**

2.28 WHEN 用户转换单个文件、拖放一个或多个文档进行转换 THEN 系统 SHALL 默认不覆盖已有的同名 `.md`；目标已存在时，单文件流程 SHALL 让用户在"覆盖 / 另存为新文件名（如 `report (1).md`）/ 仅预览不保存"之间选择，批量流程 SHALL 遵循工作台中"覆盖已存在"复选框的状态（默认不勾选），被跳过的文件在列表中标记为"已跳过"

2.29 WHEN 用户在批量工作台中对图片做 OCR THEN 每个成功的识别结果 SHALL 保存为源文件旁的同名 `.md`（覆盖规则同 2.28），该行 SHALL 记录输出路径，点击该行 SHALL 打开结果文档；识别为空的图片标记为"无文字"而不是"成功"

2.30 WHEN 批量转换或批量 OCR 结束且至少有一个输出文件 THEN "打开结果目录"按钮 SHALL 显示，点击后在三个平台上打开输出所在目录（按文件夹批量时为所选文件夹；按文件批量时为输出文件的共同父目录，没有共同父目录时为第一个输出所在目录）；没有任何输出时按钮保持隐藏

2.31 WHEN 拖放的 ZIP 压缩包解压失败 THEN 系统 SHALL 针对每个失败的压缩包显示本地化的错误提示（包含文件名和原因类别：损坏 / 过大 / 不支持 / 服务错误），其余压缩包和文件的处理继续进行

**桌面集成**

2.32 WHEN 用户在桌面版中把本地文件拖进窗口 THEN 系统 SHALL 通过 WRY 原生拖放事件获得真实文件路径：拖入的文本/Markdown 文件 SHALL 以原路径打开（非"浏览器副本"），编辑保存 SHALL 写回原文件并遵守 3.1 的保存语义，最近文件记录原路径；拖入的文档转换后输出 SHALL 位于源文件旁边；浏览器/局域网模式下的上传兜底保持可用（见 3.20）

2.33 WHEN 用户在任一平台点击"打开"或"在文件夹中显示" THEN 系统 SHALL 在 macOS 上使用 `open` / `open -R`，在 Linux 上使用 `xdg-open`（显示文件时打开其所在目录）；操作失败或路径不存在时 SHALL 返回 `{ok: false, error_code}`，前端显示本地化的失败提示，而不是一律返回成功

2.34 WHEN 用户使用"另存为" THEN 系统 SHALL 使用与 `/api/save` 相同的原子写入；覆盖已有文件时 SHALL 按 3.1 的规则生成 `.bak`；资源文件复制失败 SHALL 逐条在响应的 `warns` 中返回并由前端展示，文档中对应引用保持原路径；对话框标题和文件类型过滤器 SHALL 使用当前界面语言

**编辑器**

2.35 WHEN 用户使用编辑器工具栏的语法按钮 THEN 块级语法（标题、引用、无序/有序/任务列表）SHALL 作用于选区覆盖的整行：每一行都在行首加上前缀，有序列表按 1..n 编号，已有同类前缀时再次点击 SHALL 移除前缀；分隔线和代码块 SHALL 保证前后各有一个空行，插入分隔线后上一行不会变成标题，代码块围栏始终独占一行；行内标记（加粗、斜体、删除线、行内代码）对已被同一标记包裹的选区再次点击 SHALL 取消该标记；每次操作 SHALL 可以用一次撤销完整还原

**设置与主题**

2.36 WHEN 主题设置为"跟随系统" THEN 系统 SHALL 监听 `prefers-color-scheme` 变化并实时切换，无需重启；用户 SHALL 能在 auto / light / dark / sepia 四种主题之间明确选择（任何时候都能回到 auto）；主题按钮的图标和本地化 `aria-label` / `title` SHALL 反映当前主题，四种状态可区分；编辑器主题 SHALL 分别匹配 light、dark、sepia 三种界面底色

**国际化**

2.37 WHEN 界面显示任何用户可见的文案（提示、错误、对话框标题、按钮 `title`、`aria-label`）THEN 文案 SHALL 来自 i18n 键，并在全部 46 个语言包中存在；对于目前只返回中文 `error` / `note` 的接口，内核 SHALL 在保留原字段的前提下新增稳定的 `error_code` / `note_code` 字段，前端优先按代码映射到本地化文案；转换、OCR、导出失败时 SHALL 显示具体原因（如"文件不存在""格式不受支持""未提取到文字，可尝试 OCR"）；SHALL 提供一个在 CI 中运行的 Node 检查，断言内核会发出的每个 `error_code` 都有对应的 i18n 键、各语言包键集合一致，且 toast / `title` / `aria-label` 中没有未经 `_t()` 的硬编码中文字面量（`_t(k) || '回退文案'` 形式的回退除外）

**核心界面 UI/UX 质量**

2.38 WHEN 渲染编辑器、预览/阅读器、导出面板、转换/批量工作台、设置和插件中心 THEN 样式 SHALL 基于一套集中定义的设计令牌：颜色（语义化的背景、前景、边框、强调、成功、警告、错误）、以 4px 为基数的间距刻度、不超过 8 级的字号刻度、圆角、阴影和层级（z-index），并为 light / dark / sepia 分别给出取值；这些界面的组件样式 SHALL 只引用令牌，令牌定义之外不出现十六进制颜色字面量和像素字号字面量，`!important` 只允许出现在列入白名单的场景；媒体查询断点 SHALL 统一为不超过 3 个命名断点；未使用的主题选择器（`.theme-dark`、`.dark`）SHALL 删除；同类控件（主按钮、次按钮、图标按钮、输入框、下拉框、复选框）SHALL 在所有核心界面中使用同一套尺寸、圆角和间距；布局 SHALL 在 360×640、768×1024、1280×800 三种视口下没有文档级横向滚动，导出面板在窄视口下选项区与预览区上下堆叠；可点击区域 SHALL 不小于 32×32px（粗指针设备上不小于 44×44px）。以上约束 SHALL 由一个 CI 中运行的 Node 样式检查和 Playwright 视口截图测试共同验证

2.39 WHEN 用户用键盘或辅助技术操作核心界面和所有模态框 THEN 模态框打开时焦点 SHALL 移入弹窗（第一个可交互元素或标题），Tab / Shift+Tab 只在弹窗内循环，Esc 关闭最上层弹窗（与焦点所在位置无关），关闭后焦点回到触发控件，弹窗打开期间背景内容不可聚焦；所有只有图标的按钮 SHALL 有当前语言的 `aria-label`；所有可交互元素 SHALL 有可见的 `:focus-visible` 样式（轮廓不小于 2px，与相邻颜色的对比度不低于 3:1）；三种主题下正文文字与背景的对比度 SHALL 不低于 4.5:1，大号文字和图形元素不低于 3:1；工具栏、菜单和导出格式页签 SHALL 支持方向键导航并正确维护 `aria-selected` / `aria-expanded`。以上约束 SHALL 由 Playwright + axe-core 自动化测试（零 serious / critical 违规）和键盘操作脚本验证

2.40 WHEN 用户执行导出、单文件转换、OCR 或批量任务 THEN 系统 SHALL 在发起操作的界面内（而非只靠全局浮层）显示进度：导出显示阶段（准备 / 渲染 / 写入），批量任务和多页 PDF OCR 显示确定的进度（已完成 / 总数）；SHALL 提供取消操作——批量和 OCR 在服务端停止后续任务，导出在写入前取消且不留下半成品文件；操作进行中触发按钮 SHALL 处于禁用状态，防止重复提交；完成后 SHALL 显示结果路径、"打开 / 在文件夹中显示"操作和警告列表；失败后 SHALL 显示本地化原因和"重试"操作；超过 2 分钟没有进度更新时 SHALL 提示用户可以取消

2.41 WHEN 用户浏览核心界面 THEN `index.html` 和脚本动态生成的每个按钮、菜单项和输入控件 SHALL 要么绑定了可达的处理逻辑，要么被删除（包括 `#batch-file-input`、导出面板中没有入口的分支）；SHALL 提供一个在 CI 中运行的 Node 检查，枚举 `id`、`data-action`、`data-md`、`data-fmt`、`data-pv` 等控件标识并断言每个都有对应的绑定；SHALL 提供一个 Playwright 冒烟测试，依次点击核心界面中所有可见的工具栏按钮和菜单项，断言没有未捕获的控制台错误，也没有点击后毫无反应的控件

### Unchanged Behavior (Regression Prevention)

3.1 WHEN 用户打开、编辑并保存 UTF-8 Markdown 文档 THEN 系统 SHALL CONTINUE TO 使用原子写入、首次保存生成 `.bak` 备份、基于 `expected_mtime` 做冲突检测并返回 409 冲突信息，响应字段保持为 `ok` / `path` / `backup` / `mtime`

3.2 WHEN 前端请求保存某个路径 THEN 系统 SHALL CONTINUE TO 只允许写入经 `/api/file` 读取过的文件及其同目录下 `AI*-` 派生文件，并保持现有的 loopback Host 校验、Origin 校验和 App Token 校验

3.3 WHEN 用户在阅读器或编辑器中预览 Markdown THEN 系统 SHALL CONTINUE TO 支持现有渲染链路的全部功能（marked、KaTeX/MathJax、Mermaid/Graphviz/WaveDrom/Vega/Chart.js 等图表、代码块卡片与代码块运行、`@import` 处理、TOC、双链与反链、搜索），同一文档渲染出的语义结构（标题层级、列表、表格、公式、图表、链接目标）与现状一致；界面的视觉样式（配色、间距、字体、布局）允许按 2.38 重新设计，不要求像素级一致

3.4 WHEN 用户导出 HTML THEN 系统 SHALL CONTINUE TO 生成内联 marked 与 MathJax、本地图片以 data URI 嵌入、应用导出样式 CSS 的单文件 HTML，远程或缺失图片的警告条目保持不变（文案允许按 2.37 本地化）

3.5 WHEN 用户在应用内启动演示模式（Reveal.js）THEN 系统 SHALL CONTINUE TO 按现有主题、切换效果、字号调节、概览/全屏和标签白名单清理规则放映；2.26 新增的文件导出使用同一套主题与清理规则

3.6 WHEN 用户导出只包含 WinAnsi 字符、没有公式的 PDF THEN 系统 SHALL CONTINUE TO 应用页面尺寸、方向、边距、颜色、标题、列表、引用、表格和本地图片，保持"成功后才替换目标文件"的原子落盘，以及现有的 `{ok, path, size, warns, error, canceled}` 响应结构

3.7 WHEN 用户转换已由 Rust 原生支持的格式（`.docx`、带文本层的 `.pdf`、`.pptx`、`.xlsx`、`.doc`、`.csv/.tsv`、`.tex/.latex`、`.rtf`、`.odt`、`.epub`、代码/配置文件、`.txt`、ZIP 批量）THEN 系统 SHALL CONTINUE TO 产出与当前相同的 Markdown 内容和 engine 标识，现有转换测试夹具的结果不变

3.8 WHEN 用户在 Windows 上使用原生对话框 THEN 系统 SHALL CONTINUE TO 弹出现有对话框，返回格式（`{ok, path}` / 路径列表 / `canceled`）不变

3.9 WHEN ReadMD 以 Windows 便携版/NSIS 安装布局（`assets` 与 exe 同级）、开发布局（`cargo run`）或设置了 `READMD_ASSETS_DIR` 启动 THEN 系统 SHALL CONTINUE TO 按现有优先级找到资源目录

3.10 WHEN 前端调用任意现有 `/api/*` 路由或注入的 `pywebview` 兼容层方法 THEN 系统 SHALL CONTINUE TO 保持现有的路由表、请求参数和响应结构（包括 `ApiCode` / `LegacyError` / `PlainText` 三种错误形态），被本文件修正的行为除外

3.11 WHEN 构建和测试 Rust 工作区 THEN 系统 SHALL CONTINUE TO 支持使用固定版本依赖离线构建（`cargo build --offline`），现有的 `cargo test -p readmd-kernel` 测试继续全部通过；只有断言被本文件判定为缺陷的旧行为的测试，才允许随修复一起更新

3.12 WHEN 用户在文档中运行 Python/JavaScript/Bash/R 等代码块，且本机装有对应解释器 THEN 系统 SHALL CONTINUE TO 按现有代码块运行器的超时和安全规则执行；这属于用户内容的运行，不算 ReadMD 自身的 Python 依赖

3.13 WHEN 用户导入或使用内含 `.py` 脚本的技能包（如 `assets/upstream/**`、`assets/skills/**`）THEN 系统 SHALL CONTINUE TO 把这些脚本当作技能包数据导入和展示，ReadMD 自身不执行它们

3.14 WHEN 本机装有可选的外部工具（PlantUML/Java、Node 用于 Vega、`antiword`、`pdftotext`）THEN 系统 SHALL CONTINUE TO 把它们作为可选的增强引擎使用，缺失时按现有错误码降级

3.15 WHEN 用户使用单实例转发、文件关联、开机自启、最近文件、分享、AI 对话、桌宠（`readmd-pet-rust`）、更新检查等非本次修复范围的功能 THEN 系统 SHALL CONTINUE TO 保持现有行为

3.16 WHEN 构建官网（`npm run build`）或发布 Windows/Linux/macOS 产物（`release.yml` 中的 Cargo 构建与打包步骤）THEN 系统 SHALL CONTINUE TO 产出相同的目录结构和产物名称

3.17 WHEN 用户使用现有键盘快捷键（Ctrl+S 保存、Ctrl+E 编辑、Alt/Ctrl+←/→ 历史前进后退、演示模式快捷键、Esc 按层级关闭弹窗/菜单/搜索栏等）THEN 系统 SHALL CONTINUE TO 执行相同的操作；UI 重新设计不得移除或改绑已有快捷键

3.18 WHEN 系统读取或保存用户设置 THEN 系统 SHALL CONTINUE TO 使用现有设置键（`theme`、`fontSize`、`lineWidth`、`aiPanelWidth`、`autoReload`、`pvLayout`、`pvSync`、`pvSplitX`、`pvSplitY`）以及浏览器模式下的 `localStorage` 兜底；升级前保存的设置值（包括 `theme: 'auto'`）在升级后继续有效

3.19 WHEN 用户切换界面语言 THEN 系统 SHALL CONTINUE TO 提供现有的全部 46 个语言包，已有 i18n 键名不改名；新增的键 SHALL 同时加入所有语言包

3.20 WHEN ReadMD 以浏览器或局域网模式运行（没有原生桌面窗口）THEN 系统 SHALL CONTINUE TO 通过 `/api/upload` 上传兜底来打开、拖放和转换文件，并保持现有的局域网访问控制（`lan_guard` 路由授权）
