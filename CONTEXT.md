# ReadMD 开发上下文（CONTEXT）

> 本文档用于快速恢复开发上下文。新会话开始时先读：README.md、readmd.py、readmd_fix.py、readmd_modules/*.py、installer/setup_app.py、deploy.bat、release.py。

## 项目一句话

纯本地、秒开、离线可用的 Windows / macOS Markdown 阅读器，集成 AI 助手、转换、OCR、网页提取、主动编辑与文档导出。

## 位置与仓库

- 本地：任意工作区目录（不得在仓库记录开发者绝对路径）
- GitHub：`https://github.com/Natsummerance/readMD`（public，main 分支）
- 发布凭据仅由 GitHub Actions 管理，不在源码或文档记录个人账号、邮箱或 Token
- 当前开发/发布目标：v2.2.2（Windows + Intel macOS + Apple Silicon macOS 同一 Release）
- Win7 兼容版：**v2.1.1 Beta**（pre-release tag `v2.1.1-beta`，资产 `ReadMDSetup-2.1.1-Beta-win7-x64.exe`）——Win7 SP1 x64 + 内嵌固定版 WebView2 109 运行时；独立 Python 3.9.13 构建链（`.venv-win7` / `win7-reqs.txt` / `build_win7.bat` / `ReadMD-win7.spec` / `ReadMDSetup-win7.spec` / `tools\win7_pywebview_edgechromium.patch` / `tools\bundle_runtime.py`）；功能裁剪：仅 docx / pdf 转 MD + 导出（OCR / AI / 网页 / 其他格式在 Win7 下提示不可用）

## 功能清单（按开发顺序）

1. 初版阅读器：本地 127.0.0.1 HTTP 服务 + pywebview 原生窗口，秒开；自动修正表格/加粗/公式/标题；自动刷新、目录、搜索、三主题、字号、最近文件、文件夹浏览、打印/导出 PDF
2. AI 助手（v2.2）：公开官方预设 + 自定义连接，OpenAI 兼容 + Anthropic 双协议，Key 可读环境变量且接口不回传明文，流式输出，结果一键应用到文档
3. 万物转 MD：MarkItDown（docx/pptx/xlsx/pdf/html/csv/json）；扫描转 MD：Windows WinRT / macOS Vision；网页转 MD：Trafilatura → 系统 WebView → Mozilla Readability → 完整页面降级，支持同站最多 10 页与可选图片本地化。三者均为渲染完成后后台懒加载
4. Prompt 模板管理（14+ 内置）+ AI 多轮历史会话（可保存恢复）+ 大文档增量流式渲染（>300KB 或 6000 行分块渲染）
5. 主动编辑（v1.3）：CodeMirror 6（GitHub 开源）——18 种 Markdown 语法自动补全、工具栏插入各种 MD 字符、语法高亮、亮暗主题跟随、Ctrl+S 保存（自动 .bak 备份）
6. 插入图片（v1.4）：Canvas 所见即所得裁剪/缩放/旋转，保存到文档 images/ 目录
7. 苹果风动画安装器：毛玻璃+弹簧动效+极光背景，一键安装/升级/卸载，静默模式 --install-silent / --uninstall-silent
8. 一键部署：deploy.bat + release.py（--verify/--update/--force-upload），Release 直装包
9. 秒开（v2.0）：安装版改 PyInstaller onedir 目录安装，消除单文件 96MB 每次解压的 ~6s；固定控制端口 26891 + instance.json 单实例，新进程 ping 到常驻实例后 POST /api/control/open 秒退
10. 常驻托盘（v2.0）：pystray run_detached，「显示 / 打开文件 / 退出」；关闭按钮 → 隐藏窗口（记忆位置）；退出时清理 instance.json
11. UI 全面改版（v2.0）：44px 工具条 + 内联 SVG、「更多」二级菜单、欢迎页最近文件网格、三主题全套设计 token（DESIGN.md）、阅读排版精修、骨架屏、无障碍焦点环/减少动效
12. 导出 PDF / DOCX / HTML（v2.1.0）：打印按钮升级为导出面板；内置「简约/经典/商务」预设 + 全量可视化定制（页面/封面目录/正文/各级标题/表格/代码/引用/链接/页脚/元数据/数学 DPI/HTML 主题），自定义预设可保存；公式 PDF/DOCX 本地 matplotlib 渲染成图，HTML 内联 marked+MathJax 单文件；图片按文档目录嵌入（缺失跳过提示）
13. AI 连接自定义（v2.1.1）：AI 面板顶部新增「连接设置」卡片——提供商预设可选但 API Key 必填（本地 Ollama 除外）、Base URL 可编辑（保存为该提供商自定义覆盖，可一键恢复预设）、响应方式四选（auto/chat completions/completions/responses/Anthropic messages）、流式开关；「获取模型」按钮通过 Key 拉取模型列表填入 datalist（失败回退预设并可手输模型名）；对话实时监控 token 用量（本次 + 会话累计，随会话存入 history）
14. 编辑实时预览（v2.1.1）：编辑工具栏新增「无/左/右/下/上」五档预览布局（默认无，与旧版一致），300ms 防抖实时渲染（复用 marked + MathJax 管线 + 当前主题），可选「滚动同步」按比例双向联动；转换/OCR/网页虚拟文档解锁编辑，无文件保存走另存为并切换为文件模式
15. 转换保存 / 批量 / 质量（v2.1.1）：单文件与批量转换一律自动保存到源目录同名 .md（同名默认跳过，可勾选覆盖）；「转 Markdown」弹窗支持多选文件 / 整文件夹（递归 ≤200 个）+ 实时进度列表 + 打开结果目录；docx 专用解析（OMML 公式→LaTeX、标题、表格、等宽字体代码块、样式级列表），pdf 专用解析（PyMuPDF find_tables 还原表格 + 公式启发式），其余走 MarkItDown 并逐文件回退；统一 mdcheck 严格校验（围栏闭合/表格/公式定界符/替换符/图片存在性，安全项自动修复）
16. 网页转 MD 重构（v2.2.2）：可诊断 HTTP 下载、重定向/编码/大小限制、Trafilatura 双级抽取、WebView2/WKWebView 动态渲染、离线 Mozilla Readability、完整页面降级、批量进度/取消、图片资源随另存迁移及 SSRF/HTML 清洗
17. Windows 文件关联图标与应用 Logo 分离：`.md/.markdown` 使用多尺寸简约文档图标，ReadMD EXE、安装器和快捷方式继续使用原应用 Logo

## 关键文件

| 文件 | 作用 |
| --- | --- |
| readmd.py | 主程序（本地服务+窗口），含单实例控制/托盘、启动里程碑打点、run_selftest()、install_association()、Prompt/历史会话 API |
| readmd_fix.py | 自动修正器（纯标准库），fix_markdown() 返回 text + fixes 列表 |
| readmd_fix_test.py | 修正器 37 项单元测试，python readmd_fix_test.py |
| readmd_convert_test.py | 转换/校验/AI 协议 21 项单元测试，python readmd_convert_test.py |
| readmd_modules/mdcheck.py | 转换后严格校验（围栏/表格/公式/替换符/图片）+ 安全自动修复 |
| readmd_export_test.py | 导出模块 22 项单元测试（parser/styles/formula/三格式 smoke），python readmd_export_test.py |
| readmd_modules/mdexport/ | v2.1.0 导出包（惰性加载，不进 MODULES 自动加载）：parser.py / styles.py / formula.py / pdf_render.py / docx_render.py / html_render.py |
| readmd_modules/__init__.py | 懒加载注册表（convert/ocr/web/ai），load_all() 后台加载 |
| readmd_modules/convert.py | 转换：docx/pdf 专用解析（OMML→LaTeX、表格、代码块）+ MarkItDown 兜底 |
| readmd_modules/ocr.py | WinRT OCR + PyMuPDF |
| readmd_modules/web.py | 安全下载、Trafilatura/Readability/完整页面抽取、链接与图片资源处理 |
| readmd_web_test.py | 网页下载、抽取、安全、图片与 API 的本地 HTTP 夹具测试 |
| readmd_modules/ai.py | AI 提供商注册表 + 四协议请求（chat/completions/completions/responses/messages）+ 模型列表拉取 + 用量解析 |
| assets/ | 前端（index.html/style.css/app.js + vendor 全离线：marked/MathJax/qrcode/codemirror.bundle） |
| DESIGN.md | v2.0 设计规范：色盘/字体/间距/圆角/阴影 token，三主题 |
| installer/setup_app.py | 安装器主程序（含静默模式、onedir 整目录拷贝安装）；setup.html 动画界面；build_setup.bat 打包 |
| tools/make_icon.py | 多尺寸图标生成（纯标准库） |
| tools/cm-bundle/ | CodeMirror 6 构建源（npm + esbuild） |
| deploy.bat | 本地测试、提交并推送 main 与版本标签；不创建 Release |
| release.py | Release 维护/校验辅助工具；CI 是唯一发布方 |
| release_notes.md | GitHub Actions 使用的 Release 发布说明 |
| package.bat / setup.bat / install.bat / run.bat / uninstall.bat | 打包/安装/运行脚本 |

## 打包与发布

- `package.bat`：一次产出两版——onedir 安装版 `dist\ReadMD\ReadMD.exe`（秒开）+ 便携单文件 `dist\ReadMD-portable.exe`（windowed，console=False，图标 readmd.ico）
- `installer/build_setup.bat`：先 ReadMDUninstall.exe，再 ReadMDSetup.exe（内嵌 onedir 目录 `ReadMD/` + 卸载器；v2.0.1 起不再使用 PyInstaller splash 启动画面，避免黑屏弹窗卡死安装流程）
- `deploy.bat [--skip-tests] [--tag v2.2.2]`：测试并推送 main 与标签，明确排除用户本地 `IDEA.md`
- `release.yml` 在 Windows、Intel macOS、Apple Silicon macOS 测试后统一打包四个 v2.2.2 资产
- Release 由 CI 唯一创建，并在发布前校验架构、版本、大小与 SHA-256

## 测试现状（全部通过）

- 修正器单测 37/37；转换/AI 23/23；导出单测 24/24；网页专项 10/10；Playwright UI 4/4；`--selftest` PASSED
- dist\ReadMD\ReadMD.exe --selftest 退出码 0（console=False 无输出）；`readmd.log` 启动里程碑：start / server_up / webview_imported / window_created / page_loaded
- 安装器串行 静默安装→文件就位→静默卸载→目录清除，验证通过（并发下曾出现竞争残留，正常串行无问题）

## 环境注意事项

- 平台依赖分为 requirements-common.txt、requirements-windows.txt 与 requirements-macos.txt；macOS 构建禁止安装/打包 WinRT、winreg 和 Windows 安装器代码
- v2.0 新增依赖 pystray（托盘）；单实例控制端口 26891 固定，占用时回退随机端口并禁用单实例
- v2.1 新增依赖 reportlab（PDF）、matplotlib（公式成图），全部惰性导入（mdexport 包不进 MODULES 自动加载）；requirements.txt 保持纯 ASCII
- 注册表关联 HKCU\Software\Classes\ReadMD.markdown → dist\ReadMD\ReadMD.exe（测试残留的悬空路径已修复）

## 已知待办 / 隐患

- requirements.txt 中 lxml 6.1.1 有 `does not provide extra 'html_clean'` WARNING（无碍）
- 常驻托盘占用内存约 50–100MB；如后续更在意内存可改「关闭=销毁窗口」
- onedir 版 selftest 的 frozen 分支会调 `/api/modules` 触发重量模块加载（原 onefile 已如此），验证时注意区分
- .spec 文件被 .gitignore 忽略（`*.spec`），本地保留用于重建；新环境可用 .bat 命令行参数打包
- AI Key 无内置密钥，全部依赖用户配置/环境变量

## 最近一次变更（v2.1.0 导出）

- 新增 readmd_modules/mdexport/ 导出包：parser（块 AST + 行内节点 + 公式提取）、styles（schema + 3 预设 + sanitize）、formula（matplotlib mathtext 渲染公式为透明 PNG，含中文公式；不支持语法回退文本并提示）、pdf_render（reportlab：微软雅黑 TTF + Consolas 等宽注册、样式 token 到 ParagraphStyle/TableStyle、封面/目录/书签/页码/元数据、multiBuild）、docx_render（python-docx：内置 Heading 样式 + eastAsia 字体、表头底纹/斑马纹/双写列宽、代码块底纹、公式 run.add_picture、页脚 PAGE 域）、html_render（单文件内联 marked+MathJax+主题 CSS）
- readmd.py：Api.export_doc（SAVE_DIALOG → mdexport.export）/ reveal_path / get_export_presets / save_export_presets；selftest 增加导出三格式冒烟；VERSION=2.1.0
- 前端：btn-print 与 Ctrl+P 打开导出面板（export-modal），schema 驱动 8 组参数渲染，预设/自定义预设/恢复默认，导出后「打开 / 所在文件夹」，exportLast/exportPresets 经 settings.json 持久化
- 打包注意：package.bat 的 --collect-submodules readmd_modules 已覆盖 mdexport；reportlab/matplotlib 由 PyInstaller hook 收集；构建后用 dist exe 验证 --selftest 的 export OK

## 最近一次变更（v2.0.1 安装器修复）

修复安装器黑屏弹窗：v2.0.0 的 ReadMDSetup/ReadMDUninstall 用 PyInstaller --splash 但从未调用 pyi_splash.close()，启动画面置顶且无关闭按钮，低配机解压慢时更明显，会挡住安装界面（表现为装完退不掉、软件打不开）。v2.0.1 移除 --splash，并在 setup_app.py 加防御性 pyi_splash.close()；安装器/主程序版本号统一 2.0.1；deploy.bat 与 release.py 默认标签同步 v2.0.1。
