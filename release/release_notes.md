# ReadMD v2.3.3

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.3.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.3.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.3.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.3.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.3.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包 (UOS / 银河麒麟 / Deepin / Ubuntu / Debian)：`readmd_2.3.3_amd64.deb`
- 📱 HarmonyOS NEXT 纯血鸿蒙安装包：`ReadMD-harmonyos-v2.3.3.hap`
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.3.vsix`
- 🤖 FastMCP Server 独立包：`readmd-mcp-server-2.3.3.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

## 本次更新 (v2.3.3)

1. **Word ⇄ Markdown 原生 OMML 深度双向公式互转引擎**：
   - **DOCX ➔ Markdown**：全量支持 OMML 语法树（矩阵 `\begin{matrix}`、定界符 `\left(...\right)`、方程组 `\begin{aligned}`、极限 `\lim`、框选 `\boxed`、重音符号 `\vec` 等及 200+ 希腊/数学符号 Unicode 映射），精准还原段落物理顺序与表格内嵌公式；
   - **Markdown ➔ DOCX**：新增轻量级 `latex2omml.py` 编译器，将 LaTeX 数学公式编译为微软 Word 原生矢量 OMML 节点（`m:oMath`/`m:oMathPara`），双击原生可编辑；
   - 20 篇真实学科前沿论文与网络源学术论文全流程验证 (100% PASSED)。

2. **原生 LaTeX ⇄ Markdown 深度高精度双向互转引擎**：
   - 宏预展开引擎（`\newcommand`, `\def`, `\DeclareMathOperator` 等自定义多参数宏递归展开）与平衡大括号词法分析；
   - 全学术环境支持（`algorithm`/`algorithmic` 伪代码、`tabular`/`booktabs` 复杂表格、`thebibliography`、定理证明 callout 等）；
   - 775 份历年高考/竞赛真题库与 50 篇 NeurIPS/ICML/CVPR/ACL/IEEE 完整学术论文手稿全维度审计 100% 满分通过。

3. **超长 Markdown（>10,000 行）智能语义分页引擎与纯 SVG 翻页交互**：
   - 针对超长文档（超过 8,000~10,000 行或 >500KB）自动激活智能语义分页阅读，消除大文档直接全量排版引发的卡顿与浏览器假死；
   - 底部翻页控制栏升级为**纯 SVG 矢量图标交互**（首页 `|◀`、上一页 `◀`、页码下拉选择器、下一页 `▶`、末页 `▶|`、双模切换开关 `📄/📜`），界面极简清爽无冗余文字；
   - 状态机语法边界保护：代码块围栏、多行数学环境与 Markdown 表格行绝对不在中间腰斩截断；
   - 全局目录树 (TOC) 与全文搜索 (Ctrl+F) 跨页无缝联动；
   - 集成 MathJax `IntersectionObserver` 视口公式按需排版引擎，开屏先排版视口公式，其余公式按需异步排版，彻底根除假死。

4. **未保存标签页关闭确认弹窗 UI 深度重塑 (Frontend Design & Taste Upgrade)**：
   - 引入毛玻璃遮罩（`backdrop-filter: blur(10px)`）、`16px` 大圆角、琥珀色柔光警告图标、高对比度实色 Accent 保存按钮与微弹触感；
   - 支持 `Escape` 快捷退出、回车流式保存操作、遮罩点击取消与键盘自动聚焦。

5. **全球 46 种语言 100% 本地化词条对齐**：
   - 全部 46 种语言 JSON 字典补齐 17 项分页相关词条，实现 100% 完整覆盖。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用「更多信息 → 仍要运行」。macOS 解压后请在 Finder 中右键 ReadMD.app，选择「打开」；如仍被阻止，可在「系统设置 → 隐私与安全性」确认打开。

## SHA-256 校验

下载对应文件和 SHA256SUMS.txt 后，核对文件名对应的一行。

Windows PowerShell：

    Get-FileHash .\ReadMDSetup-v2.3.3.exe -Algorithm SHA256

macOS 终端：

    shasum -a 256 ReadMD-macos-arm64-v2.3.3.zip
