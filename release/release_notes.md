# ReadMD v2.3.7-beta.4 (离线演说与沉浸写作修复)

ReadMD 是免费、开源的本地 Markdown 智能阅读、编辑与全格式转换排版套件；纯本地离线可用、秒级极速渲染，绝不改写原文件。

## 📦 全平台发布资产 (Release Assets)

- 🪟 Windows 安装版：`ReadMDSetup-v2.3.7-beta.4.exe`
- 💼 Windows 便携版：`ReadMD-portable-v2.3.7-beta.4.exe`
- 🍏 Apple Silicon Mac：`ReadMD-macos-arm64-v2.3.7-beta.4.zip`
- 💻 Intel Mac：`ReadMD-macos-x64-v2.3.7-beta.4.zip`
- 🐧 Linux 通用 AppImage：`ReadMD-linux-x86_64-v2.3.7-beta.4.AppImage`
- 🇨🇳 Linux / 国产信创 Deb 安装包：`readmd_2.3.7-beta.4_amd64.deb`
- 🧩 VSCode 扩展离线包：`readmd-vscode-2.3.7-beta.4.vsix`
- 🤖 FastMCP Server 源码包：`readmd-mcp-server-2.3.7-beta.4.zip`
- 🔐 校验清单：`SHA256SUMS.txt`

---

## 🌟 本次版本核心修复与优化

### 1. 演讲演示完全本地离线渲染
- Reveal.js、Markdown、高亮、备注、KaTeX、主题和字体全部纳入本地 `assets/vendor`；
- 演示页在 CSP 下不再请求 jsDelivr 或其他外部 CDN，公式、表格与幻灯片初始化不再被网络安全策略截断；
- 新增端到端审计验证零外部请求、Reveal `ready` 状态、KaTeX 渲染和悬浮工具栏可用性。

### 2. 禅模式一次进入，真正沉浸
- 合并重复的 Zen 状态实现，修复 F11 被两个监听器连续处理导致“进入又立即退出”的问题；
- 进入禅模式时立即隐藏工作台噪声并稳定正文布局，工具栏保留顶部悬停唤出；
- 回归测试覆盖 F11 单次进入、Esc 退出和多帧布局稳定性。

### 3. 多标签页即时反馈与溢出对齐
- 切换标签页时先同步高亮活动标签，再用渲染代数丢弃过期结果，避免快速切换后的旧内容回写；
- 标签栏恢复父容器宽度约束，溢出时自动滚动到活动标签；
- 回归测试覆盖 18 个标签快速切换、唯一选中态、等高排列和末尾标签可见性。

### 4. 前端控件样式接入清理
- 自定义样式、代码块、图表、子文档引用和 Frontmatter 弹层统一使用共享表单与按钮样式；
- 清理裸露的 `btn-primary` / `btn-secondary` 和重复内联样式；
- 学术 Callout、AI 历史空态、大纲空态和外部修改标记补齐明确视觉状态。

---

## 🔒 隐私与安全

- **纯本地运算**：Markdown 自愈、图表解析、Code Chunk 执行、EPUB 打包均在本地沙箱完成；
- **离线演示**：Reveal/KaTeX 资源随应用分发，放映阶段不依赖第三方网络；
- **安全沙箱**：`@import` 包含越权路径防御与死循环防护，Code Chunk 具备进程隔离与超时自动终止。

---

## 🛠️ SHA-256 完整性校验

Windows PowerShell：
```powershell
Get-FileHash .\ReadMDSetup-v2.3.7-beta.4.exe -Algorithm SHA256
```

macOS / Linux 终端：
```bash
shasum -a 256 ReadMD-macos-arm64-v2.3.7-beta.4.zip
```
