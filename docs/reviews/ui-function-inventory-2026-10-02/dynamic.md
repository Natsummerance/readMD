# 动态控件族、正文与手势操作

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)

共80个交互族，每族按实际生成数据产生若干控件，不给它们虚构固定按钮总数。静态399项、动态字段98项、67命令和目录数据分别统计，存在复用/重合，不能相加成产品功能数量。前端DOM和独立桌宠native UI都列入；仅后台定时器不另算用户操作。

## 01 主窗口


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D001 / 最近文件条目 | recent-item / recent-remove | 点击路径打开；单条×移除；不存在文件提示；清空另列静态按钮 | 按当前数据/面板状态出现 | /api/recent/status、remove、file | [assets/js/core/history.js:80 · renderRecentList](../../../assets/js/core/history.js) |
| D002 / 更多菜单分组头 | more-section-title | 折叠/展开导入、互动、设置；键盘导航、外部/Esc关闭 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/app.js:81 · bindEvents](../../../assets/app.js) |

## 02 文档恢复


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D075 / 打开记录为副本 | document-history-row Open as copy | 按记录打开未保存副本，保留原目录和资源引用；不会覆盖原文 | 按当前数据/面板状态出现 | /api/documents/history op=read | [assets/js/features/document-history.js:108 · restore](../../../assets/js/features/document-history.js) |
| D076 / 恢复到当前编辑器 | document-history-row Apply | 仅匹配来源时显示；先保留检查点、核对期间输入；单事务恢复或安全副本 | 按当前数据/面板状态出现 | /api/documents/history op=record/read；保存才/api/save | [assets/js/features/document-history.js:108 · restore](../../../assets/js/features/document-history.js) |
| D077 / 删除恢复记录 | document-history-row Delete | 明确确认后删除该记录正文/元数据，刷新列表；失败保留反馈 | 按当前数据/面板状态出现 | /api/documents/history op=delete | [assets/js/features/document-history.js:132 · refresh](../../../assets/js/features/document-history.js) |

## 02 标签


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D003 / 标签标题/关闭/中键 | doc-tab / close | 切换；×/中键关闭；脏数据决策；拖动重排 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/core/tabs.js:56 · renderTabsBar](../../../assets/js/core/tabs.js) |
| D004 / 溢出标签列表 | doc-tabs-overflow-menu | 点击被折叠的标签切换；自动空间计算 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/core/tabs.js:56 · renderTabsBar](../../../assets/js/core/tabs.js) |
| D005 / 标题编辑 | file-title input | 点击标题进入重命名；Enter提交，Esc退出；磁盘重命名或虚拟命名 | 按当前数据/面板状态出现 | /api/rename | [assets/js/core/tabs.js:373 · renameTab](../../../assets/js/core/tabs.js) |

## 03 阅读


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D006 / 目录条目/折叠柄 | toc-item / toc-toggle | 点标题滚动定位；有子项时展开/折叠，滚动更新活跃标记 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/toc.js:23 · buildToc](../../../assets/js/reader/toc.js) |
| D007 / 大纲全部折叠/展开 | rd-toc-fold / rd-toc-twisty | 单个标题箭头折叠子树；顶部按钮全部展开/折叠；无子标题则隐藏 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:1057 · decorateToc](../../../assets/js/reader/enhance.js) |
| D008 / 浮动阅读设置开关 | rd-prefs-btn | 点击展开阅读偏好；再次点击/外部/Esc关闭；不改文档源码 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:868 · ensureTools](../../../assets/js/reader/enhance.js) |
| D009 / 字号/字体/行距/阅读宽度 | rd-size-btn / data-pref-key / data-pref-value | 字号70%–180%每次10%；sans/serif两项、compact/normal/relaxed三行距、narrow/normal/wide三宽度；点击或方向键切换并即时保存 | 按当前数据/面板状态出现 | /api/settings或浏览器localStorage | [assets/js/reader/enhance.js:819 · buildPrefsPanel](../../../assets/js/reader/enhance.js) |
| D010 / 目录/文件侧栏宽度 | splitter | 按住拖动调整侧栏；边界约束；保存宽度偏好 | 按当前数据/面板状态出现 | /api/settings / 前端偏好 | [assets/app.js:81 · bindEvents](../../../assets/app.js) |
| D011 / 文件树目录/文件 | tree-node | 目录展开/折叠；文件打开；活动路径高亮 | 按当前数据/面板状态出现 | /api/list、file | [assets/js/reader/folder.js:105 · renderTreeNodes](../../../assets/js/reader/folder.js) |
| D012 / 分页页码option | pg-page-select option | 按页目录选择跳转；首末/前后静态按钮与连续模式切换 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/render.js:1225 · renderContent](../../../assets/js/reader/render.js) |

## 04 正文


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D013 / 外链/本地链接/双链 | content a / wikilink | 外链宿主打开；本地Markdown切文档；标题锚点定位；未创建目标受限 | 按当前数据/面板状态出现 | /api/file、system/open-path | [assets/js/reader/render.js:1225 · renderContent](../../../assets/js/reader/render.js) |
| D014 / 标题链图标 | heading-anchor | 跳转自身#锚点；没有复制链接按钮处理 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:591 · upgradeHeadings](../../../assets/js/reader/enhance.js) |
| D015 / 折叠引用/callout/details | summary | 展开/折叠HTML details正文；不是所有blockquote都可折叠 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:1](../../../assets/js/reader/enhance.js) |
| D016 / 代码复制按钮 | code copy buttons | 复制该块代码；成功/失败反馈；代码语言/长块提示由渲染器增强 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:1](../../../assets/js/reader/enhance.js) |
| D017 / 图片大图入口 | img.rd-zoomable / rd-lightbox-close | 点击/Enter开灯箱；×/图片外区域/Esc关闭；当前灯箱没有独立缩放/适应/拖动控制按钮；图片嵌在链接内先按链接 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:723 · ensureLightbox](../../../assets/js/reader/enhance.js) |
| D018 / 脚注编号/返回 | footnote href | 跳转注释及返回引用位置；引用tooltip/弹层按加载内容提供 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:1](../../../assets/js/reader/enhance.js) |
| D019 / 引用编号与文献 | citation links | 引用查看条目/跳文献；外部DOI/URL由链接处理；BibTeX数据供详情 | 按当前数据/面板状态出现 | /api/bibtex | [assets/js/reader/render.js:1225 · renderContent](../../../assets/js/reader/render.js) |
| D020 / 表格横向区域 | table-scroll-wrap | 横向滚动长表，数字对齐；没有表头排序/表格导出按钮 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:517 · upgradeTables](../../../assets/js/reader/enhance.js) |
| D021 / 任务清单 | task-list checkbox | 阅读器禁用复选框仅展示；编辑器点击任务标记才修改Markdown | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/enhance.js:581 · upgradeTasks](../../../assets/js/reader/enhance.js) |
| D022 / 代码文件动作栏 | btn-code-to-md/edit/ai-explain/copy | 四按钮分别AI转文档、编辑源码、AI解析、复制；编辑调用toggleEdit打开CodeMirror | 按当前数据/面板状态出现 | /api/ai/chat；源码复制前端；编辑器事务与保存接口 | [assets/js/reader/render.js:1317 · bindCodeDocActions](../../../assets/js/reader/render.js) |

