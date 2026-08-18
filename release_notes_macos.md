# ReadMD v2.2.5 for macOS

ReadMD 是免费的本地 Markdown 阅读器。

- Intel Mac：`ReadMD-macos-x64-v2.2.5.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.5.zip`
- 完整性清单：`SHA256SUMS.txt`

v2.2.5 带来 TXT 智能转 Markdown（标题/表格/列表/目录识别）、剪贴板 `Ctrl+V` 一键新建 MD 文档、URL 导入全放开（含百度、维基、局域网等任意网页），以及安装器升级时自动匹配旧版本目录；启动时还会静默检查 GitHub 最新 Release 并以 Toast 推送升级提示。

> 两个 `.app` 均未签名。解压后首次启动请在 Finder 中右键 `ReadMD.app` →“打开”；如仍被阻止，请到“系统设置 → 隐私与安全性”确认打开。

校验方法：下载 ZIP 与 `SHA256SUMS.txt` 后运行 `shasum -a 256 ReadMD-macos-arm64-v2.2.5.zip`（Intel 包替换文件名），再与清单同名行比较。哈希相同表示下载内容完整，不表示代码已签名。
