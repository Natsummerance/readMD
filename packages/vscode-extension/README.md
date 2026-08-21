# ReadMD for VSCode —— 高性能 Markdown 智能预览与自愈修复

<p align="center">
  <img src="https://raw.githubusercontent.com/Natsummerance/readMD/main/assets/ReadMD.png" alt="ReadMD Logo" width="120" height="120" />
</p>

<p align="center">
  <b>轻量 · 极速 · 优雅 · 专业的本地 Markdown 智能阅读与排版编辑器扩展</b>
</p>

<p align="center">
  <a href="https://github.com/Natsummerance/readMD"><img src="https://img.shields.io/badge/GitHub-Natsummerance%2FreadMD-blue?logo=github" alt="GitHub"></a>
  <a href="https://github.com/Natsummerance/readMD/releases"><img src="https://img.shields.io/badge/Version-v2.3.6-3b82f6" alt="Version"></a>
  <a href="https://github.com/Natsummerance/readMD/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License"></a>
</p>

---

## 🌟 简介

**ReadMD for VSCode** 是开源 Markdown 阅读神器 [ReadMD](https://github.com/Natsummerance/readMD) 的官方 Visual Studio Code 扩展。它将 ReadMD 广受好评的**极速排版渲染**、**格式智能自愈**、以及 **LaTeX PRO 学术级数学公式支持** 无缝集成进 VS Code 与 Cursor 编辑器中。

无论是日常笔记编写、学术论文草稿排版，还是处理格式错乱的旧文档，ReadMD 都能为您提供极致舒适的写作与阅读体验。

---

## ✨ 核心特性

### 1. 📖 智能实时双向同步预览
- 采用高性能渲染引擎，实时同步编辑器光标与滚动位置；
- 内置优雅的排版样式体系，无论是大段正文、代码块还是嵌套列表均具有极佳的视觉韵律；
- 支持深色 / 浅色模式自适应跟随系统及编辑器主题。

### 2. 🛠️ 格式自愈与深度诊断 (Auto-Fix)
- **公式自愈**：自动识别并修复行内公式 `$ ... $` 与独立公式 `$$ ... $$` 中被破坏的标点、换行与转义符号；
- **排版规范化**：自动优化中英文间距（盘古规范）、修复错误的代码块闭合标记与缩进混乱；
- **智能表格修复**：自动对齐错位表格管道符 `|`，纠正不规范的对齐声明分隔行。

### 3. 📐 学术级 LaTeX PRO 与 BibTeX 支持
- 支持复杂多行公式环境：`aligned`、`matrix`、`cases`、`gather` 等；
- 支持定理、引理、证明、定义等学术 Callout 引用框；
- 智能解析 BibTeX 参考文献格式，自动构建可交互的高亮引用卡片。

### 4. 🔄 Markdown ⇄ 学术 LaTeX 一键互转
- 一键将当前 Markdown 文档深度解析编译为标准学术 LaTeX 源码；
- 保留完整的标题层级、粗斜体、公式、代码高亮与表格排版，方便无缝导入 Overleaf 等学术工具。

---

## 🚀 快速上手

### 快捷指令与操作入口

| 操作 | 触发方式 | 说明 |
| :--- | :--- | :--- |
| **打开实时预览** | 点击编辑器右上角 `📖 图标` 或按 `F1` 输入 `ReadMD: Open Custom Preview` | 在右侧分栏打开与当前文档绑定的实时渲染视图 |
| **格式自愈修复** | 编辑器内任意位置右键 ➔ `ReadMD: 智能诊断并自愈修复格式错误` | 一键无损重构并修正当前文件的排版与公式格式 |
| **转为 LaTeX 源码** | 编辑器内任意位置右键 ➔ `ReadMD: 一键编译转为学术 LaTeX 源码` | 生成符合学术规范的标准 `.tex` 源码 |

---

## 🧩 扩展设置与依赖

- 本扩展为纯净本地实现，**完全离线运行**，不上传任何文档内容；
- 默认支持所有 `.md`、`.markdown`、`.mdown`、`.mdx` 文件；
- 完美兼容 **VS Code** 与 **Cursor / VSCodium / Gitpod** 等生态编辑器。

---

## 📄 开源协议与反馈

- **许可证**：[MIT License](https://github.com/Natsummerance/readMD/blob/main/LICENSE)
- **问题反馈与建议**：欢迎前往 [GitHub Issues](https://github.com/Natsummerance/readMD/issues) 提交反馈与讨论！