## 05 编辑


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D023 / 正文编辑区/光标/选区 | CodeMirror contenteditable | 输入、选区、拖选、滚动；撤销/重做；快捷格式、自动补全Markdown；粘贴转换；任务标记toggle | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/editor/editor.js:53 · createEditor](../../../assets/js/editor/editor.js) |
| D024 / 选区浮条 | cm-selection-toolbar | 格式、复制/剪切/粘贴与AI操作；依选区/编辑状态显示 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/editor/editor.js:1](../../../assets/js/editor/editor.js) |
| D025 / 斜杠菜单项目 | slash item buttons | 23一级项；模糊搜索、方向键、Enter、Esc；代码语言/表格二级 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/editor/editor.js:1](../../../assets/js/editor/editor.js) |
| D026 / 语言子菜单 | lang-* | 25语言、纯文本、自定义查询；确定后插入围栏并定位光标 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/editor/editor.js:2008 · slashLanguageItems](../../../assets/js/editor/editor.js) |
| D027 / 表格网格子菜单 | table grid cells | 鼠标/方向选择行列；点击/Enter插入表格；网格范围由生成逻辑决定 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/editor/editor.js:1](../../../assets/js/editor/editor.js) |
| D028 / 预览页签和分栏 | data-pv / pv-splitter | 右侧、下方、关闭预览；同步滚动；拖动宽/高；编辑和预览共用当前草稿 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/editor/preview.js:60 · setPvLayout](../../../assets/js/editor/preview.js) |

## 06 插入


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D029 / 公式分类、模板与渲染点选 | formula-cats / formula-list / formula-preview | 7 类、46 预设、2,432 字符、446 别名；可视缩略图、分页搜索、精确符号替换、确认/撤销 | 编辑态公式草稿 | 前端/Rust 共用目录；完整 OMML 属性往返尚待实现 | [公式编辑](../../../assets/js/editor/formula-studio.js) |
| D030 / 表格矩阵单元 | table-grid | 悬停预览行列并高亮；点击生成Markdown，关闭回编辑器 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/editor/editor.js:1255 · openTableModal](../../../assets/js/editor/editor.js) |
| D031 / 图片画布/裁剪柄 | img-canvas / crop handles | 选裁剪框、移动和缩放手柄，预览；比例/旋转/镜像/输出尺寸配合静态输入 | 按当前数据/面板状态出现 | /api/image/save 写受控PNG资源 | [assets/js/editor/image.js:1](../../../assets/js/editor/image.js) |

## 07 执行


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D032 / 代码运行/复制输出/清空 | code-chunk-run-btn/copy-btn/clear-btn | 展开输出状态；运行语言可用时调用后端；复制当前输出，清空仅当前显示 | 按当前数据/面板状态出现 | /api/code/run → parity_code.rs，依本机运行时 | [assets/js/reader/render.js:1](../../../assets/js/reader/render.js) |
| D033 / 图表显式远程授权 | diagram-allow-remote-btn | PlantUML本地不可用时用户点远程渲染；没有自动隐式联网 | 按当前数据/面板状态出现 | /api/diagram/render → parity_diagram.rs | [assets/js/reader/render.js:1](../../../assets/js/reader/render.js) |

## 08 AI


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D034 / 空态提问卡/连接卡 | data-starter / data-ai-connect | 选总结/大纲/解释等示例填提示；连接卡打开设置；填入不等于自动发送 | 按当前数据/面板状态出现 | /api/ai/config、chat由后续动作触发 | [assets/js/features/ai.js:1](../../../assets/js/features/ai.js) |
| D035 / 消息动作按钮 | ai-bubble-act-btn / ai-regen-btn | 助手四项：复制、应用文档、另存MD、重新生成；用户消息目前只有上下文卡展开，不提供独立编辑再发按钮；失败时另有重试 | 按当前数据/面板状态出现 | /api/ai/chat、history；应用编辑最终另行保存 | [assets/js/features/ai.js:1320 · renderAiBubbleActions](../../../assets/js/features/ai.js) |
| D036 / 连接状态入口 | AI header connection badge | 点击/Enter/Space打开连接设置；已配置显示连接摘要，密钥不回填 | 按当前数据/面板状态出现 | /api/ai/config | [assets/js/features/ai.js:1](../../../assets/js/features/ai.js) |
| D037 / 消息代码复制/上下文折叠 | ai-code-copy / details | 代码复制；长文档上下文展开/折叠；不改用户原文 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/features/ai.js:1](../../../assets/js/features/ai.js) |
| D038 / 面板边缘拖动 | ai-resize-handle | 拖动宽度并约束最大/最小；全屏按钮与返回原宽度 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/features/ai.js:1](../../../assets/js/features/ai.js) |

## 09 AI历史


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D041 / 会话条目 | AI session rows | 点恢复；标题重命名；单条删除；搜索过滤；旧select不是另一套常驻入口 | 按当前数据/面板状态出现 | /api/ai/history list/get/rename/delete | [assets/js/features/ai.js:1150 · renderAiSessionSelect](../../../assets/js/features/ai.js) |

## 09 AI设置


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D039 / 连接option / 官方卡 | ai-provider / provider cards | 官方26预设、用户连接；选项切换表单；上游来源-only条目展示元数据且可能不能直接选 | 按当前数据/面板状态出现 | /api/ai/config | [assets/js/features/ai.js:1515 · fillAiProviders](../../../assets/js/features/ai.js) |
| D040 / 模型option | ai-model option | 从现有/获取模型列表中选择；不是任意文本模型输入 | 按当前数据/面板状态出现 | /api/ai/models；保存config.current | [assets/js/features/ai.js:1638 · fillAiModels](../../../assets/js/features/ai.js) |

## 10 Skill


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D042 / 技能列表/分类 | tpl-list dynamic rows | 搜索、点技能查看说明，选作AI模板；内置/用户权限区分 | 按当前数据/面板状态出现 | /api/skills、ai/prompts | [assets/js/features/ai.js:339 · renderTplList](../../../assets/js/features/ai.js) |
| D043 / 来源页签及预览复选框 | skill-import source / preview rows | GitHub/目录/ZIP → 预览 → 勾有效项 → skip冲突策略apply；无效项禁用；token用后清空 | 按当前数据/面板状态出现 | /api/skill-imports/preview、apply | [assets/js/features/ai.js:977 · renderGithubSkillPreview](../../../assets/js/features/ai.js) |

