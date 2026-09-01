# ReadMD V2.3.8 新增国际化 (i18n) 词条终审清单

本清单覆盖 V2.3.8 分支（`codex/v2.3.8-reliability`）全部新增/更新的前端界面词条，共 **22 个 key**。机器翻译初稿已同步写入全部语种文件，**等待用户终审；终审通过前不发布 2.3.8**（发布硬门禁）。

---

## 0. 状态总览（2026-09-01）

| 项目 | 状态 |
| :--- | :--- |
| zh-CN 基准 | 1130 词条（V2.3.7 基线为 1109，本分支净增 21） |
| 语种覆盖 | 46 语种 × 1130 key，key parity 100% |
| parity 校验 | `python tools/i18n_sync.py --validate-only` → exit 0（2026-09-01 复跑通过） |
| 机翻初稿 | 除 zh-CN 外的 45 个语种均已写入对应 `assets/i18n/<lang>.json` |
| 机翻管线 | Google 机翻：`tools/i18n_sync.py --google`；LLM：`--llm`（占位符 `__PH_N__` 掩码保护） |

**审校方式**：对照下表逐 key 检查各语种译文；直接编辑 `assets/i18n/<lang>.json` 中对应 key 的值，改完重跑 `--validate-only` 确认无缺漏。审校时请务必保留 `{...}` 占位符（如 `{done}/{total}`、`{ok}`、`{skipped}`、`{failed}`、`{canceled}`、`{fmt}`、`{count}`）。

---

## 1. 批量工作台 (batch.*)

来源：`178ec5f` feat(ui): unified batch workbench with cancel and per-row traceability

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `batch.title` | 批量工作台 | Batch Workbench | 批量工作台页签标题 |
| `batch.selectFiles` | 选择文件（可多选） | Select Files (multiple allowed) | 选择待转换文件的按钮 |
| `batch.selectFolder` | 选择文件夹 | Select Folder | 选择整文件夹批处理的按钮 |
| `batch.preparing` | 准备中… | Preparing... | 任务入队前的准备状态 |
| `batch.converting` | 转换中 {done}/{total}… | Converting {done}/{total}… | 批处理进行中的总进度提示（含占位符） |
| `batch.cancel` | 取消全部 | Cancel All | 取消全部排队/进行中任务 |
| `batch.statusQueued` | 排队中 | Queued | 单行任务状态：排队 |
| `batch.statusRunning` | 处理中 | Processing | 单行任务状态：处理中 |
| `batch.statusOk` | 成功 | Success | 单行任务状态：成功 |
| `batch.statusSkipped` | 跳过（已存在） | Skipped (Exists) | 单行任务状态：目标已存在被跳过 |
| `batch.statusError` | 失败 | Failed | 单行任务状态：失败 |
| `batch.statusCanceled` | 已取消 | Canceled | 单行任务状态：被用户取消 |
| `batch.summary` | 完成：成功 {ok} · 跳过 {skipped} · 失败 {failed} | Completed: Success {ok} · Skipped {skipped} · Failed {failed} | 批处理完成汇总（含占位符） |
| `batch.summaryCanceled` | 已取消 {canceled} | Canceled {canceled} | 汇总行附注：被取消的任务数（含占位符） |
| `batch.note` | 文档批量转换，图片逐张 OCR；单个文件失败不影响后续任务，结果逐行可追溯。 | Documents are batch-converted, images are OCR'd one by one; a single file failure won't affect later tasks, and each result is traceable per row. | 工作台顶部说明文案 |

---

## 2. 侧边菜单 — 批量工作台入口 (menu.batch*)

来源：`178ec5f` feat(ui): unified batch workbench with cancel and per-row traceability

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `menu.batch` | 批量工作台 | Batch Workbench | 侧边菜单项 |
| `menu.batchSub` | 多文件转换 / OCR | Multi-file conversion / OCR | 侧边菜单副标题 |

---

## 3. 系统托盘 (tray.*)

来源：`2ff4e0a` feat(tray): localize tray menu labels by system locale

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `tray.show` | 显示 ReadMD | Show ReadMD | 托盘菜单：显示主窗口 |
| `tray.quit` | 退出 ReadMD | Quit ReadMD | 托盘菜单：退出应用 |

> 说明：托盘为 Python 侧实现（`readmd.py` 托盘构建段），运行时按系统语言直接选用简体/英文内联文案（既有设计，含 `tests/test_tray_i18n.py` 覆盖）；同时把两个 key 收录进 i18n 字典以便统一管理。

---

## 4. 侧边菜单 — 桌宠入口 (menu.pet*)

来源：`39cf149` feat(pet): add localized optional plugin entry

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `menu.pet` | 桌宠 | Desktop pet | 侧边菜单项：桌宠（Live2D 外置插件）入口 |
| `menu.petSub` | 外置插件 | Optional plug-in | 侧边菜单副标题：以可选外置插件形式提供 |

---

## 5. 转换 — AI 自愈排版 (convert.*)

来源：`5712afb` feat: universal document viewer, AI ecosystem integration and full release audit fixes

| Key | zh-CN (简体中文) | en (English) | 用途说明 |
| :--- | :--- | :--- | :--- |
| `convert.aiFixDesc` | 使用 AI 深度修复表格对齐、公式排版与断行问题 | Use AI to deep-fix table alignments, formulas, and line wraps | 转换完成后「AI 自愈排版」按钮的说明文案 |

---

## 6. 范围说明（非本清单项，无需审校）

- `assets/js/core/history.js:301-302、454` 的「导出需使用桌面版；浏览器模式可另存或打印」「导出文档 (Ctrl+P)」「当前版本 v」为 main 分支既有裸文案，本分支未改动，遗留至后续版本统一抽 key。
- `assets/app.js:598` 的 "Browser mode is unavailable" 是 `_t('toast.convertBrowserNotice') || 兜底文案` 模式，`toast.convertBrowserNotice` key 已存在于全部语种字典，非抽取缺口。
- 前端其余中文字符串均为既有的 `_t('key') || '中文兜底'` 设计模式（兜底兜的是 key 缺失场景），不重复列入本清单。
