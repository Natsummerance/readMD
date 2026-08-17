# ReadMD v2.2.0

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.0.exe`
- Windows 便携版：`ReadMD-portable-v2.2.0.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.0.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.0.zip`

macOS 包暂未签名。解压后首次启动请在 Finder 中右键 `ReadMD.app` 并选择“打开”；若系统仍阻止运行，请在“系统设置 → 隐私与安全性”中允许打开。

## 本次更新

- 修复导出设置全部展开后遮挡的问题，弹窗标题和操作区固定，设置内容独立滚动。
- 修复顶栏“最近文件”按钮无响应，阅读和编辑状态均可打开历史面板。
- AI 配置升级到 schema v2：移除旧私人配置、支持自定义连接 CRUD、API Key 不再通过接口回传、模型改为获取后下拉选择，面板可拖拽调宽。
- 编辑工具栏收敛为单行，新增分组 Markdown 命令、可搜索命令面板和分类公式选择器。
- 图片编辑升级为八向裁剪、任意角度、翻转、画布缩放/平移、输出尺寸与撤销重做。
- 预览方向改为工具栏右侧紧凑停靠组件，支持拖拽调整编辑/预览比例；桌面窄窗口不再把左右预览错误放到下方。
- 新增 macOS 原生数据目录、Finder 打开/定位、Vision OCR、文档类型声明及 Intel/Apple Silicon 双架构构建。

## 隐私说明

v2.2.0 首次启动会清空旧版 AI 自定义连接及当前选择，用户需要重新配置。官方预设保留；源码、接口响应和发布产物不包含旧私人连接或 API Key 明文。

## 校验

Release 由 GitHub Actions 在 Windows、Intel macOS 与 Apple Silicon macOS 全部测试通过后自动生成。工作流会验证文件名、非零大小、应用版本、目标架构与 SHA-256。
