# ReadMD v2.3.2

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.2.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.2.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.2.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.2.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.2.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包 (UOS / 银河麒麟 / Deepin / Ubuntu / Debian)：`readmd_2.3.2_amd64.deb`
- 📱 HarmonyOS NEXT 纯血鸿蒙安装包：`ReadMD-harmonyos-v2.3.2.hap`
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.2.vsix`
- 🤖 FastMCP Server 独立包：`readmd-mcp-server-2.3.2.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

## 本次更新 (v2.3.2)

1. **未保存标签页关闭确认弹窗 UI 深度重塑 (Frontend Design & Taste Upgrade)**：
   - 引入现代化毛玻璃与景深效果（`backdrop-filter: blur(10px)`）与自适应环境阴影；
   - 卡片升级为 `16px` 大圆角搭配微米级精细高光边框；
   - 警示徽标升级为 `44px × 44px` 琥珀色柔光圆角图标徽标，消除红圈焦虑感；
   - 按钮体系优化：高对比度实色 Accent 保存按钮、柔和危险色不保存按钮与中性取消按钮；
   - 完善 `Escape` 快捷退出、遮罩点击取消、键盘自动聚焦与弹簧平滑入场动画；
   - 全面核验并精修全量 46 种语言的弹窗提示与取消/保存本土化表达。

2. **首页 46 语种核心功能与提示词条逐一地道化精校**：
   - 对首页全部 12 项核心词条（标语口号、打开 Markdown、打开文件夹、目录浏览、AI 助手、万物转 MD、网页转 MD、扫描转 MD、离线 OCR、最近打开、清空记录、快捷操作提示）在全量 46 个语言中进行逐一校验；
   - 修复爱尔兰语、格陵兰语、高棉语、老挝语、马来语、马耳他语、缅甸语、卢旺达语、藏语等语种中清空记录、万物转 MD 的机翻偏差，实现 100% 自然地道表达。

3. **全平台全架构构建与文档同步**：
   - 同步升级 Windows、macOS (ARM64/Intel)、Linux (AppImage/Deb)、HarmonyOS NEXT (HAP)、VSCode 扩展、FastMCP 服务端版本至 v2.3.2。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用「更多信息 → 仍要运行」。macOS 解压后请在 Finder 中右键 ReadMD.app，选择「打开」；如仍被阻止，可在「系统设置 → 隐私与安全性」确认打开。

## SHA-256 校验

下载对应文件和 SHA256SUMS.txt 后，核对文件名对应的一行。

Windows PowerShell：

    Get-FileHash .\ReadMDSetup-v2.3.2.exe -Algorithm SHA256

macOS 终端：

    shasum -a 256 ReadMD-macos-arm64-v2.3.2.zip
