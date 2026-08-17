# ReadMD v2.2.2

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.2.exe`
- Windows 便携版：`ReadMD-portable-v2.2.2.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.2.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.2.zip`

## 网页转 Markdown 重构

- 修复网页下载成功却统一提示“未爬取到正文”的问题，超时、DNS、TLS、403、429、非 HTML、超限响应与正文过短现在会给出具体提示。
- 使用 Trafilatura 智能正文与高召回双层提取；静态内容不足时，桌面应用自动使用系统 WebView 渲染，并通过离线 Mozilla Readability 提取正文。
- 新增完整页面模式、强制动态渲染、重试、取消、逐页进度与同站最多 10 页批量抓取；单页失败不会丢失已成功内容。
- 保留表格、代码、链接、图片和文章元数据，并将相对地址改为绝对地址。
- 可选下载网页图片；另存 Markdown 时自动生成同名 `.assets` 目录并重写相对路径。
- 仅访问公开 HTTP/HTTPS 地址，不绕过登录、验证码、付费墙或站点访问限制，也不保留网页 Cookie。

## Windows 文件图标

- `.md/.markdown` 文件关联改用独立的简约 Markdown 文档图标；ReadMD 应用 Logo 保持不变。
- 新图标包含多种尺寸，在资源管理器的小图标、列表、平铺视图中均保持清晰。

## 构建与校验

- Windows、Intel macOS 与 Apple Silicon macOS 使用同一标签工作流完成测试、打包和 Release 发布。
- 冻结包会验证 Trafilatura 配置、Mozilla Readability、Markdown 文件图标、应用版本、目标架构与隐私扫描。
- Release job 校验四个资产名称、非零大小并生成 SHA-256 摘要。

macOS 包尚未签名。首次打开请在 Finder 中右键 ReadMD →“打开”；若仍被拦截，请前往“系统设置 → 隐私与安全性”选择“仍要打开”。
