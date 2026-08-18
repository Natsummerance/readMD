<div align="center">

<img src="assets/icon-256.png" width="96" alt="ReadMD logo">

# 📖 ReadMD · 轻量级 Markdown 阅读器

**纯本地 · 秒开 · 离线可用** 的 Windows / macOS Markdown 阅读器。

双击 `.md` 即读，渲染前自动修正常见 Markdown 错误（表格 / 加粗 / 公式 / 标题），**只影响显示，绝不改写原文件**；集成 AI 助手、万物转 MD、扫描 OCR、网页转 MD、主动编辑与移动端共享。

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-0078d6)
![version](https://img.shields.io/github/v/release/Natsummerance/readMD?color=3b6ef5)
![webview2](https://img.shields.io/badge/runtime-WebView2-4fc08d)
![repo size](https://img.shields.io/github/repo-size/Natsummerance/readMD)

</div>

---

## ✨ 特性

- ⚡ **秒开**：安装版为 onedir 目录安装，冷启动窗口可用 ≤1.5s（低配机 / 机械硬盘 ≤2s）；关闭窗口隐藏到系统托盘常驻，再双击 `.md` 瞬时唤起（<0.3s）
- 🎨 **界面清爽**：44px 工具条 + 内联 SVG 图标、欢迎页最近文件网格、浅色 / 暗色 / sepia 三主题全套设计 token、大文档骨架屏、动画遵循系统「减弱动态效果」
- 🤖 **AI 助手（v2.2.0 隐私与自定义连接）**：官方预设 + 可增删改的自定义连接；API Key 仅本机保存且配置接口不回传明文；获取模型后通过下拉框选择，连接面板可拖拽调宽
- 🔄 **万物转 MD（v2.1.1 保存 + 批量 + 质量）**：docx / pptx / xlsx / pdf / html / csv / json 等转为 Markdown；单文件或批量（多选 / 整文件夹）一键转换，结果自动保存到源目录同名 `.md` 并直接以可编辑文件打开（同名默认跳过、可勾选覆盖）；docx 走专用解析（OMML 公式转 LaTeX、标题、表格、等宽代码块），pdf 走专用解析（表格还原 + 公式启发式），其余 MarkItDown 并逐文件回退；输出经严格校验（代码围栏 / 表格 / 公式定界符 / 编码 / 图片引用）
- 🔍 **扫描转 MD（OCR）**：Windows 使用 WinRT、macOS 使用 Vision，均为系统原生离线识别；PDF 有文字层时直接提取
- 🌐 **网页转 MD（v2.2.3）**：Trafilatura 双级抽取，静态正文不足时自动使用 WebView2 / WKWebView + 离线 Defuddle / Mozilla Readability；支持短公告与文档页、同站 1–30 页、显式内网授权、明确错误诊断和可选图片本地化。Windows 支持隔离的交互式抓取；macOS 对公网 HTML 采用断网渲染，避免页面脚本借系统 WebView 访问本机或局域网
- 📄 **独立文件图标**：Windows 文件关联使用简约的 Markdown 文档图标，应用 Logo 仅用于 ReadMD 程序和快捷方式
- ✏️ **主动编辑（v2.2.0）**：单行分组工具栏、可搜索命令面板和分类公式选择器；实时预览支持上下左右停靠及拖拽分隔；图片编辑支持八向裁剪、任意角度、翻转、画布缩放/平移、输出尺寸与撤销重做
- 📱 **移动端共享**：开启局域网共享后，手机扫码在同一 Wi-Fi 下阅读 / 转 MD / OCR / AI（随机令牌鉴权）
- 📑 **阅读体验**：目录侧栏（滚动高亮）、全文搜索、三主题、字号缩放、打印 / 导出 PDF、文件夹浏览、大文档增量渲染（>300KB 或 6000 行分块渲染不卡顿）、文件外部修改自动刷新
- 📤 **导出 PDF / DOCX / HTML（v2.2.3 修复）**：统一兼容 Windows/macOS 保存对话框路径，使用同目录临时文件原子替换；公式在 PDF / DOCX 中本地渲染为图片，HTML 为离线单文件
- 📝 **文件重命名（v2.2.3）**：打开本地文件后，点击顶栏文件名或按 F2 可直接重命名，自动同步最近文件和本地历史引用
- 🛠 **自动修正**：表格缺分隔行 / 列数不齐、未闭合 `**` `__` `*`、未闭合 `$` `$$`、`#标题` 缺空格、BOM、CRLF 等，逐处列出修改
- 🖥 **默认打开方式**：可设为 Windows 默认 `.md` 应用（当前用户级，无需管理员）

## 🚀 快速开始

**方式一：直接下载（推荐）**

不用拉取源码：到 [GitHub Releases](https://github.com/Natsummerance/readMD/releases) 下载：

| 文件 | 说明 |
| --- | --- |
| **ReadMDSetup-版本.exe** | 安装包，动画安装向导，可设为 `.md` 默认打开方式；已安装时运行即升级 |
| **ReadMD-portable-版本.exe** | 便携版，免安装，双击直接运行 |
| **ReadMD-macos-x64-v2.2.3.zip** | Intel Mac 原生 Cocoa/Vision 未签名版 |
| **ReadMD-macos-arm64-v2.2.3.zip** | Apple Silicon 原生 Cocoa/Vision 未签名版 |
| **ReadMDSetup-2.1.1-Beta-win7-x64.exe** | **Win7 兼容版**（v2.1.1 Beta，仅 Win10/11 之外的 Windows 7 SP1 x64 机器使用） |

> 安装包自带 `ReadMDUninstall.exe` 卸载器，卸载时仅移除安装器创建的关联与文件，不动你的文档与配置。
>
> v2.2.3 在同一个 Release 中提供 Windows 安装版/便携版及 Intel/Apple Silicon macOS 包。macOS 包不包含 WinRT 或 Windows 安装器依赖，首次启动需在 Finder 中右键 `ReadMD.app` →“打开”。

**方式二：源码运行（开发 / 自定义）**

环境要求：Windows 10/11（WebView2）或 macOS 12+，Python 3.9+。

```bat
双击 install.bat
```

macOS 源码运行或打包使用独立依赖：

```bash
./install.sh             # 安装 requirements-macos.txt
./setup.sh               # 构建未签名 ReadMD.app
```

脚本会创建 `.venv`、安装依赖并注册 `.md / .markdown / .mdown / .mkd` 文件关联（HKCU，无需管理员）。文件关联使用白色文档页 + 蓝色 MD 标识的独立图标，不会复用应用 Logo。之后直接双击任意 `.md` 文件即可用 ReadMD 打开；或：

```bat
run.bat                              rem 一键运行
.venv\Scripts\pythonw.exe readmd.py  rem 打开文件 / 空启动
python readmd.py --browser "文件.md"  rem 无 pywebview 时用浏览器兜底
```

> 若 Windows 仍用其他程序打开：右键 `.md` → 打开方式 → 选择 ReadMD → 始终使用；或点击阅读器工具栏「设为默认」。程序化修改默认应用受 Windows UserChoice 哈希保护，此路径已是最佳实践。

## 🪟 Win7 兼容版（v2.1.1 Beta）

> 正式版（v2.1.1）基于 Python 3.10 + pywebview 6.x，**不支持 Windows 7**。为仍在使用 Win7 的机器提供独立的 **v2.1.1 Beta** 兼容版，独立发布（pre-release tag `v2.1.1-beta`），不影响正式版。

**适用环境**：Windows 7 SP1 x64（需 .NET Framework 4.8 与 VC++ 运行库，详见 Release 说明）；安装包内嵌 **固定版 WebView2 109 运行时**（Win7 最后支持线），安装时自动放入安装目录，无需联网安装系统级运行时。

**与正式版一致的能力**：秒开（onedir 目录安装 + 单实例托盘）、浅色 / 暗色 / sepia 三主题阅读、自动修正、目录 / 搜索 / 编辑 / 导出 PDF·DOCX·HTML / 打印、docx / pdf 转 Markdown（含自动保存与严格校验）。

**Win7 版暂不支持**（入口会明确提示）：OCR（依赖 WinRT，仅 Win10+）、AI 助手、网页转 MD、以及 docx / pdf 以外的格式转换。

**构建**：独立 Python 3.9.13 构建链（`.venv-win7` + `win7-reqs.txt` + `build_win7.bat`），不污染正式版发布链。

## 🖱️ 使用

### 快捷键

| 快捷键 | 功能 |
| --- | --- |
| Ctrl+O | 打开文件 |
| Ctrl+U | 网页转 MD |
| Ctrl+E | 编辑当前 MD（Ctrl+S 保存） |
| Ctrl+F | 搜索（Enter 下一个 / Shift+Enter 上一个 / Esc 关闭） |
| Ctrl+Shift+F | 目录侧栏 |
| Ctrl+Shift+A | AI 助手面板 |
| Ctrl+Shift+P | 编辑模式下打开 Markdown 命令面板 |
| Ctrl+D | 切换主题 |
| Ctrl+= / Ctrl+- | 增大 / 减小字号 |
| Ctrl+R | 重新加载 |
| Ctrl+P | 打开导出面板（PDF / DOCX / HTML / 打印，含样式定制） |
| Ctrl+← / Ctrl+→ | 历史后退 / 前进 |

### 常用操作

- **打开**：工具栏「打开」或 Ctrl+O；「文件夹」浏览整个目录逐个阅读
- **最近文件**：欢迎页最近文件网格，一键回到上次阅读的位置
- **转换 / OCR / 网页**：可在「更多」菜单中找到；结果为虚拟文档，可「另存」为 `.md`
- **导出**：工具栏「导出」按钮或 Ctrl+P 打开导出面板，选格式与样式预设，一键导出 PDF / DOCX / HTML；导出后可「打开」或「所在文件夹」
- **修复**：渲染时自动修正会在「🛠 修复」面板中列出每一处修改
- **托盘**：关闭窗口即隐藏到系统托盘（再次打开瞬时）；托盘菜单「显示 / 打开文件 / 退出」

## 🧠 核心能力详解

### 自动修正

修正均为**保守启发式**，只发生在内存渲染阶段，并在「🛠 修复」面板中列出每一处修改：

- **表格**：检测连续的竖线行，缺少 `|---|` 表头分隔行时自动补全；各列不足时补齐空单元格；分隔行不足 3 个连字符时补足
- **加粗**：`**文字` 补全为 `**文字**`；游离的结束符转义为字面量；`2 * 3` 这类乘号转义；列表 `* 项` 与分隔线 `***` 不受影响
- **公式**：`$x^2$ 和 $y` 补全为 `$y$`；`$$` 块级公式未闭合时补 `$$`；`价格 $5` 这类货币不会被误判；代码块与行内代码内的内容一律跳过
- **标题**：`#标题` → `# 标题`

### 万物转 MD / 扫描 OCR / 网页转 MD

- **文件转换**：工具栏「转换」选择任意文件（或直接 `python readmd.py 文件.docx`），MarkItDown 转成 Markdown 后自动过修正器并渲染；未提取到文字时提示改用 OCR
- **扫描 / 图片**：工具栏「OCR」选择图片或 PDF；Windows 内置 OCR 离线识别（需系统已安装对应语言包，中文一般自带）；扫描版 PDF 逐页渲染后识别
- **网页**：工具栏「网页」输入 URL；默认智能提取正文，静态页面内容不足时桌面版会自动通过系统 WebView 动态渲染。可切换“保留完整页面”、合并同站最多 10 页，或选择把安全的远程图片下载到本地资源目录
- **网页限制**：仅处理公开 HTTP/HTTPS 页面，不绕过登录、验证码和付费墙；本机、局域网及云元数据地址会被安全策略阻止。局域网共享页面可使用静态抓取，动态渲染需在桌面应用中执行
- 转换 / OCR / 网页模块均为**首次渲染完成后的后台懒加载**，不影响 Markdown 阅读的启动速度

### 大文档增量渲染

- 超过 **300KB 或 6000 行** 的文档自动进入**增量分块渲染**：正文按围栏代码块 / 空行切成小块逐帧渲染，顶部显示「渲染中… N%」，可边渲染边滚动阅读
- 代码块与公式跨块保护不拆分，渲染完成后统一生成目录 / 搜索索引 / 公式排版，滚动位置自动恢复
- 小文档仍为一次性整篇渲染，启动速度不受影响

### 主动编辑（CodeMirror 6）

- 行号、括号配对、自动缩进、代码折叠、语法高亮（Markdown + 内嵌代码语言）、亮 / 暗主题跟随
- 输入 `#` `*` `` ` `` `[` `>` `|` `~` 等触发 18 种 Markdown 语法补全，选中即插入并定位光标
- 工具栏一键插入：加粗 / 斜体 / 删除线 / 标题 / 引用 / 列表 / 任务 / 链接 / 图片 / 行内代码 / 代码块 / 公式 / 表格 / 分隔线
- **插入图片**：本地图片可在画布上裁剪（自由 / 1:1 / 4:3 / 16:9）、旋转（90° / 任意角度）、缩放（10%~300%），导出 PNG 保存到文档同目录 `images/` 并以相对路径插入
- CodeMirror 已离线打包（`assets/vendor/codemirror.bundle.js`），仅首次进入编辑模式时加载，不影响阅读秒开

### AI 助手

- **提供商**：OpenAI / DeepSeek / Kimi / 智谱 GLM / 通义千问 / 硅基流动 / OpenRouter / Groq / xAI / Mistral / Gemini / 火山方舟 / 腾讯混元 / Ollama（本地）/ Anthropic 等公开预设，并支持完全自定义连接
- **API Key**：面板中填写即保存到本机（`%APPDATA%\ReadMD\ai.json`）；留空时自动读取环境变量（如 `DEEPSEEK_API_KEY`），无需重复填写
- **自定义**：可直接修改任意预设的地址 / 模型 / Key；兼容 OpenAI Chat Completions 与 Anthropic Messages 双协议，绝大多数聚合网关 / NewAPI / One API 可直接使用
- **动作**：快速阅读、润色、修改、扩充、续写、翻译、提问；默认处理全文，可勾选「仅处理选中文字」
- **模板**：内置 14 个常用模板（总结要点 / 生成周报 / 生成大纲 / 代码审查 / 修正格式等），支持新建 / 编辑 / 删除与 `{doc}` `{prompt}` 占位符
- **历史会话**：多轮上下文自动累积，可保存 / 恢复（最多 50 个会话 / 60 条消息）
- **落地**：流式渲染结果可「应用到文档」（进入编辑审阅后 Ctrl+S 保存）、「复制」或「另存为」

### 移动端共享

- 点工具栏「📱」开启共享，弹出二维码；手机连同一 Wi-Fi 扫码即可阅读当前文档，并使用转 MD / OCR / 网页 / AI 全部功能
- 每次开启生成随机访问令牌；局域网 API 请求（除页面与静态资源外）均需携带令牌，关闭共享即失效
- 命令行方式：`python readmd.py --share` 启动时即开启共享


### 导出 PDF / DOCX / HTML（v2.1.0）

- **入口**：工具栏「导出」按钮或 `Ctrl+P`，打开导出面板；面板内也可直接「打印当前文档」
- **三种格式**：PDF（reportlab，中文微软雅黑 + 表格 / 代码块 / 封面 / 目录 / 页码）、DOCX（python-docx，标题用 Word 内置 Heading 样式，导航窗格可直接定位）、HTML（单文件自包含，内联 marked + MathJax，任何浏览器离线打开）
- **样式定制**：内置「简约 / 经典 / 商务」预设，也可全量可视化调整——纸张大小 / 方向 / 页边距、封面与目录、正文与各级标题（颜色 / 字号 / 加粗 / 对齐）、表格（表头颜色 / 边框 / 斑马纹 / 单元格字号）、代码块、引用、链接、页脚页码、PDF 元数据、HTML 亮 / 暗 / 米色主题；「存为预设」后可在下拉中复用
- **公式与图片**：`$...$` / `$$...$$` 公式在 PDF / DOCX 中由本地 matplotlib 渲染为图片（离线、无需 LaTeX）；HTML 导出保留 MathJax 完整渲染；本地图片按文档目录解析并嵌入（缺失自动跳过并提示）
- **说明**：导出使用当前文档内容（文件模式含未保存的编辑；转换 / OCR / 网页结果同样可导出）；样式参数自动记忆
## 📦 目录结构

```
readmd/
├─ readmd.py            # 主程序（本地服务 + 窗口 + 单实例托盘 + 里程碑打点）
├─ readmd_fix.py        # 自动修正器（纯标准库）
├─ readmd_fix_test.py   # 修正器测试（37 项，python readmd_fix_test.py）
├─ readmd_modules/      # 懒加载扩展模块
│  ├─ convert.py        #   万物转 MD（MarkItDown）
│  ├─ ocr.py            #   扫描转 MD（WinRT OCR + PyMuPDF）
│  ├─ web.py            #   网页转 MD（安全下载 + Trafilatura / Readability / 图片本地化）
│  └─ ai.py             #   AI 助手（双协议 + 提供商注册表）
├─ DESIGN.md            # 设计规范（色盘 / 字体 / 间距 / 圆角 token）
├─ installer/           # 安装器（动画 UI + onedir 目录安装）
│  ├─ setup_app.py      #   安装 / 卸载 / 静默模式主程序
│  ├─ setup.html        #   动画界面（毛玻璃 / 弹簧动效 / 极光背景）
│  └─ build_setup.bat   #   构建 ReadMDSetup.exe + ReadMDUninstall.exe
├─ assets/
│  ├─ index.html        # 界面骨架
│  ├─ style.css         # 阅读主题 + 移动端响应式
│  ├─ app.js            # 渲染 / 目录 / 搜索 / 公式 / AI / 转换 / 编辑
│  ├─ readmd.ico        # 多尺寸应用图标（16~256）
│  └─ vendor/           # marked + MathJax + qrcode + codemirror.bundle（全部离线）
├─ package.bat          # 一键打包（onedir 安装版 + 便携单文件）
├─ setup.bat            # 一键：打包 + 注册默认打开 + 启动
├─ install.bat          # 一键安装依赖 + 注册文件关联
├─ run.bat              # 一键运行（venv pythonw）
├─ uninstall.bat        # 移除文件关联（保留备份）
├─ deploy.bat           # ★一键部署：测试 → 打包 → 推送 → 发布 Release
├─ release.py           # GitHub Release 工具（--verify / --update / --force-upload）
└─ release_notes.md     # Release 发布说明（deploy.bat 自动读取）
```

## 🔨 打包 / 一键安装

| 脚本 | 作用 |
| --- | --- |
| `run.bat` | 一键运行（venv pythonw，秒开） |
| `install.bat` | 安装依赖 + 生成图标 + 注册 `.md` 关联 |
| `package.bat` | 一键打包：onedir 安装版 `dist\ReadMD\ReadMD.exe` + 便携单文件 `dist\ReadMD-portable.exe` |
| `setup.bat` | 一键完成：装依赖 → 打包 onedir exe → 注册默认打开方式 → 启动 |
| `uninstall.bat` | 移除关联并尝试恢复安装前备份 |
| `installer\build_setup.bat` | 构建 `dist\ReadMDSetup.exe`（内嵌 onedir 目录）与 `dist\ReadMDUninstall.exe` |
| `release.py` | 创建 GitHub Release 并上传安装包 / 便携版，校验 SHA256 |

> 安装版为目录安装（约 200MB，含 OCR 与转换依赖，冷启动无需解压、秒开）；便携版为单文件（首次启动需解压、稍慢）。日常开发推荐源码版（`install.bat` + `run.bat`）。

## 🌍 一键部署与发布

环境要求：已安装 Git，并配置系统环境变量 `GITHUB_TOKEN`（仓库 `Natsummerance/readMD` 需 `repo` 权限）。

```bat
deploy.bat                 rem 完整流程：测试 → 打包 → 推送 → 发布 Release
deploy.bat --skip-build    rem 复用已有 dist 产物，只跑测试 + 推送 + 发布
deploy.bat --skip-tests    rem 跳过自测
deploy.bat --tag v2.0.1    rem 指定发布标签（默认 v2.0.1）
```

也可以单独使用 `release.py`：

```bat
python release.py --verify             rem 校验线上与本地产物一致性（名称 / 大小 / SHA256）
python release.py --update             rem 更新已存在 Release 的标题与说明（读 release_notes.md）
python release.py --force-upload       rem 同名资产先删除再重传
python release.py                      rem 创建 Release（已存在则跳过）+ 上传缺失资产
```

## 📝 更新日志

- **v2.1.0**：新增「导出」——PDF / DOCX / HTML 一键导出；导出面板（打印按钮升级）：内置「简约 / 经典 / 商务」样式预设 + 全量可视化定制（页面 / 标题 / 表格 / 代码块 / 引用 / 页码等），可保存自定义预设；公式在 PDF / DOCX 中本地渲染为图片，HTML 为单文件离线可开；图片按文档目录嵌入
- **v2.0.1（安装器修复）**：移除安装包 / 卸载器的 PyInstaller 启动画面，修复低配机黑屏置顶弹窗卡死安装流程的问题；安装版本号统一为 2.0.1
- **v2.0.0**：秒开（onedir 目录安装 + 单实例托盘常驻）；界面全面改版（44px 工具条 / SVG 图标 / 欢迎页最近文件网格 / 三主题设计 token）；大文档骨架屏与无障碍优化
- **v1.4.0**：插入图片（裁剪 / 缩放 / 旋转）；苹果风动画安装器

## 🗑️ 卸载

- **安装版**：「设置 → 应用」中找到 ReadMD 卸载；或运行安装目录中的 `ReadMDUninstall.exe`
- **源码版**：双击 `uninstall.bat` 移除文件关联（并尝试恢复安装前的 `.md` 关联备份），`.venv` 与 `readmd` 文件夹保留，可手动删除

## ❓ 常见问题

- **提示未安装 pywebview**：运行 `install.bat`，或在 PowerShell 执行 `python -m pip install pywebview`
- **打开时报 WebView2 相关错误**：系统缺少 Edge WebView2 运行时，下载安装 https://developer.microsoft.com/microsoft-edge/webview2/ 后重试
- **启动闪一下控制台**：双击 `.md` 走的是 `pythonw.exe`（无控制台）；手动用 `python readmd.py` 运行出现控制台属正常现象
- **为什么打开这么快**：安装版是 onedir 目录安装，启动无需解压；窗口创建约 0.1s，页面由常驻实例秒级唤起
- **安全性说明**：Markdown 中的原始 HTML（如 `<script>`）会按原样渲染，与大多数阅读器一致，仅建议打开可信文件

---

<div align="center">

**ReadMD** · 纯本地优先，你的文档不出本机。

</div>
