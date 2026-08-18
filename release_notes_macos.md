# ReadMD v2.2.4 for macOS

ReadMD 是免费的本地 Markdown 阅读器。

- Intel Mac：`ReadMD-macos-x64-v2.2.4.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.4.zip`
- 完整性清单：`SHA256SUMS.txt`

v2.2.4 将可选功能改为按需加载，增加启动探针，并支持从一次性授权剪贴板、用户选择的导出文件或公开网页地址预览并导入对话内容。导入会限制大小与危险内容，原始剪贴板数据不会写入日志。

> 两个 `.app` 均未签名。解压后首次启动请在 Finder 中右键 `ReadMD.app` →“打开”；如仍被阻止，请到“系统设置 → 隐私与安全性”确认打开。

校验方法：下载 ZIP 与 `SHA256SUMS.txt` 后运行 `shasum -a 256 ReadMD-macos-arm64-v2.2.4.zip`（Intel 包替换文件名），再与清单同名行比较。哈希相同表示下载内容完整，不表示代码已签名。
