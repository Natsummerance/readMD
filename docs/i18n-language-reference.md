# ReadMD i18n 语言本地化参考文档

> 版本：v2.3.1 | 最后更新：2026-08-19 | 语言总数：46

---

## 目录

- [语言文件结构](#语言文件结构)
- [专有名词保护规范](#专有名词保护规范)
- [占位符规范](#占位符规范)
- [分区语言清单](#分区语言清单)
- [Key-分类说明](#key-分类说明)
- [翻译质量基线](#翻译质量基线)
- [新增语言指南](#新增语言指南)

---

## 语言文件结构

所有语言字典存放于 `assets/i18n/<code>.json`，格式为 UTF-8 编码的 JSON，共 **1109 个键值对**，分 25 个模块前缀组织。键数量由 `tools/i18n_sync.py --validate-only` 在发布门禁中复核。

```
assets/i18n/
├── zh-CN.json    # 简体中文（翻译源基准）
├── zh-HK.json    # 繁体中文（香港）
├── zh-TW.json    # 繁体中文（台湾）
├── en.json       # 英语（翻译源基准）
├── ja.json       # 日语
├── ko.json       # 韩语
├── fr.json       # 法语
├── de.json       # 德语
├── es.json       # 西班牙语
├── pt.json       # 葡萄牙语
├── ru.json       # 俄语
├── it.json       # 意大利语
├── ar.json       # 阿拉伯语（RTL）
├── he.json       # 希伯来语（RTL）
├── ug.json       # 维吾尔语（RTL）
├── bo.json       # 藏语
├── mn.json       # 蒙古语
├── th.json       # 泰语
├── vi.json       # 越南语
├── id.json       # 印度尼西亚语
├── hi.json       # 印地语
├── bn.json       # 孟加拉语
├── my.json       # 缅甸语
├── lo.json       # 老挝语
├── km.json       # 高棉语
├── ms.json       # 马来语
├── ne.json       # 尼泊尔语
├── tr.json       # 土耳其语
├── el.json       # 希腊语
├── hu.json       # 匈牙利语
├── uk.json       # 乌克兰语
├── hr.json       # 克罗地亚语
├── sl.json       # 斯洛文尼亚语
├── da.json       # 丹麦语
├── no.json       # 挪威语
├── sv.json       # 瑞典语
├── fi.json       # 芬兰语
├── nl.json       # 荷兰语
├── ro.json       # 罗马尼亚语
├── ga.json       # 爱尔兰语
├── mt.json       # 马耳他语
├── tl.json       # 菲律宾语
├── eo.json       # 世界语
├── kl.json       # 格陵兰语
├── rw.json       # 卢旺达语
├── kg.json       # 刚果语
└── meta.json     # 语言元数据（名称、方向、区域）
```

---

## 专有名词保护规范

以下术语在所有语言中**保持英文原样**，不翻译：

| 类别 | 保护词汇 |
|------|---------|
| 文档格式 | `Markdown`, `PDF`, `DOCX`, `EPUB`, `HTML`, `TXT`, `LaTeX`, `BibTeX` |
| 数学渲染 | `KaTeX`, `MathJax` |
| AI 服务 | `OCR`, `API Key`, `Base URL`, `Ollama`, `DeepSeek`, `GPT`, `Claude`, `Gemini`, `Kimi`, `Qwen`, `GLM`, `Mistral` |
| 编码 | `JSON`, `URL`, `IP`, `UTF-8`, `GBK` |
| 产品名 | `ReadMD` |
| 系统 | `Windows`, `macOS`, `Linux` |
| 技术词 | `Tokens`, `LAN` |

---

## 占位符规范

字典值中的动态参数**必须原样保留**：

| 占位符 | 含义 | 示例键 |
|--------|------|--------|
| `{count}` | 数量 | `editor.wordCount` |
| `{name}` | 文件/文档名 | `dialog.unsavedMsg` |
| `{seq}` | 序号 | `ai.aiTag` |
| `{ver}` / `{version}` | 版本号 | `update.newVersion` |
| `{speed}` | 速度 | `ai.tokenSpeed` |
| `{file}` | 文件名 | `toast.fileOpened` |
| `{size}` | 文件大小 | `export.success` |
| `{time}` | 时间 | `ai.timePrefix` |
| `{source}` | 来源标识 | `ai.keyReady` |
| `language` | 目标语言名称 | `assets/skills/readmd-translate/SKILL.md` |
| `{percent}` | 百分比 | `reader.renderingProgress` |
| `{current}` / `{total}` | 当前/总数 | `search.matchCount` |
| `{done}` / `{failed}` / `{ok}` / `{skipped}` | 批处理统计 | `convert.allCompleted` |
| `{encoding}` | 文件编码 | `status.encoding` |
| `{line}` / `{col}` | 行列号 | `status.lines` |
| `{words}` | 字数 | `status.words` |
| `{backup}` | 备份路径 | `toast.backupCreated` |
| `{path}` | 文件路径 | `toast.fileSaved` |
| `{error}` | 错误信息 | `toast.error` |
| `{zoom}` | 缩放比例 | `toolbar.zoom` |

---

## 分区语言清单

### 🌏 东亚（East Asia）

| 代码 | 语言 | 本地名称 | 方向 | 母语率 |
|------|------|----------|------|--------|
| `zh-CN` | 简体中文 | 简体中文 | LTR | 99.9% |
| `zh-HK` | 繁体中文（香港） | 繁體中文（香港） | LTR | 99.9% |
| `zh-TW` | 繁体中文（台湾） | 繁體中文（台灣） | LTR | 99.9% |
| `ja` | 日语 | 日本語 | LTR | 99.9% |
| `ko` | 韩语 | 한국어 | LTR | 99.9% |
| `mn` | 蒙古语 | Монгол хэл | LTR | 99.9% |

### 🌍 欧洲主流（Major European）

| 代码 | 语言 | 本地名称 | 方向 | 母语率 |
|------|------|----------|------|--------|
| `en` | 英语 | English | LTR | — |
| `fr` | 法语 | Français | LTR | 99.6% |
| `de` | 德语 | Deutsch | LTR | 98.0% |
| `es` | 西班牙语 | Español | LTR | 99.6% |
| `pt` | 葡萄牙语 | Português | LTR | 99.6% |
| `ru` | 俄语 | Русский | LTR | 99.9% |
| `it` | 意大利语 | Italiano | LTR | 99.4% |
| `nl` | 荷兰语 | Nederlands | LTR | 97.5% |
| `tr` | 土耳其语 | Türkçe | LTR | 99%+ |
| `el` | 希腊语 | Ελληνικά | LTR | 99%+ |
| `hu` | 匈牙利语 | Magyar | LTR | 99%+ |
| `uk` | 乌克兰语 | Українська | LTR | 99.9% |
| `hr` | 克罗地亚语 | Hrvatski | LTR | 99.2% |
| `sl` | 斯洛文尼亚语 | Slovenščina | LTR | 99.2% |
| `ro` | 罗马尼亚语 | Română | LTR | 97.6% |
| `da` | 丹麦语 | Dansk | LTR | 97.7% |
| `no` | 挪威语 | Norsk | LTR | 98.7% |
| `sv` | 瑞典语 | Svenska | LTR | 98.2% |
| `fi` | 芬兰语 | Suomi | LTR | 99.7% |

### 🕌 中东与南亚（Middle East & South Asia）

| 代码 | 语言 | 本地名称 | 方向 | 母语率 |
|------|------|----------|------|--------|
| `ar` | 阿拉伯语 | العربية | RTL | 99.9% |
| `he` | 希伯来语 | עברית | RTL | 99.9% |
| `ug` | 维吾尔语 | ئۇيغۇرچە | RTL | 99.9% |
| `bo` | 藏语 | བོད་སྐད། | LTR | 99.9% |
| `hi` | 印地语 | हिन्दी | LTR | 99.9% |
| `bn` | 孟加拉语 | বাংলা | LTR | 99.9% |
| `ne` | 尼泊尔语 | नेपाली | LTR | 99.9% |

### 🌴 东南亚（Southeast Asia）

| 代码 | 语言 | 本地名称 | 方向 | 母语率 |
|------|------|----------|------|--------|
| `th` | 泰语 | ภาษาไทย | LTR | 99.9% |
| `vi` | 越南语 | Tiếng Việt | LTR | 99.3% |
| `id` | 印度尼西亚语 | Bahasa Indonesia | LTR | 99.6% |
| `ms` | 马来语 | Bahasa Melayu | LTR | 99.1% |
| `my` | 缅甸语 | မြန်မာဘာသာ | LTR | 99.9% |
| `lo` | 老挝语 | ພາສາລາວ | LTR | 99.9% |
| `km` | 高棉语 | ភាសាខ្មែរ | LTR | 99.9% |
| `tl` | 菲律宾语 | Tagalog | LTR | 98.1% |

### 🌐 其他与特色（Other & Special）

| 代码 | 语言 | 本地名称 | 方向 | 母语率 |
|------|------|----------|------|--------|
| `ga` | 爱尔兰语 | Gaeilge | LTR | 99.8% |
| `mt` | 马耳他语 | Malti | LTR | 98.3% |
| `kl` | 格陵兰语 | Kalaallisut | LTR | 99.3% |
| `eo` | 世界语 | Esperanto | LTR | 99.6% |
| `rw` | 卢旺达语 | Ikinyarwanda | LTR | 99.6% |
| `kg` | 刚果语 | Kikongo | LTR | 99.3% |

---

## Key 分类说明

| 前缀 | 键数量 | 描述 |
|------|--------|------|
| `ai` | 141 | AI 助手：模型、API、会话、提示词 |
| `app` | 32 | 应用入口：启动、最近、设置 |
| `convert` | 37 | 格式转换：DOCX/PDF/EPUB 转 MD |
| `dialog` | 30 | 对话框：表格、公式、图片、保存 |
| `editor` | 72 | 编辑器：工具栏、快捷键、格式 |
| `export` | 116 | 导出设置：PDF/HTML/DOCX 参数 |
| `fixes` | 3 | 自动修正提示 |
| `formula` | 40 | LaTeX 公式选择器 |
| `history` | 1 | 历史记录 |
| `img` | 38 | 图片编辑器 |
| `lang` | 4 | 语言切换 UI |
| `menu` | 35 | 菜单栏 |
| `ocr` | 9 | OCR 离线识别 |
| `reader` | 12 | 阅读器状态 |
| `search` | 9 | 搜索面板 |
| `share` | 11 | 分享：二维码、局域网 |
| `sidebar` | 7 | 侧边栏 |
| `status` | 19 | 状态栏 |
| `tabs` | 18 | 标签页管理 |
| `toast` | 144 | Toast 通知 |
| `toc` | 1 | 目录导航 |
| `toolbar` | 26 | 工具栏 |
| `tpl` | 14 | 内置模板 |
| `update` | 35 | 版本更新 |
| `web` | 50 | 网页转 MD |
| **总计** | **1109** | |

---

## 翻译质量基线

| 等级 | 母语率 | 描述 |
|------|--------|------|
| ✅ 优秀 | ≥ 99% | 全量母语化，生产就绪 |
| 🟡 良好 | 97%–99% | 少量技术术语保留英文（合规） |
| 🔴 待改进 | < 95% | 英文 fallback 过多，不推荐上线 |

当前候选要求 46 个 locale 的键、占位符和方向一致；语义审校与真实平台验收仍以发布门禁报告为准，不以静态字典数量推断“全量母语化”。

审计工具：

```bash
python scratch/audit_true_translation.py
```

---

## 新增语言指南

1. 在 `assets/i18n/meta.json` 添加元数据：
   ```json
   "xx": { "name": "Language Name", "native": "Native Name", "dir": "ltr", "region": "Region" }
   ```

2. 复制 `en.json` 为 `xx.json`，逐一翻译全部 1109 键

3. 遵守专有名词保护和占位符规范

4. 运行 `python scratch/audit_true_translation.py` 验证 ≥ 99%

5. 在应用语言切换器中注册该语言代码
