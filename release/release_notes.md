# ReadMD v2.3.7-beta.3 (演说模式重构与体验精细化发布)

ReadMD 是免费、开源的本地 Markdown 智能阅读、编辑与全格式转换排版套件；纯本地离线可用、秒级极速渲染，绝不改写原文件。

## 📦 全平台发布资产 (Release Assets)

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.7-beta.3.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.7-beta.3.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.3.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.3.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.3.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包 (UOS / 银河麒麟 / Deepin / Ubuntu / Debian)：`readmd_2.3.7-beta.3_amd64.deb`
- 🖥️ 麒麟 V10 / 飞腾 ARM64：`ReadMD-linux-aarch64-v2.3.7-beta.3.AppImage` 与 `readmd_2.3.7-beta.3_arm64.deb`
- 📱 HarmonyOS NEXT / OpenHarmony：ArkTS 源码工程（DevEco Studio 构建）；当前不提供预编译 HAP
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.3.vsix`
- 🤖 FastMCP Server 源码包：`readmd-mcp-server-2.3.7-beta.3.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

---

## 🌟 本次版本核心修复与优化 (v2.3.7-beta.3)

### 1. 检查更新提示 `{version}` 占位符裸露修复
- **精准版本号替换**：彻底修复 `updater.js` 与 46 种语言 i18n 字典的占位符参数映射，检查更新弹出 Toast 统一准确显示当前版本（如 `当前已是最新版本 (v2.3.7-beta.3)`），绝无裸露字段名。

### 2. 返回主页后右下角「返回主页」按钮状态联动修复
- **状态感知联动**：修复多标签页状态栏 `renderTabsBar()` 在欢迎主页时的按钮显示逻辑，无论当前是否存在后台标签，只要处于欢迎主页（`state.mode === 'welcome'`），右下角返回主页按钮严格隐藏。

### 3. 演讲演示（Reveal.js）深度优化与自定义支持 (对齐 MPE 规范)
- **排版与舒适演说字号重构**：将默认过大的 Reveal.js 字体调整为精致舒适尺寸（基准字号 24px，标题 1.5~1.8em，行高 1.65），杜绝因字号过大导致的多段落溢出与截断；
- **智能防溢出滚动**：单页幻灯片容器增加优雅平滑滚动（`overflow-y: auto`），长文章与大表格内容 100% 完整展示；
- **代码块与表格保护分片**：采用 AST 占位保护分片算法，智能自愈切片时严格保证围栏代码块（```）、数学公式块（$$）与表格不被腰斩截断；
- **MPE 语法全量对齐**：完整支持 `<!-- slide -->` / `---`（横向）、`<!-- subslide -->` / `--`（垂直下钻）、`<!-- note -->`（演讲者备注）以及 YAML Frontmatter `presentation:` 配置；
- **演说悬浮控制栏（Floating Quick Toolbar）**：在放映界面右上角提供精致毛玻璃悬浮工具栏，支持即时切换 11 款专业主题（Black, White, League, Night, Serif, Simple 等）、6 种转场特效（Slide, Fade, Zoom 等）、字号缩放（`A-` 20px / `A` 24px / `A+` 28px）、总览视图（`O` 键）与一键全屏（`F11`）。

### 4. 全球 46 国语言 i18n 与前端自动化测试 100% 覆盖
- **46 种语言 100% 对齐**：所有新增演示控制栏词条全量同步至 46 种语言 JSON 字典文件（1,017 词条，0 缺失）；
- **端到端测试全覆盖**：全量 25 项 Playwright UI 端到端测试与 340 项单元/压力测试 100% 通过。

### 5. 麒麟 V10 / 飞腾 ARM64 原生支持
- 新增 `aarch64` Deb 与 AppImage 构建，覆盖银河麒麟 V10、UKUI/X11 以及飞腾 D2000/E2000 等国产 ARM 设备；
- 启动时自动识别旧版 GPU 栈，切换到软件渲染安全回退，避免 WebKitGTK 白屏或崩溃；
- 检查更新按 CPU 架构精准选择 AMD64 或 ARM64 安装包。

### 6. 秒开启动优化
- 将 28 个阻塞启动脚本合并为单个有序运行时包，冷启动资源请求从 32 个降至约 5 个；
- 本地服务启用持久连接，减少 WebView 与静态资源之间的重复连接开销；
- 界面骨架先进入可交互状态，偏好设置与语言包在后台继续加载；
- 跨页搜索与结果跳转改为同步重建当前页高亮，避免低端设备帧调度饥饿导致定位丢失；
- 实测浏览器冷启动 DOM 就绪时间由约 1,514 ms 降至约 392 ms。

---

## 🔒 隐私与安全

- **纯本地运算**：Markdown 自愈、图表解析、Code Chunk 执行、EPUB 打包均在本地沙箱完成；
- **安全沙箱**：`@import` 包含越权路径防御与死循环防护，Code Chunk 具备进程隔离与超时自动终止；
- **零凭证泄露**：全仓经过严苛自动化隐私扫描，无任何硬编码密钥或外部未经授权的网络回传。

---

## 🛠️ SHA-256 完整性校验

下载对应文件后，核对文件名对应的一行：

Windows PowerShell：
```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.3.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.3.zip
```
