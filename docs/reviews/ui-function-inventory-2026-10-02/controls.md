# 静态控件逐项索引

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)。2026-10-07 增量更新，机器清单共 406 项（405 个有效控件、1 个已移除历史控件）。范围含输入、隐藏文件输入、summary 和 role=button；运行时列表另见 dynamic.md。全局按钮在运行时移入应用顶栏。

| 作用域 | 数量 |
| --- | --- |
| ai-history-modal | 8 |
| ai-panel | 13 |
| ai-settings-modal | 21 |
| app-titlebar | 9 |
| close-confirm-modal | 3 |
| cm-selection-toolbar | 11 |
| code-chunk-modal | 7 |
| confirm-modal | 2 |
| continuous-modal | 2 |
| convert-modal | 8 |
| diagram-modal | 5 |
| doc-import-modal | 7 |
| edit-bar | 48 |
| export-modal | 23 |
| export-preview-modal | 3 |
| fix-modal | 3 |
| formula-modal | 3 |
| frontmatter-modal | 7 |
| global | 3 |
| history-modal | 2 |
| img-modal | 28 |
| lang-modal | 2 |
| layout | 3 |
| main-col | 28 |
| pet-settings-modal | 16 |
| plugin-modal | 11 |
| readmd-pet-widget | 3 |
| save-conflict-modal | 3 |
| search-bar | 4 |
| share-modal | 4 |
| skill-create-modal | 7 |
| statusbar | 3 |
| style-custom-modal | 12 |
| tab-context-menu | 7 |
| table-modal | 1 |
| toolbar | 35 |
| tpl-modal | 28 |
| update-modal | 5 |
| url-modal | 11 |

## ai-history-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S187 / F051 | ai-history-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭会话历史", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:757](../../../assets/index.html)；assets/app.js:604 |
| S188 / F051 | ai-history-search | 搜索历史会话... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "search", "placeholder": "搜索历史会话...", "data-i18n-placeholder": "ai.searchHistory", "aria-label": "搜索会话历史", "data-i18n-aria": "ai.searchHistory"} | — | 依文档/平台/面板状态显示 | [assets/index.html:761](../../../assets/index.html)；assets/js/features/ai.js:1163；assets/app.js:606 |
| S189 / F051 | ai-history-copy | 复制回答 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.copy"} | — | 依文档/平台/面板状态显示 | [assets/index.html:763](../../../assets/index.html)；assets/app.js:607 |
| S190 / F051 | ai-history-export | 导出记录 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.exportHistory"} | — | 依文档/平台/面板状态显示 | [assets/index.html:763](../../../assets/index.html)；assets/app.js:608 |
| S191 / F051 | ai-history-clear | 清空历史 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.clearHistory"} | — | 依文档/平台/面板状态显示 | [assets/index.html:763](../../../assets/index.html)；assets/app.js:609 |
| S192 / F051 | ai-session | ai-session | 选择值触发change；完整值/显示文案见本行选项 | {"aria-hidden": "true"} | — | 初始隐藏 | [assets/index.html:765](../../../assets/index.html)；assets/js/features/ai.js:1179,1216,1452,1459,1483,2352,2369；assets/app.js:642 |
| S193 / F051 | ai-save-session | 保存 | 点击执行所属流程；禁用/隐藏时不执行 | {"tabindex": "-1", "data-i18n": "ai.save"} | — | 初始隐藏 | [assets/index.html:766](../../../assets/index.html)；assets/app.js:643 |
| S194 / F051 | ai-del-session | 删除 | 点击执行所属流程；禁用/隐藏时不执行 | {"tabindex": "-1", "data-i18n": "ai.delete"} | — | 初始隐藏 | [assets/index.html:766](../../../assets/index.html)；assets/app.js:644 |

## ai-panel

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S134 / F051 | ai-history-open | 会话历史 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "会话历史", "aria-label": "会话历史", "data-i18n-title": "ai.historyTitle", "data-i18n-aria": "ai.historyTitle"} | — | 依文档/平台/面板状态显示 | [assets/index.html:434](../../../assets/index.html)；assets/app.js:603 |
| S135 / F046 | ai-settings-open | AI 连接设置 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "AI 连接设置", "aria-label": "AI 连接设置", "data-i18n-title": "ai.settings", "data-i18n-aria": "ai.settings"} | — | 依文档/平台/面板状态显示 | [assets/index.html:437](../../../assets/index.html)；assets/js/features/ai.js:142,160,163,1623,1624；assets/app.js:600 |
| S136 / F041 | ai-expand-toggle | 切换分屏 / 全屏 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "切换全屏 / 分屏", "aria-label": "切换全屏或分屏", "data-i18n-title": "ai.toggleFullscreen", "data-i18n-aria": "ai.toggleFullscreen"} | — | 依文档/平台/面板状态显示 | [assets/index.html:440](../../../assets/index.html)；assets/app.js:646 |
| S137 / F041 | ai-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "关闭", "aria-label": "关闭 AI 对话", "data-i18n-title": "toolbar.close", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:444](../../../assets/index.html)；assets/js/editor/editor.js:1636；assets/app.js:599 |
| S138 / F041 | ai-jump-latest | ↓ 回到最新消息 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ux.latest"} | — | 初始隐藏 | [assets/index.html:454](../../../assets/index.html)；assets/js/features/ai.js:154,2158 |
| S139 / F042 | ai-template | Prompt 模版 | 选择值触发change；完整值/显示文案见本行选项 | {"title": "Prompt 模板：选择后应用到本次对话", "data-i18n-title": "ai.tplSelectTitle", "aria-label": "Prompt 模板", "data-i18n-aria": "ai.tplAria"} | — | 依文档/平台/面板状态显示 | [assets/index.html:461](../../../assets/index.html)；assets/js/features/ai.js:279,316,1934；assets/app.js:630 |
| S140 / F042 | ai-tpl-btn | 模板 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "Skill 工作台与导入", "data-i18n-title": "ai.templateManage", "aria-label": "Skill 工作台", "data-i18n-aria": "ai.templateManage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:463](../../../assets/index.html)；assets/js/features/ai.js:1935；assets/app.js:631 |
| S141 / F042 | ai-prompt | 向 AI 提问，或选择上方快捷指令... (Enter 发送, Shift+Enter 换行) | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"rows": "2", "placeholder": "向 AI 提问，或选择快捷指令... (Enter 发送, Shift+Enter 换行)", "aria-label": "向 AI 提问", "data-i18n-placeholder": "ai.promptPlaceholder", "data-i18n-aria": "ai.promptPlaceholder", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:469](../../../assets/index.html)；assets/js/editor/editor.js:1401,1402,1404；assets/js/features/ai.js:41,72,135,136,137,154,327,328,656,1325,1433,1932,2066,2533；assets/js/features/export.js:1477；assets/app.js:629,759,870,871 |
| S142 / F042 | ai-selection | ai-selection | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox"} | — | 依文档/平台/面板状态显示 | [assets/index.html:472](../../../assets/index.html)；assets/js/features/ai.js:1873,1937 |
| S143 / F042 | ai-incognito | ai-incognito | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox"} | — | 依文档/平台/面板状态显示 | [assets/index.html:473](../../../assets/index.html)；assets/js/features/ai.js:1938,2092 |
| S144 / F042 | ai-clear-ctx | 新对话 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "开始一轮新对话", "data-i18n": "ai.newSession", "data-i18n-title": "ai.newSession"} | — | 依文档/平台/面板状态显示 | [assets/index.html:477](../../../assets/index.html)；assets/js/features/ai.js:1936；assets/app.js:645 |
| S145 / F042 | ai-stop | 停止 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.stop", "disabled": null} | — | 初始隐藏 | [assets/index.html:478](../../../assets/index.html)；assets/js/features/ai.js:1909；assets/app.js:625 |
| S146 / F042 | ai-run | 发送 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "发送 (Enter)", "aria-label": "发送", "data-i18n-title": "ai.send", "data-i18n-aria": "ai.send"} | — | 依文档/平台/面板状态显示 | [assets/index.html:479](../../../assets/index.html)；assets/js/features/ai.js:1908；assets/app.js:624,629 |

## ai-settings-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S166 / F046 | ai-settings-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭连接设置", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:660](../../../assets/index.html)；assets/app.js:601 |
| S167 / F046 | ai-provider | 提供商 | 选择值触发change；完整值/显示文案见本行选项 | {"title": "选择提供商（预设或自定义）", "data-i18n-title": "ai.provider"} | — | 依文档/平台/面板状态显示 | [assets/index.html:669](../../../assets/index.html)；assets/js/features/ai.js:1225,1437,1526,1540,1541,1551,1560,1570,1578,1671,1672,1673,1743,1785,2353；assets/app.js:610,611,612,614,615 |
| S168 / F047 | ai-provider-new | 新建自定义连接 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "新建自定义连接", "aria-label": "新建自定义连接", "data-i18n-title": "ai.newProvider", "data-i18n-aria": "ai.newProvider"} | — | 依文档/平台/面板状态显示 | [assets/index.html:670](../../../assets/index.html)；assets/app.js:614 |
| S169 / F047 | ai-provider-delete | 删除当前自定义连接 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "删除当前自定义连接", "aria-label": "删除当前自定义连接", "data-i18n-title": "ai.deleteProvider", "data-i18n-aria": "ai.deleteProvider"} | — | 依文档/平台/面板状态显示 | [assets/index.html:671](../../../assets/index.html)；assets/js/features/ai.js:1673；assets/app.js:615 |
| S170 / F047 | ai-model | 模型 | 选择值触发change；完整值/显示文案见本行选项 | {"title": "选择模型", "data-i18n-title": "ai.model"} | — | 依文档/平台/面板状态显示 | [assets/index.html:677](../../../assets/index.html)；assets/js/features/ai.js:190,191,675,1227,1438,1562,1584,1649,1812,1814,1973,1988,2093,2305,2353；assets/app.js:612,620,621 |
| S171 / F047 | ai-models-btn | 获取模型 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "通过 API Key 获取可用模型列表", "data-i18n": "ai.fetchModels", "data-i18n-title": "ai.fetchModels"} | — | 依文档/平台/面板状态显示 | [assets/index.html:678](../../../assets/index.html)；assets/js/features/ai.js:1973,2305；assets/app.js:620 |
| S172 / F048 | ai-key | 必填（本地 Ollama 可留空） | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "password", "placeholder": "必填（本地 Ollama 可留空）", "data-i18n-placeholder": "ai.apiKeyPlaceholder", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:684](../../../assets/index.html)；assets/js/features/ai.js:1611,1682,1685,1686,1773,1954,2008,2010,2011,2020；assets/app.js:618,619 |
| S173 / F048 | ai-key-toggle | 显示 / 隐藏 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-pressed": "false", "title": "显示 / 隐藏", "aria-label": "显示或隐藏 API Key", "data-i18n-title": "ai.toggleKey", "data-i18n-aria": "ai.toggleKey"} | — | 依文档/平台/面板状态显示 | [assets/index.html:685](../../../assets/index.html)；assets/js/features/ai.js:1685,1686,2010,2011；assets/app.js:618 |
| S174 / F048 | ai-key-clear | 清除已保存的 API Key | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "清除已保存的 API Key", "aria-label": "清除已保存的 API Key", "data-i18n-title": "ai.clearKey", "data-i18n-aria": "ai.clearKey"} | — | 依文档/平台/面板状态显示 | [assets/index.html:688](../../../assets/index.html)；assets/app.js:619 |
| S175 / F046 | {"data-i18n": "ai.advanced"} | 高级参数 | 点击/Enter展开或折叠所在details；不直接调用业务API | {"data-i18n": "ai.advanced"} | — | 依文档/平台/面板状态显示 | [assets/index.html:694](../../../assets/index.html)； |
| S176 / F049 | ai-provider-name | 我的 AI 连接 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "我的 AI 连接", "data-i18n-placeholder": "ai.customNamePlaceholder", "maxlength": "60", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:698](../../../assets/index.html)；assets/js/features/ai.js:1671,1672,1743,1785 |
| S177 / F049 | ai-base-url | Base URL | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "https://api.example.com/v1", "autocomplete": "off", "spellcheck": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:703](../../../assets/index.html)；assets/js/features/ai.js:1665,1774,1953,2028,2095,2320 |
| S178 / F049 | ai-url-reset | 恢复预设地址 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "恢复预设地址", "aria-label": "恢复预设地址", "data-i18n-title": "ai.resetUrl", "data-i18n-aria": "ai.resetUrl"} | — | 依文档/平台/面板状态显示 | [assets/index.html:704](../../../assets/index.html)；assets/app.js:617 |
| S179 / F049 | ai-mode | 响应方式 | 选择值触发change；完整值/显示文案见本行选项 | {"title": "接口协议 / 端点", "data-i18n-title": "ai.protocol"} | [{"value": "auto", "text": "自动（按提供商）"}, {"value": "chat", "text": "OpenAI chat/completions"}, {"value": "completion", "text": "OpenAI completions"}, {"value": "responses", "text": "OpenAI responses"}, {"value": "messages", "text": "Anthropic messages"}] | 依文档/平台/面板状态显示 | [assets/index.html:711](../../../assets/index.html)；assets/js/features/ai.js:190,191,675,1227,1438,1562,1584,1649,1667,1775,1812,1814,1955,1973,1988,2030,2093,2094,2305,2320,2353；assets/app.js:612,616,620,621 |
| S180 / F049 | ai-endpoint-mode | 选择 URL 是前缀还是完整端点 | 选择值触发change；完整值/显示文案见本行选项 | {"title": "选择 URL 是前缀还是完整端点", "data-i18n-title": "ai.endpointModeTip"} | [{"value": "prefix", "text": "Base URL（自动追加接口路径）"}, {"value": "full_url", "text": "完整接口 URL（不追加路径）"}] | 依文档/平台/面板状态显示 | [assets/index.html:721](../../../assets/index.html)；assets/js/features/ai.js:1668,1776,1956,2097,2321 |
| S181 / F049 | ai-headers | 自定义请求头（JSON，可选） | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"rows": "3", "placeholder": "{\"X-Organization\":\"example\"}", "spellcheck": "false", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:728](../../../assets/index.html)；assets/js/features/ai.js:1638,1674 |
| S182 / F049 | ai-stream | ai-stream | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "checked": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:731](../../../assets/index.html)；assets/js/features/ai.js:2096 |
| S183 / F046 | {"data-i18n": "ai.providerDirectory"} | 提供商目录 | 点击/Enter展开或折叠所在details；不直接调用业务API | {"data-i18n": "ai.providerDirectory"} | — | 依文档/平台/面板状态显示 | [assets/index.html:735](../../../assets/index.html)； |
| S184 / F046 | ai-provider-search | 搜索/过滤提供商… | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "search", "placeholder": "搜索/过滤提供商…", "aria-label": "搜索提供商", "data-i18n-placeholder": "ai.providerSearchPlaceholder", "data-i18n-aria": "ai.providerSearchPlaceholder", "autocomplete": "off", "spellcheck": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:737](../../../assets/index.html)；assets/js/features/ai.js:1541；assets/app.js:611 |
| S185 / F049 | ai-save-key | 保存连接设置 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.saveSettings"} | — | 依文档/平台/面板状态显示 | [assets/index.html:745](../../../assets/index.html)；assets/js/features/ai.js:2304；assets/app.js:623 |
| S186 / F050 | ai-test-connection | 测试连接 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.testConnection"} | — | 依文档/平台/面板状态显示 | [assets/index.html:746](../../../assets/index.html)；assets/js/features/ai.js:2302；assets/app.js:622 |

