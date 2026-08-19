# ReadMD v2.3.1

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.1.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.1.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.1.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.1.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.1.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包 (UOS / 银河麒麟 / Deepin / Ubuntu / Debian)：`readmd_2.3.1_amd64.deb`
- 📱 HarmonyOS NEXT 纯血鸿蒙安装包：`ReadMD-harmonyos-v2.3.1.hap`
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.1.vsix`
- 🤖 FastMCP Server 独立包：`readmd-mcp-server-2.3.1.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

## 本次更新 (v2.3.1)

1. **46-语种完整母语化 i18n 深度审核**：
   - 全面深度审核 42 个非中英文语言文件，消除系统性机翻错误（波兰语混淆/西塔误译/农作物=裁剪/派系群组=π 等经典翻译事故）；
   - 修复日语 formula.theta「西塔」→「θ（シータ）」、俄语「西部之塔」→「Тета (θ)」、韩语「그룹」→「π（파이）」、法/西/葡/丹/挪/瑞典语 ai.polish「Polish 语文本」→「改进文本」等 30+ 语言的公式标签与关键词错误；
   - 实现 46 语种 × 904 词条全部 100% 完整，与英文等价的母语化覆盖率。

2. **Linux 全架构与国产信创支持**（x86_64 / aarch64 / loongarch64）：
   - 新增 src/readmd_modules/linux_native.py 原生适配模块，识别银河麒麟 KylinOS V10 SP2、统信 UOS 20、Deepin、openEuler、Anolis OS 等国产操作系统；
   - 自动适配 Wayland / X11 双显示协议；支持 DDE（统信/Deepin）、UKUI（麒麟）、GNOME、KDE 桌面深色模式检测；
   - 提供全架构 AppImage 打包脚本（scripts/linux/build_linux.sh）、FreeDesktop .desktop 文件与 .md MIME 类型注册；
   - 提供 packages/linglong/linglong.yaml 玲珑容器格式声明，适配统信 UOS 应用商店分发。

3. **HarmonyOS NEXT 鸿蒙原生应用**（实验性，packages/harmonyos-app）：
   - 基于 ArkTS + ArkUI 声明式 UI，通过 ArkWeb 容器完整复用 ReadMD Web 渲染层（Marked + KaTeX + 46 语种 i18n + LaTeX PRO）；
   - ReadMDBridge 桥接层对接鸿蒙系统剪贴板（`@ohos.pasteboard`）、文件选择器（`@ohos.file.picker`）、系统语言检测（`@ohos.i18n`）与原生 OCR（`@ohos.ai.OCR`）；
   - 适配 HarmonyOS NEXT 纯血鸿蒙与 OpenHarmony 4.1+/5.0，使用 DevEco Studio NEXT 编译为 .hap 应用包。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用「更多信息 → 仍要运行」。macOS 解压后请在 Finder 中右键 ReadMD.app，选择「打开」；如仍被阻止，可在「系统设置 → 隐私与安全性」确认打开。

## SHA-256 校验

下载对应文件和 SHA256SUMS.txt 后，核对文件名对应的一行。

Windows PowerShell：

    Get-FileHash .\ReadMDSetup-v2.3.1.exe -Algorithm SHA256

macOS 终端：

    shasum -a 256 ReadMD-macos-arm64-v2.3.1.zip