## 11 转换


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D044 / 批量任务行 | batch-row | 排队、运行、成功/跳过/失败/取消；点可用输出打开；错误明细显示 | 按当前数据/面板状态出现 | /api/convert/progress、file | [assets/js/features/batch.js:59 · makeBatchRow](../../../assets/js/features/batch.js) |

## 12 插件


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D045 / 分类tab | plugin category tabs | 过滤六类；不改变能力状态 | 按当前数据/面板状态出现 | /api/plugins/list | [assets/js/features/convert.js:395 · renderPluginCards](../../../assets/js/features/convert.js) |
| D046 / 卡片安装/重试 | data-action=install | 原生扩展未安装时显示；确认 → 安装并校验profile/manifest → 轮询 | 按当前数据/面板状态出现 | /api/plugins/install → 原生扩展配置安装/校验/失败回滚 | [assets/js/features/convert.js:582 · startPluginInstall](../../../assets/js/features/convert.js) |
| D047 / 卡片启用开关 | data-action=toggle | 已安装包 → change → 锁/保存/刷新或回滚 → 同能力互斥 | 按当前数据/面板状态出现 | /api/plugins/toggle → manifest | [assets/js/features/convert.js:547 · setPluginToggle](../../../assets/js/features/convert.js) |
| D048 / 卡片卸载按钮 | data-action=uninstall | 已安装包 → 危险确认 → 清理 → 刷新 | 按当前数据/面板状态出现 | /api/plugins/uninstall | [assets/js/features/convert.js:616 · startPluginUninstall](../../../assets/js/features/convert.js) |
| D049 / 错误与运行信息折叠 | summary/details | 错误detail/last_log、ffmpeg/runtime说明展开折叠 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/features/convert.js:369 · pluginErrorMarkup](../../../assets/js/features/convert.js) |

## 13 网页


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D074 / 隔离系统WebView授权/提取按钮 | readmd-capture-bar | 登录后点击授权此站点；交互提取点提取此页；×取消窗口任务；不把输入值或脚本并入Markdown | 按当前数据/面板状态出现 | main.rs nativeWeb IPC → desktop_web.rs；按任务和来源保存短期grant | [rust/readmd-kernel/src/desktop_web.rs:158 · capture_script](../../../rust/readmd-kernel/src/desktop_web.rs) |

## 14 导出


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D050 / 预设option / 推荐卡 | exp-preset / export preset cards | 默认、自定义、3内置及用户项；点卡/选择载入并刷新预览 | 按当前数据/面板状态出现 | /api/export/presets | [assets/js/features/export.js:1201 · renderExportPresetSelect](../../../assets/js/features/export.js) |
| D051 / 配置分组标题 | export section header | 14组按格式筛选；折叠/展开；与98编辑字段及1说明分别统计 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/features/export.js:1040 · renderExportSections](../../../assets/js/features/export.js) |
| D052 / 全部字段/heading inputs | data-exp-k | 98个参数修改进入collect/options；数字、颜色、checkbox、select完整枚举见exports.md | 按当前数据/面板状态出现 | /api/export、export/epub/presentation按格式 | [assets/js/features/export.js:1148 · collectExportOptions](../../../assets/js/features/export.js) |
| D053 / 预览分页/大预览 | export preview page sheets | 缩略预览卡点击/Enter；大预览前后页、真实页面PNG/PDF视图；参数更新取消旧请求并同步产物；关闭释放资源。DOCX显示共享AST/设置的版式参考并注明Word分页边界 | 按当前数据/面板状态出现 | /api/export/preview → export_preview.rs；PDF实际产物缓存10分钟 | [assets/js/features/export.js:902 · updateExportLivePreview](../../../assets/js/features/export.js) |

## 15 演示


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D054 / 主题/转场select | presentation controls | 11主题、6转场；字体20/24/28；总览/全屏/退出/关闭；iframe消息同步 | 按当前数据/面板状态出现 | /api/export/presentation | [assets/js/reader/render.js:2690 · launchPresentationMode](../../../assets/js/reader/render.js) |
| D055 / Reveal内置交互 | Reveal controls/progress/slides | 上下左右、前后/垂直页、进度、overview、Esc、键盘/触摸；仅有相应页和配置时可用 | 按当前数据/面板状态出现 | 前端；相关存盘由所属流程处理 | [assets/js/reader/render.js:2690 · launchPresentationMode](../../../assets/js/reader/render.js) |

## 16 关联


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D056 / 图谱节点/画布 | graph canvas | 单击选择看链接；双击打开；拖节点；Shift拖平移/滚轮缩放；搜索/列表定位 | 按当前数据/面板状态出现 | /api/links/graph、file | [assets/js/features/graph.js:179 · onPointerDown](../../../assets/js/features/graph.js) |
| D057 / 图谱列表/关系行 | graph rows / node details | 列表选中节点，点关联文件打开；未创建目标无有效打开 | 按当前数据/面板状态出现 | /api/file | [assets/js/features/graph.js:243 · renderList](../../../assets/js/features/graph.js) |
| D058 / 反向链接页签/行 | data-tab=incoming/outgoing | 反向/正向切换；搜索；点有效文件；deadlink禁用 | 按当前数据/面板状态出现 | /api/links/backlinks | [assets/js/features/graph.js:696 · createDrawer](../../../assets/js/features/graph.js) |

## 17 设置


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D059 / 语言按钮 | lang grid buttons | 46语言按名/代码搜，点选翻译/RTL及保存；资源缺词走fallback | 按当前数据/面板状态出现 | localStorage locale + i18n资源 | [assets/js/core/i18n.js:1](../../../assets/js/core/i18n.js) |

## 18 更新


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D060 / 状态栏版本徽标 | status-update-badge | 条件显示；点击复用检查更新弹窗 | 按当前数据/面板状态出现 | /api/update/check | [assets/js/features/updater.js:15 · checkUpdate](../../../assets/js/features/updater.js) |

