# ReadMD v2.3.8 更新说明

ReadMD 是本地优先的 Markdown 阅读、编辑与格式转换工具。本版本在延续 AI Skills、Provider 配置与跨端 Core Service 的基础上，完成 46 种语言的本地化收尾、文档转换与图表渲染的安全加固，并引入统一批量工作台。

## 正式支持矩阵与发布资产

| 系统 | 架构 | 交付物 | 资产文件名 |
| --- | --- | --- | --- |
| Windows 10/11 | x64、ARM64 | 安装版、便携版 | `ReadMDSetup-v2.3.8.exe`、`ReadMD-portable-v2.3.8.exe`（x64）；`ReadMDSetup-arm64-v2.3.8.exe`、`ReadMD-portable-arm64-v2.3.8.exe`（ARM64） |
| macOS 13+ | Intel x64、Apple Silicon ARM64 | 原生压缩包 | `ReadMD-macos-x64-v2.3.8.zip` / `ReadMD-macos-arm64-v2.3.8.zip` |
| Ubuntu 22.04/24.04、Debian 12 | x64、ARM64 | AppImage、Deb | `ReadMD-linux-x86_64-v2.3.8.AppImage` / `ReadMD-linux-aarch64-v2.3.8.AppImage`；`readmd_2.3.8_amd64.deb` / `readmd_2.3.8_arm64.deb` |
| 统信 UOS 20、银河麒麟 V10、Deepin 23 | x64、ARM64 | 目标 Deb | `readmd_2.3.8_amd64.deb` / `readmd_2.3.8_arm64.deb`；真实系统证据完成前不构成正式支持承诺 |
| VS Code | Extension Host 支持的桌面架构 | VSIX | `readmd-vscode-2.3.8.vsix` |
| MCP 客户端 | Python 3.11+ / stdio | MCP ZIP | `readmd-mcp-server-2.3.8.zip` |
| 校验清单 | 全架构 | SHA-256 | `SHA256SUMS.txt` |

本版本经 `READMD_SELF_USE_RELEASE` 自用通道发布（仅 `v2.3.8` tag 生效）：资产未做 Authenticode / macOS codesign 签名，原生平台证据清单按参考口径（informational）提供；多平台原生构建、冒烟自检、隐私扫描与校验和生成照常执行。首次运行如遇系统"未知发布者"提示，请先核对 `SHA256SUMS.txt` 再安装。

HarmonyOS/OpenHarmony、Windows 7/8、LoongArch、MIPS、SW64、RISC-V、Alpine、AUR、Flatpak 和 Linglong 在本版本不属于正式支持范围。`packages/harmonyos-app` 仅保留为未支持的源码预览，不提供功能或兼容性承诺。

## 主要变化

### 新增

- 统一批量工作台：支持批量任务取消与逐行可追溯结果。
- 图表：Chart.js 代码块离线渲染。
- 文档转换：PPTX/XLSX 改用原生 OOXML 读取器，旧 Office 格式给出明确错误提示。
- VS Code 扩展与 MCP 服务：动态技能列表、流式输出取消、核心连接状态展示与 22 项命令覆盖。
- 技能导入升级为来源感知的事务化流程：全量替换失败时自动回滚。
- 更新器集中管理发布选择并按频道感知版本；系统托盘菜单按系统语言本地化。

### 修复

- docx/pdf/OCR/HTML 转换的边界问题；新增纯 Python OLE2/.doc 解析器，`READMD_ENABLE_WORD_COM` 环境变量按严格语义解析，弱化对 Office COM 的依赖。
- AI 生成期间的文本选区保护与派生文档保存授权；Escape 分层关闭浮层、AI 面板事件绑定健壮性；图表选项与无障碍文案保持语言中立。
- 预览自检与数据路径可靠化；打包环境的图表运行时误报；本地 RC 元数据从候选版本派生。
- 修复 41 种语言"请先选中文本"提示词条的英文回退。

### 安全与隐私

- zip 解压全面加固：拒绝路径穿越/UNC/盘符与绝对路径，限制解压总量与条目数（防 zip 炸弹），损坏压缩包不留残留目录，二进制解压需显式确认，请求体读取设上限。
- Vega/Vega-Lite 图表统一由服务端沙箱渲染（拒绝一切外部数据加载），应用 CSP 保持不含 `unsafe-eval`；PlantUML 远程渲染前需用户确认；d2 引擎退役，离线/在线引擎诚实标注。
- 转换与 Web API 的失败信息脱敏。

### 本地化

- 46 种语言全量覆盖收尾：系统托盘菜单、技能导入错误、图表与工作流消息、导出组 EPUB/LaTeX 设置文案等全部界面文案。

### 网站与产品展示

- 官网新增能力影院（124 帧序列）与 GSAP 渐进动效，打磨技能导入体验；v2.3.7 证据海报卡组。

### 工程与质量

- Playwright 浏览器覆盖扩展至 Chromium/Firefox/WebKit；平台证据矩阵与版本绑定；自检与数据路径可靠化；vendored 资源规范化。
- 质量基线：Python 全量单测、三平台 CI 回归、启动预算（`ready < 900ms`、FCP `< 400ms`、传输 ≤ 880,000 bytes）。

## 离线来源与许可证

上游原文随包存放在 `assets/upstream/`，由 `assets/upstream/manifest.json` 固定逐文件 SHA-256。ReadMD 适配层位于 `assets/skills/` 和 `assets/providers/`，与原文严格分离。许可证和归属文件随快照保留；Research Paper Writing 为 ReadMD 独立重写，不复制无许可证来源内容。

## 发布口径（自用通道）

本版本按自用通道发布：`READMD_SELF_USE_RELEASE` 仅对 `v2.3.8` tag 生效，豁免 Authenticode / macOS 公证签名与原生平台证据硬门禁（证据清单以参考口径随包提供）。除此之外的全部常规校验照常执行：多平台原生构建与应用自检、安装器版本一致性、隐私扫描、上游清单与 Provider 目录检查、SHA256SUMS.txt 生成。链接指向的资产在 GitHub Release 创建后方可下载。
