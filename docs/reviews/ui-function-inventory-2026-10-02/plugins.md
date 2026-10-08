# 插件中心：全部目录项与操作状态

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)

入口是万物转MD → 插件管理，以及命令面板。14个目录ID保留兼容名称，界面显示对应Rust扩展名称/实际引擎；安装落盘的是原生扩展配置，体量小于16KB。runtime_connected只在有效原生profile安装后为true；cached/installed/enabled分别记录缓存、安装和启用。系统OCR/语音不可用时显示能力不足，不声称已装同名Python包。


| ID / 名称 | 分类 / 能力 | 描述 | 目录体量 / 模型 | 互斥同能力项 | 当前卡片 / 内置engine | 隔离installed / enabled / cached |
| --- | --- | --- | --- | --- | --- | --- |
| easyocr / 系统 OCR 版面 | ocr / ocr | Windows OCR 识别图片、扫描 PDF，保留阅读顺序。 | &lt; 16 KB · Rust / False | ["rapidocr"] | 可安装原生扩展 / system-ocr | False / False / True |
| pylatexenc / LaTeX 文档解析 | latex / latex | Rust 解析器转换 TeX 结构、公式和表格。 | &lt; 16 KB · Rust / False | [] | 可安装原生扩展 / texmd | False / False / False |
| rapidocr / 系统 OCR 转换 | ocr / ocr | 原生图片识别、PDF OCR 回退，共用互斥槽位。 | &lt; 16 KB · Rust / False | ["easyocr"] | 可安装原生扩展 / system-ocr | False / False / False |
| rapid_table / 对齐表格恢复 | document / table | 将 OCR、PDF 文本连续对齐的列恢复为表格，保留代码围栏。 | &lt; 16 KB · Rust / False | [] | 可安装原生扩展 / native-table-layout | False / False / False |
| whisper / 离线语音正文 | audio / audio | 系统语音识别及 FFmpeg 解码，正文不带时间戳，不下载 Whisper 模型。 | &lt; 16 KB · Rust / False | ["faster_whisper"] | 可安装原生扩展 / system-speech | False / False / True |
| jieba / 文档关键词 | text / keywords | Rust 词频分析提取重复的中文短语和英文词，追加关键词段落。 | &lt; 16 KB · Rust / False | [] | 可安装原生扩展 / native-keyphrases | False / False / False |
| pygments / 代码文档适配 | code / highlight | 源码转为带语言标记的 Markdown 围栏，接入语法高亮。 | &lt; 16 KB · Rust / False | [] | 可安装原生扩展 / code-highlight | False / False / False |
| pandoc_bridge / Office 文档适配 | tools / document | 原生 DOC/DOCX、XLS/XLSX、PPT/PPTX、ODT、RTF、EPUB 读取器。 | &lt; 16 KB · Rust / False | [] | 可安装原生扩展 / native-converters | False / False / False |
| pymupdf4llm / PDF 文本与版面 | document / pdf | Rust PDF 文本、表格、内嵌图片提取，扫描页回退 OCR。 | &lt; 16 KB · Rust / False | ["docling"] | 可安装原生扩展 / pdf-text | False / False / False |
| docling / PDF 扫描识别 | document / pdf | 优先原生 PDF OCR，识别器不可用时回退文本提取。 | &lt; 16 KB · Rust / False | ["pymupdf4llm"] | 可安装原生扩展 / pdf-text | False / False / False |
| faster_whisper / 离线分段转写 | audio / audio | 系统语音识别及 FFmpeg 解码，保留分段时间戳。 | &lt; 16 KB · Rust / False | ["whisper"] | 可安装原生扩展 / system-speech | False / False / False |
| markdownify / 本地 HTML 适配 | document / web | Rust HTML 读取器转换本地网页，移除可执行内容。 | &lt; 16 KB · Rust / False | ["trafilatura"] | 可安装原生扩展 / readability | False / False / False |
| trafilatura / 网页正文恢复 | document / web | 原生文字密度提取器选择正文，减少导航和侧栏噪声。 | &lt; 16 KB · Rust / False | ["markdownify"] | 可安装原生扩展 / readability | False / False / False |
| charset_normalizer / 文本编码识别 | text / encoding | 原生 UTF-8、GB18030、Big5 等文本解码参与转换。 | &lt; 16 KB · Rust / False | [] | 可安装原生扩展 / encoding-detect | False / False / False |

## 每个可操作入口


| 控件 | 可见前提 | 流程与前端反馈 | 实际后端/持久化 |
| --- | --- | --- | --- |
| 分类tab | 已打开中心 | 全部/OCR/学术文档/语音/网页/工具链切换过滤；不改变enabled | 前端过滤renderPluginCards |
| 运行信息summary | 已打开中心 | 展开ffmpeg状态、沙箱路径等；关闭只折叠 | GET/list的runtime信息 |
| 安装 / 重试 | 未安装原生扩展或上次失败 | 确认 → install → 轮询 → 原子profile与manifest写入/回读 → 成功或错误详情/回滚 | [rust/readmd-kernel/src/plugin_manager.rs:1106 · install_plugin_async](../../../rust/readmd-kernel/src/plugin_manager.rs) |
| 启用复选框 | installed扩展 | 锁定开关 → toggle → 保存并回读 → GET刷新 → 解锁；失败回滚；同能力互斥 | [rust/readmd-kernel/src/plugin_manager.rs:946 · set_plugin_enabled](../../../rust/readmd-kernel/src/plugin_manager.rs)；[rust/readmd-kernel/src/plugin_manager.rs:492 · save_manifest](../../../rust/readmd-kernel/src/plugin_manager.rs) |
| 卸载 | installed扩展 | 确认 → 检查安装中/运行占用 → 删除profile/旧沙箱文件 → 写manifest/回读 → 刷新 | [rust/readmd-kernel/src/plugin_manager.rs:1042 · uninstall_plugin](../../../rust/readmd-kernel/src/plugin_manager.rs) |
| 错误详情summary | 卡片有错误detail | 展开错误码/详情/最近日志；不触发修复 | [assets/js/features/convert.js:369 · pluginErrorMarkup](../../../assets/js/features/convert.js) |
| 关闭 | 中心打开 | 返回转换面板；轮询由状态逻辑处理，不把关闭当成取消安装 | 前端ReadMDModal状态 |

执行入口 [rust/readmd-kernel/src/plugin_manager.rs:678 · convert_document](../../../rust/readmd-kernel/src/plugin_manager.rs) 在真实转换前选取启用配置并登记占用；关键词提取、对齐表格、音频时间戳策略、扫描PDF优先OCR、网页正文提取等参与结果。安装/开关/执行/卸载已用隔离数据端到端验证；不触碰用户沙箱。