## 19 桌宠


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D061 / 设置页签/启用 | data-pet-section / pet-enabled | 角色/陪伴/设置切换；启用自动提交；预览与运行实例不同 | 按当前数据/面板状态出现 | /api/pets/status、configure | [assets/js/features/pet-workbench.js:98 · init](../../../assets/js/features/pet-workbench.js) |
| D062 / 角色卡/收藏星 | pet-roster buttons | 点击角色选择；星按钮独立收藏不触发选角；搜索和精灵/Live2D过滤；失败恢复 | 按当前数据/面板状态出现 | /api/pets/active、configure；favorites localStorage | [assets/js/features/pet-workbench.js:258 · renderRoster](../../../assets/js/features/pet-workbench.js) |
| D063 / 养成动作按钮 | data-pet-action | 摸头、喂食、玩耍、休息/唤醒；状态、冷却、体力反馈 | 按当前数据/面板状态出现 | /api/pets/interact | [assets/js/features/pet-companion-actions.js:40 · init](../../../assets/js/features/pet-companion-actions.js) |
| D064 / 安静/活跃与气泡 | pet-mode / bubble toggle | 陪伴风格、提示气泡、打招呼/AI；安静不等于禁止所有互动 | 按当前数据/面板状态出现 | /api/pets/configure / 前端台词 | [assets/js/features/pet-workbench.js:203 · quiet](../../../assets/js/features/pet-workbench.js) |
| D065 / 阅读器内角色与快捷条 | pet-character-wrap | 点击台词/动作；阈值拖动、锁定；设置/收起/AI/安静四快捷项 | 按当前数据/面板状态出现 | /api/pets/configure及前端位置 | [assets/js/features/pet-batch.js:576 · initPetDirectManipulation](../../../assets/js/features/pet-batch.js) |

## 20 独立桌宠


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D066 / 实体点击/拖动/透明区 | pet native window | 实体点击打开互动；4px拖动阈值；alpha区穿透；锁定；置顶/透明度 | 按当前数据/面板状态出现 | Rust pet_host/desktop_pet及原生窗口 | [packages/readmd-pet-rust/renderer/bongocat.js:1](../../../packages/readmd-pet-rust/renderer/bongocat.js) |
| D067 / 六项右键菜单 | native context menu | ReadMD、剪贴板、角色、玩耍、设置、隐藏；取消后恢复点击实体区域 | 按当前数据/面板状态出现 | /api/control/pet-menu + durable queue | [packages/readmd-pet-rust/renderer/bongocat.js:1](../../../packages/readmd-pet-rust/renderer/bongocat.js) |
| D068 / 角色子菜单 | native character menu | 返回与逐角色选择，当前标识；Bongo/精灵/Arch-Chan按自身渲染器 | 按当前数据/面板状态出现 | /api/pets/configure/active | [packages/readmd-pet-rust/renderer/bongocat.js:1](../../../packages/readmd-pet-rust/renderer/bongocat.js) |
| D069 / 六项气泡动作 | native pet-life actions | 摸头、喂食、玩耍、休息/唤醒、AI聊天、关闭 | 按当前数据/面板状态出现 | /api/pets/interact + 主程序唤起 | [packages/readmd-hermes-pet-adapter/src/pet-life.ts:1](../../../packages/readmd-hermes-pet-adapter/src/pet-life.ts) |
| D070 / 文件拖入 | native file-drop | 取得完整路径，耐久队列唤起ReadMD，消费后批量转换/OCR/打开 | 按当前数据/面板状态出现 | /api/control/pet-batch、next + 复用导入 | [assets/js/features/pet-batch.js:178 · receivePetBatch](../../../assets/js/features/pet-batch.js) |

## 21 Shell


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D071 / 命令列表/最近文件/标签 | palette option rows | 查询排序、上/下选择、Enter/点击执行；最近命令16；快捷键表搜索 | 按当前数据/面板状态出现 | 各动作复用已有实现 | [assets/js/shell/shell-deferred.js:1](../../../assets/js/shell/shell-deferred.js) |

## 21 全局


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D073 / Toast结果/任务取消 | task feedback / toast actions | 消息反馈、忙碌防重复；可取消任务显示取消；按任务实际状态处理 | 按当前数据/面板状态出现 | /api/task/cancel或所属任务接口 | [assets/js/core/task-feedback.js:1](../../../assets/js/core/task-feedback.js) |

## 21 对话框


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D072 / 动态选择按钮 | choice-modal buttons | 调用者给label/id/kind；如同名输出决策；Esc/backdrop取消，Tab在框内 | 按当前数据/面板状态出现 | 确认后的调用者决定实际写入 | [assets/js/core/tabs.js:620 · askChoice](../../../assets/js/core/tabs.js) |

## 21 桌面退出


| ID / 控件族 | 定位 | 全部动作与内容 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D078 / 原生标题栏关闭 | native CloseRequested | ×/Alt+F4收集全部脏标签；取消不退出、放弃先保留记录；重复关闭受保护 | 按当前数据/面板状态出现 | main.rs CloseRequested / readmd:quit | [assets/js/features/document-history.js:192 · prepareClose](../../../assets/js/features/document-history.js) |

## 原始动态HTML位置索引

正则另找到60处单行动态HTML标签，下面保留定位以便复核；多行模板、createElement、Canvas和委托事件由上面的族覆盖，此60不是完整动态按钮数量。


