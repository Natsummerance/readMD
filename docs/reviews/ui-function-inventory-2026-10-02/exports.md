# 导出：六种格式、全部配置字段

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)

共有14组、98个可编辑字段和1条说明；标题组生成H1–H6×4项。表中“未设”表示defaults GET没有这个字段，界面/后端可能另有fallback，不能把未设误读成后端无默认。所有select的选项完整记录，数字范围是前端属性，后端还有独立校验/归一化。


| 格式 | 前端执行入口 | 真实产物及后端 | 限制 |
| --- | --- | --- | --- |
| PDF | runExportOnce → /api/export fmt=pdf | PDF；[rust/readmd-kernel/src/mdexport.rs:2291 · export_document](../../../rust/readmd-kernel/src/mdexport.rs) → pdf_render.rs / export_preview.rs | 预览实际PDF产物：WinRT页图、页尺寸与页文字；相同输入且缓存有效时导出复用同一份字节 |
| DOCX | runExportOnce → /api/export fmt=docx | OOXML ZIP；docx_writer.rs + export_styles.rs | Word版本/字体可改变打开后的排版 |
| EPUB | /api/export/epub | OPF/章节/图片/样式ZIP；[rust/readmd-kernel/src/mdexport.rs:1821 · export_epub](../../../rust/readmd-kernel/src/mdexport.rs) | 章节拆分和元数据用epub.*；无同级通用task取消 |
| HTML | /api/export fmt=html | HTML、样式和本地资源处理；mdexport.rs | 正文净化、脚本资源与离线嵌入由HTML导出实现 |
| LaTeX | /api/export fmt=tex | 源码.tex + 资源目录；[rust/readmd-kernel/src/mdexport.rs:1163 · export_tex](../../../rust/readmd-kernel/src/mdexport.rs) | 输出TeX源码；选择的文献引擎与引用命令进入文件，bibliography复制为相对路径资源；此入口不编译TeX |
| 演示HTML | Rust /api/export fmt=presentation；浏览器fallback /api/export/presentation | Reveal standalone HTML；[rust/readmd-kernel/src/mdexport.rs:4325 · render_presentation_html](../../../rust/readmd-kernel/src/mdexport.rs) | 主题/转场专属；播放与导出是两种操作 |

## 电子书元数据


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| epub.title | 书籍标题（留空自动使用文件名） | text | ["epub"] | {} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.author | 书籍作者 / 译者 | text | ["epub"] | {} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.publisher | 出版方 / 制作方 | text | ["epub"] | {} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.isbn | 标准 ISBN 书号 | text | ["epub"] | {} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.language | 主要语言 | select | ["epub"] | {"opts":[["zh-CN","简体中文 (zh-CN)"],["en","English (en)"],["ja","日本語 (ja)"],["zh-TW","繁體中文 (zh-TW)"],["fr","Français (fr)"],["de","Deutsch (de)"],["es","Español (es)"]]} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.splitLevel | 章节拆分策略 | select | ["epub"] | {"opts":[["h1","按一级标题 (H1) 智能切分多章节"],["h2","按一/二级标题 (H1+H2) 切分章节"],["none","单章节长文档 (不切分)"]]} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |

## 电子书阅读版式


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| epub.fontSize | 字号 pt | number | ["epub"] | {"min":8,"max":24} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.lineHeight | 行距 | number | ["epub"] | {"min":1.2,"max":2.5,"step":0.1} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.marginV | 垂直页边距 % | number | ["epub"] | {"min":0,"max":20} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |
| epub.marginH | 水平页边距 % | number | ["epub"] | {"min":0,"max":20} | 未设 | mdexport::export_epub / EPUB元数据、章节拆分、CSS |

