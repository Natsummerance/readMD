# ReadMD v2.3.7-beta.1 (修复与预览版)

ReadMD 是免费、开源的本地 Markdown 智能阅读、编辑与全格式转换排版套件；纯本地离线可用、秒级极速渲染，绝不改写原文件。

## 📦 全平台发布资产 (Release Assets)

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.7-beta.1.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.7-beta.1.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.1.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.1.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.1.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包 (UOS / 银河麒麟 / Deepin / Ubuntu / Debian)：`readmd_2.3.7-beta.1_amd64.deb`
- 📱 HarmonyOS NEXT 纯血鸿蒙安装包：`ReadMD-harmonyos-v2.3.7-beta.1.hap`
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.1.vsix`
- 🤖 FastMCP Server 独立包：`readmd-mcp-server-2.3.7-beta.1.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

---

## 🌟 本次版本核心修复与优化 (v2.3.7-beta.1)

### 1. 顶栏交互升级与禅模式 (Zen Mode) 全局常驻
- **沉浸专注体验**：从编辑工具栏剥离独立的禅模式，在顶部主工具栏右侧外观区域（主题与字号旁）新增常驻按钮 `#btn-zen`（专属全屏图标与 Tooltip）；
- **全场景一键直达**：在阅读模式与编辑模式下均可一键开启沉浸阅读，支持 `Esc` 或 `F11` 极速退出。

### 2. 侧边「更多」菜单重构与文档状态动态联动
- **极简手风琴折叠卡片**：将 15 个散乱选项重构为三组清晰折叠模块（「导入」「互动」「设置」），彻底告别界面拥挤与滚动冗余；
- **状态强联动防护**：在未打开文档（欢迎页）时，文档依赖功能（`演讲演示`、`运行代码`、`另存为`、`修复详情`、`扫码共享`）自动进入禁用状态，打开文档后实时激活；
- **文案精简规范**：原「运行全部代码块」精简为「**运行代码**」（副标题：执行代码块）。

### 3. Reveal.js 演说模式深度重构
- **长文档自动智能分片**：彻底修复长文档进入演示模式仅剩 3 页且内容丢失的缺陷，支持按 `H1`/`H2`/`H3` 标题及段落长度阈值平滑分片；
- **模板语法保真渲染**：修复 `<textarea data-template>` 破坏性实体转义，完整保留代码块、HTML 标签、样式及 LaTeX 数学公式；
- **多级幻灯片语法支持**：完整支持标准 `---`（横向主页）、`--` 与 `<!-- subslide -->`（垂直下钻）与 `<!-- note -->` 演讲者备注。

### 4. PDF 转 Markdown 高级排版引擎加固
- **智能字号统计分级**：计算页面众数字号基准，自适应映射为 `# 标题1`、`## 标题2`、`### 标题3`，根治字体忽大忽小问题；
- **等宽代码块检测与封装**：精准识别 `Courier`、`Consolas`、`Monaco`、`FiraCode` 等等宽代码段并自动包裹为 Markdown 围栏代码块；
- **智能段落重组 (De-hyphenation & Reflow)**：识别跨行断句与连字符折行，平滑拼接段落（区分中英文空格），保留列表与表格结构；
- **拖拽/选择文件真实命名保持**：拖入或选择文件转换生成的虚拟标签页与保存文件名严格等于原始文件名（如 `报告.pdf` -> `报告.md`），彻底杜绝哈希随机乱码。

### 5. 自定义样式 (CSS/Head) 弹窗标准重构
- **视觉风格统一**：重构为标准系统弹窗规范，与整体 UI 保持高度一致；
- **高级排版注入**：完整支持全局注入自定义 CSS 规则与 HTML Head 标签，即时生效于实时预览与导出排版。

### 6. 全球 46 国语言 i18n 与前端 UI 自动化测试 100% 覆盖
- **46 种语言 100% 同步**：所有新增、精简键名全部补齐，实现 0 缺失；
- **自动化测试全量通过**：338 项 Python 核心单测与 23 项 Playwright UI 端到端测试 100% 通过。

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
Get-FileHash .\ReadMDSetup-v2.3.7-beta.1.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.1.zip
```

