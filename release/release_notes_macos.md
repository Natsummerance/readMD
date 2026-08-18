# ReadMD v2.2.6 for macOS

ReadMD 是免费的本地 Markdown 阅读器。

- Intel Mac：`ReadMD-macos-x64-v2.2.6.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.6.zip`
- 完整性清单：`SHA256SUMS.txt`

v2.2.6 修复主页工具栏状态（未打开文件时导出/字号按钮置灰、AI/转换/网页/OCR 恢复可点击）、删除与网页转 MD 重叠的导入对话模块、新增右下角「回到主页」、文件夹浏览改为可折叠的 VSCode 式侧边栏目录树（过滤上级目录名）、目录按钮移至最左侧、修复渲染后目录无法跳转章节，并将「从剪贴板新建」移入「更多」下拉菜单；主页固定六个模块。

> 两个 `.app` 均未签名。解压后首次启动请在 Finder 中右键 `ReadMD.app` →“打开”；如仍被阻止，请到“系统设置 → 隐私与安全性”确认打开。

校验方法：下载 ZIP 与 `SHA256SUMS.txt` 后运行 `shasum -a 256 ReadMD-macos-arm64-v2.2.6.zip`（Intel 包替换文件名），再与清单同名行比较。哈希相同表示下载内容完整，不表示代码已签名。
