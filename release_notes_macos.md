# ReadMD v2.2.1 for macOS

这是拆分平台依赖后的首个独立 macOS 版本：

- Intel Mac：`ReadMD-macos-x64-v2.2.1.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.1.zip`

macOS 构建只安装 common 依赖及 Cocoa、WebKit、Vision、Quartz、Uniform Type Identifiers 等原生框架，不包含 WinRT、注册表或 Windows 安装器代码。Finder 打开/定位、错误弹窗和 OCR 均通过 PyObjC 调用系统 API；应用声明 Markdown 文档编辑器角色并显式使用 Cocoa 后端。

本版本延续 v2.2.0 的导出滚动、历史记录、AI schema v2 脱敏与模型下拉、图片编辑、Markdown 命令/公式选择器和预览停靠升级。

> 两个 `.app` 均未签名。解压后首次启动请在 Finder 中右键 `ReadMD.app` →“打开”；如仍被阻止，请到“系统设置 → 隐私与安全性”允许打开。