## app-titlebar

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S002 / F001 | btn-open | 打开 (Ctrl+O) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "打开 Markdown 文件 (Ctrl+O)", "aria-label": "打开文件", "data-i18n-title": "toolbar.open", "data-i18n-aria": "toolbar.open"} | — | 依文档/平台/面板状态显示 | [assets/index.html:38](../../../assets/index.html)；assets/js/shell/shell.js:103；assets/app.js:87,139,1022 |
| S003 / F002 | btn-folder | 打开文件夹 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "打开文件夹", "aria-label": "打开文件夹", "data-i18n-title": "toolbar.openFolder", "data-i18n-aria": "toolbar.openFolder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:41](../../../assets/index.html)；assets/js/shell/shell-deferred.js:155；assets/js/shell/shell.js:103；assets/app.js:88,1202 |
| S004 / F005 | btn-recent | 最近打开 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "最近文件", "aria-label": "最近文件", "data-i18n-title": "app.recent", "data-i18n-aria": "app.recent"} | — | 依文档/平台/面板状态显示 | [assets/index.html:44](../../../assets/index.html)；assets/js/shell/shell.js:103；assets/app.js:462 |
| S008 / F096 | btn-palette | 命令面板 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "title": "命令面板 (Ctrl+K)", "aria-label": "命令面板", "aria-haspopup": "dialog", "aria-keyshortcuts": "Control+K Meta+K", "data-i18n-title": "shell.toolbar.paletteTip", "data-i18n-aria": "editor.command"} | — | 依文档/平台/面板状态显示 | [assets/index.html:58](../../../assets/index.html)；assets/js/shell/shell.js:73,103 |
| S011 / F016 | btn-theme | 切换主题 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "切换主题 (Ctrl+D)", "aria-label": "切换主题"} | — | 依文档/平台/面板状态显示 | [assets/index.html:71](../../../assets/index.html)；assets/js/core/settings.js:92；assets/js/shell/shell.js:103；assets/app.js:522 |
| S018 / F007 | btn-more | 更多功能 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "更多功能", "aria-label": "更多功能", "data-i18n-title": "toolbar.more", "data-i18n-aria": "toolbar.more", "aria-haspopup": "true", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:88](../../../assets/index.html)；assets/js/features/pet-batch.js:1192；assets/js/shell/shell.js:103；assets/app.js:74,92 |
| S392 / F104 | window-minimize | 最小化 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n-aria": "window.minimize"} | — | 依文档/平台/面板状态显示 | [assets/index.html:28](../../../assets/index.html)；assets/js/shell/shell.js:129 |
| S393 / F104 | window-maximize | 最大化/还原 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n-aria": "window.maximize"} | — | 依文档/平台/面板状态显示 | [assets/index.html:29](../../../assets/index.html)；assets/js/shell/shell.js:130,153,155 |
| S394 / F104 | window-close | 关闭/收起到托盘 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n-aria": "window.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:30](../../../assets/index.html)；assets/js/shell/shell.js:131,160 |

## close-confirm-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S278 / F097 | close-confirm-save | 保存修改 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.save"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1046](../../../assets/index.html)；assets/js/core/tabs.js:602,610 |
| S279 / F097 | close-confirm-discard | 不保存 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.dontSave"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1047](../../../assets/index.html)；assets/js/core/tabs.js:603,611 |
| S280 / F097 | close-confirm-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1048](../../../assets/index.html)；assets/js/core/tabs.js:604,612,617 |

## cm-selection-toolbar

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S092 / F024 | cm-sel-ai | AI 改写 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "AI 改写 (Alt+K)", "aria-label": "AI 改写", "data-i18n-title": "editor.aiInlineTip", "data-i18n-aria": "editor.aiInline"} | — | 依文档/平台/面板状态显示 | [assets/index.html:224](../../../assets/index.html)；assets/js/editor/editor.js:1635 |
| S093 / F024 | cm-sel-bold | 加粗 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-cmd": "bold", "aria-pressed": "false", "title": "加粗 (Ctrl+B)", "aria-label": "加粗", "data-i18n-title": "editor.boldTip", "data-i18n-aria": "editor.bold"} | — | 依文档/平台/面板状态显示 | [assets/index.html:229](../../../assets/index.html)；assets/js/editor/editor.js:186 |
| S094 / F024 | cm-sel-italic | 斜体 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-cmd": "italic", "aria-pressed": "false", "title": "斜体 (Ctrl+I)", "aria-label": "斜体", "data-i18n-title": "editor.italicTip", "data-i18n-aria": "editor.italic"} | — | 依文档/平台/面板状态显示 | [assets/index.html:232](../../../assets/index.html)；assets/js/editor/editor.js:186 |
| S095 / F024 | cm-sel-strike | 删除线 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-cmd": "strike", "aria-pressed": "false", "title": "删除线 (Ctrl+Shift+X)", "aria-label": "删除线", "data-i18n-title": "editor.strikeTip", "data-i18n-aria": "editor.strikethrough"} | — | 依文档/平台/面板状态显示 | [assets/index.html:235](../../../assets/index.html)；assets/js/editor/editor.js:186 |
| S096 / F024 | cm-sel-code | 行内代码 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-cmd": "code", "aria-pressed": "false", "title": "行内代码 (Ctrl+E)", "aria-label": "行内代码", "data-i18n-title": "editor.codeTip", "data-i18n-aria": "editor.codeInline"} | — | 依文档/平台/面板状态显示 | [assets/index.html:238](../../../assets/index.html)；assets/js/editor/editor.js:187 |
| S097 / F024 | cm-sel-link | 链接 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-cmd": "link", "aria-pressed": "false", "title": "链接 (Ctrl+K)", "aria-label": "链接", "data-i18n-title": "editor.linkTip", "data-i18n-aria": "editor.link"} | — | 依文档/平台/面板状态显示 | [assets/index.html:241](../../../assets/index.html)；assets/js/editor/editor.js:187 |
| S098 / F024 | cm-sel-heading | 标题 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-cmd": "h2", "title": "标题 (Ctrl+2)", "aria-label": "标题", "data-i18n-title": "editor.headingTip", "data-i18n-aria": "editor.h2"} | — | 依文档/平台/面板状态显示 | [assets/index.html:244](../../../assets/index.html)；assets/js/editor/editor.js:187 |
| S099 / F024 | cm-sel-quote | 引用 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-cmd": "quote", "title": "引用 (Ctrl+Shift+.)", "aria-label": "引用", "data-i18n-title": "editor.quoteTip", "data-i18n-aria": "editor.quote"} | — | 依文档/平台/面板状态显示 | [assets/index.html:247](../../../assets/index.html)；assets/js/editor/editor.js:187 |
| S100 / F024 | cm-sel-copy | 复制 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "复制 (Ctrl+C)", "aria-label": "复制", "data-i18n-title": "editor.copy", "data-i18n-aria": "editor.copy"} | — | 依文档/平台/面板状态显示 | [assets/index.html:251](../../../assets/index.html)；assets/app.js:365 |
| S101 / F024 | cm-sel-cut | 剪切 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "剪切 (Ctrl+X)", "aria-label": "剪切", "data-i18n-title": "editor.cut", "data-i18n-aria": "editor.cut"} | — | 依文档/平台/面板状态显示 | [assets/index.html:254](../../../assets/index.html)；assets/app.js:366 |
| S102 / F024 | cm-sel-paste | 粘贴 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "粘贴 (Ctrl+V)", "aria-label": "粘贴", "data-i18n-title": "editor.paste", "data-i18n-aria": "editor.paste"} | — | 依文档/平台/面板状态显示 | [assets/index.html:257](../../../assets/index.html)；assets/app.js:367 |

## code-chunk-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S366 / F033 | code-chunk-modal-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1531](../../../assets/index.html)；assets/app.js:237,707 |
| S367 / F033 | code-chunk-lang | 编程语言： | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "python", "text": "Python (Pyodide / 本地运行)"}, {"value": "javascript", "text": "JavaScript (Node / Web)"}, {"value": "bash", "text": "Bash / Shell"}, {"value": "r", "text": "R (科学计算)"}, {"value": "php", "text": "PHP"}, {"value": "go", "text": "Go"}, {"value": "ruby", "text": "Ruby"}] | 依文档/平台/面板状态显示 | [assets/index.html:1536](../../../assets/index.html)；assets/js/editor/editor.js:759,774；assets/app.js:240,711,712,714 |
| S368 / F033 | code-chunk-opt-plot | code-chunk-opt-plot | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "checked": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:1548](../../../assets/index.html)；assets/js/editor/editor.js:775 |
| S369 / F033 | code-chunk-opt-hide | code-chunk-opt-hide | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1552](../../../assets/index.html)；assets/js/editor/editor.js:776 |
| S370 / F033 | code-chunk-code | 代码内容： | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"placeholder": "import matplotlib.pyplot as plt\\nimport numpy as np\\nx = np.linspace(0, 10, 100)\\nplt.plot(x, np.sin(x))\\nplt.show()"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1558](../../../assets/index.html)；assets/js/editor/editor.js:760,777,778；assets/app.js:241,713,714 |
| S371 / F033 | code-chunk-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1561](../../../assets/index.html)；assets/app.js:238,708 |
| S372 / F033 | code-chunk-insert | 插入 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.insert"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1562](../../../assets/index.html)；assets/app.js:239,709 |

## confirm-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S284 / F097 | confirm-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1099](../../../assets/index.html)；assets/js/core/dialog.js:13；assets/js/core/tabs.js:604,612,617 |
| S285 / F097 | confirm-action | 确认 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.confirm"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1100](../../../assets/index.html)；assets/js/core/dialog.js:12 |

## continuous-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S286 / F097 | continuous-confirm | 全卷连续 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "pagination.continuousBadge"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1117](../../../assets/index.html)；assets/js/reader/render.js:657,663,664 |
| S287 / F097 | continuous-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1118](../../../assets/index.html)；assets/js/reader/render.js:658,665 |

## convert-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S317 / F058 | convert-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1270](../../../assets/index.html)；assets/app.js:134 |
| S318 / F058 | convert-files | 选择文件... | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "convert.selectFiles"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1276](../../../assets/index.html)；assets/js/features/batch.js:312；assets/app.js:132 |
| S319 / F058 | convert-folder | 选择文件夹... | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "convert.selectFolder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1277](../../../assets/index.html)；assets/js/features/batch.js:312；assets/app.js:133 |
| S320 / F062 | btn-open-plugins | 管理 Docling、EasyOCR、LaTeX 增强与 Whisper 音视频转录等扩展 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "管理扩展插件", "data-i18n-title": "convert.pluginsTooltip", "aria-haspopup": "dialog"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1278](../../../assets/index.html)；assets/app.js:139 |
| S321 / F058 | convert-overwrite | convert-overwrite | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1284](../../../assets/index.html)；assets/js/core/dragdrop.js:94；assets/js/features/batch.js:234,312；assets/js/features/convert.js:54,59,73 |
| S322 / F100 | convert-speech-language | 语音识别语言 系统语言 中文 English 日本語 | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "auto", "text": "系统语言"}, {"value": "zh", "text": "中文"}, {"value": "en", "text": "English"}, {"value": "ja", "text": "日本語"}] | 依文档/平台/面板状态显示 | [assets/index.html:1288](../../../assets/index.html)；assets/js/features/batch.js:312；assets/js/features/convert.js:17 |
| S323 / F059 | batch-cancel | 取消全部 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "batch.cancel"} | — | 初始隐藏 | [assets/index.html:1297](../../../assets/index.html)；assets/js/features/batch.js:48,138,332；assets/js/features/convert.js:31；assets/app.js:145 |
| S324 / F059 | convert-open-dir | 打开结果目录 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "convert.openDir"} | — | 初始隐藏 | [assets/index.html:1298](../../../assets/index.html)；assets/js/features/batch.js:43,280；assets/js/features/convert.js:32；assets/app.js:135 |

## diagram-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S373 / F034 | diagram-modal-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1573](../../../assets/index.html)；assets/app.js:249,720 |
| S374 / F034 | diagram-type | 图表类型： | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "plantuml", "text": "PlantUML (时序图 / 架构图 / UML)"}, {"value": "tikz", "text": "TikZ / PGFPlots (LaTeX 矢量几何与函数图 · 离线)"}, {"value": "wavedrom", "text": "WaveDrom (数字电路波形图 · 离线)"}, {"value": "vega-lite", "text": "Vega-Lite (统计数据可视化与图表 · 离线)"}, {"value": "chart", "text": "Chart.js (图表 · 离线)"}, {"value": "graphviz", "text": "Graphviz / DOT (有向图与网络拓扑 · 离线)"}, {"value": "bitfield", "text": "BitField (硬件寄存器与协议字段图 · 离线)"}] | 依文档/平台/面板状态显示 | [assets/index.html:1578](../../../assets/index.html)；assets/js/editor/editor.js:808,823；assets/js/reader/render.js:2502；assets/app.js:252,724,725,727 |
| S375 / F034 | diagram-code | 图表代码模版： | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {} | — | 依文档/平台/面板状态显示 | [assets/index.html:1590](../../../assets/index.html)；assets/js/editor/editor.js:809,824,825；assets/js/reader/render.js:1010；assets/app.js:253,726,727 |
| S376 / F034 | diagram-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1593](../../../assets/index.html)；assets/app.js:250,721 |
| S377 / F034 | diagram-insert | 插入 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.insert"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1594](../../../assets/index.html)；assets/app.js:251,722 |

