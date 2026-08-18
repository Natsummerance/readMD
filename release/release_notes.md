# ReadMD v2.2.7

ReadMD 是免费的本地 Markdown 阅读器；不会要求订阅或内置账号。

## 下载

- Windows 安装版：`ReadMDSetup-v2.2.7.exe`
- Windows 便携版：`ReadMD-portable-v2.2.7.exe`
- Intel Mac：`ReadMD-macos-x64-v2.2.7.zip`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.2.7.zip`
- 校验清单：`SHA256SUMS.txt`

## 本次更新

1. **多标签页窗口模式 (Multi-Tab System)**：
   - 顶栏中央自适应标签条（窄屏降级独立条）；
   - 标签过多自动折叠下拉（Hover 临时显示 / Click 锁定常驻）；
   - 支持 HTML5 拖拽调整标签顺序；
   - 双击标签标题就地重命名（联动本地磁盘文件或虚拟文档名称）；
   - 右键菜单支持“关闭标签”、“关闭其他”、“关闭所有”、“复制文件路径”；
   - 未保存文档关闭时弹窗确认，全部关闭后自动返回主页。
2. **全格式拖拽支持与质感悬浮动效**：
   - 拦截系统默认拖入导航，实现智能分流（Markdown 标签页打开 / Office & PDF 批量转换 / URL 网页抓取 / 纯文本极速建档）；
   - 全屏毛玻璃光晕遮罩与动态提示。
3. **剪贴板极速建档与保存**：
   - 剪贴板有文本时在主界面按 `Ctrl+V` 即刻生成虚拟 Markdown 页面；
   - 按 `Ctrl+S` 可选择位置直接持久化保存。
4. **导出文档高保真实时预览与配置折叠**：
   - 导出弹窗右侧配置项默认折叠收拢；
   - 左侧下方嵌入实时微缩纸张排版预览，支持点击呼出放大预览大窗。
5. **返回主页按钮位置调整**：
   - 整合移至底部状态栏（`#statusbar`）右侧模块控制区旁。
6. **侧边栏收起逻辑修复**：
   - 打开状态下点击目录按钮（`btn-toc`）直接收回侧边栏，不再错误切换到 TOC。
7. **编辑界面预览方向按键修复**：
   - 修复 `.pv-grid` 中上方预览按钮（`↑`）错位至左上的问题。
8. **网页抓取默认允许本地网络**：
   - 默认开启局域网/本地网络服务网页正文提取。
9. **Win7 兼容版同步更新到 v2.2.7**：
   - 更新 Win7 打包构建链并保持核心功能兼容。

## 安装与安全提示

Windows 与 macOS 安装包均为**未签名**版本。Windows 首次运行可能显示 SmartScreen；请确认文件来自本 Release，并按下方 SHA-256 方法核对后，再使用“更多信息 → 仍要运行”。macOS 解压后请在 Finder 中右键 `ReadMD.app`，选择“打开”；如仍被阻止，可在“系统设置 → 隐私与安全性”确认打开。

## SHA-256 校验

下载对应文件和 `SHA256SUMS.txt` 后，核对文件名对应的一行。

Windows PowerShell：

```powershell
Get-FileHash .\ReadMDSetup-v2.2.7.exe -Algorithm SHA256
Get-Content .\SHA256SUMS.txt
```

macOS 终端：

```bash
shasum -a 256 ReadMD-macos-arm64-v2.2.7.zip
cat SHA256SUMS.txt
```

两处哈希值完全一致才继续使用。SHA-256 用于完整性校验，不代表代码签名。

