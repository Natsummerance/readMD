# ReadMD v2.2.4

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.4.exe`
- Windows 便携版：`ReadMD-portable-v2.2.4.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.4.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.4.zip`
- 校验清单：`SHA256SUMS.txt`

## 本次更新

- 启动时只加载阅读所需部分；转换、OCR、网页与 AI 等可选模块改为按功能请求加载，并增加隐私安全的启动探针。
- AI 面板可导入用户主动提供的对话内容：一次性授权的剪贴板、用户选择的导出文件，以及公开网页地址；导入前预览，解析后的内容才进入当前会话。
- 对话导入限制大小、压缩包展开量和危险链接；剪贴板授权与文件授权均为一次性、短时有效，不记录原始剪贴板内容。
- Windows、Intel macOS 和 Apple Silicon macOS 由同一标签工作流构建；发布前执行回归、隐私扫描、冻结包/Bundle 资源检查、架构检查和 SHA-256 校验。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用“更多信息 → 仍要运行”。macOS 解压后请在 Finder 中右键 `ReadMD.app`，选择“打开”；如仍被阻止，可在“系统设置 → 隐私与安全性”确认打开。

## SHA-256 校验

下载对应文件和 `SHA256SUMS.txt` 后，核对文件名对应的一行。

Windows PowerShell：

```powershell
Get-FileHash .\ReadMDSetup-v2.2.4.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

macOS 终端：

```bash
shasum -a 256 ReadMD-macos-arm64-v2.2.4.zip
cat SHA256SUMS.txt
```

两处哈希值完全一致才继续使用。SHA-256 用于完整性校验，不代表代码签名。
