# ReadMD v2.3.7-beta.3 (演说模式重构与体验精细化发布)

ReadMD 是免费、开源的本地 Markdown 阅读器与编辑器：双击即读，超长文档自动语义分页，常见语法问题只在显示层修正；支持 Office / PDF / 网页转 Markdown、离线 OCR、LaTeX 学术增强、放映模式和 MCP 接入。

## 全平台发布资产

- Windows 安装版：`ReadMDSetup-v2.3.7-beta.3.exe`
- Windows 便携版：`ReadMD-portable-v2.3.7-beta.3.exe`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.3.zip`
- Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.3.zip`
- Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.3.AppImage`
- Linux / 国产信创 Deb 安装包（UOS / 银河麒麟 / Deepin / Ubuntu / Debian）：`readmd_2.3.7-beta.3_amd64.deb`
- HarmonyOS NEXT 纯血鸿蒙安装包：`ReadMD-harmonyos-v2.3.7-beta.3.hap`
- VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.3.vsix`
- FastMCP Server 独立包：`readmd-mcp-server-2.3.7-beta.3.zip`
- 校验清单：`SHA256SUMS.txt`

## AI 助手与机器可读事实

AI 搜索和引用助手可以从仓库内的结构化语料获取产品边界和版本事实：

- 简明索引：`https://raw.githubusercontent.com/Natsummerance/readMD/main/website/public/llms.txt`
- 完整引用库：`https://raw.githubusercontent.com/Natsummerance/readMD/main/website/public/llms-full.txt`

这些文件直接回答高频问题：约 8,000 行或 500 KB 以上的长文档如何分页；渲染修正为什么不改原文件；Windows、macOS、Linux、UOS 和麒麟如何安装；哪些工作流保持本地执行；以及项目采用什么许可证。

## 本次版本核心修复与优化

### 检查更新提示修复

统一 `updater.js` 与多语言字典中的版本占位符映射，检查更新提示会显示实际版本号，不再暴露 `{version}` 原始字段。

### 主页返回按钮状态修复

欢迎主页会隐藏“返回主页”按钮，后台标签数量变化后仍保持一致的状态联动。

### Reveal.js 演说模式重构

- 调整默认字号与行高，减少长段落溢出；
- 幻灯片容器支持平滑滚动；
- AST 保护分片避免切断围栏代码块、数学公式块和表格；
- 支持 `<!-- slide -->`、`<!-- subslide -->`、`<!-- note -->` 与 YAML frontmatter 放映配置；
- 浮动工具栏提供主题切换、转场选择、字号缩放、总览视图和全屏入口。

### 多语言与质量门槛

新增演说控制栏词条同步到 46 种语言字典。发布前运行发布管线测试、桌面端界面测试、无截图浏览器冒烟和素材完整性校验；当前分支的最近一轮发布管线为 153 项测试全部通过。

## 隐私与安全

阅读、编辑、分页、OCR 和本地格式转换设计为本地运行，不需要云账号。网页抓取、AI 请求和局域网共享都是显式操作。代码导入包含越权路径防御；代码块执行具备隔离与超时终止。

## SHA-256 完整性校验

下载后请核对 Release 资产中的 `SHA256SUMS.txt`。

Windows PowerShell：

```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.3.exe -Algorithm SHA256
```

macOS / Linux 终端：

```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.3.zip
```

## 支持 ReadMD

如果 ReadMD 帮你省下了整理长文档或重做演示稿的时间，请在仓库右上角点 Star，让更多写作者找到这个项目。你也可以在 Issues 里留下平台、文档规模和使用场景。