## LaTeX 学术编译与宏包


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| tex.docClass | LaTeX 文档类 | select | ["tex"] | {"opts":[["ctexart","ctexart"],["article","article"],["ctexrep","ctexrep"],["report","report"],["book","book"],["beamer","beamer"]]} | 未设 | mdexport::export_tex → latex_writer：文档类、字号、纸张、geometry、ctex |
| tex.fontSize | 排版字号 | select | ["tex"] | {"opts":[["10pt","10pt"],["11pt","11pt"],["12pt","12pt"]]} | 未设 | mdexport::export_tex → latex_writer：文档类、字号、纸张、geometry、ctex |
| tex.paperSize | 纸张 | select | ["tex"] | {"opts":[["a4paper","A4"],["letterpaper","US Letter"]]} | 未设 | mdexport::export_tex → latex_writer：文档类、字号、纸张、geometry、ctex |
| tex.margin | 页面边距 (Geometry) | select | ["tex"] | {"opts":[["2.5cm","2.5 cm"],["1in","1 in"],["2cm","2.0 cm"],["3cm","3.0 cm"]]} | 未设 | mdexport::export_tex → latex_writer：文档类、字号、纸张、geometry、ctex |
| tex.bibEngine | 参考文献引擎 | select | ["tex"] | {"opts":[["biblatex","BibLaTeX"],["natbib","Natbib"],["bibtex","BibTeX"]]} | 未设 | mdexport::export_tex读取引擎；latex_writer写biblatex/bibtex/natbib及引用；bibliography局部资源复制 |
| tex.useCtex | 启用 CJK 中文宏包 (UTF-8 原生支持) | checkbox | ["tex"] | {} | 未设 | mdexport::export_tex → latex_writer：文档类、字号、纸张、geometry、ctex |

