# ReadMD v2.3.9 更新说明

ReadMD 是本地优先的 Markdown 阅读、编辑与格式转换工具。v2.3.9 在延续本地优先、全语种母语化与格式转换能力的基础上，正式推出 **CC0 许可的 Arch-Chan Live2D 与 73 款 Petdex 精灵图桌面伴读宠物系统（含活态微呼吸自适应引擎与全端台词联动）**，重构 **纯正苹果风原生拖动条（Apple HIG 4px 细轨道与 18px 物理触觉旋钮）**，新增 **原生矢量高保真 PDF 编辑器及 FastMCP 21 项扩展工具链**，强化 **纯 Python Word .doc 二进制流（FIB/CLX）与复杂表格聚类重构**，达成 **46 种语言 100% 字典对齐（1563 词条零裸露、零空值、零英文照搬）**，并实现 **打包彻底瘦身至 28.2MB 与 2 秒极速冷启动**。

## 正式支持矩阵与发布资产

| 系统 | 架构 | 交付物 | 资产文件名 |
| --- | --- | --- | --- |
| Windows 10/11 | x64、ARM64 | 安装版、便携版 | `ReadMDSetup-v2.3.9.exe`、`ReadMD-portable-v2.3.9.exe`（x64）；`ReadMDSetup-arm64-v2.3.9.exe`、`ReadMD-portable-arm64-v2.3.9.exe`（ARM64） |
| macOS 13+ | Intel x64、Apple Silicon ARM64 | 原生压缩包 | `ReadMD-macos-x64-v2.3.9.zip` / `ReadMD-macos-arm64-v2.3.9.zip` |
| Ubuntu 22.04/24.04、Debian 12 | x64、ARM64 | AppImage、Deb | `ReadMD-linux-x86_64-v2.3.9.AppImage` / `ReadMD-linux-aarch64-v2.3.9.AppImage`；`readmd_2.3.9_amd64.deb` / `readmd_2.3.9_arm64.deb` |
| 统信 UOS 20、银河麒麟 V10、Deepin 23 | x64、ARM64 | 目标 Deb | `readmd_2.3.9_amd64.deb` / `readmd_2.3.9_arm64.deb`；真实系统证据完成前不构成正式支持承诺 |
| VS Code | Extension Host 支持的桌面架构 | VSIX | `readmd-vscode-2.3.9.vsix` |
| MCP 客户端 | Python 3.11+ / stdio | MCP ZIP | `readmd-mcp-server-2.3.9.zip` |
| 校验清单 | 全架构 | SHA-256 | `SHA256SUMS.txt` |

本版本经 `READMD_SELF_USE_RELEASE` 自用通道发布（仅 `v2.3.9` tag 生效）：资产未做 Authenticode / macOS codesign 签名，原生平台证据清单按参考口径（informational）提供；多平台原生构建、冒烟自检、隐私扫描与校验和生成照常执行。首次运行如遇系统"未知发布者"提示，请先核对 `SHA256SUMS.txt` 再安装。

HarmonyOS/OpenHarmony、Windows 7/8、LoongArch、MIPS、SW64、RISC-V、Alpine、AUR、Flatpak 和 Linglong 在本版本不属于正式支持范围。`packages/harmonyos-app` 仅保留为未支持的源码预览，不提供功能或兼容性承诺。

## 主要变化

### 新增功能 (Features)

