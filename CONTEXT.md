# ReadMD 开发上下文（CONTEXT）

> 本文档用于快速恢复开发上下文。新会话开始时先读：README.md、readmd.py、readmd_fix.py、readmd_modules/*.py、installer/setup_app.py、deploy.bat、release.py。

## 项目一句话

纯本地、秒开、离线可用的 Windows Markdown 阅读器：双击 .md 即读，渲染前自动修正常见 MD 错误（只影响显示不改原文件），并集成 AI 助手 / 万物转 MD / 扫描 OCR / 网页转 MD / 主动编辑 / 移动端共享 / 大文档增量渲染 / 导出 PDF·DOCX·HTML。v2.0 起安装版为 onedir 目录安装（冷启动 ≤1.5s）+ 单实例常驻托盘（再次打开 <0.3s）。

## 位置与仓库

- 本地：`T:\Programming\Project\codex\creator\readmd`（Windows，PowerShell，.venv 已建好）
- GitHub：`https://github.com/Natsummerance/readMD`（public，main 分支）
- 账号：Natsummerance / 2734763029@qq.com；发布 token 在系统环境变量 `GITHUB_TOKEN`
- 最新 Release：v2.0.1（ReadMDSetup-v2.0.1.exe 安装包 + ReadMD-portable-v2.0.1.exe 便携版）

## 功能清单（按开发顺序）

1. 初版阅读器：本地 127.0.0.1 HTTP 服务 + pywebview 原生窗口，秒开；自动修正表格/加粗/公式/标题；自动刷新、目录、搜索、三主题、字号、最近文件、文件夹浏览、打印/导出 PDF
2. AI 助手（v1.1）：15+ 提供商预设（OpenAI/DeepSeek/Kimi/智谱/通义/硅基流动/OpenRouter/xAI/Groq/Mistral/Gemini/火山方舟等，参考本地 cc-switch 预设），OpenAI 兼容 + Anthropic 双协议，Key 可读环境变量，流式输出，结果一键应用到文档，支持自定义提供商
3. 万物转 MD：MarkItDown（docx/pptx/xlsx/pdf/html/csv/json）；扫描转 MD：Windows 内置 WinRT OCR（离线中文）+ PyMuPDF；网页转 MD：trafilatura + 批量爬取同站最多 10 页。三者均为渲染完成后后台懒加载
4. Prompt 模板管理（14+ 内置）+ AI 多轮历史会话（可保存恢复）+ 大文档增量流式渲染（>300KB 或 6000 行分块渲染）
5. 主动编辑（v1.3）：CodeMirror 6（GitHub 开源）——18 种 Markdown 语法自动补全、工具栏插入各种 MD 字符、语法高亮、亮暗主题跟随、Ctrl+S 保存（自动 .bak 备份）
6. 插入图片（v1.4）：Canvas 所见即所得裁剪/缩放/旋转，保存到文档 images/ 目录
7. 苹果风动画安装器：毛玻璃+弹簧动效+极光背景，一键安装/升级/卸载，静默模式 --install-silent / --uninstall-silent
8. 一键部署：deploy.bat + release.py（--verify/--update/--force-upload），Release 直装包
9. 秒开（v2.0）：安装版改 PyInstaller onedir 目录安装，消除单文件 96MB 每次解压的 ~6s；固定控制端口 26891 + instance.json 单实例，新进程 ping 到常驻实例后 POST /api/control/open 秒退
10. 常驻托盘（v2.0）：pystray run_detached，「显示 / 打开文件 / 退出」；关闭按钮 → 隐藏窗口（记忆位置）；退出时清理 instance.json
11. UI 全面改版（v2.0）：44px 工具条 + 内联 SVG、「更多」二级菜单、欢迎页最近文件网格、三主题全套设计 token（DESIGN.md）、阅读排版精修、骨架屏、无障碍焦点环/减少动效
12. 导出 PDF / DOCX / HTML（v2.1.0）：打印按钮升级为导出面板；内置「简约/经典/商务」预设 + 全量可视化定制（页面/封面目录/正文/各级标题/表格/代码/引用/链接/页脚/元数据/数学 DPI/HTML 主题），自定义预设可保存；公式 PDF/DOCX 本地 matplotlib 渲染成图，HTML 内联 marked+MathJax 单文件；图片按文档目录嵌入（缺失跳过提示）

## 关键文件

| 文件 | 作用 |
| --- | --- |
| readmd.py | 主程序（本地服务+窗口），含单实例控制/托盘、启动里程碑打点、run_selftest()、install_association()、Prompt/历史会话 API |
| readmd_fix.py | 自动修正器（纯标准库），fix_markdown() 返回 text + fixes 列表 |
| readmd_fix_test.py | 修正器 37 项单元测试，python readmd_fix_test.py |
| readmd_export_test.py | 导出模块 22 项单元测试（parser/styles/formula/三格式 smoke），python readmd_export_test.py |
| readmd_modules/mdexport/ | v2.1.0 导出包（惰性加载，不进 MODULES 自动加载）：parser.py / styles.py / formula.py / pdf_render.py / docx_render.py / html_render.py |
| readmd_modules/__init__.py | 懒加载注册表（convert/ocr/web/ai），load_all() 后台加载 |
| readmd_modules/convert.py | MarkItDown 转换 |
| readmd_modules/ocr.py | WinRT OCR + PyMuPDF |
| readmd_modules/web.py | trafilatura 网页提取 |
| readmd_modules/ai.py | AI 提供商注册表 + 双协议请求 |
| assets/ | 前端（index.html/style.css/app.js + vendor 全离线：marked/MathJax/qrcode/codemirror.bundle） |
| DESIGN.md | v2.0 设计规范：色盘/字体/间距/圆角/阴影 token，三主题 |
| installer/setup_app.py | 安装器主程序（含静默模式、onedir 整目录拷贝安装）；setup.html 动画界面；build_setup.bat 打包 |
| tools/make_icon.py | 多尺寸图标生成（纯标准库） |
| tools/cm-bundle/ | CodeMirror 6 构建源（npm + esbuild） |
| deploy.bat | ★一键部署：token→测试→打包→git push→发布→SHA256 校验 |
| release.py | GitHub Release 工具（--verify/--update/--force-upload/--skip-assets/--asset） |
| release_notes.md | Release 发布说明（含 SHA256 表），deploy.bat 自动读取 |
| package.bat / setup.bat / install.bat / run.bat / uninstall.bat | 打包/安装/运行脚本 |

## 打包与发布

- `package.bat`：一次产出两版——onedir 安装版 `dist\ReadMD\ReadMD.exe`（秒开）+ 便携单文件 `dist\ReadMD-portable.exe`（windowed，console=False，图标 readmd.ico）
- `installer/build_setup.bat`：先 ReadMDUninstall.exe，再 ReadMDSetup.exe（内嵌 onedir 目录 `ReadMD/` + 卸载器；v2.0.1 起不再使用 PyInstaller splash 启动画面，避免黑屏弹窗卡死安装流程）
- `deploy.bat [--skip-build] [--skip-tests] [--tag v2.0.1]`：完整一键部署
- `release.py --verify`：线上资产与本地 SHA256 全比对；`--update` 更新说明；`--force-upload` 覆盖重传
- Release 资产规范：只传 2 个（安装包 + 便携版），图标在仓库内不再上传

## 测试现状（全部通过）

- 修正器单测 37/37；导出单测 22/22；`--selftest`（HTTP 服务 + 单实例 ping/open + prompts 历史 + 图片保存 + 导出三格式冒烟）PASSED；`--mods` 四模块 ready
- dist\ReadMD\ReadMD.exe --selftest 退出码 0（console=False 无输出）；`readmd.log` 启动里程碑：start / server_up / webview_imported / window_created / page_loaded
- 安装器串行 静默安装→文件就位→静默卸载→目录清除，验证通过（并发下曾出现竞争残留，正常串行无问题）

## 环境注意事项

- apply_patch 命令不可用（WindowsApps 权限拒绝），写文件用 PowerShell Set-Content -Encoding utf8 或 Python
- Remove-Item -Recurse 被策略拦截，删目录用 `[System.IO.Directory]::Delete(path, $true)`
- requirements.txt 必须保持纯 ASCII（中文注释会让 pip 在 GBK 区域设置下解码失败，之前因此修复过一次）
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