## doc-import-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S378 / F035 | doc-import-modal-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1605](../../../assets/index.html)；assets/app.js:261,733 |
| S379 / F035 | doc-import-path | chapter1.md 或 ./subdocs/appendix.md | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "chapter1.md 或 ./subdocs/appendix.md", "data-i18n-placeholder": "subdoc.pathPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1612](../../../assets/index.html)；assets/js/editor/editor.js:841,850,854,900 |
| S380 / F035 | doc-import-browse | 浏览… | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "editor.browseFile"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1613](../../../assets/index.html)；assets/app.js:264 |
| S381 / F035 | doc-import-mode | 引用模式： | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "markdown", "text": "Markdown (递归解析渲染)"}, {"value": "code", "text": "Code (作为高亮代码块嵌入)"}, {"value": "html", "text": "HTML (原样 HTML 包含)"}] | 依文档/平台/面板状态显示 | [assets/index.html:1619](../../../assets/index.html)；assets/js/editor/editor.js:851 |
| S382 / F035 | doc-import-lines | 例如: 10-50 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "例如: 10-50", "data-i18n-placeholder": "subdoc.linesPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1627](../../../assets/index.html)；assets/js/editor/editor.js:852,859 |
| S383 / F035 | doc-import-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1631](../../../assets/index.html)；assets/app.js:262,734 |
| S384 / F035 | doc-import-insert | 插入 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.insert"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1632](../../../assets/index.html)；assets/app.js:263,735 |

## edit-bar

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S044 / F022 | {"class": "md-tool-btn icon", "data-md": "bold", "title": "加粗 (Ctrl+B)", "aria-label": "加粗", "data-i18n-title": "editor.boldTip", "data-i18n-aria": "editor.bold"} | 加粗 | 插入/变换Markdown：bold | {"data-md": "bold", "title": "加粗 (Ctrl+B)", "aria-label": "加粗", "data-i18n-title": "editor.boldTip", "data-i18n-aria": "editor.bold"} | — | 依文档/平台/面板状态显示 | [assets/index.html:149](../../../assets/index.html)； |
| S045 / F022 | {"class": "md-tool-btn icon", "data-md": "italic", "title": "斜体 (Ctrl+I)", "aria-label": "斜体", "data-i18n-title": "editor.italicTip", "data-i18n-aria": "editor.italic"} | 斜体 | 插入/变换Markdown：italic | {"data-md": "italic", "title": "斜体 (Ctrl+I)", "aria-label": "斜体", "data-i18n-title": "editor.italicTip", "data-i18n-aria": "editor.italic"} | — | 依文档/平台/面板状态显示 | [assets/index.html:152](../../../assets/index.html)； |
| S046 / F022 | {"class": "md-tool-btn icon md-wide-only", "data-md": "strike", "title": "删除线 (Ctrl+Shift+X)", "aria-label": "删除线", "data-i18n-title": "editor.strikeTip", "data-i18n-aria": "editor.strikethrough"} | 删除线 | 插入/变换Markdown：strike | {"data-md": "strike", "title": "删除线 (Ctrl+Shift+X)", "aria-label": "删除线", "data-i18n-title": "editor.strikeTip", "data-i18n-aria": "editor.strikethrough"} | — | 依文档/平台/面板状态显示 | [assets/index.html:155](../../../assets/index.html)； |
| S047 / F022 | {"class": "md-tool-btn icon md-wide-only", "data-md": "code", "title": "行内代码 (Ctrl+E)", "aria-label": "行内代码", "data-i18n-title": "editor.codeTip", "data-i18n-aria": "editor.codeInline"} | 行内代码 | 插入/变换Markdown：code | {"data-md": "code", "title": "行内代码 (Ctrl+E)", "aria-label": "行内代码", "data-i18n-title": "editor.codeTip", "data-i18n-aria": "editor.codeInline"} | — | 依文档/平台/面板状态显示 | [assets/index.html:158](../../../assets/index.html)； |
| S048 / F022 | {"class": "md-tool-btn icon", "data-md": "link", "title": "链接 (Ctrl+K)", "aria-label": "链接", "data-i18n-title": "editor.linkTip", "data-i18n-aria": "editor.link"} | 链接 | 插入/变换Markdown：link | {"data-md": "link", "title": "链接 (Ctrl+K)", "aria-label": "链接", "data-i18n-title": "editor.linkTip", "data-i18n-aria": "editor.link"} | — | 依文档/平台/面板状态显示 | [assets/index.html:161](../../../assets/index.html)； |
| S049 / F023 | {"class": "md-tool-btn md-group", "data-menu": "md-text-menu", "aria-haspopup": "true", "aria-expanded": "false"} | 文本 | 展开菜单：md-text-menu | {"data-menu": "md-text-menu", "aria-haspopup": "true", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:166](../../../assets/index.html)； |
| S050 / F022 | {"data-md": "strike", "role": "menuitem"} | 删除线 Ctrl+Shift+X | 插入/变换Markdown：strike | {"data-md": "strike", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:167](../../../assets/index.html)； |
| S051 / F022 | {"data-md": "code", "role": "menuitem"} | 行内代码 Ctrl+E | 插入/变换Markdown：code | {"data-md": "code", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:167](../../../assets/index.html)； |
| S052 / F022 | {"data-md": "math", "role": "menuitem"} | 行内公式 | 插入/变换Markdown：math | {"data-md": "math", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:167](../../../assets/index.html)； |
| S053 / F022 | {"data-md": "link", "role": "menuitem"} | 链接 Ctrl+K | 插入/变换Markdown：link | {"data-md": "link", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:167](../../../assets/index.html)； |
| S054 / F023 | {"class": "md-tool-btn md-group", "data-menu": "md-structure-menu", "aria-haspopup": "true", "aria-expanded": "false"} | 结构 | 展开菜单：md-structure-menu | {"data-menu": "md-structure-menu", "aria-haspopup": "true", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:168](../../../assets/index.html)； |
| S055 / F022 | {"data-md": "h1", "role": "menuitem"} | 一级标题 Ctrl+1 | 插入/变换Markdown：h1 | {"data-md": "h1", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S056 / F022 | {"data-md": "h2", "role": "menuitem"} | 标题 Ctrl+2 | 插入/变换Markdown：h2 | {"data-md": "h2", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S057 / F022 | {"data-md": "h3", "role": "menuitem"} | 三级标题 Ctrl+3 | 插入/变换Markdown：h3 | {"data-md": "h3", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S058 / F022 | {"data-md": "para", "role": "menuitem"} | 正文 Ctrl+0 | 插入/变换Markdown：para | {"data-md": "para", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S059 / F022 | {"data-md": "quote", "role": "menuitem"} | 引用 Ctrl+Shift+. | 插入/变换Markdown：quote | {"data-md": "quote", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S060 / F022 | {"data-md": "callout", "role": "menuitem"} | 提示块 | 插入/变换Markdown：callout | {"data-md": "callout", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S061 / F022 | {"data-md": "list", "role": "menuitem"} | 无序列表 Ctrl+Shift+8 | 插入/变换Markdown：list | {"data-md": "list", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S062 / F022 | {"data-md": "ordered", "role": "menuitem"} | 有序列表 Ctrl+Shift+7 | 插入/变换Markdown：ordered | {"data-md": "ordered", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S063 / F022 | {"data-md": "task", "role": "menuitem"} | 任务列表 Ctrl+Shift+9 | 插入/变换Markdown：task | {"data-md": "task", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S064 / F022 | {"data-md": "codeblock", "role": "menuitem"} | 代码块 | 插入/变换Markdown：codeblock | {"data-md": "codeblock", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S065 / F022 | {"data-md": "mathblock", "role": "menuitem"} | 公式块 | 插入/变换Markdown：mathblock | {"data-md": "mathblock", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S066 / F022 | {"data-md": "hr", "role": "menuitem"} | 分隔线 | 插入/变换Markdown：hr | {"data-md": "hr", "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:169](../../../assets/index.html)； |
| S067 / F023 | {"class": "md-tool-btn md-group", "data-menu": "md-insert-menu", "aria-haspopup": "true", "aria-expanded": "false"} | 插入 | 展开菜单：md-insert-menu | {"data-menu": "md-insert-menu", "aria-haspopup": "true", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:170](../../../assets/index.html)； |
| S068 / F022 | {"data-md": "image", "role": "menuitem", "data-i18n": "editor.image"} | 图片 | 插入/变换Markdown：image | {"data-md": "image", "role": "menuitem", "data-i18n": "editor.image"} | — | 依文档/平台/面板状态显示 | [assets/index.html:172](../../../assets/index.html)； |
| S069 / F031 | btn-insert-table | 表格 | 插入/变换Markdown：table | {"data-md": "table", "role": "menuitem", "data-i18n": "editor.table"} | — | 依文档/平台/面板状态显示 | [assets/index.html:173](../../../assets/index.html)；assets/app.js:700 |
| S070 / F022 | {"data-md": "link", "role": "menuitem", "data-i18n": "editor.link"} | 链接 | 插入/变换Markdown：link | {"data-md": "link", "role": "menuitem", "data-i18n": "editor.link"} | — | 依文档/平台/面板状态显示 | [assets/index.html:174](../../../assets/index.html)； |
| S071 / F033 | btn-insert-code-chunk | 交互代码块 | 插入/变换Markdown：codechunk | {"data-md": "codechunk", "role": "menuitem", "data-i18n": "editor.codeChunk"} | — | 依文档/平台/面板状态显示 | [assets/index.html:176](../../../assets/index.html)；assets/app.js:701 |
| S072 / F034 | btn-insert-diagram | 科学工程图表 | 插入/变换Markdown：diagram | {"data-md": "diagram", "role": "menuitem", "data-i18n": "editor.diagram"} | — | 依文档/平台/面板状态显示 | [assets/index.html:177](../../../assets/index.html)；assets/app.js:702 |
| S073 / F035 | btn-insert-doc-import | 子文档引用 | 插入/变换Markdown：docimport | {"data-md": "docimport", "role": "menuitem", "data-i18n": "editor.docImport"} | — | 依文档/平台/面板状态显示 | [assets/index.html:178](../../../assets/index.html)；assets/app.js:703 |
| S074 / F036 | btn-insert-frontmatter | 样式元数据 | 插入/变换Markdown：frontmatter | {"data-md": "frontmatter", "role": "menuitem", "data-i18n": "editor.frontmatter"} | — | 依文档/平台/面板状态显示 | [assets/index.html:179](../../../assets/index.html)；assets/app.js:704 |
| S075 / F032 | formula-open | 公式选择器 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "公式选择器", "data-i18n-title": "dialog.formulaTitle"} | — | 依文档/平台/面板状态显示 | [assets/index.html:181](../../../assets/index.html)；assets/app.js:347 |
| S076 / F025 | edit-slash-btn | 插入块 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "插入块 (/ 或 Ctrl+/)", "aria-label": "插入块", "data-i18n-title": "slash.buttonTip", "data-i18n-aria": "slash.button"} | — | 依文档/平台/面板状态显示 | [assets/index.html:182](../../../assets/index.html)；assets/js/editor/editor.js:2433 |
| S077 / F023 | edit-view-trigger | 视图 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-haspopup": "true", "aria-expanded": "false", "title": "视图", "aria-label": "视图", "data-i18n-title": "editor.view", "data-i18n-aria": "editor.view"} | — | 依文档/平台/面板状态显示 | [assets/index.html:189](../../../assets/index.html)；assets/js/editor/editor.js:2416 |
| S078 / F023 | edit-view-focus | 专注模式 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitemcheckbox", "aria-checked": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:193](../../../assets/index.html)；assets/js/editor/editor.js:628,2428 |
| S079 / F023 | edit-view-typewriter | 打字机滚动 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitemcheckbox", "aria-checked": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:194](../../../assets/index.html)；assets/js/editor/editor.js:628,2428 |
| S080 / F023 | edit-view-lines | 行号 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitemcheckbox", "aria-checked": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:195](../../../assets/index.html)；assets/js/editor/editor.js:628,2428 |
| S081 / F023 | edit-undo | 撤销 (Ctrl+Z) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "撤销 (Ctrl+Z)", "aria-label": "撤销", "data-i18n-title": "editor.undo", "data-i18n-aria": "editor.undo"} | — | 依文档/平台/面板状态显示 | [assets/index.html:198](../../../assets/index.html)；assets/app.js:361 |
| S082 / F023 | edit-redo | 重做 (Ctrl+Y) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "重做 (Ctrl+Y)", "aria-label": "重做", "data-i18n-title": "editor.redo", "data-i18n-aria": "editor.redo"} | — | 依文档/平台/面板状态显示 | [assets/index.html:201](../../../assets/index.html)；assets/app.js:362 |
| S083 / F027 | pv-trigger | 预览：无 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-haspopup": "true", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:205](../../../assets/index.html)；assets/js/editor/editor.js:732；assets/js/editor/preview.js:83,89,90；assets/app.js:372,378 |
| S084 / F027 | {"class": "pv-btn", "data-pv": "top", "title": "上方预览", "data-i18n-title": "editor.previewTop"} | 上方预览 | 切换预览布局：top | {"data-pv": "top", "title": "上方预览", "data-i18n-title": "editor.previewTop"} | — | 依文档/平台/面板状态显示 | [assets/index.html:209](../../../assets/index.html)； |
| S085 / F027 | {"class": "pv-btn", "data-pv": "none", "title": "关闭预览", "data-i18n-title": "editor.previewNone"} | 关闭预览 | 切换预览布局：none | {"data-pv": "none", "title": "关闭预览", "data-i18n-title": "editor.previewNone"} | — | 依文档/平台/面板状态显示 | [assets/index.html:209](../../../assets/index.html)； |
| S086 / F027 | {"class": "pv-btn", "data-pv": "left", "title": "左侧预览", "data-i18n-title": "editor.previewLeft"} | 左侧预览 | 切换预览布局：left | {"data-pv": "left", "title": "左侧预览", "data-i18n-title": "editor.previewLeft"} | — | 依文档/平台/面板状态显示 | [assets/index.html:210](../../../assets/index.html)； |
| S087 / F027 | {"class": "pv-btn", "data-pv": "right", "title": "右侧预览", "data-i18n-title": "editor.previewRight"} | 右侧预览 | 切换预览布局：right | {"data-pv": "right", "title": "右侧预览", "data-i18n-title": "editor.previewRight"} | — | 依文档/平台/面板状态显示 | [assets/index.html:210](../../../assets/index.html)； |
| S088 / F027 | {"class": "pv-btn", "data-pv": "bottom", "title": "下方预览", "data-i18n-title": "editor.previewBottom"} | 下方预览 | 切换预览布局：bottom | {"data-pv": "bottom", "title": "下方预览", "data-i18n-title": "editor.previewBottom"} | — | 依文档/平台/面板状态显示 | [assets/index.html:211](../../../assets/index.html)； |
| S089 / F027 | pv-sync | pv-sync | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox"} | — | 依文档/平台/面板状态显示 | [assets/index.html:213](../../../assets/index.html)；assets/js/editor/preview.js:435；assets/app.js:381 |
| S090 / F021 | edit-save | 保存 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.save"} | — | 依文档/平台/面板状态显示 | [assets/index.html:217](../../../assets/index.html)；assets/js/editor/preview.js:595,644,680；assets/app.js:353 |
| S091 / F021 | edit-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:219](../../../assets/index.html)；assets/app.js:358 |

## export-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S291 / F070 | export-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1146](../../../assets/index.html)；assets/app.js:531 |
| S292 / F070 | export-tab-pdf | PDF | 点击执行所属流程；禁用/隐藏时不执行 | {"data-fmt": "pdf", "role": "tab", "aria-selected": "true", "aria-controls": "export-opts", "data-i18n": "export.fmtPdf"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1156](../../../assets/index.html)； |
| S293 / F070 | export-tab-docx | DOCX | 点击执行所属流程；禁用/隐藏时不执行 | {"data-fmt": "docx", "role": "tab", "aria-selected": "false", "aria-controls": "export-opts", "data-i18n": "export.fmtDocx"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1157](../../../assets/index.html)； |
| S294 / F070 | export-tab-epub | EPUB | 点击执行所属流程；禁用/隐藏时不执行 | {"data-fmt": "epub", "role": "tab", "aria-selected": "false", "aria-controls": "export-opts", "data-i18n": "export.fmtEpub"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1158](../../../assets/index.html)； |
| S295 / F070 | export-tab-html | HTML | 点击执行所属流程；禁用/隐藏时不执行 | {"data-fmt": "html", "role": "tab", "aria-selected": "false", "aria-controls": "export-opts", "data-i18n": "export.fmtHtml"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1164](../../../assets/index.html)； |
| S296 / F070 | export-tab-tex | LaTeX | 点击执行所属流程；禁用/隐藏时不执行 | {"data-fmt": "tex", "role": "tab", "aria-selected": "false", "aria-controls": "export-opts", "data-i18n": "export.fmtTex"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1165](../../../assets/index.html)； |
| S297 / F070 | export-tab-presentation | 幻灯片 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-fmt": "presentation", "role": "tab", "aria-selected": "false", "aria-controls": "export-opts", "data-i18n": "export.fmtSlides"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1166](../../../assets/index.html)； |
| S298 / F071 | {"data-i18n": "export.presetList"} | 全部预设 | 点击/Enter展开或折叠所在details；不直接调用业务API | {"data-i18n": "export.presetList"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1172](../../../assets/index.html)； |
| S299 / F071 | exp-preset | 样式预设 | 选择值触发change；完整值/显示文案见本行选项 | {} | — | 依文档/平台/面板状态显示 | [assets/index.html:1175](../../../assets/index.html)；assets/js/features/export.js:905,1094,1201,1228,1230,1231,1240,1245,1254,1258,1389,1578；assets/app.js:586 |
| S300 / F071 | exp-save-preset | 存为预设… | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "export.savePreset"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1180](../../../assets/index.html)；assets/app.js:582 |
| S301 / F071 | exp-reset | 恢复默认 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "export.restoreDefaults"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1181](../../../assets/index.html)；assets/app.js:583 |
| S302 / F071 | exp-save-input | 预设名称 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "预设名称", "aria-label": "预设名称", "data-i18n-placeholder": "export.presetName", "data-i18n-aria": "export.presetName", "maxlength": "64", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1184](../../../assets/index.html)；assets/js/features/export.js:1428 |
| S303 / F071 | exp-save-ok | 保存 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.save"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1186](../../../assets/index.html)；assets/js/features/export.js:1431,1432 |
| S304 / F071 | exp-save-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1187](../../../assets/index.html)；assets/js/features/export.js:1470 |
| S305 / F072 | export-preview-card | 点击打开全屏排版预览 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-label": "打开全屏排版预览", "data-i18n-aria": "export.previewCardTitle", "title": "点击打开全屏排版预览", "data-i18n-title": "export.previewCardTitle"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1192](../../../assets/index.html)；assets/app.js:533,534 |
| S306 / F072 | {"data-i18n": "export.customize"} | 定制排版 | 点击/Enter展开或折叠所在details；不直接调用业务API | {"data-i18n": "export.customize"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1209](../../../assets/index.html)； |
| S307 / F075 | export-print | 使用浏览器打印对话框（可另存为 PDF） | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "使用浏览器打印对话框（可另存为 PDF）", "data-i18n-title": "export.printTitle"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1210](../../../assets/index.html)；assets/app.js:580 |
| S308 / F073 | exp-ai-prompt | 例如：学术顶会 LaTeX 论文排版 / 莫兰迪画册风格... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "例如：学术顶会 LaTeX 论文排版 / 莫兰迪画册风格...", "data-i18n-placeholder": "exportai.placeholder", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1221](../../../assets/index.html)；assets/js/features/export.js:1477 |
| S309 / F073 | exp-ai-gen-btn | AI 生成 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "exportai.generateBtn"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1222](../../../assets/index.html)；assets/js/features/export.js:1476,1490 |
| S310 / F074 | export-open | 打开 Markdown | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "app.open"} | — | 初始隐藏 | [assets/index.html:1234](../../../assets/index.html)；assets/js/features/export.js:264,1272,1384,1386 |
| S311 / F074 | export-reveal | 所在文件夹 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "export.openFolder"} | — | 初始隐藏 | [assets/index.html:1235](../../../assets/index.html)；assets/js/features/export.js:265,1273,1385,1387 |
| S312 / F074 | export-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "task.cancel"} | — | 初始隐藏 | [assets/index.html:1236](../../../assets/index.html)；assets/js/features/export.js:1282 |
| S313 / F074 | export-run | 导出 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "export.exportAction"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1237](../../../assets/index.html)；assets/js/features/export.js:1279；assets/app.js:581 |

