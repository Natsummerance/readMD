# ReadMD v2.2.5

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.5.exe`
- Windows 便携版：`ReadMD-portable-v2.2.5.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.5.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.5.zip`
- 校验清单：`SHA256SUMS.txt`

## 本次更新

- 纯文本 TXT 智能转 Markdown：自动识别标题层级、对齐表格、分点列表并生成目录锚点；打开 `.txt` 直接结构化渲染，转 Markdown 输出智能 MD。
- 剪贴板一键新建：复制任意 Markdown（网页端与 GPT、Claude、Gemini、DeepSeek 等 AI 对话回复）后，在首页按 `Ctrl+V` 即可创建可编辑 MD 文档；也可点击「从剪贴板新建」，原「从剪贴板获取对话」复用同一解析路径。
- URL 导入全放开：百度、维基百科、局域网与私有地址等任何网页均可转换为 Markdown。ReadMD 纯本地运行、数据不上传，不再做无谓的安全拦截。
- 安装器升级自动匹配旧版本目录：检测到已安装版本时预填原安装位置，无需手动选择目录。
- 内置升级推送：启动时静默检查 GitHub 最新 Release，发现新版本时以 Toast 提示并一键跳转下载页；检查失败静默，不阻塞使用，不上传任何数据。
- LaTeX 相关调研按计划推迟到 v2.2.6。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用“更多信息 → 仍要运行”。macOS 解压后请在 Finder 中右键 `ReadMD.app`，选择“打开”；如仍被阻止，可在“系统设置 → 隐私与安全性”确认打开。

## SHA-256 校验

下载对应文件和 `SHA256SUMS.txt` 后，核对文件名对应的一行。

Windows PowerShell：

```powershell
Get-FileHash .\ReadMDSetup-v2.2.5.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

macOS 终端：

```bash
shasum -a 256 ReadMD-macos-arm64-v2.2.5.zip
cat SHA256SUMS.txt
```

两处哈希值完全一致才继续使用。SHA-256 用于完整性校验，不代表代码签名。
