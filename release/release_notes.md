# ReadMD v2.3.7-beta.5 (全平台全生态全架构原生自愈与零依赖开箱即用)

ReadMD 是免费、开源的本地 Markdown 智能阅读、编辑与全格式转换排版套件；纯本地离线可用、秒级极速渲染，绝不改写原文件。

## 📦 全平台发布资产 (Release Assets)

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.7-beta.5.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.7-beta.5.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.5.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.5.zip`
- 🐧 Linux x86_64 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.5.AppImage`
- 🇨🇳 国产信创 AMD64 Deb：`readmd_2.3.7-beta.5_amd64.deb`
- 🖥️ 麒麟 V10 / 飞腾 ARM64 AppImage：`ReadMD-linux-aarch64-v2.3.7-beta.5.AppImage`
- 🖥️ 麒麟 V10 / 飞腾 ARM64 Deb：`readmd_2.3.7-beta.5_arm64.deb`
- 📱 HarmonyOS NEXT / OpenHarmony：ArkTS 源码工程（DevEco Studio 构建；当前不提供预编译 HAP）
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.5.vsix`
- 🤖 FastMCP Server 源码包：`readmd-mcp-server-2.3.7-beta.5.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

## 🌟 本次版本核心特性与重大演进

### 1. 银河麒麟 Kylin V10 与国产信创 Linux 依赖彻底解耦
- **杜绝依赖报错**：彻底废除 `Depends: gir1.2-webkit2-4.0` 硬依赖，改为通用系统底座，将 WebKitGTK 4.0/4.1/6.0 列为软性推荐，`dpkg -i` 在 Ubuntu 24、Debian 12、麒麟 V10 等系统上 **100% 零依赖报错直接安装**。
- **四重图形降级自愈管道**：WebKitGTK 4.1/4.0/6.0 → QtWebEngine → **独立 Browser App 模式**（调用 `kylin-browser` / `uos-browser` / `chromium` / `chrome` / `firefox` 独立应用窗口秒开）→ `xdg-open` 兜底。
- **飞腾硬件显卡自愈**：自动识别 Phytium D2000 / E2000 / FT2000 芯片，启用 Mesa 软件光栅化自愈与 X11 后端自适应，彻底杜绝黑屏与花屏。

### 2. Windows 全版本 (Win7~Win11 & WoA ARM64) 原生适配加固
- **Windows on ARM (WoA)**：精准识别高通骁龙 X Elite / Surface Pro X 原生架构。
- **WebView2 异常自愈**：遇精简版 Windows 缺少 WebView2 时，自动平滑降级至 `msedge.exe --app=` 独立无边框应用窗口模式秒开。
- **安装器防闪退**：安装程序在极度精简无 WebView2 环境下自动降级为本地浏览器安装向导。

### 3. macOS (Apple Silicon M1~M4 & Intel) 双轨自愈
- 原生 Cocoa WKWebView 桥接 + 离线私网隔离规则。
- 独立应用窗口模式降级与 `AppleInterfaceStyle` 原生深色模式探针。

### 4. 全生态分发工具链矩阵补齐
- 新增华为 openEuler / Fedora / RHEL / CentOS / Anolis OS 的标准 RPM 打包工具链（`scripts/linux/build_rpm.sh`）。
- 新增 Flatpak / Flathub 通用沙箱清单（`scripts/linux/org.readmd.ReadMD.yaml`）。
- 新增统信 UOS / 深度 Deepin 玲珑 Linyaps 清单（`scripts/linux/linglong.yaml`）。
- 新增 Arch Linux / Manjaro AUR 规范（`scripts/linux/PKGBUILD`）。
- 新增 Alpine Linux 极简容器镜像与 Docker Compose 部署配置。

### 5. 跨平台命令行统一自检诊断探针
- 新增 `readmd --diagnose` / `readmd --check-system` / `readmd --check-linux` / `readmd --check-windows` / `readmd --check-macos` 命令行参数，一秒输出操作系统、CPU 指令集与渲染引擎自愈健康度报告。

## 🛠️ SHA-256 完整性校验

Windows PowerShell：
```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.5.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.5.zip
```