## export-preview-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S314 / F072 | export-preview-prev-btn | 上一页 (Alt+←) | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "title": "上一页", "aria-label": "上一页", "data-i18n-title": "pagination.prevPage", "data-i18n-aria": "pagination.prevPage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1249](../../../assets/index.html)；assets/js/features/export.js:892,1003 |
| S315 / F072 | export-preview-next-btn | 下一页 (Alt+→) | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "title": "下一页", "aria-label": "下一页", "data-i18n-title": "pagination.nextPage", "data-i18n-aria": "pagination.nextPage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1250](../../../assets/index.html)；assets/js/features/export.js:892,1004 |
| S316 / F072 | export-preview-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭预览", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1252](../../../assets/index.html)；assets/app.js:542,543 |

## fix-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S288 / F082 | fix-ai-btn | AI 深度修复 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "fixes.aiFix"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1132](../../../assets/index.html)；assets/js/reader/fixes.js:34,123；assets/app.js:472 |
| S289 / F081 | fix-save | 另存为 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "app.saveAs"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1133](../../../assets/index.html)；assets/app.js:474 |
| S290 / F081 | fix-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1134](../../../assets/index.html)；assets/app.js:473 |

## formula-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S225 / F032 | formula-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:827](../../../assets/index.html)；assets/app.js:348 |
| S226 / F032 | formula-search | 搜索名称或 TeX… | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "search", "placeholder": "搜索名称或 TeX…", "aria-label": "搜索名称或 TeX", "data-i18n-placeholder": "formula.searchPlaceholder", "data-i18n-aria": "formula.searchPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:827](../../../assets/index.html)；assets/js/editor/editor.js:1088,1101；assets/app.js:349 |
| S227 / F032 | formula-mode | 行内公式 | 选择值触发change；完整值/显示文案见本行选项 | {"aria-label": "行内公式"} | [{"value": "inline", "text": "行内公式"}, {"value": "block", "text": "块级公式"}] | 依文档/平台/面板状态显示 | [assets/index.html:827](../../../assets/index.html)；assets/js/editor/editor.js:1088,1118；assets/app.js:452,947 |

## frontmatter-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S385 / F036 | frontmatter-modal-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1643](../../../assets/index.html)；assets/app.js:739 |
| S386 / F036 | fm-input-title | 我的演说 / 文档标题 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "我的演说 / 报告文档", "data-i18n-placeholder": "presentation.titlePlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1649](../../../assets/index.html)；assets/js/editor/editor.js:1010,1021 |
| S387 / F036 | fm-input-author | 作者 / 演讲者： | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "ReadMD User"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1653](../../../assets/index.html)；assets/js/editor/editor.js:1022 |
| S388 / F036 | fm-select-theme | Reveal.js 演示主题： | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "black", "text": "Black (现代深色 / 默认)"}, {"value": "white", "text": "White (明亮纯白)"}, {"value": "league", "text": "League (几何现代灰)"}, {"value": "beige", "text": "Beige (复古暖米色)"}, {"value": "night", "text": "Night (暗夜高对比)"}, {"value": "serif", "text": "Serif (优雅衬线文学)"}, {"value": "simple", "text": "Simple (极简清爽)"}, {"value": "solarized", "text": "Solarized (程序员经典太阳色)"}, {"value": "blood", "text": "Blood (深红暗黑风格)"}, {"value": "moon", "text": "Moon (蓝调月夜)"}] | 依文档/平台/面板状态显示 | [assets/index.html:1658](../../../assets/index.html)；assets/js/editor/editor.js:1023 |
| S389 / F036 | fm-select-transition | 幻灯片切页动效： | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "slide", "text": "Slide (平滑滑动 / 默认)"}, {"value": "fade", "text": "Fade (淡入淡出)"}, {"value": "convex", "text": "Convex (凸面翻转)"}, {"value": "concave", "text": "Concave (凹面翻转)"}, {"value": "zoom", "text": "Zoom (缩放进出)"}, {"value": "none", "text": "None (无动画)"}] | 依文档/平台/面板状态显示 | [assets/index.html:1673](../../../assets/index.html)；assets/js/editor/editor.js:1024 |
| S390 / F036 | fm-modal-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1684](../../../assets/index.html)；assets/app.js:740 |
| S391 / F036 | fm-modal-insert | 插入 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.insert"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1685](../../../assets/index.html)；assets/app.js:741 |

## global

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S338 / F099 | top-btn | 回到顶部 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "回到顶部", "data-i18n-title": "toolbar.backToTop"} | — | 初始隐藏 | [assets/index.html:1398](../../../assets/index.html)；assets/app.js:595,680 |
| S339 / F001 | file-input | file-input | 由可见导入/选择按钮触发文件选择；change进入相应处理流程 | {"type": "file"} | — | 初始隐藏 | [assets/index.html:1399](../../../assets/index.html)；assets/js/features/convert.js:44,141；assets/js/reader/render.js:3182；assets/app.js:388,389,649 |
| S340 / F028 | img-file-input | img-file-input | 由可见导入/选择按钮触发文件选择；change进入相应处理流程 | {"type": "file", "accept": "image/*"} | — | 初始隐藏 | [assets/index.html:1400](../../../assets/index.html)；assets/app.js:388,389 |

## history-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S195 / F005 | history-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:778](../../../assets/index.html)；assets/js/features/document-history.js:102,104,172；assets/app.js:466,604 |
| S196 / F005 | history-clear | 清空记录 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "app.clearRecent"} | — | 依文档/平台/面板状态显示 | [assets/index.html:782](../../../assets/index.html)；assets/js/core/history.js:191；assets/app.js:465,609 |

