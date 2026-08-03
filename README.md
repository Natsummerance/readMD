# ReadMD · 轻量级 Markdown 阅读器

一个**纯本地、秒开、离线可用**的 Markdown 阅读器。双击 `.md` 文件即可阅读，
渲染前会自动修正常见 Markdown 错误（表格、加粗、公式、标题），**只影响显示，绝不改写原文件**。

## 功能

- ⚡ **快**：打开 .md 秒开——先渲染正文（marked + MathJax 全离线），**渲染完成后再在后台懒加载**转换/OCR/网页模块，不影响阅读速度
- 🤖 **AI 助手**：接入外部大模型 API（OpenAI / DeepSeek / Kimi / 智谱 GLM / 通义千问 / 硅基流动 / OpenRouter / Groq / xAI / Ollama / Anthropic 等 15+ 预设，并兼容 cc-switch 里的自定义提供商），支持快速阅读、修改、扩充、续写、润色、翻译、提问；流式输出，结果可一键应用到文档；提供商与 API Key 完全自定义（Key 也可读系统环境变量）；内置 14 个 Prompt 模板（总结要点 / 生成周报 / 大纲 / 代码审查 / 翻译成英文等）且可自定义增删改；多轮对话可保存为历史会话，随时恢复续聊
- 📱 **移动端共享**：开启局域网共享后，手机扫码即可在同一 Wi-Fi 下阅读 / 转 MD / OCR / AI（随机令牌鉴权，关停即失效）
- 📦 **一键打包安装**：`run.bat` 运行 · `package.bat` 打包单文件 exe · `setup.bat` 打包 + 安装 + 设为默认 + 启动
- 🛠 **自动修正**：表格缺分隔行 / 列数不齐、未闭合的 `**` `__` `*`、未闭合的 `$` `$$` `\(` `\)`、`#标题` 缺空格、BOM、CRLF 等
- 🔄 **万物转 MD**：docx / pptx / xlsx / pdf / html / csv / json 等一键转为 Markdown（基于 MarkItDown），转换结果自动过修正器
- 🔍 **扫描转 MD（OCR）**：图片、扫描件 PDF 用 Windows 10/11 内置 OCR（离线、免费、无次数限制，支持中文）；PDF 有文字层直接提取，无文字层逐页渲染 OCR
- 🌐 **网页转 MD**：输入 URL 抓取正文（trafilatura），支持勾选“批量爬取”同站链接最多 10 页合并为一个文档
- ✏️ **编辑 MD**：Ctrl+E 进入编辑模式，Ctrl+S 保存（首次保存自动生成 `.bak` 备份），保存后自动重新渲染
- 📑 阅读体验：目录侧栏、全文搜索（Ctrl+F）、亮/暗/护眼三主题、字号缩放、打印/导出 PDF
- 📂 文件夹浏览：打开整个文件夹，侧栏列出全部 Markdown 逐个阅读
- 🚀 **大文档增量渲染**：超过 300KB 或 6000 行的超大文档自动分块增量渲染——先出开头、渲染过程中显示进度，界面始终保持流畅不卡顿
- 🔄 自动刷新：文件在外部被修改后自动重新加载
- 🖥 可设为 Windows 默认打开方式（当前用户级，无需管理员）

## 安装

环境要求：Windows 10/11（自带 WebView2 运行时）、Python 3.9+（已安装并加入 PATH）。

```bat
双击 install.bat
```

脚本会：
1. 在本目录创建 `.venv` 虚拟环境并安装 `pywebview`
2. 注册 `.md / .markdown / .mdown / .mkd` 文件关联（HKCU，无需管理员）

安装后直接双击任意 `.md` 文件即可用 ReadMD 打开。

> 若系统仍用其他程序打开（Windows 默认应用设置有优先级），
> 右键 `.md` → 打开方式 → 选择 ReadMD → 始终使用；或点击阅读器工具栏的「设为默认」。
> 注意：程序化修改默认应用受 Windows 的 UserChoice 哈希保护，此路径已是最佳实践。

## 使用

```bat
rem 直接打开文件
readmd\.venv\Scripts\pythonw.exe readmd\readmd.py "C:\path\to\file.md"

rem 无 pywebview 时用浏览器兜底
python readmd\readmd.py --browser "C:\path\to\file.md"
```

### 快捷键

