# 全部Shell命令、快捷键和编辑器子菜单

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)

注册数量见总清单统计；hidden命令只作注册记录，不计作可见列表按钮。Mod在Windows/Linux通常Ctrl，macOS通常Cmd；实际键盘优先级由Shell/CodeMirror/弹层状态决定。available列是无文档主页隔离快照，false不意味着永久不可用。ctx.hasDoc=file/virtual且original非空；ctx.isFile=实际文件；ctx.editing=编辑；ctx.notWelcome=非主页；ctx.paged=启用分页。


| ID | 分组/名称 | 快捷键 | 可用条件 | hidden / 主页available | 执行内容 | 注册源码 |
| --- | --- | --- | --- | --- | --- | --- |
| file.new | file / 新建文档 | Mod+N | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('newDocument') | [assets/js/shell/shell-deferred.js:153](../../../assets/js/shell/shell-deferred.js) |
| file.open | file / 打开文件… | Mod+O | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('loadFileDialog') | [assets/js/shell/shell-deferred.js:154](../../../assets/js/shell/shell-deferred.js) |
| file.openFolder | file / 打开文件夹… | — | () =&gt; enabled('btn-folder') | False / False | () =&gt; call('openFolder') | [assets/js/shell/shell-deferred.js:155](../../../assets/js/shell/shell-deferred.js) |
| file.recent | file / 最近文件… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('openHistoryModal') | [assets/js/shell/shell-deferred.js:156](../../../assets/js/shell/shell-deferred.js) |
| file.save | file / 保存 | Mod+S | ctx.editing | False / False | () =&gt; call('saveEdit') | [assets/js/shell/shell-deferred.js:157](../../../assets/js/shell/shell-deferred.js) |
| file.saveAs | file / 另存为… | Mod+Shift+S | () =&gt; enabled('btn-saveas') | False / False | () =&gt; call('saveAs') | [assets/js/shell/shell-deferred.js:158](../../../assets/js/shell/shell-deferred.js) |
| file.editCopy | file / 创建编辑副本 | — | ctx.hasDoc | False / False | () =&gt; window.ReadMDRecovery.createCopy() | [assets/js/shell/shell-deferred.js:159](../../../assets/js/shell/shell-deferred.js) |
| file.recovery | file / 恢复与版本历史 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; window.ReadMDRecovery.open() | [assets/js/shell/shell-deferred.js:160](../../../assets/js/shell/shell-deferred.js) |
| file.reload | file / 从磁盘重新加载 | Mod+R | ctx.isFile | False / False | () =&gt; call('loadFile', S().file, { force: true }) | [assets/js/shell/shell-deferred.js:161](../../../assets/js/shell/shell-deferred.js) |
| file.rename | file / 重命名文件 | F2 | () =&gt; ctx.hasDoc() && !ctx.editing() | False / False | () =&gt; call('openFileRename') | [assets/js/shell/shell-deferred.js:162](../../../assets/js/shell/shell-deferred.js) |
| file.clipboard | file / 从剪贴板新建 | Mod+V | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('createFromClipboard') | [assets/js/shell/shell-deferred.js:163](../../../assets/js/shell/shell-deferred.js) |
| file.home | file / 返回主页 | — | ctx.notWelcome | False / False | () =&gt; call('goHome') | [assets/js/shell/shell-deferred.js:164](../../../assets/js/shell/shell-deferred.js) |
| import.convert | import / 万物转 MD… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('openConvertModal') | [assets/js/shell/shell-deferred.js:166](../../../assets/js/shell/shell-deferred.js) |
| import.web | import / 网页转 MD… | Mod+U | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('openWebDialog') | [assets/js/shell/shell-deferred.js:167](../../../assets/js/shell/shell-deferred.js) |
| import.ocr | import / 扫描识别图片或 PDF（OCR）… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('chooseFile', 'ocr') | [assets/js/shell/shell-deferred.js:168](../../../assets/js/shell/shell-deferred.js) |
| export.open | export / 导出文档… | Mod+P | ctx.hasDoc：file/virtual 且original非空 | False / False | () =&gt; call('openExportModal') | [assets/js/shell/shell-deferred.js:170](../../../assets/js/shell/shell-deferred.js) |
| export.pdf | export / 导出为 PDF… | — | ctx.hasDoc：file/virtual 且original非空 | False / False | () =&gt; openExportAs(fmt) | [assets/js/shell/shell-deferred.js:170](../../../assets/js/shell/shell-deferred.js) |
| export.docx | export / 导出为 Word (DOCX)… | — | ctx.hasDoc：file/virtual 且original非空 | False / False | () =&gt; openExportAs(fmt) | [assets/js/shell/shell-deferred.js:170](../../../assets/js/shell/shell-deferred.js) |
| export.epub | export / 导出为 EPUB… | — | ctx.hasDoc：file/virtual 且original非空 | False / False | () =&gt; openExportAs(fmt) | [assets/js/shell/shell-deferred.js:170](../../../assets/js/shell/shell-deferred.js) |
| export.html | export / 导出为 HTML… | — | ctx.hasDoc：file/virtual 且original非空 | False / False | () =&gt; openExportAs(fmt) | [assets/js/shell/shell-deferred.js:170](../../../assets/js/shell/shell-deferred.js) |
| export.tex | export / 导出为 LaTeX… | — | ctx.hasDoc：file/virtual 且original非空 | False / False | () =&gt; openExportAs(fmt) | [assets/js/shell/shell-deferred.js:170](../../../assets/js/shell/shell-deferred.js) |
| export.presentation | export / 导出为 Reveal.js… | — | ctx.hasDoc：file/virtual 且original非空 | False / False | () =&gt; openExportAs(fmt) | [assets/js/shell/shell-deferred.js:170](../../../assets/js/shell/shell-deferred.js) |
| view.presentation | view / 开始演示 | F5 | ctx.hasDoc | False / False | () =&gt; call('launchPresentationMode') | [assets/js/shell/shell-deferred.js:175](../../../assets/js/shell/shell-deferred.js) |
| export.share | export / 扫码共享到手机… | — | () =&gt; enabled('btn-share') | False / False | () =&gt; call('openShareModal') | [assets/js/shell/shell-deferred.js:176](../../../assets/js/shell/shell-deferred.js) |
| view.toc | view / 显示/隐藏目录 | Mod+Shift+F | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('toggleSide', 'toc') | [assets/js/shell/shell-deferred.js:178](../../../assets/js/shell/shell-deferred.js) |
| view.files | view / 显示/隐藏文件侧栏 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('toggleSide', 'files') | [assets/js/shell/shell-deferred.js:179](../../../assets/js/shell/shell-deferred.js) |
| view.search | view / 在文档中查找 | Mod+F | ctx.hasDoc | False / False | () =&gt; call('toggleSearch') | [assets/js/shell/shell-deferred.js:180](../../../assets/js/shell/shell-deferred.js) |
| view.zen | view / 切换禅模式 | F11 | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('toggleZenMode') | [assets/js/shell/shell-deferred.js:181](../../../assets/js/shell/shell-deferred.js) |
| view.reading | view / 切换分页 / 连续阅读 | — | ctx.paged | False / False | () =&gt; call('togglePaginationMode') | [assets/js/shell/shell-deferred.js:182](../../../assets/js/shell/shell-deferred.js) |
| view.zoomIn | view / 增大字号 | Mod+= | ctx.notWelcome | False / False | () =&gt; call('zoom', 10) | [assets/js/shell/shell-deferred.js:183](../../../assets/js/shell/shell-deferred.js) |
| view.zoomOut | view / 减小字号 | Mod+- | ctx.notWelcome | False / False | () =&gt; call('zoom', -10) | [assets/js/shell/shell-deferred.js:184](../../../assets/js/shell/shell-deferred.js) |
| view.zoomReset | view / 重置字号 | Mod+0 | ctx.notWelcome | False / False | () =&gt; { if (typeof state === 'undefined') return; state.fontSize = 100; call('applySettings'); call('saveSettings'); } | [assets/js/shell/shell-deferred.js:185](../../../assets/js/shell/shell-deferred.js) |
| view.graph | view / 打开知识图谱 | Mod+G | () =&gt; ctx.hasDoc() && !!window.ReadMDGraph | False / False | () =&gt; {&lt;br&gt;    const s = S();&lt;br&gt;    const f = s.file \|\| '';&lt;br&gt;    const dir = f ? f.substring(0, Math.max(f.lastIndexOf('/'), f.lastIndexOf('\\'))) : '';&lt;br&gt;    if (window.ReadMDGraph) window.ReadMDGraph.open(dir);&lt;br&gt;  } | [assets/js/shell/shell-deferred.js:186](../../../assets/js/shell/shell-deferred.js) |
| view.backlinks | view / 查看反向链接 | — | () =&gt; enabled('btn-backlinks-menu') | False / False | () =&gt; byId('btn-backlinks-menu').click() | [assets/js/shell/shell-deferred.js:187](../../../assets/js/shell/shell-deferred.js) |
| view.fix | view / 查看自动修正报告 | — | () =&gt; enabled('btn-fix') | False / False | () =&gt; call('showFixModal') | [assets/js/shell/shell-deferred.js:188](../../../assets/js/shell/shell-deferred.js) |
| theme.cycle | theme / 切换主题 | Mod+D | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('toggleTheme') | [assets/js/shell/shell-deferred.js:190](../../../assets/js/shell/shell-deferred.js) |
| theme.auto | theme / 主题：跟随系统 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; setThemeChoice(choice) | [assets/js/shell/shell-deferred.js:190](../../../assets/js/shell/shell-deferred.js) |
| theme.light | theme / 主题：浅色 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; setThemeChoice(choice) | [assets/js/shell/shell-deferred.js:190](../../../assets/js/shell/shell-deferred.js) |
| theme.dark | theme / 主题：深色 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; setThemeChoice(choice) | [assets/js/shell/shell-deferred.js:190](../../../assets/js/shell/shell-deferred.js) |
| theme.sepia | theme / 主题：护眼 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; setThemeChoice(choice) | [assets/js/shell/shell-deferred.js:190](../../../assets/js/shell/shell-deferred.js) |
| edit.toggle | edit / 进入/退出编辑 | Mod+E | () =&gt; ctx.editing() \|\| enabled('btn-edit') | False / False | () =&gt; call('toggleEdit') | [assets/js/shell/shell-deferred.js:197](../../../assets/js/shell/shell-deferred.js) |
| edit.bold | edit / 加粗 | Mod+B | ctx.editing | False / False | () =&gt; call('cmInsertSyntax', 'bold') | [assets/js/shell/shell-deferred.js:198](../../../assets/js/shell/shell-deferred.js) |
| edit.italic | edit / 斜体 | Mod+I | ctx.editing | False / False | () =&gt; call('cmInsertSyntax', 'italic') | [assets/js/shell/shell-deferred.js:199](../../../assets/js/shell/shell-deferred.js) |
| edit.link | edit / 链接 | — | ctx.editing | False / False | () =&gt; call('cmInsertSyntax', 'link') | [assets/js/shell/shell-deferred.js:200](../../../assets/js/shell/shell-deferred.js) |
| edit.heading | edit / 标题 | — | ctx.editing | False / False | () =&gt; call('cmInsertSyntax', 'h2') | [assets/js/shell/shell-deferred.js:201](../../../assets/js/shell/shell-deferred.js) |
| edit.codeblock | edit / 代码块 | — | ctx.editing | False / False | () =&gt; call('cmInsertSyntax', 'codeblock') | [assets/js/shell/shell-deferred.js:202](../../../assets/js/shell/shell-deferred.js) |
| edit.table | edit / 插入表格… | — | ctx.editing | False / False | () =&gt; call('openTableModal') | [assets/js/shell/shell-deferred.js:203](../../../assets/js/shell/shell-deferred.js) |
| edit.formula | edit / 插入公式… | — | ctx.editing | False / False | () =&gt; call('openFormulaModal', 'inline') | [assets/js/shell/shell-deferred.js:204](../../../assets/js/shell/shell-deferred.js) |
| edit.previewRight | edit / 右侧预览 | — | ctx.editing | False / False | () =&gt; call('setPvLayout', 'right') | [assets/js/shell/shell-deferred.js:205](../../../assets/js/shell/shell-deferred.js) |
| edit.previewBottom | edit / 下方预览 | — | ctx.editing | False / False | () =&gt; call('setPvLayout', 'bottom') | [assets/js/shell/shell-deferred.js:206](../../../assets/js/shell/shell-deferred.js) |
| edit.previewNone | edit / 关闭预览 | — | ctx.editing | False / False | () =&gt; call('setPvLayout', 'none') | [assets/js/shell/shell-deferred.js:207](../../../assets/js/shell/shell-deferred.js) |
| ai.toggle | ai / 打开/关闭 AI 助手 | Mod+Shift+A | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; (typeof window.handleTopAiButtonClick === 'function' ? window.handleTopAiButtonClick() : call('toggleAiPanel')) | [assets/js/shell/shell-deferred.js:209](../../../assets/js/shell/shell-deferred.js) |
| ai.summary | ai / AI：总结当前文档 | — | ctx.hasDoc | False / False | () =&gt; call('openAiPanelWithPrompt', 'ask', _t('ux.askSummary')) | [assets/js/shell/shell-deferred.js:210](../../../assets/js/shell/shell-deferred.js) |
| ai.questions | ai / AI：带着问题阅读 | — | ctx.hasDoc | False / False | () =&gt; call('openAiPanelWithPrompt', 'ask', _t('ux.askQuestions')) | [assets/js/shell/shell-deferred.js:211](../../../assets/js/shell/shell-deferred.js) |
| ai.inline | ai / AI：编辑选中内容 | Alt+K | ctx.editing | False / False | () =&gt; call('openEditAiBar') | [assets/js/shell/shell-deferred.js:212](../../../assets/js/shell/shell-deferred.js) |
| ai.settings | ai / AI 连接设置… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('openAiModal', 'ai-settings-modal', byId('btn-ai')) | [assets/js/shell/shell-deferred.js:213](../../../assets/js/shell/shell-deferred.js) |
| ai.history | ai / AI 会话历史… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; { call('openAiModal', 'ai-history-modal', byId('btn-ai')); call('loadAiSessions'); } | [assets/js/shell/shell-deferred.js:214](../../../assets/js/shell/shell-deferred.js) |
| ai.skills | ai / Skill 工作台… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('openTplModal') | [assets/js/shell/shell-deferred.js:215](../../../assets/js/shell/shell-deferred.js) |
| settings.style | settings / 样式定制（CSS / Head）… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('openStyleModal') | [assets/js/shell/shell-deferred.js:217](../../../assets/js/shell/shell-deferred.js) |
| settings.language | settings / 切换界面语言… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; window.i18n && window.i18n.openModal() | [assets/js/shell/shell-deferred.js:218](../../../assets/js/shell/shell-deferred.js) |
| settings.pet | settings / 桌宠设置… | — | () =&gt; typeof window.openPetSettings === 'function' | False / True | () =&gt; call('openPetSettings') | [assets/js/shell/shell-deferred.js:219](../../../assets/js/shell/shell-deferred.js) |
| settings.plugins | settings / 插件中心… | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('openPluginModal') | [assets/js/shell/shell-deferred.js:220](../../../assets/js/shell/shell-deferred.js) |
| settings.assoc | settings / 设为 .md 默认打开方式 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('installAssoc') | [assets/js/shell/shell-deferred.js:221](../../../assets/js/shell/shell-deferred.js) |
| settings.autostart | settings / 切换开机自启 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('toggleAutostart') | [assets/js/shell/shell-deferred.js:222](../../../assets/js/shell/shell-deferred.js) |
| settings.update | settings / 检查更新 | — | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; call('checkUpdate', false) | [assets/js/shell/shell-deferred.js:223](../../../assets/js/shell/shell-deferred.js) |
| help.shortcuts | help / 键盘快捷键 | Mod+/ | 无注册级when；调用目标仍有平台/状态保护 | False / True | () =&gt; window.ReadMDShortcuts && window.ReadMDShortcuts.open() | [assets/js/shell/shell-deferred.js:225](../../../assets/js/shell/shell-deferred.js) |
| help.palette | help / 命令面板 | Mod+K | 无注册级when；调用目标仍有平台/状态保护 | True / True | () =&gt; {} | [assets/js/shell/shell-deferred.js:226](../../../assets/js/shell/shell-deferred.js) |