## img-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S197 / F030 | img-close-x | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:791](../../../assets/index.html)；assets/app.js:413 |
| S198 / F028 | img-file | 选择本地图片… | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "img.selectLocal"} | — | 依文档/平台/面板状态显示 | [assets/index.html:793](../../../assets/index.html)；assets/app.js:388,389 |
| S199 / F028 | img-url-input | 或粘贴图片 URL 直接插入 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "或粘贴图片 URL 直接插入", "aria-label": "图片 URL", "data-i18n-placeholder": "img.urlPlaceholder", "data-i18n-aria": "img.urlPlaceholder", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:794](../../../assets/index.html)；assets/js/editor/image.js:329；assets/app.js:395 |
| S200 / F028 | img-url-load | 插入 URL | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "img.insertUrl"} | — | 依文档/平台/面板状态显示 | [assets/index.html:795](../../../assets/index.html)；assets/app.js:394 |
| S201 / F029 | {"class": "crop-handle crop-nw", "data-handle": "nw", "aria-label": "左上裁剪手柄", "data-i18n-aria": "img.cropHandleNw"} | 左上裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "nw", "aria-label": "左上裁剪手柄", "data-i18n-aria": "img.cropHandleNw"} | — | 依文档/平台/面板状态显示 | [assets/index.html:802](../../../assets/index.html)； |
| S202 / F029 | {"class": "crop-handle crop-n", "data-handle": "n", "aria-label": "上裁剪手柄", "data-i18n-aria": "img.cropHandleN"} | 上裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "n", "aria-label": "上裁剪手柄", "data-i18n-aria": "img.cropHandleN"} | — | 依文档/平台/面板状态显示 | [assets/index.html:802](../../../assets/index.html)； |
| S203 / F029 | {"class": "crop-handle crop-ne", "data-handle": "ne", "aria-label": "右上裁剪手柄", "data-i18n-aria": "img.cropHandleNe"} | 右上裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "ne", "aria-label": "右上裁剪手柄", "data-i18n-aria": "img.cropHandleNe"} | — | 依文档/平台/面板状态显示 | [assets/index.html:802](../../../assets/index.html)； |
| S204 / F029 | {"class": "crop-handle crop-e", "data-handle": "e", "aria-label": "右裁剪手柄", "data-i18n-aria": "img.cropHandleE"} | 右裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "e", "aria-label": "右裁剪手柄", "data-i18n-aria": "img.cropHandleE"} | — | 依文档/平台/面板状态显示 | [assets/index.html:803](../../../assets/index.html)； |
| S205 / F029 | {"class": "crop-handle crop-se", "data-handle": "se", "aria-label": "右下裁剪手柄", "data-i18n-aria": "img.cropHandleSe"} | 右下裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "se", "aria-label": "右下裁剪手柄", "data-i18n-aria": "img.cropHandleSe"} | — | 依文档/平台/面板状态显示 | [assets/index.html:803](../../../assets/index.html)； |
| S206 / F029 | {"class": "crop-handle crop-s", "data-handle": "s", "aria-label": "下裁剪手柄", "data-i18n-aria": "img.cropHandleS"} | 下裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "s", "aria-label": "下裁剪手柄", "data-i18n-aria": "img.cropHandleS"} | — | 依文档/平台/面板状态显示 | [assets/index.html:803](../../../assets/index.html)； |
| S207 / F029 | {"class": "crop-handle crop-sw", "data-handle": "sw", "aria-label": "左下裁剪手柄", "data-i18n-aria": "img.cropHandleSw"} | 左下裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "sw", "aria-label": "左下裁剪手柄", "data-i18n-aria": "img.cropHandleSw"} | — | 依文档/平台/面板状态显示 | [assets/index.html:804](../../../assets/index.html)； |
| S208 / F029 | {"class": "crop-handle crop-w", "data-handle": "w", "aria-label": "左裁剪手柄", "data-i18n-aria": "img.cropHandleW"} | 左裁剪手柄 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-handle": "w", "aria-label": "左裁剪手柄", "data-i18n-aria": "img.cropHandleW"} | — | 依文档/平台/面板状态显示 | [assets/index.html:804](../../../assets/index.html)； |
| S209 / F029 | img-rot-l | ↶ 90° | 点击执行所属流程；禁用/隐藏时不执行 | {} | — | 依文档/平台/面板状态显示 | [assets/index.html:810](../../../assets/index.html)；assets/app.js:396 |
| S210 / F029 | img-rot-r | ↷ 90° | 点击执行所属流程；禁用/隐藏时不执行 | {} | — | 依文档/平台/面板状态显示 | [assets/index.html:810](../../../assets/index.html)；assets/app.js:397 |
| S211 / F029 | img-flip-x | 水平翻转 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "img.flipH"} | — | 依文档/平台/面板状态显示 | [assets/index.html:810](../../../assets/index.html)；assets/app.js:403 |
| S212 / F029 | img-flip-y | 垂直翻转 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "img.flipV"} | — | 依文档/平台/面板状态显示 | [assets/index.html:810](../../../assets/index.html)；assets/app.js:404 |
| S213 / F029 | img-angle | img-angle | 拖动或方向键调整；input即时反馈，change按所属流程提交 | {"type": "range", "min": "-180", "max": "180", "step": "0.1", "value": "0"} | — | 依文档/平台/面板状态显示 | [assets/index.html:811](../../../assets/index.html)；assets/js/editor/image.js:203；assets/app.js:398,399,400 |
| S214 / F029 | img-angle-number | 角度 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "number", "min": "-180", "max": "180", "step": "0.1", "value": "0", "aria-label": "角度", "data-i18n-aria": "img.angle"} | — | 依文档/平台/面板状态显示 | [assets/index.html:811](../../../assets/index.html)；assets/js/editor/image.js:203；assets/app.js:400 |
| S215 / F029 | img-view-zoom | img-view-zoom | 拖动或方向键调整；input即时反馈，change按所属流程提交 | {"type": "range", "min": "25", "max": "400", "value": "100"} | — | 依文档/平台/面板状态显示 | [assets/index.html:812](../../../assets/index.html)；assets/js/editor/image.js:203；assets/app.js:401,402 |
| S216 / F029 | img-ratio | 裁剪比例 | 选择值触发change；完整值/显示文案见本行选项 | {"title": "裁剪比例", "data-i18n-title": "img.ratio"} | [{"value": "free", "text": "自由比例"}, {"value": "1:1", "text": "1:1 方形"}, {"value": "4:3", "text": "4:3"}, {"value": "3:2", "text": "3:2"}, {"value": "16:9", "text": "16:9"}, {"value": "orig", "text": "原图比例"}] | 依文档/平台/面板状态显示 | [assets/index.html:813](../../../assets/index.html)；assets/js/editor/image.js:203；assets/app.js:405,452,947 |
| S217 / F029 | img-out-w | img-out-w | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "number", "min": "1", "max": "16000"} | — | 依文档/平台/面板状态显示 | [assets/index.html:814](../../../assets/index.html)；assets/js/editor/image.js:161；assets/app.js:407 |
| S218 / F029 | img-size-lock | 锁定宽高比 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-pressed": "true", "aria-label": "锁定宽高比", "title": "锁定宽高比", "data-i18n-title": "img.lockRatio", "data-i18n-aria": "img.lockRatio"} | — | 依文档/平台/面板状态显示 | [assets/index.html:814](../../../assets/index.html)；assets/js/editor/image.js:203；assets/app.js:406 |
| S219 / F029 | img-out-h | img-out-h | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "number", "min": "1", "max": "16000"} | — | 依文档/平台/面板状态显示 | [assets/index.html:814](../../../assets/index.html)；assets/js/editor/image.js:161；assets/app.js:408 |
| S220 / F029 | img-undo | 撤销 (Ctrl+Z) | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.undo", "disabled": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:815](../../../assets/index.html)；assets/js/editor/image.js:22；assets/app.js:409 |
| S221 / F029 | img-redo | 重做 (Ctrl+Y) | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.redo", "disabled": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:815](../../../assets/index.html)；assets/js/editor/image.js:22；assets/app.js:409 |
| S222 / F029 | img-reset | 重置 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "img.reset"} | — | 依文档/平台/面板状态显示 | [assets/index.html:815](../../../assets/index.html)；assets/app.js:410 |
| S223 / F030 | img-insert | 插入到文档 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.imgInsert", "disabled": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:820](../../../assets/index.html)；assets/js/editor/image.js:59,183,188；assets/app.js:411 |
| S224 / F030 | img-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:821](../../../assets/index.html)；assets/app.js:412,413 |

## lang-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S348 / F084 | lang-modal-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1436](../../../assets/index.html)；assets/app.js:935 |
| S349 / F084 | lang-search-input | 搜索语言 / Search language... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "搜索语言 / Search language...", "aria-label": "搜索语言", "aria-controls": "lang-grid", "data-i18n-placeholder": "lang.search", "data-i18n-aria": "lang.search"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1439](../../../assets/index.html)；assets/js/core/i18n.js:358,431；assets/app.js:936 |

## layout

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S103 / F013 | tab-toc | 目录大纲 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "tab", "aria-selected": "true", "aria-controls": "toc-list", "data-i18n": "sidebar.toc"} | — | 依文档/平台/面板状态显示 | [assets/index.html:266](../../../assets/index.html)；assets/js/reader/folder.js:250,276；assets/app.js:677 |
| S104 / F013 | tab-files | 文件列表 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "tab", "aria-selected": "false", "aria-controls": "file-list", "tabindex": "-1", "data-i18n": "sidebar.files"} | — | 依文档/平台/面板状态显示 | [assets/index.html:267](../../../assets/index.html)；assets/js/reader/folder.js:250,276；assets/app.js:678 |
| S105 / F013 | side-close-btn | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "关闭侧栏 (Esc)", "aria-label": "关闭侧栏", "data-i18n-title": "toolbar.close", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:268](../../../assets/index.html)；assets/js/reader/folder.js:299 |

## main-col

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S106 / F001 | w-open | 打开 (Ctrl+O) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "打开本地 Markdown 文件", "data-i18n-title": "toolbar.open"} | — | 依文档/平台/面板状态显示 | [assets/index.html:286](../../../assets/index.html)；assets/js/core/history.js:431 |
| S107 / F002 | w-folder | 打开文件夹 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "打开文件夹，浏览其中所有 Markdown", "data-i18n-title": "toolbar.openFolder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:290](../../../assets/index.html)；assets/js/core/history.js:432 |
| S108 / F003 | w-new | 新建文档 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "新建文档", "data-i18n-title": "menu.new"} | — | 依文档/平台/面板状态显示 | [assets/index.html:294](../../../assets/index.html)；assets/js/core/history.js:433 |
| S109 / F096 | {"type": "button", "class": "welcome-search", "data-action": "palette", "aria-haspopup": "dialog"} | 搜索命令、文件与设置… | 委托操作：palette | {"type": "button", "data-action": "palette", "aria-haspopup": "dialog"} | — | 依文档/平台/面板状态显示 | [assets/index.html:299](../../../assets/index.html)； |
| S110 / F004 | w-convert | 转换 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "万物转 MD", "data-i18n-title": "menu.convert"} | — | 依文档/平台/面板状态显示 | [assets/index.html:307](../../../assets/index.html)；assets/js/core/history.js:435；assets/js/core/modules.js:42 |
| S111 / F004 | w-web | 网页 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "网页转 MD", "data-i18n-title": "menu.web"} | — | 依文档/平台/面板状态显示 | [assets/index.html:311](../../../assets/index.html)；assets/js/core/history.js:436；assets/js/core/modules.js:42 |
| S112 / F004 | w-ocr | OCR | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "扫描转 MD", "data-i18n-title": "menu.ocr"} | — | 依文档/平台/面板状态显示 | [assets/index.html:315](../../../assets/index.html)；assets/js/core/history.js:437；assets/js/core/modules.js:42 |
| S113 / F004 | w-ai | AI 助手 (Ctrl+Shift+A) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "AI 助手", "data-i18n-title": "toolbar.ai"} | — | 依文档/平台/面板状态显示 | [assets/index.html:319](../../../assets/index.html)；assets/js/core/history.js:434；assets/js/core/modules.js:42 |
| S114 / F005 | recent-clear | 清空记录 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "app.clearRecent"} | — | 依文档/平台/面板状态显示 | [assets/index.html:328](../../../assets/index.html)；assets/js/core/history.js:438；assets/app.js:464 |
| S115 / F096 | {"type": "button", "class": "welcome-hint-btn", "data-action": "palette"} | 命令面板 | 委托操作：palette | {"type": "button", "data-action": "palette"} | — | 依文档/平台/面板状态显示 | [assets/index.html:337](../../../assets/index.html)； |
| S116 / F096 | {"type": "button", "class": "welcome-hint-btn", "data-action": "shortcuts"} | ? 键盘快捷键 | 委托操作：shortcuts | {"type": "button", "data-action": "shortcuts"} | — | 依文档/平台/面板状态显示 | [assets/index.html:338](../../../assets/index.html)； |
| S117 / F015 | pg-first-btn | 首页 (Alt+Home) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "首页 (Alt+Home)", "aria-label": "首页", "data-i18n-title": "pagination.firstPage", "data-i18n-aria": "pagination.firstPage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:354](../../../assets/index.html)；assets/js/reader/render.js:576,802 |
| S118 / F015 | pg-prev-btn | 上一页 (Alt+←) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "上一页 (Alt+←)", "aria-label": "上一页", "data-i18n-title": "pagination.prevPage", "data-i18n-aria": "pagination.prevPage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:357](../../../assets/index.html)；assets/js/reader/render.js:579,805 |
| S119 / F015 | pg-page-select | 选择页码 | 选择值触发change；完整值/显示文案见本行选项 | {"aria-label": "选择页码", "data-i18n-aria": "pagination.selectPage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:361](../../../assets/index.html)；assets/js/reader/render.js:588,814 |
| S120 / F015 | pg-next-btn | 下一页 (Alt+→) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "下一页 (Alt+→)", "aria-label": "下一页", "data-i18n-title": "pagination.nextPage", "data-i18n-aria": "pagination.nextPage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:364](../../../assets/index.html)；assets/js/reader/render.js:582,808 |
| S121 / F015 | pg-last-btn | 末页 (Alt+End) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "末页 (Alt+End)", "aria-label": "末页", "data-i18n-title": "pagination.lastPage", "data-i18n-aria": "pagination.lastPage"} | — | 依文档/平台/面板状态显示 | [assets/index.html:367](../../../assets/index.html)；assets/js/reader/render.js:585,811 |
| S122 / F015 | pg-mode-toggle | 切换阅读模式（分页阅读 / 全卷连续） | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "切换阅读模式（分页阅读 / 全卷连续）", "aria-label": "切换阅读模式", "aria-pressed": "true", "data-i18n-title": "pagination.toggleTip", "data-i18n-aria": "pagination.toggleTip"} | — | 依文档/平台/面板状态显示 | [assets/index.html:372](../../../assets/index.html)；assets/js/reader/render.js:618,822 |
| S123 / F021 | edit-area | 编辑 (Ctrl+E) | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"spellcheck": "false", "aria-label": "Markdown 编辑器", "data-i18n-aria": "toolbar.edit"} | — | 初始隐藏 | [assets/index.html:379](../../../assets/index.html)；assets/js/core/tabs.js:512,513；assets/js/editor/editor.js:170,176,1137,1245；assets/js/editor/preview.js:57,330,398,454,467,482,483,484,486,531,541,568；assets/js/features/ai.js:1863,2665,2666；assets/js/features/export.js:234；assets/js/reader/render.js:1911,1948；assets/app.js:354 |
| S124 / F045 | edit-ai-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:391](../../../assets/index.html)；assets/js/editor/editor.js:1636 |
| S125 / F045 | edit-ai-input | 告诉 AI 如何修改，或直接点击下方快捷动作... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "告诉 AI 如何修改，或直接点击下方快捷动作...", "data-i18n-placeholder": "editai.placeholder", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:397](../../../assets/index.html)；assets/js/editor/editor.js:1365,1394,1638 |
| S126 / F045 | edit-ai-submit | 发送 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.send"} | — | 依文档/平台/面板状态显示 | [assets/index.html:398](../../../assets/index.html)；assets/js/editor/editor.js:1377,1423,1637 |
| S127 / F045 | {"class": "edit-ai-act-chip", "data-act": "complete", "data-i18n": "editai.actComplete"} | 智能续写补全 | 文内AI快捷动作：complete | {"data-act": "complete", "data-i18n": "editai.actComplete"} | — | 依文档/平台/面板状态显示 | [assets/index.html:401](../../../assets/index.html)； |
| S128 / F045 | {"class": "edit-ai-act-chip", "data-act": "polish", "data-i18n": "editai.actPolish"} | 润色选中文本 | 文内AI快捷动作：polish | {"data-act": "polish", "data-i18n": "editai.actPolish"} | — | 依文档/平台/面板状态显示 | [assets/index.html:402](../../../assets/index.html)； |
| S129 / F045 | {"class": "edit-ai-act-chip", "data-act": "fix", "data-i18n": "editai.actFix"} | 排版与语法自愈 | 文内AI快捷动作：fix | {"data-act": "fix", "data-i18n": "editai.actFix"} | — | 依文档/平台/面板状态显示 | [assets/index.html:403](../../../assets/index.html)； |
| S130 / F045 | {"class": "edit-ai-act-chip", "data-act": "translate", "data-i18n": "editai.actTranslate"} | 翻译为英文/中文 | 文内AI快捷动作：translate | {"data-act": "translate", "data-i18n": "editai.actTranslate"} | — | 依文档/平台/面板状态显示 | [assets/index.html:404](../../../assets/index.html)； |
| S131 / F045 | edit-ai-apply | 应用替换 (Tab) | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editai.apply"} | — | 依文档/平台/面板状态显示 | [assets/index.html:410](../../../assets/index.html)；assets/js/editor/editor.js:1425,1535,1639 |
| S132 / F045 | edit-ai-insert | 插入光标处 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editai.insert"} | — | 依文档/平台/面板状态显示 | [assets/index.html:411](../../../assets/index.html)；assets/js/editor/editor.js:1425,1535,1640 |
| S133 / F045 | edit-ai-discard | 放弃 (Esc) | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editai.discard"} | — | 依文档/平台/面板状态显示 | [assets/index.html:412](../../../assets/index.html)；assets/js/editor/editor.js:1641 |

