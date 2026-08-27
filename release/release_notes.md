# ReadMD v2.3.7-beta.4 (安全放映与轻量阅读修复)

ReadMD 是免费、开源的本地 Markdown 智能阅读、编辑与全格式转换排版套件；纯本地离线可用、秒级极速渲染，绝不改写原文件。

## 📦 全平台发布资产 (Release Assets)

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.7-beta.4.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.7-beta.4.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.4.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.4.zip`
- 🐧 Linux x86_64 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage`
- 🇨🇳 国产信创 AMD64 Deb：`readmd_2.3.7-beta.4_amd64.deb`
- 🖥️ 麒麟 V10 / 飞腾 ARM64 AppImage：`ReadMD-linux-aarch64-v2.3.7-beta.4.AppImage`
- 🖥️ 麒麟 V10 / 飞腾 ARM64 Deb：`readmd_2.3.7-beta.4_arm64.deb`
- 📱 HarmonyOS NEXT / OpenHarmony：ArkTS 源码工程（DevEco Studio 构建；当前不提供预编译 HAP）
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.4.vsix`
- 🤖 FastMCP Server 源码包：`readmd-mcp-server-2.3.7-beta.4.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

## 🌟 本次版本核心修复与优化

### 1. 演讲演示完全本地离线渲染
- Reveal.js、Markdown、高亮、备注、KaTeX、主题和字体全部纳入本地 `assets/vendor`；
- 演示页在 CSP 下不再请求 jsDelivr 或其他外部 CDN，公式、表格与幻灯片初始化不再被网络安全策略截断；
- 应用内 Reveal.js 只加载同源离线资源；幻灯片 HTML 走白名单净化，移除脚本、iframe、事件处理器和危险 URL。

### 2. 禅模式一次进入，真正沉浸
- 合并重复的 Zen 状态实现，修复 F11 被两个监听器连续处理导致“进入又立即退出”的问题；
- 进入禅模式时立即隐藏工作台噪声并稳定正文布局，工具栏保留顶部悬停唤出；
- 回归测试覆盖 F11 单次进入、Esc 退出和多帧布局稳定性。

### 3. 多标签页即时反馈与溢出对齐
- 切换标签页时先同步高亮活动标签，再用渲染代数丢弃过期结果，避免快速切换后的旧内容回写；
- 标签栏恢复父容器宽度约束，溢出时自动滚动到活动标签；
- 回归测试覆盖 18 个标签快速切换、唯一选中态、等高排列和末尾标签可见性。

### 4. 前端控件样式与无障碍 (ARIA) 接入
- 自定义样式、代码块、图表、子文档引用和 Frontmatter 弹层统一使用共享表单与按钮样式；
- 清理冗余内联样式，为所有模态框、表单控件与分栏拖拽条补齐 ARIA 标准属性；
- 46 种语言国际化全量覆盖（1064 词条 100% 完整）。

### 5. 安全边界与启动性能
- 更新器只接受官方 GitHub Release 资产；下载先写入临时文件，SHA-256 通过后原子发布并在执行前复检。
- LAN 共享安全加固：限制在共享目录内，防止越权访问。
- 图片保存限制大小、验证真实图像格式，并将锚定后的文件名限制在当前文档 `images/` 目录内。
- 入口脚本有序延迟加载（`defer`），二维码库改为共享面板按需动态加载。

## 平台与兼容

- 持续提供 x86_64 与 ARM64 Linux 资产，覆盖银河麒麟 V10、飞腾 D2000/E2000、UKUI/X11 和旧版 WebKit 运行环境。
- ARM64 构建内置软件渲染安全回退，检查更新按 CPU 架构选择对应包。
- 打包发布前执行五次启动探针，校验 `page_loaded` 中位数和服务到窗口创建开销。

## 🛠️ SHA-256 完整性校验

Windows PowerShell：
```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.4.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.4.zip
```
