# ReadMD v2.3.7-beta.4 (安全放映与轻量阅读修复)

ReadMD 是免费、开源的本地 Markdown 智能阅读、编辑与全格式转换排版套件；纯本地离线可用、秒级极速渲染，绝不改写原文件。

## 发布资产

- Windows 安装版：`ReadMDSetup-v2.3.7-beta.4.exe`
- Windows 便携版：`ReadMD-portable-v2.3.7-beta.4.exe`
- Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.4.zip`
- Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.4.zip`
- Linux x86_64 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage`
- 国产信创 AMD64 Deb：`readmd_2.3.7-beta.4_amd64.deb`
- 麒麟 V10 / 飞腾 ARM64 AppImage：`ReadMD-linux-aarch64-v2.3.7-beta.4.AppImage`
- 麒麟 V10 / 飞腾 ARM64 Deb：`readmd_2.3.7-beta.4_arm64.deb`
- HarmonyOS NEXT / OpenHarmony：ArkTS 源码工程（DevEco Studio 构建；当前不提供预编译 HAP）
- VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.4.vsix`
- FastMCP Server 源码包：`readmd-mcp-server-2.3.7-beta.4.zip`
- 校验清单：`SHA256SUMS.txt`

## 本次修复

### 1. 演示放映与前端审计
- 应用内 Reveal.js 只加载同源离线资源；幻灯片 HTML 继续走白名单净化，移除脚本、iframe、事件处理器和危险 URL。
- 清理重复 Zen 实现，F11 在编辑态和阅读态都稳定进入沉浸模式；Esc 退出路径保持一致。
- 标签页立即刷新选中态和 ARIA 状态；渲染代数丢弃过期结果，溢出时自动滚动到活动标签。
- 补齐弹层表单、AI 历史空态、导出预览标题、外部图片链接和外部变更标签等样式状态。

### 2. 安全边界
- 更新器只接受官方 GitHub Release 资产；文件名固定在更新目录内，下载先写入临时文件，SHA-256 通过后原子发布并在执行前复检。
- 更新安装器改用参数向量启动，不再通过 shell 拼接命令。
- LAN 共享不再携带桌面控制令牌；保存、上传、代码执行、更新执行等特权路由被拒绝，文档访问限定在共享目录。
- 图片保存限制大小、验证真实图像格式，并将锚定后的文件名限制在当前文档 `images/` 目录内。

### 3. 启动与交互性能
- 入口脚本有序延迟加载，二维码库改为共享面板按需加载，并新增启动资源和耗时预算门禁。
- 本地静态资源提供强 ETag 和 Last-Modified，支持 `If-None-Match` / `If-Modified-Since` 304 重验证。
- 移动端和粗指针环境提升工具栏、标签页、搜索、分页与演示控件触控目标。
- 搜索 Enter 可连续导航且保持输入焦点；欢迎页 Ctrl+P 与按钮守卫行为一致。

## 平台与兼容

- 持续提供 x86_64 与 ARM64 Linux 资产，覆盖银河麒麟 V10、飞腾 D2000/E2000、UKUI/X11 和旧版 WebKit 运行环境。
- ARM64 构建内置软件渲染安全回退，检查更新按 CPU 架构选择对应包。
- 打包发布前执行五次启动探针，校验 `page_loaded` 中位数和服务到窗口创建开销。

## 完整性校验

Windows PowerShell：
```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.4.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.4.zip
```