## pet-settings-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S147 / F086 | pet-settings-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:498](../../../assets/index.html)；assets/js/features/pet-batch.js:1215 |
| S148 / F089 | pet-runtime | 运行位置 | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "in-app", "text": "阅读器内"}, {"value": "desktop", "text": "独立桌面（需桌宠扩展）"}] | 依文档/平台/面板状态显示 | [assets/index.html:523](../../../assets/index.html)；assets/js/features/pet-batch.js:850,859,1012,1013,1054,1055,1085,1094,1294；assets/js/features/pet-workbench.js:241 |
| S149 / F087 | pet-gallery | 桌宠库 | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "", "text": "伴读使者"}] | 依文档/平台/面板状态显示 | [assets/index.html:530](../../../assets/index.html)；assets/js/features/pet-batch.js:350,498,968,1003,1008,1053,1394,1414,1422,1423,1600,1616,1617；assets/js/features/pet-workbench.js:118,145,268,409 |
| S150 / F088 | pet-gallery-delete | 删除 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "pet.gallery.delete"} | — | 初始隐藏 | [assets/index.html:531](../../../assets/index.html)；assets/js/features/pet-batch.js:1422,1600 |
| S151 / F088 | pet-gallery-import | 导入精灵图 | 由可见导入/选择按钮触发文件选择；change进入相应处理流程 | {"type": "file", "accept": "image/png,image/webp", "hidden": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:533](../../../assets/index.html)；assets/js/features/pet-batch.js:1394 |
| S152 / F089 | pet-enabled | 启用桌宠 | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "data-i18n-aria": "pet.enable"} | — | 依文档/平台/面板状态显示 | [assets/index.html:541](../../../assets/index.html)；assets/js/features/pet-batch.js:695,843,1048,1293；assets/js/features/pet-workbench.js:178 |
| S153 / F087 | pet-renderer | 渲染器 | 选择值触发change；完整值/显示文案见本行选项 | {"data-i18n-aria": "pet.renderer"} | [{"value": "hermes-sprite", "text": "精灵图"}, {"value": "live2d", "text": "Live2D"}] | 依文档/平台/面板状态显示 | [assets/index.html:551](../../../assets/index.html)；assets/js/features/pet-batch.js:844,1051,1122,1450,1638；assets/js/features/pet-workbench.js:131,136,236,270,403,420 |
| S154 / F089 | pet-scale | 大小 | 拖动或方向键调整；input即时反馈，change按所属流程提交 | {"type": "range", "min": "8", "max": "48", "value": "22", "step": "1", "data-i18n-aria": "pet.size"} | — | 依文档/平台/面板状态显示 | [assets/index.html:564](../../../assets/index.html)；assets/js/features/pet-batch.js:845,1018,1036,1049,1453,1455 |
| S155 / F089 | pet-opacity | 不透明度 | 拖动或方向键调整；input即时反馈，change按所属流程提交 | {"type": "range", "min": "35", "max": "100", "value": "100", "step": "1", "data-i18n-aria": "pet.opacity"} | — | 依文档/平台/面板状态显示 | [assets/index.html:575](../../../assets/index.html)；assets/js/features/pet-batch.js:846,1019,1037,1050,1454,1456 |
| S156 / F091 | pet-bubble-toggle | 伴读提示气泡 | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "checked": null, "data-i18n-aria": "pet.bubbleEnable"} | — | 依文档/平台/面板状态显示 | [assets/index.html:586](../../../assets/index.html)；assets/js/features/pet-batch.js:794,854,1065,1292；assets/js/features/pet-workbench.js:118 |
| S157 / F089 | pet-topmost | 保持置顶 | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "checked": null, "data-i18n-aria": "pet.alwaysOnTop"} | — | 依文档/平台/面板状态显示 | [assets/index.html:594](../../../assets/index.html)；assets/js/features/pet-batch.js:854,1063,1292 |
| S158 / F089 | pet-lock-position | 锁定位置 | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "data-i18n-aria": "pet.lockPosition"} | — | 依文档/平台/面板状态显示 | [assets/index.html:598](../../../assets/index.html)；assets/js/features/pet-batch.js:611,854,1064,1292 |
| S159 / F089 | pet-sound | BongoCat 按键音效 | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "data-i18n-aria": "pet.sound"} | — | 依文档/平台/面板状态显示 | [assets/index.html:602](../../../assets/index.html)；assets/js/features/pet-batch.js:854,996,1067,1292 |
| S160 / F089 | pet-reset-pos | 重置桌宠位置 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button"} | — | 依文档/平台/面板状态显示 | [assets/index.html:620](../../../assets/index.html)；assets/js/features/pet-batch.js:1222 |
| S161 / F090 | pet-install-runtime | 一键安装桌面扩展 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "pet.runtime.install"} | — | 依文档/平台/面板状态显示 | [assets/index.html:625](../../../assets/index.html)；assets/js/features/pet-batch.js:863,1296,1378,1379,1579 |
| S162 / F090 | pet-install | 立即安装 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "pet.install"} | — | 依文档/平台/面板状态显示 | [assets/index.html:626](../../../assets/index.html)；assets/js/features/pet-batch.js:862,863,1225,1226,1296,1297,1378,1379,1579 |

## plugin-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S325 / F062 | plugin-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1312](../../../assets/index.html)；assets/app.js:141 |
| S326 / F062 | {"data-i18n": "plugin.runtimeDetails"} | 运行信息 | 点击/Enter展开或折叠所在details；不直接调用业务API | {"data-i18n": "plugin.runtimeDetails"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1317](../../../assets/index.html)； |
| S327 / F062 | {"class": "plugin-tab-pill active", "data-category": "all", "data-i18n": "plugin.filter.all"} | 全部 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-category": "all", "data-i18n": "plugin.filter.all"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1339](../../../assets/index.html)； |
| S328 / F062 | {"class": "plugin-tab-pill", "data-category": "ocr", "data-i18n": "plugin.category.ocr"} | 复杂 OCR | 点击执行所属流程；禁用/隐藏时不执行 | {"data-category": "ocr", "data-i18n": "plugin.category.ocr"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1340](../../../assets/index.html)； |
| S329 / F062 | {"class": "plugin-tab-pill", "data-category": "document", "data-i18n": "plugin.category.document"} | 学术与文档 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-category": "document", "data-i18n": "plugin.category.document"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1341](../../../assets/index.html)； |
| S330 / F062 | {"class": "plugin-tab-pill", "data-category": "audio", "data-i18n": "plugin.category.audio"} | 语音识别 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-category": "audio", "data-i18n": "plugin.category.audio"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1342](../../../assets/index.html)； |
| S331 / F062 | {"class": "plugin-tab-pill", "data-category": "web", "data-i18n": "plugin.category.web"} | 网页提取 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-category": "web", "data-i18n": "plugin.category.web"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1343](../../../assets/index.html)； |
| S332 / F062 | {"class": "plugin-tab-pill", "data-category": "tools", "data-i18n": "plugin.category.tools"} | 工具链桥接 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-category": "tools", "data-i18n": "plugin.category.tools"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1344](../../../assets/index.html)； |
| S397 / F062 | plugin-search | 搜索插件 | 输入后即时筛选 | {"type": "search", "data-i18n-placeholder": "plugin.search", "data-i18n-aria": "plugin.search", "placeholder": "搜索插件与功能", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1333](../../../assets/index.html)；assets/js/features/convert.js:282,419 |
| S398 / F062 | plugin-installed-filter | 仅看已安装 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-pressed": "false", "data-i18n": "plugin.installedOnly"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1334](../../../assets/index.html)；assets/js/features/convert.js:283,285 |
| S399 / F062 | plugin-refresh | 刷新插件状态 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "plugin.refresh"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1335](../../../assets/index.html)；assets/js/features/convert.js:288,569,608 |

## readmd-pet-widget

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S163 / F092 | pet-character-wrap | 伴读桌宠交互区 | 点击执行所属流程；禁用/隐藏时不执行 | {"tabindex": "0", "role": "button", "aria-label": "与桌宠互动", "data-i18n-aria": "pet.characterAria"} | — | 依文档/平台/面板状态显示 | [assets/index.html:638](../../../assets/index.html)；assets/js/features/pet-batch.js:578 |
| S164 / F092 | pet-quick-settings | 桌宠设置 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "桌宠设置", "aria-label": "桌宠设置", "data-i18n-title": "pet.settingsTitle", "data-i18n-aria": "pet.settingsTitle"} | — | 依文档/平台/面板状态显示 | [assets/index.html:646](../../../assets/index.html)；assets/js/features/pet-batch.js:688 |
| S165 / F092 | pet-quick-hide | 收起桌宠 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "收起桌宠", "aria-label": "收起桌宠", "data-i18n-title": "pet.hideTitle", "data-i18n-aria": "pet.hideTitle"} | — | 依文档/平台/面板状态显示 | [assets/index.html:649](../../../assets/index.html)；assets/js/features/pet-batch.js:693 |

## save-conflict-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S281 / F097 | save-conflict-save-as | 另存为 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "toolbar.saveAs"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1080](../../../assets/index.html)；assets/js/editor/preview.js:655,669 |
| S282 / F097 | save-conflict-reload | 重新加载 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "toolbar.reload"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1081](../../../assets/index.html)；assets/js/editor/preview.js:647,656,670 |
| S283 / F097 | save-conflict-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1082](../../../assets/index.html)；assets/js/editor/preview.js:650,657 |

## search-bar

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S040 / F014 | search-input | 在文档中搜索… (Enter 下一个, Shift+Enter 上一个) | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "在文档中搜索...", "aria-label": "在文档中搜索", "data-i18n-placeholder": "search.placeholder", "data-i18n-aria": "search.placeholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:138](../../../assets/index.html)；assets/js/core/i18n.js:358,431；assets/js/reader/search.js:258,259；assets/app.js:492,502,936 |
| S041 / F014 | search-prev | 上一个 (Shift+Enter) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "上一个 (Shift+Enter)", "data-i18n-title": "search.prev", "data-i18n-aria": "search.prev"} | — | 依文档/平台/面板状态显示 | [assets/index.html:140](../../../assets/index.html)；assets/app.js:488 |
| S042 / F014 | search-next | 下一个 (Enter) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "下一个 (Enter)", "data-i18n-title": "search.next", "data-i18n-aria": "search.next"} | — | 依文档/平台/面板状态显示 | [assets/index.html:141](../../../assets/index.html)；assets/app.js:487 |
| S043 / F014 | search-close | 关闭搜索 (Esc) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "关闭 (Esc)", "data-i18n-title": "search.close", "data-i18n-aria": "search.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:142](../../../assets/index.html)；assets/app.js:486 |

## share-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S263 / F077 | share-start | 开启共享 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "share.startServer"} | — | 依文档/平台/面板状态显示 | [assets/index.html:952](../../../assets/index.html)；assets/js/features/share.js:14,21,65,71,106,110；assets/app.js:670 |
| S264 / F077 | share-stop | 关闭共享 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "share.stopServer", "disabled": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:953](../../../assets/index.html)；assets/js/features/share.js:14,22,66,72,106,110；assets/app.js:671 |
| S265 / F077 | share-refresh | 刷新状态 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "audit.refreshStatus"} | — | 依文档/平台/面板状态显示 | [assets/index.html:954](../../../assets/index.html)；assets/js/features/share.js:14,85,106,110；assets/app.js:672 |
| S266 / F077 | share-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:955](../../../assets/index.html)；assets/app.js:673 |

## skill-create-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S256 / F054 | skill-create-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:919](../../../assets/index.html)；assets/js/features/ai.js:576 |
| S257 / F054 | skill-create-example | 填入精读示例 ↗ | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "ux.skillExample"} | — | 依文档/平台/面板状态显示 | [assets/index.html:924](../../../assets/index.html)；assets/js/features/ai.js:579 |
| S258 / F054 | skill-create-name | 例如：论文深度精读 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "maxlength": "80", "required": null, "placeholder": "例如：论文深度精读", "data-i18n-placeholder": "tpl.createNamePlaceholder", "autocomplete": "off", "aria-describedby": "skill-create-error"} | — | 依文档/平台/面板状态显示 | [assets/index.html:927](../../../assets/index.html)；assets/js/features/ai.js:572 |
| S259 / F054 | skill-create-purpose | 描述这个 Skill 要完成什么、输出格式与注意事项… | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"rows": "6", "maxlength": "4000", "required": null, "placeholder": "描述这个 Skill 要完成什么、输出格式与注意事项…", "data-i18n-placeholder": "tpl.createPurposePlaceholder", "aria-describedby": "skill-create-error"} | — | 依文档/平台/面板状态显示 | [assets/index.html:931](../../../assets/index.html)；assets/js/features/ai.js:573 |
| S260 / F054 | skill-create-format | 输出形式 | 选择值触发change；完整值/显示文案见本行选项 | {} | [{"value": "Markdown", "text": "Markdown"}, {"value": "table", "text": "结构化表格"}, {"value": "checklist", "text": "行动清单"}] | 依文档/平台/面板状态显示 | [assets/index.html:933](../../../assets/index.html)；assets/js/features/ai.js:578 |
| S261 / F054 | skill-create-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:937](../../../assets/index.html)；assets/js/features/ai.js:575 |
| S262 / F054 | skill-create-go | 生成草稿 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "tpl.createGo"} | — | 依文档/平台/面板状态显示 | [assets/index.html:938](../../../assets/index.html)；assets/js/features/ai.js:574 |

## statusbar

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S351 / F015 | status-pagination | 切换分页阅读模式 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "title": "点击切换阅读模式", "data-i18n-title": "pagination.switchModeTitle"} | — | 初始隐藏 | [assets/index.html:1465](../../../assets/index.html)；assets/js/reader/render.js:554,621,825 |
| S352 / F085 | status-update-badge | 点击查看新版本并更新 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "点击查看新版本并更新", "data-i18n-title": "status.clickToUpdate"} | — | 初始隐藏 | [assets/index.html:1470](../../../assets/index.html)；assets/js/features/updater.js:69,70,81；assets/app.js:689 |
| S353 / F006 | btn-home | 回到主页 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "返回主页", "data-i18n-title": "toolbar.home"} | — | 初始隐藏 | [assets/index.html:1475](../../../assets/index.html)；assets/js/core/history.js:350；assets/js/core/tabs.js:64；assets/app.js:86 |