| 快捷键 | 功能 |
| --- | --- |
| Ctrl+O | 打开文件 |
| Ctrl+U | 网页转 MD |
| Ctrl+E | 编辑当前 MD（Ctrl+S 保存） |
| Ctrl+F | 搜索（Enter 下一个，Shift+Enter 上一个，Esc 关闭） |
| Ctrl+Shift+F | 目录侧栏 |
| Ctrl+Shift+A | AI 助手面板 |
| Ctrl+D | 切换主题 |
| Ctrl+= / Ctrl+- | 增大 / 减小字号 |
| Ctrl+R | 重新加载 |
| Ctrl+P | 打印 / 导出 PDF |
| Ctrl+← / Ctrl+→ | 历史后退 / 前进 |

## 目录结构

```
readmd/
├─ readmd.py            # 主程序（本地服务 + 原生窗口）
├─ readmd_fix.py        # 自动修正器（纯标准库）
├─ readmd_fix_test.py   # 修正器测试（python readmd_fix_test.py）
├─ readmd_modules/      # 懒加载扩展模块
│  ├─ __init__.py       #   模块注册表 / 后台加载
│  ├─ convert.py        #   万物转 MD（MarkItDown）
│  ├─ ocr.py            #   扫描转 MD（WinRT OCR + PyMuPDF）
│  ├─ web.py            #   网页转 MD（trafilatura + 批量爬取）
│  └─ ai.py             #   AI 助手（OpenAI / Anthropic 双协议 + 提供商注册表）
├─ run.bat              # 一键运行（venv pythonw）
├─ install.bat          # 一键安装 + 注册文件关联
├─ package.bat          # 一键打包单文件 exe（PyInstaller）
├─ setup.bat            # 一键：打包 + 安装 + 设为默认 + 启动
├─ uninstall.bat        # 移除文件关联（保留备份）
├─ requirements.txt
├─ assets/
│  ├─ index.html        # 界面骨架
│  ├─ style.css         # 阅读主题 + 移动端响应式
│  ├─ app.js            # 渲染 / 目录 / 搜索 / 公式 / AI / 转换 / 编辑
│  ├─ readmd.ico        # 多尺寸应用图标（16~256）
│  ├─ icon-256.png      # 256px 图标预览
│  └─ vendor/           # marked + MathJax + qrcode（全部离线）
└─ tools/make_icon.py   # 多尺寸图标生成器（纯标准库）
```

## 自动修正说明

修正均为**保守启发式**，只发生在内存渲染阶段，并会在「🛠 修复」面板中列出每一处修改：

- **表格**：检测连续的竖线行，缺少 `|---|` 表头分隔行时自动补全；各列不足时补齐空单元格；分隔行不足 3 个连字符时补足
- **加粗**：`**文字` 补全为 `**文字**`；`文字**` 中游离的结束符转义为字面量；`2 * 3` 这类乘号转义；列表 `* 项` 与分隔线 `***` 不受影响
- **公式**：`$x^2$ 和 $y` 补全为 `$y$`；`$$` 块级公式未闭合时补 `$$`；`价格 $5` 这类货币不会被误判；代码块与行内代码内的内容一律跳过
- **标题**：`#标题` → `# 标题`

## 万物转 MD 说明

- **文件转换**：工具栏「转换」选择任意文件（或直接 `python readmd.py 文件.docx`），MarkItDown 转成 Markdown 后自动过修正器并渲染；未提取到文字时提示改用 OCR
- **扫描 / 图片**：工具栏「OCR」选择图片或 PDF；Windows 内置 OCR 离线识别（需要系统已安装对应语言包，中文一般自带）；扫描版 PDF 会逐页渲染后识别
- **网页**：工具栏「网页」输入 URL；勾选“批量爬取”会抓取同站最多 10 个链接合并为一份文档
- **另存**：转换 / 网页 / OCR 结果为虚拟文档，点「另存」保存为 .md 文件
- 转换 / OCR / 网页模块均为**首次渲染完成后的后台懒加载**，工具栏按钮在模块就绪前保持禁用，不影响 Markdown 阅读的启动速度


## 大文档渲染说明

- 超过 **300KB 或 6000 行** 的文档自动进入**增量分块渲染**：正文按围栏代码块 / 空行切成小块，逐帧渲染（每帧 8 块），顶部显示「渲染中… N%」，可边渲染边滚动阅读
- 代码块与公式跨块保护不拆分，渲染完成后统一生成目录 / 搜索索引 / 公式排版，滚动位置自动恢复
- 小文档仍为一次性整篇渲染，启动速度不受影响


## AI 助手说明