- **Live2D 与 73 款 Petdex 精灵图桌面伴读宠物系统**：
  - 集成 CC0 许可的 Arch-Chan Live2D 模型，基于 PixiJS 运行时与 `devicePixelRatio` 高 DPI 视网膜屏幕自适应缩放；
  - 核心几何自动推断算法：使用纯 Python `struct` 极速解析精灵图头部尺寸，智能识别 4×2、8×11（Petdex 高密序列）与单立绘小图，彻底杜绝切片越界与撕裂错位；
  - 桌面 Canvas 活态微呼吸动效：单立绘立绘以脚底为锚点做周期 3.2 秒的正弦波柔和微缩放与浮动，注入生动生命感；
  - 伴读伴侣生活状态引擎：支持等级、体力、心情、亲密度持久化成长与行为冷却机制；
  - 全端对话气泡与快捷动作打通：摸摸头、喂食、玩耍、休息、唤醒动作实时联动桌面悬浮窗、应用内小部件与设置舞台三端对话气泡，Electron 右键菜单与后端双向同步；
  - 双向进程生命周期看门狗（`READMD_PARENT_PID` + `isHostProcessAlive` 宿主失活自动安全退出）；
  - 原生穿透与平滑连续拖拽，动态窗口边界同步与高灵敏点击命中测试（hit testing）；
  - 支持拖拽文件直接扔给桌宠打开阅读；
  - 气泡系统：持久化优先级保护、平滑淡入淡出透明度区间截断（opacity range clamping）、队列批处理隔离（`receivePetBatch` 异常隔离防阻塞）；
  - 插件包搜索路径扩充与 zip fallback 保护（防御 `pet_plugin_bundle_missing`）；
  - 守护进程桥接：`HermesPetBridge` 原子化文件命令置换，防止并发指令丢失。
- **原生高保真矢量 PDF 编辑器与 MCP 21 项工具链**：
  - 新增 `readmd_modules.pdf_editor` 原生矢量 PDF 编辑引擎，支持文本层高保真修复与标注；
  - 扩充 ReadMD MCP Server 工具集至完整的 21 项工具：增加 `readmd_pdf_audit`（结构与底色噪点采样）、`readmd_pdf_preview_edit`（沙箱预览差分质检）、`readmd_pdf_apply_edit`（物理更新与 .bak 备份）、`readmd_pdf_rollback`（一键回滚），全面赋能外部 Agent 集成；
  - MCP 协议健壮性：严格校验 JSON-RPC 请求参数（非法参数保留 `req_id` 并返回标准错误码 `-32602`），修复深层配置合并，增加防 TOCTOU 竞争条件的符号链接与排他写入防护。
- **插件中心现代化与离线安装支持**：
  - 为 PyInstaller 冻结环境引入 `distlib` wheel 离线安装器；
  - 内置隔离的 pip 依赖运行环境；
  - 显式持久化插件启用/禁用/卸载状态，提供结构化错误反馈与安装进度。
- **复杂文档解析与转换增强**：
  - 纯 Python 解析 Word .doc 二进制流（FIB/CLX 数据流），彻底摆脱 Windows Office COM 绑定；
  - 引入 flyingmouse 复杂表格聚类算法，实现跨行跨列 Markdown 表格的高精度结构重建；
  - EPUB 与 LaTeX 数学公式渲染管线增强，修复多行块级公式首行丢失问题；
  - 增强型 OCR 回退机制，对低对比度或扫描类 PDF 提供自动降级补偿。

### 修复与改进 (Fixes)

- **VS Code 扩展深度优化**：解决 bugs 001-008，引入快照保护机制、内置容错 JSONC 解析器、UTF-8 解码健壮性增强、39 项单元测试通过与现代 Webview 适配。
- **阅读器交互与边界加固**：`loadFile` 强制重载的空指针保护（`existingTab` 判空防护，防止 `browserCopy TypeError`）；代码块结束反引号行严格匹配。
- **国际化与多语言深度治理（100% 词条母语化对齐）**：
  - 基准字典扩充并规范至 **1563 词条**，46 种语言 100% 键位 Parity；
  - 彻底消除非英语语言中的英文照搬与占位符，达成 **0 裸露 key、0 空值、0 未授权英文复制**；
  - 前端 HTML 模板原生中文 100% 绑定 `data-i18n*`，彻底消除硬编码与缺失属性。
- **轻量秒开与打包优化**：
  - 严格排除重型 AI 与计算库（torch, paddle, cv2, scipy, numba 等），可执行文件由 122MB 瘦身至 28.2MB；
  - 彻底解决冷启动卡顿问题，Windows 窗体建立时间缩短至 85ms，完整冷启动耗时缩短至 2.0 秒。