export.presentation切换Reveal导出；view.presentation/F5开启演讲播放器。两项独立注册，命令列表与键盘入口一致。[assets/js/shell/shell-deferred.js:168](../../../assets/js/shell/shell-deferred.js)；[assets/js/shell/shell.js:32 · onGlobalKey](../../../assets/js/shell/shell.js)。

## 编辑器斜杠菜单：23个一级条目

输入/或点编辑命令按钮 → 模糊搜索 → 上下键/鼠标选择 → Enter执行；代码块再选语言、表格再选网格。代码语言除25个已知项外还有纯文本和自定义语言查询；插入操作改变编辑事务，须保存才写磁盘。


| ID | 分组 | 文案 | 作用/下一步 | 快捷提示 |
| --- | --- | --- | --- | --- |
| text | basic | 正文 | 普通段落 | — |
| h1 | basic | 一级标题 | 大号章节标题 | # |
| h2 | basic | 二级标题 | 中号章节标题 | ## |
| h3 | basic | 三级标题 | 小号章节标题 | ### |
| bullet | basic | 无序列表 | 简单的项目列表 | - |
| numbered | basic | 有序列表 | 带编号的列表 | 1. |
| task | basic | 任务列表 | 用复选框跟踪待办 | [ ] |
| quote | basic | 引用 | 引用一段文字 | &gt; |
| divider | basic | 分隔线 | 分隔不同部分 | --- |
| code | insert | 代码块 | 接着选择语言 → lang | ``` |
| table | insert | 表格 | 接着选择行列数 → table | \| |
| image | insert | 图片 | 通过 URL 或路径嵌入，也可直接粘贴 | ![] |
| math | insert | 公式块 | LaTeX 行间公式 | $$ |
| mermaid | insert | Mermaid 图表 | 流程图、时序图、甘特图… | — |
| calloutNote | callout | 说明 | 补充信息提示块 | [!NOTE] |
| calloutTip | callout | 技巧 | 实用建议提示块 | [!TIP] |
| calloutImportant | callout | 重要 | 关键信息提示块 | [!IMPORTANT] |
| calloutWarning | callout | 警告 | 需要留意的提示块 | [!WARNING] |
| calloutCaution | callout | 危险 | 风险操作提示块 | [!CAUTION] |
| toc | more | 目录 | 生成全部标题的链接目录 | [TOC] |
| footnote | more | 脚注 | 此处引用，文末注释 | [^1] |
| date | more | 今天日期 | 插入 YYYY-MM-DD | — |
| time | more | 当前时间 | 插入 HH:MM | — |

## 代码语言子菜单：25个目录项


| 语言标识 | 文案 | 搜索别名 |
| --- | --- | --- |
| javascript | JavaScript | ["js","node","jsx"] |
| typescript | TypeScript | ["ts","tsx"] |
| python | Python | ["py"] |
| bash | Bash | ["sh","shell","zsh","terminal"] |
| json | JSON | [] |
| html | HTML | ["xml"] |
| css | CSS | ["scss","style"] |
| markdown | Markdown | ["md"] |
| rust | Rust | ["rs"] |
| go | Go | ["golang"] |
| java | Java | [] |
| c | C | [] |
| cpp | C++ | ["c++","cxx"] |
| csharp | C# | ["cs","dotnet"] |
| sql | SQL | ["mysql","postgres"] |
| yaml | YAML | ["yml"] |
| toml | TOML | ["ini"] |
| diff | Diff | ["patch"] |
| php | PHP | [] |
| ruby | Ruby | ["rb"] |
| swift | Swift | [] |
| kotlin | Kotlin | ["kt"] |
| latex | LaTeX | ["tex"] |
| powershell | PowerShell | ["ps1","pwsh"] |
| dockerfile | Dockerfile | ["docker"] |

## 公式模板：28个可点击项目

分类按钮、搜索框、行内/块级模式 → 点击公式模板将对应TeX插入光标/选区 → 即时预览。无后端调用，最终保存仍走文档保存。


| 分类 | 名称 | 别名 | 插入内容 |
| --- | --- | --- | --- |
| 常用 | 平方根 | sqrt root | \sqrt{x} |
| 常用 | 分式 | fraction frac | \frac{a}{b} |
| 常用 | 幂与下标 | power subscript | x^{n}_{i} |
| 常用 | 二次公式 | quadratic | x=\frac{-b\pm\sqrt{b^2-4ac}}{2a} |
| 希腊 | 阿尔法 | alpha | \alpha |
| 希腊 | 贝塔 | beta | \beta |
| 希腊 | 伽马 | gamma | \gamma |
| 希腊 | 派 | pi | \pi |
| 希腊 | 西塔 | theta | \theta |
| 希腊 | 欧米伽 | omega | \omega |
| 运算 | 加减 | plus minus | \pm |
| 运算 | 乘号 | times multiply | \times |
| 运算 | 除号 | divide | \div |
| 关系 | 小于等于 | less equal | \le |
| 关系 | 大于等于 | greater equal | \ge |
| 关系 | 不等于 | not equal | \ne |
| 关系 | 约等于 | approx | \approx |
| 箭头 | 右箭头 | right arrow | A\rightarrow B |
| 箭头 | 双向箭头 | leftright arrow | A\leftrightarrow B |
| 箭头 | 推出 | implies | A\Rightarrow B |
| 函数 | 正弦 | sin | \sin x |
| 函数 | 对数 | log | \log_{a}x |
| 函数 | 指数 | exp | e^{x} |
| 结构 | 求和 | sum | \sum_{i=1}^{n} x_i |
| 结构 | 积分 | integral | \int_{a}^{b} f(x)\,dx |
| 结构 | 极限 | limit | \lim_{x\to 0} f(x) |
| 结构 | 矩阵 | matrix | \begin{bmatrix}a&b\\c&d\end{bmatrix} |
| 结构 | 分段函数 | cases | f(x)=\begin{cases}x,&x\ge0\\-x,&x&lt;0\end{cases} |