- **提供商**：内置 OpenAI / DeepSeek / Kimi / 智谱 GLM / 通义千问 / 硅基流动 / OpenRouter / Groq / xAI / Mistral / Gemini / 火山方舟 / 腾讯混元 / Ollama（本地）/ Anthropic 共 15+ 预设（地址取自 cc-switch 与各官方文档），并自动导入你在 cc-switch 中配置过的提供商（DeepSeek / xem8k5 / hotapi / penguinsaichat 等）
- **API Key**：面板中填写即保存到本机配置（`%APPDATA%\ReadMD\ai.json`）；留空时自动读取系统环境变量（如 `DEEPSEEK_API_KEY`、`XEM8K5_API_KEY`），无需重复填写
- **自定义提供商**：选择「自定义」或在面板直接修改任意预设的地址 / 模型 / Key，点「存」保存；兼容 OpenAI Chat Completions 与 Anthropic Messages 两种协议，绝大多数聚合网关 / NewAPI / One API 均可直接使用
- **动作**：快速阅读（概述+要点+目录）、润色、修改（可填要求）、扩充、续写、翻译（可填目标语言）、提问；默认处理全文，勾选「仅处理选中文字」可只处理页面中选中的片段
- **Prompt 模板**：AI 面板的「模板」下拉内置 14 个常用模板（总结要点 / 生成周报 / 生成大纲 / 翻译成英文 / 代码审查 / 提取行动项 / 修正 Markdown 格式等）；点「模板」按钮可新建、编辑、删除自定义模板，内置模板也可覆盖；模板支持 `{doc}`（文档内容）与 `{prompt}`（补充要求）占位符
- **历史会话**：每次对话自动累积上下文（多轮连续提问），点「存」把当前会话保存到本机（`%APPDATA%\ReadMD\chat_history.json`，最多 50 个会话 / 60 条消息）；下拉选择历史会话即可恢复提供商、模型与全部对话，继续续聊；「清空」开始新一轮
- **流式输出与落地**：结果实时流式渲染（支持表格 / 公式），完成后可「应用到文档」（进入编辑模式审阅后 Ctrl+S 保存，首存自动备份）、「复制」或「另存为」

## 移动端共享说明

- 点工具栏「📱」开启共享，弹出二维码与访问 URL；手机连同一 Wi-Fi 后扫码即可在手机浏览器阅读当前文档，并可使用转 MD / OCR / 网页 / AI 全部功能
- 每次开启生成随机访问令牌；局域网内的 API 请求（除页面与静态资源外）均需携带令牌，关闭共享即失效
- 命令行方式：`python readmd.py --share` 启动时即开启共享

## 打包 / 一键安装

| 脚本 | 作用 |
| --- | --- |
| `run.bat` | 一键运行（venv pythonw，秒开） |
| `install.bat` | 安装依赖 + 生成图标 + 注册 .md 关联（已存在 `dist\ReadMD.exe` 时优先关联 exe） |
| `package.bat` | 一键打包为单文件 `dist\ReadMD.exe`（PyInstaller，自动安装构建依赖并生成图标） |
| `setup.bat` | 一键完成：装依赖 → 打包 exe → 注册 .md 默认打开方式 → 启动 |
| `uninstall.bat` | 移除关联并尝试恢复安装前备份 |

> 打包版与源码版功能完全一致；打包版体积较大（约 100MB，含 OCR 与转换依赖），日常使用推荐源码版（`install.bat` + `run.bat`）。
> Windows 默认应用受 UserChoice 哈希保护：若注册后系统仍用其他程序打开，右键 .md → 打开方式 → 选择 ReadMD → 始终使用。

## 卸载

```bat
双击 uninstall.bat
```

会删除已注册的文件关联（并尝试恢复安装前的 `.md` 关联备份），
`.venv` 与整个 `readmd` 文件夹保留，可手动删除。

## 常见问题

- **提示未安装 pywebview**：运行 `install.bat`，或在 PowerShell 中执行
  `python -m pip install pywebview`
- **打开时报 WebView2 相关错误**：系统缺少 Edge WebView2 运行时，
  下载安装 https://developer.microsoft.com/microsoft-edge/webview2/ 后重试
- **启动闪一下控制台**：双击 `.md` 走的是 `pythonw.exe`（无控制台）；
  若你手动用 `python readmd.py` 运行，控制台属正常现象
- **安全性说明**：Markdown 中的原始 HTML（如 `<script>`）会按原样渲染，
  与大多数阅读器一致，仅建议打开可信文件