## style-custom-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S354 / F083 | style-modal-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1487](../../../assets/index.html)；assets/app.js:745 |
| S355 / F083 | style-load-retry | 重新加载 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "audit.retryLoad"} | — | 初始隐藏 | [assets/index.html:1493](../../../assets/index.html)；assets/app.js:748,1313 |
| S356 / F083 | btn-preset-indent | 首行缩进 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "dialog.stylePresetIndent"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1498](../../../assets/index.html)；assets/app.js:898 |
| S357 / F083 | btn-preset-table | 精美表格 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "dialog.stylePresetTable"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1499](../../../assets/index.html)；assets/app.js:899 |
| S358 / F083 | btn-preset-font | 等宽代码字体 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "dialog.stylePresetFont"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1500](../../../assets/index.html)；assets/app.js:900 |
| S359 / F083 | btn-preset-print | 打印分页优化 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "dialog.stylePresetPrint"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1501](../../../assets/index.html)；assets/app.js:901 |
| S360 / F083 | style-ai-prompt | 输入排版风格诉求，例如：适合学术论文的衬线体与优雅边距... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "输入排版风格诉求，例如：适合学术论文的衬线体与优雅边距...", "data-i18n-placeholder": "styleai.placeholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1505](../../../assets/index.html)；assets/app.js:759,870,871 |
| S361 / F083 | style-ai-gen-btn | AI 生成样式 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "data-i18n": "styleai.generate"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1506](../../../assets/index.html)；assets/app.js:767,869,1318 |
| S362 / F083 | style-custom-css | /* 例如：\n.markdown-body { font-size: 16px; }\n*/ | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"placeholder": "/* 例如：\\n.markdown-body p { text-indent: 2em; }\\n*/", "data-i18n-placeholder": "dialog.styleCustomCssPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1512](../../../assets/index.html)；assets/app.js:837,838,887,904,1340,1361,1385 |
| S363 / F083 | style-custom-head | <!-- 例如：\n<link rel="stylesheet" href="https://fonts.googleapis.com/...">\n--> | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"style": "min-height:90px;", "placeholder": "<!-- 例如：\\n<link rel='stylesheet' href='https://fonts.googleapis.com/...'>\\n-->", "data-i18n-placeholder": "dialog.styleCustomHeadPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1516](../../../assets/index.html)；assets/app.js:840,841,904,1341,1362,1385 |
| S364 / F083 | style-modal-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.cancel"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1520](../../../assets/index.html)；assets/app.js:746 |
| S365 / F083 | style-modal-save | 保存并即时生效 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "dialog.styleCustomSave"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1521](../../../assets/index.html)；assets/app.js:747,1318,1355 |

## tab-context-menu

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S341 / F010 | {"role": "menuitem", "tabindex": "-1", "data-action": "move-left", "data-i18n": "tabs.moveLeft"} | 左移 | 委托操作：move-left | {"role": "menuitem", "tabindex": "-1", "data-action": "move-left", "data-i18n": "tabs.moveLeft"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1404](../../../assets/index.html)； |
| S342 / F010 | {"role": "menuitem", "tabindex": "-1", "data-action": "move-right", "data-i18n": "tabs.moveRight"} | 右移 | 委托操作：move-right | {"role": "menuitem", "tabindex": "-1", "data-action": "move-right", "data-i18n": "tabs.moveRight"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1405](../../../assets/index.html)； |
| S343 / F010 | {"role": "menuitem", "tabindex": "-1", "data-action": "close", "data-i18n": "tabs.closeTab"} | 关闭标签 | 委托操作：close | {"role": "menuitem", "tabindex": "-1", "data-action": "close", "data-i18n": "tabs.closeTab"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1407](../../../assets/index.html)； |
| S344 / F010 | {"role": "menuitem", "tabindex": "-1", "data-action": "close-others", "data-i18n": "tabs.closeOthers"} | 关闭其他标签 | 委托操作：close-others | {"role": "menuitem", "tabindex": "-1", "data-action": "close-others", "data-i18n": "tabs.closeOthers"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1408](../../../assets/index.html)； |
| S345 / F010 | {"role": "menuitem", "tabindex": "-1", "data-action": "close-all", "data-i18n": "tabs.closeAll"} | 关闭所有标签 | 委托操作：close-all | {"role": "menuitem", "tabindex": "-1", "data-action": "close-all", "data-i18n": "tabs.closeAll"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1409](../../../assets/index.html)； |
| S346 / F010 | {"role": "menuitem", "tabindex": "-1", "data-action": "rename", "data-i18n": "tabs.rename"} | 重命名 (F2 / 双击) | 委托操作：rename | {"role": "menuitem", "tabindex": "-1", "data-action": "rename", "data-i18n": "tabs.rename"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1411](../../../assets/index.html)； |
| S347 / F010 | {"role": "menuitem", "tabindex": "-1", "data-action": "copy-path", "data-i18n": "tabs.copyPath"} | 复制文件路径 | 委托操作：copy-path | {"role": "menuitem", "tabindex": "-1", "data-action": "copy-path", "data-i18n": "tabs.copyPath"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1412](../../../assets/index.html)； |

## table-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S350 / F031 | table-modal-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1450](../../../assets/index.html)；assets/app.js:937 |

## toolbar

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S001 / F013 | btn-toc | 目录大纲 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "目录 (Ctrl+Shift+F)", "aria-label": "目录", "data-i18n-title": "sidebar.toc", "data-i18n-aria": "sidebar.toc"} | — | 依文档/平台/面板状态显示 | [assets/index.html:35](../../../assets/index.html)；assets/app.js:470 |
| S005 / F008 | btn-reload | 刷新列表 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "重新加载 (Ctrl+R)", "aria-label": "重新加载", "data-i18n-title": "sidebar.refresh", "data-i18n-aria": "sidebar.refresh"} | — | 依文档/平台/面板状态显示 | [assets/index.html:47](../../../assets/index.html)；assets/js/core/history.js:307；assets/js/reader/render.js:3081；assets/app.js:463 |
| S006 / F011 | file-title | 当前文档 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "title": "当前文件名", "aria-label": "当前文件名", "data-i18n-title": "reader.currentDoc", "data-i18n-aria": "reader.currentDoc", "disabled": null} | — | 初始隐藏 | [assets/index.html:50](../../../assets/index.html)；assets/js/reader/render.js:10,30,49；assets/app.js:461 |
| S007 / F009 | doc-tabs-overflow-btn | 查看被折叠的标签页 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "查看被折叠的标签页", "data-i18n-title": "tabs.overflowTooltip", "aria-haspopup": "menu", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:54](../../../assets/index.html)；assets/js/core/dragdrop.js:235 |
| S009 / F078 | btn-graph | 知识图谱 | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "title": "知识图谱 (Ctrl+G)", "aria-label": "知识图谱", "data-i18n-title": "graph.title", "data-i18n-aria": "graph.title"} | — | 初始隐藏 | [assets/index.html:65](../../../assets/index.html)；assets/js/core/history.js:337,338,339,345,346,347；assets/js/features/graph.js:756；assets/app.js:169,170 |
| S010 / F014 | btn-search | 搜索 (Ctrl+F) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "搜索 (Ctrl+F)", "aria-label": "搜索", "data-i18n-title": "toolbar.search", "data-i18n-aria": "toolbar.search"} | — | 依文档/平台/面板状态显示 | [assets/index.html:68](../../../assets/index.html)；assets/js/core/history.js:321,322；assets/js/reader/search.js:268；assets/app.js:485 |
| S012 / F016 | btn-zen | 禅模式 (F11) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "禅模式 (F11 / Esc)", "aria-label": "禅模式", "data-i18n-title": "toolbar.zen", "data-i18n-aria": "toolbar.zen"} | — | 依文档/平台/面板状态显示 | [assets/index.html:72](../../../assets/index.html)；assets/app.js:154,155,699 |
| S013 / F016 | btn-a | 缩小 (Ctrl+-) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "减小字号 (Ctrl+-)", "aria-label": "减小字号", "data-i18n-title": "toolbar.zoomOut", "data-i18n-aria": "toolbar.zoomOut"} | — | 依文档/平台/面板状态显示 | [assets/index.html:75](../../../assets/index.html)；assets/js/core/history.js:319；assets/js/core/modules.js:34；assets/js/shell/shell-deferred.js:213,214；assets/js/shell/shell.js:128,132；assets/app.js:523,594,598,686,1099 |
| S014 / F016 | btn-A | 放大 (Ctrl++) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "增大字号 (Ctrl+=)", "aria-label": "增大字号", "data-i18n-title": "toolbar.zoomIn", "data-i18n-aria": "toolbar.zoomIn"} | — | 依文档/平台/面板状态显示 | [assets/index.html:76](../../../assets/index.html)；assets/js/core/history.js:320；assets/app.js:524 |
| S015 / F070 | btn-print | 导出 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "导出文档 (Ctrl+P)", "aria-label": "导出", "data-i18n-title": "toolbar.export", "data-i18n-aria": "toolbar.export"} | — | 依文档/平台/面板状态显示 | [assets/index.html:77](../../../assets/index.html)；assets/js/core/history.js:311,313,315,316,317；assets/app.js:530 |
| S016 / F021 | btn-edit | 编辑 (Ctrl+E) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "编辑当前 Markdown (Ctrl+E)", "aria-label": "编辑", "data-i18n-title": "toolbar.edit", "data-i18n-aria": "toolbar.edit", "disabled": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:80](../../../assets/index.html)；assets/js/core/history.js:305,306；assets/js/editor/editor.js:1634；assets/js/reader/render.js:275；assets/js/shell/shell-deferred.js:197；assets/app.js:339,1029 |
| S017 / F041 | btn-ai | AI 助手 (Ctrl+Shift+A) | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "AI 助手 (Ctrl+Shift+A)", "aria-label": "AI 助手", "data-i18n-title": "toolbar.ai", "data-i18n-aria": "toolbar.ai"} | — | 依文档/平台/面板状态显示 | [assets/index.html:84](../../../assets/index.html)；assets/js/core/modules.js:34；assets/js/shell/shell-deferred.js:213,214；assets/app.js:598,1099 |
| S019 / F001 | {"class": "more-group-header", "type": "button", "aria-expanded": "false"} | 导入 › | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:93](../../../assets/index.html)； |
| S020 / F058 | btn-convert | 万物转换 格式转 MD | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:95](../../../assets/index.html)；assets/js/core/modules.js:34；assets/app.js:131 |
| S021 / F067 | btn-web | 网页提取 网页转 MD | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:96](../../../assets/index.html)；assets/js/core/modules.js:34；assets/app.js:149 |
| S022 / F066 | btn-ocr | 扫描识别 OCR 识别 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:97](../../../assets/index.html)；assets/js/core/modules.js:34；assets/app.js:148 |
| S023 / F069 | btn-clipboard-new | 剪贴新建 剪贴板转 MD | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:98](../../../assets/index.html)；assets/app.js:150 |
| S024 / F001 | {"class": "more-group-header", "type": "button", "aria-expanded": "false"} | 互动 › | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:102](../../../assets/index.html)； |
| S025 / F080 | btn-backlinks-menu | 反向链接 双向关联 | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:104](../../../assets/index.html)；assets/js/core/history.js:341,342,343；assets/js/features/graph.js:704；assets/js/shell/shell-deferred.js:187；assets/app.js:171 |
| S026 / F076 | btn-presentation-menu | 演讲演示 Reveal.js (F5) | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:105](../../../assets/index.html)；assets/js/core/history.js:323；assets/app.js:151 |
| S027 / F038 | btn-run-all-chunks | 运行代码 执行代码块 | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "role": "menuitem"} | — | 初始隐藏 | [assets/index.html:106](../../../assets/index.html)；assets/js/core/history.js:324,327,329,331；assets/js/reader/render.js:1768；assets/app.js:152 |
| S028 / F083 | btn-style-custom | 样式定制 CSS / Head 注入 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:107](../../../assets/index.html)；assets/app.js:153 |
| S029 / F012 | btn-saveas | 另存为 .md 文件 | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:108](../../../assets/index.html)；assets/js/core/history.js:308,310；assets/js/editor/preview.js:595,680；assets/js/shell/shell-deferred.js:158；assets/app.js:460,1031 |
| S030 / F102 | btn-document-copy | 创建编辑副本 先编辑，保存时再选位置 | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:109](../../../assets/index.html)；assets/js/core/history.js:309；assets/js/features/document-history.js:179 |
| S031 / F103 | btn-document-history | 恢复与版本历史 草稿与保存前版本 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:110](../../../assets/index.html)；assets/js/features/document-history.js:178,184 |
| S032 / F081 | btn-fix | 修复详情 自动修正报告 | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:111](../../../assets/index.html)；assets/js/core/history.js:335,336；assets/js/reader/render.js:287；assets/js/shell/shell-deferred.js:188；assets/app.js:471 |
| S033 / F077 | btn-share | 扫码共享 局域网移动端 | 点击执行所属流程；禁用/隐藏时不执行 | {"disabled": null, "role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:112](../../../assets/index.html)；assets/js/core/history.js:334；assets/js/shell/shell-deferred.js:176；assets/app.js:669 |
| S034 / F001 | {"class": "more-group-header", "type": "button", "aria-expanded": "false"} | 设置 › | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-expanded": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:116](../../../assets/index.html)； |
| S035 / F084 | btn-lang | 界面语言 简体中文 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:118](../../../assets/index.html)；assets/app.js:684 |
| S036 / F086 | btn-pet | 桌宠 未开启 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:119](../../../assets/index.html)；assets/app.js:685 |
| S037 / F084 | btn-assoc | 设为默认 .md 打开方式 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:122](../../../assets/index.html)；assets/app.js:594 |
| S038 / F084 | btn-autostart | 开机自启 未开启 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:123](../../../assets/index.html)；assets/app.js:686 |
| S039 / F085 | btn-check-update | 检查更新 当前版本 v2.3.9 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 依文档/平台/面板状态显示 | [assets/index.html:126](../../../assets/index.html)；assets/js/features/updater.js:27；assets/app.js:687 |
| S395 / F104 | btn-close-to-tray | 关闭时驻留托盘 | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitemcheckbox", "aria-checked": "true"} | — | 初始隐藏 | [assets/index.html:124](../../../assets/index.html)；assets/js/shell/shell.js:96,127,133,161 |
| S396 / F104 | btn-app-exit | 完全退出 ReadMD | 点击执行所属流程；禁用/隐藏时不执行 | {"role": "menuitem"} | — | 初始隐藏 | [assets/index.html:125](../../../assets/index.html)；assets/js/shell/shell.js:128,132 |

