# ReadMD v2.2.3

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.3.exe`
- Windows 便携版：`ReadMD-portable-v2.2.3.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.3.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.3.zip`

## 本次更新

- 修复 Windows 保存对话框返回 tuple 时 PDF、DOCX、HTML 全部导出失败的问题；三种格式均改为同目录临时文件写入并原子替换，失败不会覆盖原文件。
- 导出错误增加保存对话框、依赖、解析、公式、渲染、写入和最终替换阶段诊断；冻结包显式收集 python-docx、ReportLab、Matplotlib 与 Trafilatura 资源。
- 网页转 Markdown 增加离线 Defuddle 0.19.2，并保留 Mozilla Readability 独立回退；支持错误 Content-Type、`noscript`、短公告、代码页和文档页。
- 403、429、代理、空响应和 JavaScript 壳页面会自动尝试系统 WebView；隐藏渲染仍失败时可在隔离临时窗口完成合法登录或验证后提取，任务结束即清理 Cookie。
- 同站批量抓取可设置 1–30 页；桌面端可为单任务、单源站签发 10 分钟内网页面授权，浏览器/局域网接口不能签发。
- 顶栏文件名支持点击或 F2 行内重命名，自动同步窗口标题、最近文件、浏览历史、侧栏和 AI 会话文档引用。

## macOS 提示

macOS 包仍未签名。首次启动请在 Finder 中右键 `ReadMD.app`，选择“打开”，并在系统提示中再次确认。