### 安全与内核审计 (Security & Audit)

- **进程控制与代码执行沙箱**：
  - Windows Job Object FFI 显式声明 64 位指针类型（HANDLE），防止 32 位句柄截断导致的 FFI 调用失败；
  - POSIX 选择器管道优化：遇到管道提前 EOF 且进程未超时，使用剩余截止时间安全等待，防止正常进程被误杀；
  - SQL 执行沙箱隔离：状态机词法分析器支持标准 SQL 引号语法；`query.sql` 写入移至 `try...finally` 清理块，杜绝临时目录泄漏；
  - 临时执行脚本目录创建异常时自动回滚清理（`shutil.rmtree`）。
- **导入处理器（`import_processor`）边界加固**：
  - 当 `max_output_bytes` 极小时对错误标签进行安全 UTF-8 字节截断，严格遵守输出字节预算限制；
  - 行号属性解析增加 `isascii() and isdigit()` 防护，拦截 Unicode 数字上标（如 `²`）引发的未捕获 `ValueError`；
  - 不等宽 CSV 转 Markdown 以最大列数自动对齐补齐空列，杜绝超出表头的列数据静默丢失；
  - 递归与嵌套保护：块引用深度限制（最大 64 层），防止深度嵌套导致递归栈溢出。

### 前端规范与体验 (UI & Experience)

- **纯正苹果原生质感拖动条重塑 (/apple-design & Apple HIG)**：
  - 彻底抛弃笨重臃肿样式，采用 4px 极简精致轨道（`border-radius: 999px`），亮色模式活力蓝、深色模式通透半透明磨砂轨道；
  - 18px 实体金属/塑料微环境光泽悬浮圆钮，Active 拖动触觉能量光晕反馈；
  - San Francisco 等宽排版数字药丸读数（`tabular-nums`），拖动实时平滑反馈无抖动。
- **桌宠设置工作台空间精致化重排**：
  - 彻底解决底栏遮挡与 120px 巨大留白死区，双轴拉伸自适应；
  - 角色库操作栏（搜索、图鉴下拉、导入与删除）置顶排列；
  - 支持角色即时搜索与收藏置顶（Star Favorites）。
- **前端核心 UI 全面去硬编码 Emoji**：
  - CSS 伪元素、关系图谱弹窗、反向链接抽屉、工具栏及更多菜单中的硬编码 Emoji 全面升级为内联高精度矢量 SVG 与自适应图标系统；
  - 严格规范化保留 `index.html` 与 `ai.js` 既有基础排版符号（`&#9789;`、`✕`、`⏸`、`⚙`）；
  - 伴读宠物对话表情纳入 46 种语言 100% 动态调度覆盖。

### 质量与测试工程 (Engineering & Tests)

- 补充完整的审计测试套件（`test_audit_001.py` ~ `008.py` 及 `test_audit_batch1_*.py` 共 58 项审计相关测试用例）；
- 全量 1079 项 Python 单元与集成测试套件通过；
- 自动化录制套件（Playwright v239 full coverage）覆盖桌宠、插件中心、万能格式转换与知识图谱。

## 离线来源与许可证

上游原文随包存放在 `assets/upstream/`，由 `assets/upstream/manifest.json` 固定逐文件 SHA-256。ReadMD 适配层位于 `assets/skills/` 和 `assets/providers/`，与原文严格分离。许可证和归属文件随快照保留；Live2D 模型遵循 CC0 协议。

## 发布口径（自用通道）

本版本按自用通道发布：`READMD_SELF_USE_RELEASE` 仅对 `v2.3.9` tag 生效，豁免 Authenticode / macOS 公证签名与原生平台证据硬门禁（证据清单以参考口径随包提供）。除此之外的全部常规校验照常执行：多平台原生构建与应用自检、安装器版本一致性、隐私扫描、上游清单与 Provider 目录检查、SHA256SUMS.txt 生成。链接指向的资产在 GitHub Release 创建后方可下载。
