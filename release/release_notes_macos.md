# ReadMD v2.3.3 for macOS

## 下载

- Intel Mac：`ReadMD-macos-x64-v2.3.3.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.3.zip`
- 校验清单：`SHA256SUMS.txt`

v2.3.3 带来超长 Markdown（>10,000 行）智能语义分页引擎与纯 SVG 翻页交互栏（支持全局 TOC 跨页跳转与 Ctrl+F 跨页搜索）、LaTeX 学术公式视口懒排版加速、全量 46 语种本地化词条 100% 同步及全平台版本发布。

本构建仅供 macOS 12+ 系统使用，依赖 macOS Vision 原生框架与 WebKit 原生渲染。

校验方法：下载 ZIP 与 `SHA256SUMS.txt` 后运行 `shasum -a 256 ReadMD-macos-arm64-v2.3.3.zip`（Intel 包替换文件名），再与清单同名行比较。哈希相同表示下载内容完整，不表示代码已签名。