## tpl-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S228 / F052 | tpl-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭模板管理", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:834](../../../assets/index.html)；assets/app.js:641,665 |
| S229 / F052 | tpl-search | 搜索模板名称或内容... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "search", "placeholder": "搜索模板...", "aria-label": "搜索模板", "data-i18n-placeholder": "tpl.searchPlaceholder", "data-i18n-aria": "tpl.searchPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:839](../../../assets/index.html)；assets/js/features/ai.js:343；assets/app.js:647 |
| S230 / F056 | tpl-import-btn | 支持导入 Markdown (.md) 或 JSON (.json) 格式的 Prompt 模板 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "支持导入 .md / .json 格式的提示词模板文件", "data-i18n-title": "tpl.importHint", "aria-expanded": "false", "aria-controls": "tpl-import-menu"} | — | 依文档/平台/面板状态显示 | [assets/index.html:840](../../../assets/index.html)；assets/js/features/ai.js:903；assets/app.js:648 |
| S231 / F056 | tpl-file-input | tpl-file-input | 由可见导入/选择按钮触发文件选择；change进入相应处理流程 | {"type": "file", "accept": ".md,.markdown,.json,.txt", "multiple": null, "hidden": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:844](../../../assets/index.html)；assets/app.js:649 |
| S232 / F057 | tpl-folder-input | tpl-folder-input | 由可见导入/选择按钮触发文件选择；change进入相应处理流程 | {"type": "file", "webkitdirectory": null, "directory": null, "multiple": null, "hidden": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:845](../../../assets/index.html)；assets/js/features/ai.js:936；assets/app.js:653 |
| S233 / F057 | tpl-zip-input | tpl-zip-input | 由可见导入/选择按钮触发文件选择；change进入相应处理流程 | {"type": "file", "accept": ".zip,application/zip", "hidden": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:846](../../../assets/index.html)；assets/js/features/ai.js:937；assets/app.js:658 |
| S234 / F057 | tpl-import-source-github | GitHub | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "role": "tab", "aria-selected": "true"} | — | 依文档/平台/面板状态显示 | [assets/index.html:849](../../../assets/index.html)；assets/app.js:650 |
| S235 / F057 | tpl-import-source-folder | 打开文件夹 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "role": "tab", "aria-selected": "false", "data-i18n": "app.folder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:850](../../../assets/index.html)；assets/app.js:651 |
| S236 / F057 | tpl-import-source-zip | ZIP | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "role": "tab", "aria-selected": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:851](../../../assets/index.html)；assets/app.js:652 |
| S237 / F057 | tpl-github-url | 输入网页 URL (如 https://example.com/post)... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "url", "inputmode": "url", "placeholder": "https://github.com/owner/repository", "aria-label": "GitHub", "data-i18n-aria": "web.urlPlaceholder", "autocomplete": "off"} | — | 依文档/平台/面板状态显示 | [assets/index.html:857](../../../assets/index.html)；assets/js/features/ai.js:925,1024；assets/app.js:663 |
| S238 / F057 | tpl-github-credential | API密钥 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "password", "placeholder": "API key", "data-i18n-placeholder": "ai.apiKey", "aria-label": "API key", "data-i18n-aria": "ai.apiKey", "autocomplete": "off", "spellcheck": "false"} | — | 依文档/平台/面板状态显示 | [assets/index.html:861](../../../assets/index.html)；assets/js/features/ai.js:941,942,1025,1031 |
| S239 / F057 | tpl-github-preview-btn | 导入模板 (.md/.json) | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-label": "导入模板", "data-i18n-aria": "tpl.importMd"} | — | 依文档/平台/面板状态显示 | [assets/index.html:863](../../../assets/index.html)；assets/js/features/ai.js:1027；assets/app.js:662 |
| S240 / F056 | tpl-export-btn | 导出全部模板为 JSON 备份文件 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "将全部模板导出为 .json 备份", "data-i18n-title": "tpl.exportHint"} | — | 依文档/平台/面板状态显示 | [assets/index.html:870](../../../assets/index.html)；assets/app.js:664 |
| S241 / F054 | tpl-ai-generate | AI 生成 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "exportai.generateBtn", "data-i18n-title": "tpl.title"} | — | 依文档/平台/面板状态显示 | [assets/index.html:874](../../../assets/index.html)；assets/js/features/ai.js:669；assets/app.js:636 |
| S242 / F053 | tpl-id | tpl-id | 隐藏表单元数据；非直接可点击入口 | {"type": "hidden"} | — | 初始隐藏 | [assets/index.html:879](../../../assets/index.html)；assets/js/features/ai.js:489,503,516,541,551,685,701,734,753,1091,1115,2721 |
| S243 / F053 | tpl-action | tpl-action | 隐藏表单元数据；非直接可点击入口 | {"type": "hidden", "value": "custom"} | — | 初始隐藏 | [assets/index.html:880](../../../assets/index.html)；assets/js/features/ai.js:518,1093；assets/app.js:452,947 |
| S244 / F053 | tpl-name | 模板名称（如：总结要点 / 生成周报） | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "text", "placeholder": "模板名称（如：深度论文精读 / 结构化周报）", "aria-label": "模板名称", "data-i18n-placeholder": "tpl.namePlaceholder", "data-i18n-aria": "tpl.namePlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:883](../../../assets/index.html)；assets/js/features/ai.js:499,517,522,552,555,559,686,705,1092 |
| S245 / F053 | tpl-system | 系统提示词：角色设定与任务说明 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"rows": "8", "placeholder": "SKILL.md：标准 frontmatter 与指令（使用 {{document}} 等受限变量）", "aria-label": "SKILL.md 指令", "data-i18n-placeholder": "tpl.systemPlaceholder", "data-i18n-aria": "tpl.systemPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:884](../../../assets/index.html)；assets/js/features/ai.js:519,523,553,555,687,702,1094 |
| S246 / F053 | tpl-user | 用户消息模板（可选）：可用 {doc} 代表文档内容、{prompt} 代表补充要求；留空则自动拼接文档 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"rows": "3", "placeholder": "变量/补充请求模板（可选）；发布前会执行结构与安全校验", "aria-label": "Skill 变量与补充请求", "data-i18n-placeholder": "tpl.userPlaceholder", "data-i18n-aria": "tpl.userPlaceholder"} | — | 依文档/平台/面板状态显示 | [assets/index.html:885](../../../assets/index.html)；assets/js/features/ai.js:520,521,524,554,555,655,688,1095 |
| S247 / F053 | tpl-new | 新对话 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "ai.newSession"} | — | 依文档/平台/面板状态显示 | [assets/index.html:893](../../../assets/index.html)；assets/app.js:632 |
| S248 / F053 | tpl-edit | 编辑 (Ctrl+E) | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "toolbar.edit"} | — | 依文档/平台/面板状态显示 | [assets/index.html:894](../../../assets/index.html)；assets/js/features/ai.js:492,493,494,495；assets/app.js:633 |
| S249 / F053 | tpl-copy | 复制 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.copy"} | — | 依文档/平台/面板状态显示 | [assets/index.html:895](../../../assets/index.html)；assets/app.js:634 |
| S250 / F053 | tpl-del | 删除 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "tpl.delete"} | — | 依文档/平台/面板状态显示 | [assets/index.html:896](../../../assets/index.html)；assets/js/features/ai.js:525；assets/app.js:640 |
| S251 / F053 | tpl-save | 保存 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "editor.save"} | — | 依文档/平台/面板状态显示 | [assets/index.html:900](../../../assets/index.html)；assets/js/features/ai.js:497,556；assets/app.js:635 |
| S252 / F055 | tpl-publish | 排版出版 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "export.groupPublish"} | — | 依文档/平台/面板状态显示 | [assets/index.html:901](../../../assets/index.html)；assets/js/features/ai.js:498,557,689；assets/app.js:637 |
| S253 / F055 | tpl-toggle | 停用 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "tpl.disable"} | — | 依文档/平台/面板状态显示 | [assets/index.html:902](../../../assets/index.html)；assets/js/features/ai.js:526；assets/app.js:638 |
| S254 / F055 | tpl-export-one | 导出此 Skill | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "tpl.exportOne"} | — | 依文档/平台/面板状态显示 | [assets/index.html:906](../../../assets/index.html)；assets/js/features/ai.js:532；assets/app.js:639 |
| S255 / F052 | tpl-close-btn | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:908](../../../assets/index.html)；assets/app.js:665 |

## update-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S333 / F085 | update-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭更新窗口", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1361](../../../assets/index.html)；assets/app.js:690 |
| S334 / F085 | update-use-mirror | update-use-mirror | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1382](../../../assets/index.html)；assets/js/features/updater.js:157 |
| S335 / F085 | btn-update-browser | 在浏览器中打开 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "update.viewOnBrowser"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1386](../../../assets/index.html)；assets/app.js:693 |
| S336 / F085 | btn-update-cancel | 稍后提醒 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "update.cancel"} | — | 初始隐藏 | [assets/index.html:1388](../../../assets/index.html)；assets/js/features/updater.js:164,198,250,258,273,292,300；assets/app.js:692 |
| S337 / F085 | btn-update-start | 立即下载并更新 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "update.installNow"} | — | 依文档/平台/面板状态显示 | [assets/index.html:1389](../../../assets/index.html)；assets/js/features/updater.js:120,121,127,128,163,199,241,248,249,256,257,274,299,305,306,321,322；assets/app.js:691 |

## url-modal

| 清单/流程 | 控件 | 名称 | 操作 | 属性/取值 | 全部选项 | 显示条件 | HTML与前端引用 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| S267 / F067 | url-close | 关闭 | 点击执行所属流程；禁用/隐藏时不执行 | {"aria-label": "关闭网页转换", "data-i18n-aria": "toolbar.close"} | — | 依文档/平台/面板状态显示 | [assets/index.html:970](../../../assets/index.html)；assets/app.js:333 |
| S268 / F067 | url-input | 输入网页 URL (如 https://example.com/post)... | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "url", "inputmode": "url", "placeholder": "输入网页 URL (如 https://example.com/post)...", "aria-label": "网页 URL", "data-i18n-placeholder": "web.urlPlaceholder", "data-i18n-aria": "web.urlPlaceholder", "autocomplete": "url"} | — | 依文档/平台/面板状态显示 | [assets/index.html:974](../../../assets/index.html)；assets/js/core/dragdrop.js:209；assets/js/editor/image.js:329；assets/js/features/clipboard.js:94；assets/js/features/web.js:45,154,254；assets/app.js:304,311,323,328,335,395 |
| S269 / F067 | url-paste-btn | 粘贴网址 | 点击执行所属流程；禁用/隐藏时不执行 | {"title": "从剪贴板粘贴", "aria-label": "粘贴网址", "data-i18n-title": "web.pasteTitle", "data-i18n-aria": "web.pasteAria"} | — | 依文档/平台/面板状态显示 | [assets/index.html:975](../../../assets/index.html)；assets/app.js:298,299 |
| S270 / F067 | url-pages-dec | 减少页数 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-label": "减少页数", "data-i18n-aria": "web.decPages"} | — | 依文档/平台/面板状态显示 | [assets/index.html:984](../../../assets/index.html)；assets/app.js:277,278 |
| S271 / F067 | url-pages | 抓取页数 | 点击聚焦并编辑；提交/Enter与验证由所属流程规定 | {"type": "number", "min": "1", "max": "30", "value": "1", "step": "1", "inputmode": "numeric", "aria-label": "抓取页数", "data-i18n-aria": "web.pagesAria"} | — | 依文档/平台/面板状态显示 | [assets/index.html:985](../../../assets/index.html)；assets/js/features/web.js:48,165；assets/app.js:277,278,279,283,284,285,289,290,293,324,329 |
| S272 / F067 | url-pages-inc | 增加页数 | 点击执行所属流程；禁用/隐藏时不执行 | {"type": "button", "aria-label": "增加页数", "data-i18n-aria": "web.incPages"} | — | 依文档/平台/面板状态显示 | [assets/index.html:986](../../../assets/index.html)；assets/app.js:283,284 |
| S273 / F067 | url-images | url-images | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox"} | — | 依文档/平台/面板状态显示 | [assets/index.html:991](../../../assets/index.html)；assets/js/features/web.js:49,164 |
| S274 / F068 | url-go | 智能提取正文 安全提取 | 点击执行所属流程；禁用/隐藏时不执行 | {} | — | 依文档/平台/面板状态显示 | [assets/index.html:1004](../../../assets/index.html)；assets/js/core/dragdrop.js:212；assets/js/features/web.js:41；assets/app.js:322,335 |
| S275 / F068 | url-render | 完整动态渲染 保留复杂排版与脚本 | 点击执行所属流程；禁用/隐藏时不执行 | {} | — | 依文档/平台/面板状态显示 | [assets/index.html:1013](../../../assets/index.html)；assets/js/features/web.js:42,247；assets/app.js:327 |
| S276 / F068 | url-cancel | 取消 | 点击执行所属流程；禁用/隐藏时不执行 | {"data-i18n": "convert.cancel"} | — | 初始隐藏 | [assets/index.html:1022](../../../assets/index.html)；assets/js/features/web.js:44；assets/app.js:332 |
| S277 / F067 | url-private | url-private | 点击切换开关；按所属流程立即保存、仅影响本次任务或待保存表单 | {"type": "checkbox", "checked": null} | — | 依文档/平台/面板状态显示 | [assets/index.html:1027](../../../assets/index.html)；assets/js/features/web.js:50,171,248 |

## 公式辅助编辑增量（2026-10-07）

以下入口补入原公式面板；历史行号是定位参考，以机器清单中的元素身份为准。

| 编号 | 控件 | 操作 | 实现 |
| --- | --- | --- | --- |
| S401 / F032 | formula-source | 展开后修改源码，防抖渲染，验证通过才能确认 | [公式草稿](../../../assets/js/editor/formula-studio.js) |
| S402 / F032 | formula-token | 点击渲染符号后编辑；Enter 替换所选源范围 | [公式草稿](../../../assets/js/editor/formula-studio.js) |
| S403 / F032 | formula-token-replace | 在公式草稿中替换当前符号 | [公式草稿](../../../assets/js/editor/formula-studio.js) |
| S404 / F032 | formula-more | 每次增加 72 项，不一次挂载全部字符 | [公式草稿](../../../assets/js/editor/formula-studio.js) |
| S405 / F032 | formula-apply | 确认一次写入；原文修订变化时拒绝覆盖 | [公式草稿](../../../assets/js/editor/formula-studio.js) |
| S406 / F032 | LaTeX 源码 summary | 展开/折叠源码；折叠不丢失公式草稿 | [公式草稿](../../../assets/js/editor/formula-studio.js) |