## 页面设置


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| page.size | 纸张 | select | ["pdf","docx"] | {"opts":["A3","A4","A5","B5","Letter","Legal","Custom"]} | A4 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| page.width | 自定义纸宽（mm） | number | ["pdf","docx"] | {"min":80,"max":600} | 210 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| page.height | 自定义纸高（mm） | number | ["pdf","docx"] | {"min":80,"max":600} | 297 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| page.orientation | 方向 | select | ["pdf","docx"] | {"opts":[["portrait","纵向"],["landscape","横向"]]} | portrait | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| page.marginTop | 上边距 mm | number | ["pdf","docx"] | {"min":0,"max":60} | 20 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| page.marginRight | 右边距 mm | number | ["pdf","docx"] | {"min":0,"max":60} | 18 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| page.marginBottom | 下边距 mm | number | ["pdf","docx"] | {"min":0,"max":60} | 20 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| page.marginLeft | 左边距 mm | number | ["pdf","docx"] | {"min":0,"max":60} | 18 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 封面与目录


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| cover.enabled | 启用封面页 | checkbox | ["pdf","docx"] | {} | false | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| cover.title | 封面标题（留空用文件名） | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| cover.subtitle | 封面副标题 | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| cover.date | 封面日期 | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| cover.align | 封面对齐 | select | ["pdf","docx"] | {"opts":[["center","居中"],["left","左对齐"],["right","右对齐"]]} | center | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| toc.enabled | PDF 目录页 | checkbox | ["pdf","docx"] | {} | false | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h1.pageBreakBefore | 一级标题另起一页 | checkbox | ["pdf","docx"] | {} | 未设 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 正文排版


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| typography.font | 正文字体 | select | ["pdf","docx","html"] | {"opts":[["MicrosoftYaHei","MicrosoftYaHei"],["SimHei","SimHei"],["SimSun","SimSun"],["KaiTi","KaiTi"],["DengXian","DengXian"],["Arial","Arial"]]} | MicrosoftYaHei | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| typography.size | 字号 pt | number | ["pdf","docx","html"] | {"min":8,"max":20} | 11 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| typography.lineHeight | 行距 | number | ["pdf","docx","html"] | {"min":1,"max":2.5,"step":0.1} | 1.6 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| typography.spacing | 段间距 pt | number | ["pdf","docx","html"] | {"min":0,"max":30} | 6 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| typography.firstLineIndent | 首行缩进（mm） | number | ["pdf","docx","html"] | {"min":0,"max":30} | 未设 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| typography.color | 正文颜色 | color | ["pdf","docx","html"] | {} | #262626 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| typography.align | 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左对齐"],["center","居中"],["right","右对齐"],["justify","两端对齐"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 标题（各级颜色 / 字号 / 加粗 / 对齐）


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| headings.h1.size | H1 字号 pt | number | ["pdf","docx","html"] | {"min":8,"max":40} | 20 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h1.color | H1 颜色 | color | ["pdf","docx","html"] | {} | #1a1a1a | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h1.bold | H1 加粗 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h1.align | H1 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左"],["center","中"],["right","右"],["justify","两端"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h2.size | H2 字号 pt | number | ["pdf","docx","html"] | {"min":8,"max":40} | 16 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h2.color | H2 颜色 | color | ["pdf","docx","html"] | {} | #1f2937 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h2.bold | H2 加粗 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h2.align | H2 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左"],["center","中"],["right","右"],["justify","两端"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h3.size | H3 字号 pt | number | ["pdf","docx","html"] | {"min":8,"max":40} | 14 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h3.color | H3 颜色 | color | ["pdf","docx","html"] | {} | #2d3748 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h3.bold | H3 加粗 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h3.align | H3 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左"],["center","中"],["right","右"],["justify","两端"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h4.size | H4 字号 pt | number | ["pdf","docx","html"] | {"min":8,"max":40} | 12 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h4.color | H4 颜色 | color | ["pdf","docx","html"] | {} | #374151 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h4.bold | H4 加粗 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h4.align | H4 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左"],["center","中"],["right","右"],["justify","两端"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h5.size | H5 字号 pt | number | ["pdf","docx","html"] | {"min":8,"max":40} | 11 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h5.color | H5 颜色 | color | ["pdf","docx","html"] | {} | #4a5568 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h5.bold | H5 加粗 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h5.align | H5 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左"],["center","中"],["right","右"],["justify","两端"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h6.size | H6 字号 pt | number | ["pdf","docx","html"] | {"min":8,"max":40} | 10.5 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h6.color | H6 颜色 | color | ["pdf","docx","html"] | {} | #4a5568 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h6.bold | H6 加粗 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| headings.h6.align | H6 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左"],["center","中"],["right","右"],["justify","两端"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 表格


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| table.headerBg | 表头背景 | color | ["pdf","docx","html"] | {} | #3b6ef5 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.headerColor | 表头文字色 | color | ["pdf","docx","html"] | {} | #ffffff | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.headerBold | 表头加粗 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.borderColor | 边框颜色 | color | ["pdf","docx","html"] | {} | #c8cdd4 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.borderWidth | 边框宽度 pt | number | ["pdf","docx","html"] | {"min":0,"max":3,"step":0.25} | 0.75 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.banded | 斑马纹 | checkbox | ["pdf","docx","html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.bandColor | 斑马纹颜色 | color | ["pdf","docx","html"] | {} | #f3f5f9 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.cellSize | 单元格字号 pt | number | ["pdf","docx","html"] | {"min":7,"max":16} | 10 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.cellPadding | 单元格内边距 pt | number | ["pdf","docx","html"] | {"min":0,"max":20} | 6 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.align | 对齐 | select | ["pdf","docx","html"] | {"opts":[["left","左对齐"],["center","居中"],["right","右对齐"],["justify","两端对齐"]]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| table.widthPct | 表格宽度 % | number | ["pdf","docx","html"] | {"min":50,"max":100} | 100 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 代码块


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| code.bg | 背景色 | color | ["pdf","docx","html"] | {} | #f5f6f8 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| code.color | 文字色 | color | ["pdf","docx","html"] | {} | #2f3b4a | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| code.font | 等宽字体 | select | ["pdf","docx","html"] | {"opts":[["Consolas","Consolas"],["Courier New","Courier New"],["SimHei","SimHei"]]} | Consolas | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| code.size | 字号 pt | number | ["pdf","docx","html"] | {"min":6,"max":16} | 9.5 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| code.borderColor | 边框颜色 | color | ["pdf","docx","html"] | {} | #dfe3e8 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| code.borderWidth | 边框宽度 pt | number | ["pdf","docx","html"] | {"min":0,"max":3,"step":0.25} | 0.5 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| code.rounded | 圆角（HTML） | checkbox | ["html"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 引用与链接


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| quote.barColor | 引用左边条色 | color | ["pdf","docx","html"] | {} | #3b6ef5 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| quote.bg | 引用背景 | color | ["pdf","docx","html"] | {} | #f3f6ff | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| quote.color | 引用文字色 | color | ["pdf","docx","html"] | {} | #4a5568 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| link.color | 链接颜色 | color | ["pdf","docx","html"] | {} | #2b6cb0 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| hr.color | 分割线颜色 | color | ["pdf","docx","html"] | {} | #d8dce2 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 页脚与元数据


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| header.text | 页眉 | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| header.align | 对齐 | select | ["pdf","docx"] | {"opts":["left","center","right"]} | left | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| images.widthPct | 图片宽度（版心百分比） | number | ["pdf","docx"] | {"min":10,"max":100} | 92 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| images.maxHeightPct | 图片高度上限（版心百分比） | number | ["pdf","docx"] | {"min":10,"max":100} | 85 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| footer.pageNumbers | 显示页码 | checkbox | ["pdf","docx"] | {} | true | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| footer.text | 页脚文字 | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| meta.title | 文档标题（PDF 元数据） | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| meta.author | 作者 | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |
| meta.subject | 主题 | text | ["pdf","docx"] | {} |  | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## 数学公式


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| math.dpi | 渲染分辨率 DPI | number | ["pdf","docx"] | {"min":100,"max":500,"step":10} | 220 | sanitize_options → PDF原生排版/DOCX OOXML；适用HTML项进入build_export_css；具体格式按formats列 |

