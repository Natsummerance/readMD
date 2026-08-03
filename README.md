# ReadMD · 轻量级 Markdown 阅读器

一个**纯本地、秒开、离线可用**的 Markdown 阅读器。双击 `.md` 文件即可阅读，
渲染前会自动修正常见 Markdown 错误（表格、加粗、公式、标题），**只影响显示，绝不改写原文件**。

## 功能

- ⚡ **快**：打开 .md 秒开——先渲染正文（marked + MathJax 全离线），**渲染完成后再在后台懒加载**转换/OCR/网页模块，不影响阅读速度
- 🛠 **自动修正**：表格缺分隔行 / 列数不齐、未闭合的 `**` `__` `*`、未闭合的 `$` `$$` `\(` `\)`、`#标题` 缺空格、BOM、CRLF 等
- 🔄 **万物转 MD**：docx / pptx / xlsx / pdf / html / csv / json 等一键转为 Markdown（基于 MarkItDown），转换结果自动过修正器
- 🔍 **扫描转 MD（OCR）**：图片、扫描件 PDF 用 Windows 10/11 内置 OCR（离线、免费、无次数限制，支持中文）；PDF 有文字层直接提取，无文字层逐页渲染 OCR
- 🌐 **网页转 MD**：输入 URL 抓取正文（trafilatura），支持勾选“批量爬取”同站链接最多 10 页合并为一个文档
- ✏️ **编辑 MD**：Ctrl+E 进入编辑模式，Ctrl+S 保存（首次保存自动生成 `.bak` 备份），保存后自动重新渲染
- 📑 阅读体验：目录侧栏、全文搜索（Ctrl+F）、亮/暗/护眼三主题、字号缩放、打印/导出 PDF
- 📂 文件夹浏览：打开整个文件夹，侧栏列出全部 Markdown 逐个阅读
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
│  └─ web.py            #   网页转 MD（trafilatura + 批量爬取）
├─ install.bat          # 一键安装 + 注册文件关联
├─ uninstall.bat        # 移除文件关联（保留备份）
├─ requirements.txt
├─ assets/
│  ├─ index.html        # 界面骨架
│  ├─ style.css         # 阅读主题
│  ├─ app.js            # 渲染 / 目录 / 搜索 / 公式 / 转换 / 编辑
│  ├─ readmd.ico        # 应用图标
│  └─ vendor/           # marked + MathJax（离线）
└─ tools/make_icon.py   # 图标生成器
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