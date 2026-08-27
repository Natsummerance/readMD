# ReadMD V2.3.7-beta.6 新增国际化 (i18n) 词条跟踪清单

本清单实时跟踪本次「万物皆可开 · 万物皆可转 MD · AI 深度生态集成」所有新增的前端界面文字与 i18n key。功能全部实现后，将对照此清单批量同步并校验 46 种语言。

---

## 1. 万物皆可开 · 通用代码与配置查看器 (codebar.*)

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `codebar.format` | 格式：{fmt} | Format: {fmt} | 代码/配置文件顶部信息栏格式展示 |
| `codebar.lines` | {count} 行 | {count} lines | 行数统计 |
| `codebar.aiToMd` | AI 结构化转 MD | AI Convert to MD | 顶部操作栏：调用 AI 转化为结构化 Markdown |
| `codebar.edit` | 编辑源码 (Ctrl+E) | Edit Source (Ctrl+E) | 顶部操作栏：进入源码编辑器 |
| `codebar.aiExplain` | AI 深度解析 | AI Explain | 顶部操作栏：调用 AI 分析代码或配置 |
| `codebar.copyCode` | 复制代码 | Copy Code | 复制代码内容 |

---

## 2. 编辑器 AI 助手与行内补全 (editai.*)

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `editai.title` | AI 编辑助手 (Alt+K) | AI Editor Assistant (Alt+K) | 编辑器 AI 浮层标题 |
| `editai.placeholder` | 告诉 AI 如何修改，或直接点击下方快捷动作... | Tell AI how to edit, or click quick actions below... | AI 编辑输入框占位符 |
| `editai.actComplete` | 智能续写补全 | Smart Completion | 快捷动作：根据光标前文自动补全下一段落 |
| `editai.actPolish` | 润色选中文本 | Polish Selection | 快捷动作：优化语法、去除语病与繁复表达 |
| `editai.actFix` | 排版与语法自愈 | Fix Formatting & Syntax | 快捷动作：自动修复表格对齐、公式与格式 |
| `editai.actTranslate` | 翻译为英文/中文 | Translate | 快捷动作：选区精准翻译 |
| `editai.apply` | 应用替换 (Tab) | Apply (Tab) | 浮层操作按钮：应用生成的文本 |
| `editai.insert` | 插入光标处 | Insert at Cursor | 浮层操作按钮：在光标处插入内容 |
| `editai.discard` | 放弃 (Esc) | Discard (Esc) | 浮层操作按钮：放弃生成内容 |
| `editai.generating` | AI 正在深度生成中... | AI is generating... | 生成状态提示 |

---

## 3. AI 自定义导出排版设计师 (exportai.*)

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `exportai.title` | AI 排版 | AI Typography | 导出面板中的 AI 排版标题（右上侧） |
| `exportai.placeholder` | 例如：学术顶会 LaTeX 论文排版 / 莫兰迪极简画册风格 / 商务黑金极简... | e.g. Academic LaTeX paper / Morandi minimal / Business dark gold... | 风格输入框占位符 |
| `exportai.generateBtn` | AI 生成排版风格 | Generate Style with AI | 生成按钮 |
| `exportai.generating` | 正在分析并设计排版参数... | Designing typography parameters... | 正在生成提示 |
| `exportai.applied` | 已应用 AI 生成的排版样式预设 | AI typography preset applied | 成功应用提示 |

---

## 4. 万物转 MD 与自愈操作 (convert.*)

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `convert.aiFixFormat` | AI 自愈排版 | AI Fix Formatting | 转换完成后的排版自愈按钮 |
| `convert.aiFixDesc` | 使用 AI 深度修复表格对齐、公式排版与断行问题 | Use AI to deep-fix table alignments, formulas, and line wraps | 说明提示 |