## HTML 主题


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| htmlTheme | 页面主题 | select | ["html"] | {"opts":[["light","亮色"],["dark","暗色"],["sepia","米色"]]} | light | mdexport HTML样式主题 |

## 幻灯片


| 字段 | 文案 | 输入类型 | 可见格式 | 范围/步长/完整选项 | 默认值 | 后端消费 |
| --- | --- | --- | --- | --- | --- | --- |
| theme | 主题 | select | ["presentation"] | {"opts":["black","white","league","beige","night","serif","simple","solarized","blood","moon","sky"]} | 未设 | mdexport::render_presentation_html / Reveal配置；说明项不提交可编辑值 |
| transition | 切换效果 | select | ["presentation"] | {"opts":["slide","fade","zoom","convex","concave","none"]} | 未设 | mdexport::render_presentation_html / Reveal配置；说明项不提交可编辑值 |
| slidesHint | 用单独一行 --- 分隔幻灯片。导出为可离线打开的单个 HTML 文件（reveal.js）。 | note | ["presentation"] | {} | 未设 | mdexport::render_presentation_html / Reveal配置；说明项不提交可编辑值 |

## 预设与交互流程

选预设先合并defaults、builtin/custom，再重新绘制表单和预览；字段修改产生自定义配置。保存预设输入名称，空白/保留名/现有名冲突会阻止提交；成功写custom/last。Rust桥缺失时POST预设接口，普通无Rust环境打开导出有禁用提示，并非任意网页均有本地持久化。恢复默认、取消命名、分组折叠、AI生成、预览翻页/大预览、任务取消、打开/定位结果、系统打印分别见主体F070–F075附近相关标题。当前预设值另见export-presets.json。

前端字段定义 [assets/js/features/export.js:22 · getExportSections](../../../assets/js/features/export.js)，实际收集 [assets/js/features/export.js:1148 · collectExportOptions](../../../assets/js/features/export.js)，预设保存 [assets/js/features/export.js:1426 · expSavePreset](../../../assets/js/features/export.js)，后端 [rust/readmd-kernel/src/server.rs:7409 · h_export](../../../rust/readmd-kernel/src/server.rs)。
