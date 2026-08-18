# ReadMD v2.2.6

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.6.exe`
- Windows 便携版：`ReadMD-portable-v2.2.6.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.6.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.6.zip`
- 校验清单：`SHA256SUMS.txt`

## 本次更新

- 主页未打开任何文件时，「导出」与字体大小加减按钮置灰不可点；打开文档后才可用。
- 顶栏「AI / 转换 / 网页 / OCR」入口恢复正常可点击。
- 删除与网页转 MD 重叠的「导入对话」模块，仅保留「从剪贴板新建」。
- 打开文件后右下角新增「回到主页」按钮。
- 打开文件夹改为 VSCode 式侧边栏目录树：可折叠文件夹，过滤上级目录名，文件名更清晰。
- 「目录」按钮移动到工具栏最左侧。
- 修复 Markdown 渲染后开头目录无法跳转到对应章节的问题。
- 主页固定六个模块（打开文件 / 打开文件夹 / AI / 转换 / 网页 / OCR）；「从剪贴板新建」移入右上角「更多」下拉菜单。
- 根目录文件整理归档，开发目录更整洁。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用“更多信息 → 仍要运行”。macOS 解压后请在 Finder 中右键 `ReadMD.app`，选择“打开”；如仍被阻止，可在“系统设置 → 隐私与安全性”确认打开。

## SHA-256 校验

下载对应文件和 `SHA256SUMS.txt` 后，核对文件名对应的一行。

Windows PowerShell：

```powershell
Get-FileHash .\ReadMDSetup-v2.2.6.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

macOS 终端：

```bash
shasum -a 256 ReadMD-macos-arm64-v2.2.6.zip
cat SHA256SUMS.txt
```

两处哈希值完全一致才继续使用。SHA-256 用于完整性校验，不代表代码签名。
