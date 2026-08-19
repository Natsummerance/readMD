# ReadMD v2.3.0

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- Windows 安装版：`ReadMDSetup-v2.3.0.exe`
- Windows 便携版：`ReadMD-portable-v2.3.0.exe`
- Intel Mac：`ReadMD-macos-x64-v2.3.0.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.0.zip`
- 校验清单：`SHA256SUMS.txt`

## 本次更新

1. **软件内更新器排查与关键修复**：
   - 修复在应用内下载完安装包点击安装后卡死的问题：通过后台线程延迟主动退出主进程释放 Windows 文件句柄锁；
   - 启动时自动扫描并清理 `%TEMP%` 历史残留安装包；
   - 引入 GitHub 镜像源毫秒级智能降级与手动检查更新 Loading 状态反馈。
2. **全球 45+ 语种 i18n 体系与多模型自动翻译工具链**：
   - 首次启动自动侦测宿主操作系统语言（精准识别繁中台/港、简体中文、英文、日韩西法德等）；
   - 支持 46 个全球语种（涵盖西欧、东亚、东南亚、阿拉伯语/希伯来语 RTL 双向排版，以及藏语、维吾尔语、蒙古语等少数民族语言）；
   - 提供基于 Google Translate 与 OpenAI 兼容协议（DeepSeek / Qwen / Mimo / GLM）的多模型自动化翻译与字典校验工具链。
3. **LaTeX PRO 学术增强引擎**：
   - 零配置自动扫描同目录 `.bib` 参考文献文件，自动解析 BibTeX 并生成浮动卡片交互与文末 References 引用；
   - 支持定理 (Theorem)、引理 (Lemma)、证明 (Proof with Q.E.D.)、定义 (Definition)、推论 (Corollary) 等学术 Callout 盒子。
4. **Editor Studio PRO 极致编辑体验**：
   - **Zen Mode 沉浸禅模式**：一键切换或按 F11 / Esc 隐藏所有工具栏与侧栏，专注于深度写作；
   - **10x10 可视化表格网格设计器**：鼠标滑选行列一键插入格式规范的 Markdown 表格；
   - **智能 Excel / CSV 粘贴转换**：从 Excel、WPS、Numbers 或网页复制表格直接粘贴即转为高质量 Markdown 表格；
   - **实时文档统计**：编辑栏实时显示字数、词数与预计阅读时间。
5. **VSCode 插件与 MCP Server 分包架构**：
   - 采用 Monorepo 统一分包架构（`packages/mcp-server` & `packages/vscode-extension`）；
   - 为 Claude Desktop、Cursor、Antigravity 提供标准 FastMCP 工具支持，客户端安装包零体积冗余。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用“更多信息 → 仍要运行”。macOS 解压后请在 Finder 中右键 `ReadMD.app`，选择“打开”；如仍被阻止，可在“系统设置 → 隐私与安全性”确认打开。

## SHA-256 校验

下载对应文件和 `SHA256SUMS.txt` 后，核对文件名对应的一行。

Windows PowerShell：

```powershell
Get-FileHash .\ReadMDSetup-v2.3.0.exe -Algorithm SHA256
```

macOS 终端：

```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.0.zip
```