| 文件/行 | 标签 | 属性 | 源码片段 |
| --- | --- | --- | --- |
| [assets/js/features/ai.js:114](../../../assets/js/features/ai.js) | button | {"type":"button","data-starter":"${ask}","data-starter-id":"${id}"} | &lt;button type="button" data-starter="${ask}" data-starter-id="${id}"&gt; |
| [assets/js/features/ai.js:121](../../../assets/js/features/ai.js) | button | {"type":"button","class":"tb-btn accent"} | &lt;button type="button" class="tb-btn accent" data-ai-connect&gt;${esc(_t('ai.connectAction'))}&lt;/button&gt; |
| [assets/js/features/convert.js:378](../../../assets/js/features/convert.js) | summary | {} | &lt;summary&gt;${escapeHtml(_t('plugin.errorDetails'))}&lt;/summary&gt; |
| [assets/js/features/convert.js:458](../../../assets/js/features/convert.js) | input | {"type":"checkbox","class":"plugin-switch-input","data-action":"toggle","aria-label":"${escapeHtml(title)}"} | &lt;input type="checkbox" class="plugin-switch-input" ${p.enabled ? 'checked' : ''} data-action="toggle" aria-label="${escapeHtml(title)}"&gt; |
| [assets/js/features/convert.js:464](../../../assets/js/features/convert.js) | button | {"class":"plugin-action-uninstall-btn","data-action":"uninstall"} | &lt;button class="plugin-action-uninstall-btn" data-action="uninstall"&gt;${_t('plugin.uninstall')}&lt;/button&gt; |
| [assets/js/features/convert.js:494](../../../assets/js/features/convert.js) | button | {"data-action":"install"} | footRight = `&lt;button class="plugin-action-get-btn ${hasErr ? 'is-retry' : ''}" data-action="install"&gt;${hasErr ? _t('plugin.retry') : _t('plugin.install')}&lt;/button&gt;`; |
| [assets/js/features/document-history.js:102](../../../assets/js/features/document-history.js) | button | {"id":"document-history-close","type":"button","class":"tb-btn"} | modal.innerHTML = '&lt;div class="modal-box document-history-box"&gt;&lt;div class="modal-header"&gt;&lt;h2 id="document-history-title"&gt;&lt;/h2&gt;&lt;button id="document-history-close" type="button" class="tb-btn" data-modal-close&gt;×&lt;/button&gt;&lt;/div&gt;&lt;p id="document-history-policy" clas |
| [assets/js/features/document-history.js:102](../../../assets/js/features/document-history.js) | button | {"id":"document-history-refresh","class":"tb-btn","type":"button"} | modal.innerHTML = '&lt;div class="modal-box document-history-box"&gt;&lt;div class="modal-header"&gt;&lt;h2 id="document-history-title"&gt;&lt;/h2&gt;&lt;button id="document-history-close" type="button" class="tb-btn" data-modal-close&gt;×&lt;/button&gt;&lt;/div&gt;&lt;p id="document-history-policy" clas |
| [assets/js/features/export.js:1073](../../../assets/js/features/export.js) | input | {"type":"number","min":"8","max":"40"} | '&lt;input type="number" data-k="headings.h' + i + '.size" min="8" max="40" title="' + (_t('export.bodySize') \|\| '') + '"&gt;' + |
| [assets/js/features/export.js:1074](../../../assets/js/features/export.js) | input | {"type":"color"} | '&lt;input type="color" data-k="headings.h' + i + '.color" title="' + (_t('export.bodyColor') \|\| '') + '"&gt;' + |
| [assets/js/features/export.js:1075](../../../assets/js/features/export.js) | input | {"type":"checkbox"} | '&lt;label class="exp-check"&gt;' + (_t('export.bold') \|\| '') + '&lt;input type="checkbox" data-k="headings.h' + i + '.bold"&gt;&lt;/label&gt;' + |
| [assets/js/features/export.js:1076](../../../assets/js/features/export.js) | select | {} | '&lt;select data-k="headings.h' + i + '.align"&gt;' + EXPORT_ALIGNS.map(a =&gt; '&lt;option value="' + a + '"&gt;' + a + '&lt;/option&gt;').join('') + '&lt;/select&gt;'; |
| [assets/js/features/export.js:1116](../../../assets/js/features/export.js) | select | {} | inner += '&lt;select data-k="' + f.k + '"&gt;' + (f.opts \|\| []).map(o =&gt; |
| [assets/js/features/export.js:1126](../../../assets/js/features/export.js) | input | {"type":"checkbox"} | inner = '&lt;label class="exp-check"&gt;&lt;input id="' + fieldId + '" type="checkbox" data-k="' + f.k + '"&gt; ' + f.label + '&lt;/label&gt;'; |
| [assets/js/features/export.js:1128](../../../assets/js/features/export.js) | input | {"type":"color"} | inner += '&lt;input id="' + fieldId + '" type="color" data-k="' + f.k + '"&gt;'; |
| [assets/js/features/export.js:1130](../../../assets/js/features/export.js) | input | {"type":"number"} | inner += '&lt;input id="' + fieldId + '" type="number" data-k="' + f.k + '" min="' + (f.min != null ? f.min : '') + '" max="' + (f.max != null ? f.max : '') + '" step="' + (f.step != null ? f.step : '1') + '"&gt;'; |
| [assets/js/features/export.js:1132](../../../assets/js/features/export.js) | input | {"type":"text"} | inner += '&lt;input id="' + fieldId + '" type="text" data-k="' + f.k + '"&gt;'; |
| [assets/js/features/graph.js:107](../../../assets/js/features/graph.js) | button | {"id":"graph-btn-fx","class":"graph-tool-btn graph-fx-btn","aria-pressed":"${fxOn}"} | &lt;div class="graph-toolbar"&gt;&lt;button id="graph-btn-fx" class="graph-tool-btn graph-fx-btn" aria-pressed="${fxOn}" title="${t('graph.effects')}" aria-label="${t('graph.effects')}"&gt;✦&lt;/button&gt;&lt;button id="graph-btn-dimension" class="graph-tool-btn" aria-pressed="tru |
| [assets/js/features/graph.js:107](../../../assets/js/features/graph.js) | button | {"id":"graph-btn-dimension","class":"graph-tool-btn","aria-pressed":"true"} | &lt;div class="graph-toolbar"&gt;&lt;button id="graph-btn-fx" class="graph-tool-btn graph-fx-btn" aria-pressed="${fxOn}" title="${t('graph.effects')}" aria-label="${t('graph.effects')}"&gt;✦&lt;/button&gt;&lt;button id="graph-btn-dimension" class="graph-tool-btn" aria-pressed="tru |
| [assets/js/features/graph.js:107](../../../assets/js/features/graph.js) | button | {"id":"graph-btn-reset","class":"graph-tool-btn"} | &lt;div class="graph-toolbar"&gt;&lt;button id="graph-btn-fx" class="graph-tool-btn graph-fx-btn" aria-pressed="${fxOn}" title="${t('graph.effects')}" aria-label="${t('graph.effects')}"&gt;✦&lt;/button&gt;&lt;button id="graph-btn-dimension" class="graph-tool-btn" aria-pressed="tru |
| [assets/js/features/graph.js:107](../../../assets/js/features/graph.js) | button | {"id":"graph-btn-close","class":"graph-tool-btn"} | &lt;div class="graph-toolbar"&gt;&lt;button id="graph-btn-fx" class="graph-tool-btn graph-fx-btn" aria-pressed="${fxOn}" title="${t('graph.effects')}" aria-label="${t('graph.effects')}"&gt;✦&lt;/button&gt;&lt;button id="graph-btn-dimension" class="graph-tool-btn" aria-pressed="tru |
| [assets/js/features/graph.js:108](../../../assets/js/features/graph.js) | input | {"id":"graph-search","type":"search"} | &lt;div class="graph-workspace"&gt;&lt;aside class="graph-sidebar"&gt;&lt;label class="ux-search"&gt;&lt;span&gt;⌕&lt;/span&gt;&lt;input id="graph-search" type="search" placeholder="${t('ux.graphSearch')}" aria-label="${t('ux.graphSearch')}"&gt;&lt;/label&gt; |
| [assets/js/features/graph.js:109](../../../assets/js/features/graph.js) | input | {"id":"graph-labels","type":"checkbox"} | &lt;div class="graph-options"&gt;&lt;label&gt;&lt;input id="graph-labels" type="checkbox" checked&gt; ${t('ux.labels')}&lt;/label&gt;&lt;label&gt;&lt;input id="graph-neighbors" type="checkbox"&gt; ${t('ux.neighbors')}&lt;/label&gt;&lt;/div&gt; |
| [assets/js/features/graph.js:109](../../../assets/js/features/graph.js) | input | {"id":"graph-neighbors","type":"checkbox"} | &lt;div class="graph-options"&gt;&lt;label&gt;&lt;input id="graph-labels" type="checkbox" checked&gt; ${t('ux.labels')}&lt;/label&gt;&lt;label&gt;&lt;input id="graph-neighbors" type="checkbox"&gt; ${t('ux.neighbors')}&lt;/label&gt;&lt;/div&gt; |
| [assets/js/features/graph.js:114](../../../assets/js/features/graph.js) | button | {"id":"graph-btn-zoom-out","class":"graph-tool-btn"} | &lt;div class="graph-viewport-tools"&gt;&lt;button id="graph-btn-zoom-out" class="graph-tool-btn" aria-label="${t('graph.zoomOut')}"&gt;−&lt;/button&gt;&lt;output id="graph-zoom"&gt;100%&lt;/output&gt;&lt;button id="graph-btn-zoom-in" class="graph-tool-btn" aria-label="${t('graph.zoomIn')}"&gt;+ |
| [assets/js/features/graph.js:114](../../../assets/js/features/graph.js) | button | {"id":"graph-btn-zoom-in","class":"graph-tool-btn"} | &lt;div class="graph-viewport-tools"&gt;&lt;button id="graph-btn-zoom-out" class="graph-tool-btn" aria-label="${t('graph.zoomOut')}"&gt;−&lt;/button&gt;&lt;output id="graph-zoom"&gt;100%&lt;/output&gt;&lt;button id="graph-btn-zoom-in" class="graph-tool-btn" aria-label="${t('graph.zoomIn')}"&gt;+ |
| [assets/js/features/graph.js:246](../../../assets/js/features/graph.js) | button | {"style":"--h:${n.hue};--i:${Math.min(i, 24)}","data-node":"${esc(n.key)}","aria-pressed":"${n === selected}"} | list.innerHTML = filtered.length ? filtered.map((n, i) =&gt; `&lt;button class="graph-node-row ${n === selected ? 'selected' : ''} ${animateList ? 'enter' : ''}" style="--h:${n.hue};--i:${Math.min(i, 24)}" data-node="${esc(n.key)}" aria-pressed="${n === selected}"&gt;&lt; |
| [assets/js/features/graph.js:273](../../../assets/js/features/graph.js) | button | {"class":"graph-chip","style":"--h:${m.hue}","data-node":"${esc(m.key)}"} | detail.innerHTML = `&lt;span class="graph-detail-orb" aria-hidden="true"&gt;&lt;/span&gt;&lt;div&gt;&lt;strong&gt;${esc(n.label)}&lt;/strong&gt;&lt;p&gt;${esc(n.path \|\| t('graph.deadlinkUncreated'))}&lt;/p&gt;&lt;small&gt;${t('ux.connections', { count: n.neighbors.size })}&lt;/small&gt;${near.length ? `&lt;div class |
| [assets/js/features/graph.js:273](../../../assets/js/features/graph.js) | button | {"class":"tb-btn accent graph-open-btn"} | detail.innerHTML = `&lt;span class="graph-detail-orb" aria-hidden="true"&gt;&lt;/span&gt;&lt;div&gt;&lt;strong&gt;${esc(n.label)}&lt;/strong&gt;&lt;p&gt;${esc(n.path \|\| t('graph.deadlinkUncreated'))}&lt;/p&gt;&lt;small&gt;${t('ux.connections', { count: n.neighbors.size })}&lt;/small&gt;${near.length ? `&lt;div class |
| [assets/js/features/graph.js:699](../../../assets/js/features/graph.js) | button | {"id":"backlinks-btn-close","class":"graph-tool-btn"} | drawer.innerHTML=`&lt;header class="backlinks-header"&gt;&lt;div&gt;&lt;h3 id="backlinks-title"&gt;${t('graph.backlinks')}&lt;/h3&gt;&lt;p id="backlinks-file"&gt;&lt;/p&gt;&lt;/div&gt;&lt;button id="backlinks-btn-close" class="graph-tool-btn" aria-label="${t('toolbar.close')}"&gt;×&lt;/button&gt;&lt;/header&gt; |
| [assets/js/features/graph.js:700](../../../assets/js/features/graph.js) | input | {"id":"backlinks-search","type":"search"} | &lt;label class="ux-search"&gt;&lt;span&gt;⌕&lt;/span&gt;&lt;input id="backlinks-search" type="search" placeholder="${t('ux.filterLinks')}" aria-label="${t('ux.filterLinks')}"&gt;&lt;/label&gt; |
| [assets/js/features/graph.js:701](../../../assets/js/features/graph.js) | button | {"data-tab":"incoming","role":"tab","aria-selected":"true"} | &lt;div class="backlinks-tabs" role="tablist"&gt;&lt;button data-tab="incoming" role="tab" aria-selected="true"&gt;&lt;/button&gt;&lt;button data-tab="outgoing" role="tab" aria-selected="false"&gt;&lt;/button&gt;&lt;/div&gt; |
| [assets/js/features/graph.js:701](../../../assets/js/features/graph.js) | button | {"data-tab":"outgoing","role":"tab","aria-selected":"false"} | &lt;div class="backlinks-tabs" role="tablist"&gt;&lt;button data-tab="incoming" role="tab" aria-selected="true"&gt;&lt;/button&gt;&lt;button data-tab="outgoing" role="tab" aria-selected="false"&gt;&lt;/button&gt;&lt;/div&gt; |
| [assets/js/features/graph.js:721](../../../assets/js/features/graph.js) | button | {} | return `&lt;button class="backlink-item ${!path?'deadlink':''}" data-path="${esc(path\|\|'')}" ${!path?'disabled':''}&gt;&lt;span class="backlink-title"&gt;${esc(title)}&lt;/span&gt;&lt;span class="backlink-context"&gt;${esc(item.alias\|\|'')}${item.line_no?' · '+t('graph.linePrefix',{li |
| [assets/js/features/pet-workbench.js:158](../../../assets/js/features/pet-workbench.js) | button | {"id":"pet-mode-social","type":"button","data-i18n":"ux.petSocial"} | companion.innerHTML = `&lt;h4 data-i18n="ux.petStyle"&gt;${t('ux.petStyle')}&lt;/h4&gt;&lt;div class="pet-mode-buttons"&gt;&lt;button id="pet-mode-social" type="button" data-i18n="ux.petSocial"&gt;${t('ux.petSocial')}&lt;/button&gt;&lt;button id="pet-mode-quiet" type="button" data-i18n="ux.pe |
| [assets/js/features/pet-workbench.js:158](../../../assets/js/features/pet-workbench.js) | button | {"id":"pet-mode-quiet","type":"button","data-i18n":"ux.petQuiet"} | companion.innerHTML = `&lt;h4 data-i18n="ux.petStyle"&gt;${t('ux.petStyle')}&lt;/h4&gt;&lt;div class="pet-mode-buttons"&gt;&lt;button id="pet-mode-social" type="button" data-i18n="ux.petSocial"&gt;${t('ux.petSocial')}&lt;/button&gt;&lt;button id="pet-mode-quiet" type="button" data-i18n="ux.pe |
| [assets/js/features/pet-workbench.js:158](../../../assets/js/features/pet-workbench.js) | button | {"id":"pet-say-hello","type":"button","data-i18n":"ux.petHello"} | companion.innerHTML = `&lt;h4 data-i18n="ux.petStyle"&gt;${t('ux.petStyle')}&lt;/h4&gt;&lt;div class="pet-mode-buttons"&gt;&lt;button id="pet-mode-social" type="button" data-i18n="ux.petSocial"&gt;${t('ux.petSocial')}&lt;/button&gt;&lt;button id="pet-mode-quiet" type="button" data-i18n="ux.pe |
| [assets/js/features/pet-workbench.js:158](../../../assets/js/features/pet-workbench.js) | button | {"id":"pet-chat-open","type":"button"} | companion.innerHTML = `&lt;h4 data-i18n="ux.petStyle"&gt;${t('ux.petStyle')}&lt;/h4&gt;&lt;div class="pet-mode-buttons"&gt;&lt;button id="pet-mode-social" type="button" data-i18n="ux.petSocial"&gt;${t('ux.petSocial')}&lt;/button&gt;&lt;button id="pet-mode-quiet" type="button" data-i18n="ux.pe |
| [assets/js/reader/render.js:383](../../../assets/js/reader/render.js) | a | {"class":"wikilink","data-target":"${safeTarget}","href":"javascript:void(0)"} | return `&lt;a class="wikilink" data-target="${safeTarget}" href="javascript:void(0)" title="${escapeHtml((window.i18n ? window.i18n.t('wikilink.jump', { target: fullTarget }) : '双链跳转: ' + fullTarget))}"&gt;${safeDisplay}&lt;/a&gt;`; |
| [assets/js/reader/render.js:982](../../../assets/js/reader/render.js) | button | {"class":"code-chunk-run-btn"} | &lt;button class="code-chunk-run-btn" title="${_t('menu.runCode')} (Shift+Enter)" aria-label="${_t('menu.runCode')}"&gt;▶ ${_t('menu.runCode')}&lt;/button&gt; |
| [assets/js/reader/render.js:992](../../../assets/js/reader/render.js) | button | {"class":"code-chunk-copy-btn"} | &lt;button class="code-chunk-copy-btn" title="${_t('reader.copyOutput')}" aria-label="${_t('reader.copyOutput')}"&gt;${_t('reader.copyOutput')}&lt;/button&gt; |
| [assets/js/reader/render.js:993](../../../assets/js/reader/render.js) | button | {"class":"code-chunk-clear-btn"} | &lt;button class="code-chunk-clear-btn" title="${_t('reader.clearOutput')}" aria-label="${_t('reader.clearOutput')}"&gt;${_t('reader.clearOutput')}&lt;/button&gt; |
| [assets/js/reader/render.js:1009](../../../assets/js/reader/render.js) | button | {"class":"diagram-reload-btn"} | &lt;button class="diagram-reload-btn" title="${_t('reader.refresh')}" aria-label="${_t('reader.refresh')}"&gt;⟳ ${_t('reader.refresh')}&lt;/button&gt; |
| [assets/js/reader/render.js:1012](../../../assets/js/reader/render.js) | summary | {} | &lt;details class="diagram-src-wrap"&gt;&lt;summary&gt;${_t('reader.viewCode')}&lt;/summary&gt;&lt;pre&gt;&lt;code class="language-${lang}"&gt;${escaped ? code : (window.escapeHtml ? escapeHtml(code) : code)}&lt;/code&gt;&lt;/pre&gt;&lt;/details&gt; |
| [assets/js/reader/render.js:1279](../../../assets/js/reader/render.js) | button | {"class":"btn btn-sm btn-primary","id":"btn-code-to-md","data-i18n":"codebar.aiToMd"} | &lt;button class="btn btn-sm btn-primary" id="btn-code-to-md" data-i18n="codebar.aiToMd" title="${escapeHtml(_t('codebar.aiToMdTip') \|\| '转换为结构化 Markdown 文档')}"&gt;${_t('codebar.aiToMd') \|\| 'AI 结构化转 MD'}&lt;/button&gt; |
| [assets/js/reader/render.js:1280](../../../assets/js/reader/render.js) | button | {"class":"btn btn-sm","id":"btn-code-edit","data-i18n":"codebar.edit"} | &lt;button class="btn btn-sm" id="btn-code-edit" data-i18n="codebar.edit" title="${escapeHtml(_t('codebar.editTip') \|\| '进入源码编辑器 (Ctrl+E)')}"&gt;${_t('codebar.edit') \|\| '编辑源码 (Ctrl+E)'}&lt;/button&gt; |
| [assets/js/reader/render.js:1281](../../../assets/js/reader/render.js) | button | {"class":"btn btn-sm","id":"btn-code-ai-explain","data-i18n":"codebar.aiExplain"} | &lt;button class="btn btn-sm" id="btn-code-ai-explain" data-i18n="codebar.aiExplain" title="${escapeHtml(_t('codebar.aiExplainTip') \|\| '调用 AI 进行深度解析与排错')}"&gt;${_t('codebar.aiExplain') \|\| 'AI 深度解析'}&lt;/button&gt; |
| [assets/js/reader/render.js:1282](../../../assets/js/reader/render.js) | button | {"class":"btn btn-sm","id":"btn-code-copy","data-i18n":"codebar.copyCode"} | &lt;button class="btn btn-sm" id="btn-code-copy" data-i18n="codebar.copyCode" title="${escapeHtml(_t('codebar.copyCodeTip') \|\| '复制代码正文')}"&gt;${_t('codebar.copyCode') \|\| '复制代码'}&lt;/button&gt; |
| [assets/js/reader/render.js:2108](../../../assets/js/reader/render.js) | button | {"type":"button","class":"diagram-inline-retry","aria-label":"${refreshLabel}"} | return `&lt;div class="diagram-fallback-wrap"&gt;&lt;div class="diagram-fallback-hint"&gt;${safeMessage}&lt;/div&gt;&lt;button type="button" class="diagram-inline-retry" aria-label="${refreshLabel}"&gt;&lt;svg class="tb-ic" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-wi |
| [assets/js/reader/render.js:2516](../../../assets/js/reader/render.js) | button | {"type":"button","class":"diagram-inline-retry diagram-allow-remote-btn","aria-label":"${btnText}"} | previewEl.innerHTML = `&lt;div class="diagram-fallback-wrap"&gt;&lt;div class="diagram-fallback-hint"&gt;${confirmText}&lt;/div&gt;&lt;button type="button" class="diagram-inline-retry diagram-allow-remote-btn" aria-label="${btnText}"&gt;&lt;svg class="tb-ic" viewBox="0 0 24 24" fill="no |
| [assets/js/reader/render.js:2719](../../../assets/js/reader/render.js) | select | {"id":"presentation-theme-select","class":"presentation-select","title":"演说主题","data-i18n-title":"presentation.themeTitle"} | &lt;select id="presentation-theme-select" class="presentation-select" title="演说主题" data-i18n-title="presentation.themeTitle"&gt; |
| [assets/js/reader/render.js:2734](../../../assets/js/reader/render.js) | select | {"id":"presentation-transition-select","class":"presentation-select","title":"转场特效","data-i18n-title":"presentation.transitionTitle"} | &lt;select id="presentation-transition-select" class="presentation-select" title="转场特效" data-i18n-title="presentation.transitionTitle"&gt; |
| [assets/js/reader/render.js:2744](../../../assets/js/reader/render.js) | button | {"type":"button","class":"presentation-btn","id":"presentation-font-dec","title":"缩小字号 (20px)","data-i18n-title":"presentation.fontDec"} | &lt;button type="button" class="presentation-btn" id="presentation-font-dec" title="缩小字号 (20px)" data-i18n-title="presentation.fontDec"&gt;A-&lt;/button&gt; |
| [assets/js/reader/render.js:2745](../../../assets/js/reader/render.js) | button | {"type":"button","class":"presentation-btn active","id":"presentation-font-norm","title":"标准字号 (24px)","data-i18n-title":"presentation.fontNorm"} | &lt;button type="button" class="presentation-btn active" id="presentation-font-norm" title="标准字号 (24px)" data-i18n-title="presentation.fontNorm"&gt;A&lt;/button&gt; |
| [assets/js/reader/render.js:2746](../../../assets/js/reader/render.js) | button | {"type":"button","class":"presentation-btn","id":"presentation-font-inc","title":"放大字号 (28px)","data-i18n-title":"presentation.fontInc"} | &lt;button type="button" class="presentation-btn" id="presentation-font-inc" title="放大字号 (28px)" data-i18n-title="presentation.fontInc"&gt;A+&lt;/button&gt; |
| [assets/js/reader/render.js:2748](../../../assets/js/reader/render.js) | button | {"type":"button","class":"presentation-btn","id":"presentation-overview-btn"} | &lt;button type="button" class="presentation-btn" id="presentation-overview-btn" title="${_t('presentation.overviewTitle')}"&gt;${_t('presentation.overviewLabel')}&lt;/button&gt; |
| [assets/js/reader/render.js:2749](../../../assets/js/reader/render.js) | button | {"type":"button","class":"presentation-btn","id":"presentation-fullscreen-btn"} | &lt;button type="button" class="presentation-btn" id="presentation-fullscreen-btn" title="${_t('presentation.fullscreenTitle')}"&gt;${_t('presentation.fullscreenLabel')}&lt;/button&gt; |
| [assets/js/reader/render.js:2750](../../../assets/js/reader/render.js) | button | {"type":"button","class":"presentation-close-btn","id":"presentation-close-btn","data-i18n-title":"presentation.closeTitle"} | &lt;button type="button" class="presentation-close-btn" id="presentation-close-btn" title="${_t('presentation.closeTitle')}" data-i18n-title="presentation.closeTitle"&gt;&lt;svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width=" |
| [assets/js/reader/toc.js:32](../../../assets/js/reader/toc.js) | button | {"type":"button","class":"side-empty-close","id":"toc-empty-close"} | &lt;button type="button" class="side-empty-close" id="toc-empty-close"&gt;${_t('toolbar.close') \|\| '收起侧栏'}&lt;/button&gt; |
| [assets/js/shell/shell-deferred.js:510](../../../assets/js/shell/shell-deferred.js) | input | {"id":"cmdp-input","type":"text","role":"combobox","aria-expanded":"true","aria-controls":"cmdp-list","aria-autocomplete":"list","autocomplete":"off","autocapitalize":"off","spellcheck":"false"} | &lt;input id="cmdp-input" type="text" role="combobox" aria-expanded="true" aria-controls="cmdp-list" aria-autocomplete="list" autocomplete="off" autocapitalize="off" spellcheck="false"&gt; |
| [assets/js/shell/shell-deferred.js:669](../../../assets/js/shell/shell-deferred.js) | input | {"id":"shortcuts-filter","type":"search","autocomplete":"off","spellcheck":"false"} | &lt;input id="shortcuts-filter" type="search" autocomplete="off" spellcheck="false"&gt; |
| [assets/js/shell/shell-deferred.js:670](../../../assets/js/shell/shell-deferred.js) | button | {"id":"shortcuts-close","type":"button","class":"rm-icon-btn"} | &lt;button id="shortcuts-close" type="button" class="rm-icon-btn"&gt; |


## 2026-10-04 桌面窗口补充

| ID / 控件族 | 定位 | 全部动作 | 出现条件 | 前后端 | 源码 |
| --- | --- | --- | --- | --- | --- |
| D079 | window-drag-region / window-resize-edge | 空白区按下并移动拖动窗口；双击最大化/还原；上/右上/右/右下/下/左下/左/左上八个边缘原生缩放，最大化/全屏时隐藏。 | Windows桌面宿主 | main.rs Tao drag_window/drag_resize_window | [assets/js/shell/shell.js:99 · initWindowChrome](../../../assets/js/shell/shell.js) |
| D080 | ReadMD.Tray.Owner / Shell_NotifyIcon | 单击/双击恢复驻留窗口；右键菜单：打开ReadMD、打开文件、退出ReadMD；完全退出时保留脏修改保护。 | Windows通知区域可用 | native_tray.rs Win32通知图标和原生菜单 → Tao事件 | [rust/readmd-kernel/src/native_tray.rs:49 · procedure](../../../rust/readmd-kernel/src/native_tray.rs) |
