# ReadMD v2.3.7-beta.4 (前端审计修复与安全放映)

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

### 1. 前端样式接入审计
- 补齐危险操作按钮、AI 历史空态、导出预览标题、外部图片链接、目录当前页和标签外部变更提示等状态样式。
- 将模态框表单控件的重复内联样式收敛为统一样式规则，降低主题适配漂移风险。
- 外部变更标签现在使用独立警示色，不再与普通未保存点混淆。

### 2. Render PPT 安全策略与真实放映
- 移除 `<meta>` 中无法生效且会持续告警的 `frame-ancestors`，首页 HTTP 响应改用 `X-Frame-Options: DENY` 提供防嵌入保护。
- 应用内 Reveal.js 演示继续只加载同源离线资源；Playwright 在禁用 CSP 绕过后验证两页幻灯片真实初始化。
- 用户幻灯片 HTML 继续走白名单净化：脚本、iframe、事件处理器和危险 URL 会被移除，Markdown、代码样例和安全 HTML 保留。

### 3. 禅模式稳定进入沉浸阅读
- 清理新旧两套禅模式实现冲突，移除重复 `toggleZenMode` 覆盖。
- 进入时禁用首帧工具栏过渡，不弹出打扰阅读的提示，并保持正文原位不跳动。
- 阅读态焦点转移到正文容器；编辑态仍回到编辑器。`Esc` 退出路径保持可用。

### 4. 文件标签页响应与状态一致性
- 切换标签时立即刷新选中态和 ARIA 状态，不再等待大文档渲染完成。
- 连续阅读位置同时同步到标签与全局缓存，往返切换后恢复到原滚动位置。
- 标签溢出检测延后到布局帧复核，减少字体加载或窗口变化造成的错位与滞后。
- 下拉列表改为安全文本节点渲染，避免异常文件名注入 HTML。

## 平台与兼容

- 持续提供 x86_64 与 ARM64 Linux 资产，覆盖银河麒麟 V10、飞腾 D2000/E2000、UKUI/X11 和旧版 WebKit 运行环境。
- ARM64 构建内置软件渲染安全回退，检查更新按 CPU 架构选择对应包。
- 更新器仅应用 updater 校验过的文件，安装前复检 SHA-256 并拒绝不安全文件名。

## 完整性校验

Windows PowerShell：
```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.4.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.4.zip
```
