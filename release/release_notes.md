# ReadMD v2.2.8

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.8.exe`
- Windows 便携版：`ReadMD-portable-v2.2.8.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.8.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.8.zip`
- 校验清单：`SHA256SUMS.txt`

## 本次更新

1. **软件内自动检查与本地更新系统**：
   - 启动后后台静默检测 GitHub Releases 最新版本；
   - 底部状态栏微光徽章 + 更多功能菜单小红点提示；
   - 更新弹窗内嵌完整 Markdown 渲染的更新日志，展示对应资产大小；
   - 客户端一键下载、实时进度条、实时速度展示与 SHA256 完整性散列校验；
   - 支持 Windows 安装版自动重启覆盖安装、便携版进程级热更替换以及国内镜像加速通道。
2. **LaTeX 公式全量兼容与公式自修复算法**：
   - 全面兼容 TeX 原生多行环境（`\begin{cases}`, `\begin{align}`, `\begin{aligned}`, `\begin{matrix}`, `\begin{pmatrix}`, `\begin{equation}`, `\begin{gather}` 等）；
   - 启发式自修复引擎：自动配平未闭合花括号 `{}`、恢复双反斜杠换行转义、还原 HTML 实体符号、数学常用 Unicode 符号（`×`, `÷`, `≤`, `≥`, `≠`, `±`, `∞`, `∑`, `∫`, `√`, `α-ω` 等）自动规范化为 LaTeX 指令；
   - 异常容错机制：公式解析出错优雅降级为源码卡片，杜绝页面崩溃白屏。
3. **OCR 扫描结果智能排版与格式规范化**：
   - 清除 CJK 汉字之间由 OCR 引擎注入的无意义虚假空格；
   - 修复英文跨行断字连字符（如 `infor-\nmation` -> `information`）；
   - 智能合并同一句子内被 OCR 强制切断的碎片化硬换行，保留段落自然段；
   - 自动识别提取章节标题（`#` / `##`）与有序/无序列表项（`1.` / `-`）。
4. **无打开文档时搜索功能置灰与保护**：
   - 欢迎页与空文档状态下，顶栏搜索按钮自动设为 `disabled` 置灰，拦截 `Ctrl+F` 快捷键；
   - 关闭所有标签返回主页时自动关闭搜索栏并清理高亮。
5. **欢迎页 AI 助手按钮事件响应修复**：
   - 彻底修复欢迎页模块按钮由于双重事件绑定导致单次点击被立即关闭的问题，保证首次点击即可稳定唤出 AI 助手面板。
6. **安装程序窗口化界面去重**：

   - 移除 HTML 内部冗余的模拟关闭/最小化按钮，统一由操作系统原生窗口外框统一控制，解决界面上方出现“两个叉叉”的问题。
7. **万物转 MD 拖拽与批量转换自动开标签**：
   - 拖入 Word、PDF、PPT、Excel、EPUB、TXT 或在弹窗中批量转换后，自动将转换生成的 Markdown 文档打开至新标签页。
8. **导出排版动态高保真预览**：
   - 根据预设（简约/经典/商务）与个性化配置实时生成样式表，精准映射正文字体、段距、标题阶梯、表格边框斑马纹、代码块与引用，彻底解决暗色模式及主题切换下的白底白字/黑底黑字问题。
9. **Win7 兼容版同步升级到 v2.2.8**：
   - 更新 Win7 打包构建链并保持核心功能兼容。



## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用“更多信息 → 仍要运行”。macOS 解压后请在 Finder 中右键 `ReadMD.app`，选择“打开”；如仍被阻止，可在“系统设置 → 隐私与安全性”确认打开。

## SHA-256 校验

下载对应文件和 `SHA256SUMS.txt` 后，核对文件名对应的一行。

Windows PowerShell：

```powershell
Get-FileHash .\ReadMDSetup-v2.2.8.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

macOS 终端：

```bash
shasum -a 256 ReadMD-macos-arm64-v2.2.8.zip
cat SHA256SUMS.txt
```

两处哈希值完全一致才继续使用。SHA-256 用于完整性校验，不代表代码签名。
