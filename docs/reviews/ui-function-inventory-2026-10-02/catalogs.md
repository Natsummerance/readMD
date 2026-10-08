# 运行抽查、完整数据目录与边界

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)

## 抽查方式

使用临时隔离数据目录，现有Rust二进制和本工作树前端资源；仅本机HTTP。两种视口打开8个面板，没有外部AI请求/插件安装卸载/导出写盘/覆盖文件/更新/开启共享/原生桌宠启动。记录的是整个页面可见控件，包含顶栏，不代表某面板自身控件数。pageerror为空；没有逐像素遮挡测量，也没有逐按钮业务成功测试。


| 窗口 | 打开面板 | 全页面可见控件 | 部分隐藏/禁用状态 |
| --- | --- | --- | --- |
| 1160×820 | convert | 31 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1160×820 | plugin | 47 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1160×820 | pet | 190 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1160×820 | web | 35 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1160×820 | skills | 42 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1160×820 | ai-settings | 46 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1160×820 | language | 73 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1160×820 | shortcuts | 27 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | convert | 31 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | plugin | 47 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | pet | 190 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | web | 35 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | skills | 39 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | ai-settings | 46 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | language | 73 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |
| 1024×680 | shortcuts | 27 | 各控件disabled已采集；数值受欢迎态、展开状态和视口影响 |

另建立一份临时虚拟Markdown（标题/任务/脚注/表格/代码），读取六种导出动态面板并检查编辑器初始化；editorReady=True，斜杠一级目录23项。API安全GET包括plugins/list、modules、skills、ai/config（只取公开目录，不记录密钥）、pets、pets/status、diagram/capabilities、export/presets。

## 可筛选完整目录


| 文件 | 数量 | 字段与适用范围 |
| --- | --- | --- |
| [providers.csv](providers.csv) | 1174 | 26官方 + 1148上游条目；公开URL/协议/模型/来源/能力，未含用户凭据；目录快照不是外部服务当前在线保证 |
| [skills.csv](skills.csv) | 25 | 隔离环境内置本地Skill的id、标题、说明、状态、源路径；用户导入后可增加 |
| [pets.csv](pets.csv) | 79 | 伴读使者、3原创精灵、Bongo、Arch-Chan + 73目录角色；缩略/has_local不是当前已安装运行证明 |
| [languages.csv](languages.csv) | 46 | 全部语言code、中文名/native名、LTR/RTL |
| [formulas.csv](formulas.csv) | 2,478 | 46 个结构/函数预设与 2,432 个字符；分类、名称、搜索别名及源写法，不代表完整 OMML 排版兼容 |
| [slash-languages.csv](slash-languages.csv) | 25 | 全部语言子菜单及别名，另支持plain/自定义 |
| [export-fields.csv](export-fields.csv) | 99 | 98可编辑字段+1说明、各格式、范围和全部选项 |
| [export-presets.json](export-presets.json) | 3 | defaults、3内置预设、空隔离custom/last；未复制用户自定义数据 |
| [api.csv](api.csv) | 100 | 注册HTTP路由与准确handler/行、前端引用、native桥 |
| [controls.csv](controls.csv) | 391 | 全部静态控件完整属性和option、所在流程、后台说明 |
| [dynamic.csv](dynamic.csv) | 78 | 生成控件族与手势内容、前提、后端 |
| [formats.csv](formats.csv) | 143 | 逐后缀收集/实际分派/拒绝/回退/条件 |
| [plugins.csv](plugins.csv) | 14 | 插件目录、互斥、体量/模型、native与隔离状态 |
| [commands.csv](commands.csv) | 67 | 全部注册命令、快捷键、条件、函数执行内容 |

## 实现与验收

本轮已补齐离线ASR、动态网页及私有授权、原生扩展生命周期、代码顶部编辑、文献引擎与资源、真实PDF预览、ZIP选择转换、MIME/MSG邮件、PDF子文档、富文本剪贴转换及WSD/D2/ditaa基础离线SVG；修复批次保持、空编辑导出、叠加弹窗焦点、语音时间戳和隐藏桌宠预览加载。平台条件和实际验收详见 [收尾工作表](../readmd-ui-completion-2026-10-02.md)。DOCX采用共享内容/设置的版式参考，分页以Word为准；外部AI、系统识别器、平台对话框等条件入口均保留清单。


后续按101项进行真实实现复查、补齐与回归，见 [逐项复查](../readmd-ui-feature-verification-2026-10-03.md)。上述只读快照保持原始边界，后续实际业务测试另有日志。
