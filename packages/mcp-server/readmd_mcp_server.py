# -*- coding: utf-8 -*-
"""ReadMD MCP Server —— 基于标准 Model Context Protocol (stdio) 的全功能 Markdown 与文档处理服务。

向 AI Agents (Claude Desktop, Cursor, Antigravity, VSCode, GitHub Copilot 等) 提供：
1. `readmd_fix_markdown`: 自动诊断并修复 Markdown 格式错误（公式断裂、表格错位、代码块、中英文标点与空格规范）；
2. `readmd_convert_to_markdown`: 将本地 Word (.docx), PDF (.pdf), PPT (.pptx), Excel (.xlsx), LaTeX (.tex), HTML, TXT 等转为高质量 Markdown；
3. `readmd_web_to_markdown`: 抓取网页 URL 并深度抽取清洗为高可读 Markdown；
4. `readmd_ocr_to_markdown`: 本地图片或扫描件 PDF 通过 WinRT / macOS Vision / Tesseract 离线 OCR 识别为 Markdown；
5. `readmd_export_document`: 将 Markdown 编译导出为排版级 PDF、Word (.docx)、HTML 或学术 LaTeX；
6. `readmd_latex_to_md`: LaTeX 论文/公式源码精确转为标准 Markdown；
7. `readmd_md_to_latex`: Markdown 文档编译为标准学术 LaTeX 源码；
8. `readmd_parse_bibtex`: 自动解析 BibTeX 文献数据库并生成学术引用映射；
9. `readmd_latex_to_omml`: 将 LaTeX 公式转为 Word 原生 Office Math (OMML) / MathML XML；
10. `readmd_ai_assistant`: 获取 ReadMD 内置 12 种专业文档 AI 提示词流程（快速阅读、润色、大纲、代码审查等）。
"""

import json
import logging
import os
import sys
from typing import Any, Dict, List, Optional

# 引入 ReadMD 核心算法路径
ROOT_DIR = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

try:
    from src.readmd_core import readmd_fix
    from src.readmd_core.toc_engine import process_toc_markers, generate_toc_markdown, extract_headings
    from src.readmd_modules import bibtex, convert, latex2omml, mdexport, ocr, texmd, txtmd, web
    from src.readmd_modules.import_processor import process_markdown_imports
    from src.readmd_modules.mdexport.presentation_render import render_presentation_html
    from src.readmd_modules.mdexport.epub_render import export_epub
    from src.readmd_modules.code_chunk_runner import execute_python_chunk, execute_code_chunk
except ImportError as e:
    logging.warning("ReadMD modules import warning in MCP server: %s", e)

PROMPT_WORKFLOWS = {
    "quick_read": {
        "name": "快速阅读",
        "system": "你是 ReadMD 的文档阅读助手。对用户给出的 Markdown 文档做快速阅读，输出：1) 一句话概述；2) 核心要点列表；3) 文档结构目录；4) 值得注意的细节或疑问。使用 Markdown 格式。"
    },
    "polish": {
        "name": "智能润色",
        "system": "你是资深中文编辑。润色用户给出的 Markdown 文档：修正错别字、病句、表达生硬之处，保留原有结构与全部 Markdown 标记，只输出润色后的完整文档，不要加任何解释。"
    },
    "modify": {
        "name": "语法修改",
        "system": "你是文档修订助手。根据用户要求修改文档，修正明显错误（错别字、标点、Markdown 格式错误）。只输出修改后的完整文档，不要加任何解释。"
    },
    "expand": {
        "name": "内容扩充",
        "system": "你是文档扩充助手。在保持原有结构与语气的前提下，为文档补充细节、示例、解释，使内容更丰富。只输出扩充后的完整文档，不要加任何解释。"
    },
    "continue": {
        "name": "自然续写",
        "system": "你是文档续写助手。从文档末尾自然延续写作，保持风格一致。只输出续写的新增内容，不要重复原文。"
    },
    "translate": {
        "name": "学术翻译",
        "system": "你是专业翻译。将用户给出的文档翻译成目标语言，保留 Markdown 结构、公式、表格与代码块，只输出译文。"
    },
    "ask": {
        "name": "文档问答",
        "system": "你是文档问答助手。基于用户给出的文档内容回答问题；文档中没有的内容请明确说明。"
    },
    "summary": {
        "name": "核心总结",
        "system": "你是文档总结助手。用 5 条以内要点概括用户文档的核心内容，输出为 Markdown 列表；最后用一句话总结全文。"
    },
    "outline": {
        "name": "生成大纲",
        "system": "你是文档策划。为用户文档生成层级目录大纲（# / ## / ###），只输出大纲，不要其他内容。"
    },
    "weekly": {
        "name": "周报整理",
        "system": "你是周报助手。根据用户给出的工作内容，整理成结构化周报：本周完成 / 下周计划 / 风险与求助。只输出周报正文。"
    },
    "to_english": {
        "name": "英文翻译",
        "system": "你是专业翻译。将用户给出的文档翻译成地道专业英文，保留 Markdown 结构、公式、表格与代码块，只输出译文。"
    },
    "code_review": {
        "name": "代码审查",
        "system": "你是资深代码审查员。审查用户文档中的代码块：指出 bug、安全隐患、可读性问题，并给出修改建议与示例代码。用 Markdown 输出。"
    }
}

TOOLS: List[Dict[str, Any]] = [
    {
        "name": "readmd_fix_markdown",
        "description": "自动诊断并修复 Markdown 文本中的各类语法格式错误（公式断裂、表格错位、代码块未闭合、中英文标点空格等盘古排版规范）。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "content": {"type": "string", "description": "待修复的 Markdown 原始内容"}
            },
            "required": ["content"]
        }
    },
    {
        "name": "readmd_convert_to_markdown",
        "description": "将本地各种格式的文档（docx, pdf, pptx, xlsx, tex, html, epub, txt, zip 等）转换为高质量 Markdown。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "本地文档的绝对路径"}
            },
            "required": ["file_path"]
        }
    },
    {
        "name": "readmd_web_to_markdown",
        "description": "抓取目标网页 URL 并深度抽取清洗正文为干净的高可读 Markdown 文本。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "目标网页的 HTTP/HTTPS URL 地址"}
            },
            "required": ["url"]
        }
    },
    {
        "name": "readmd_ocr_to_markdown",
        "description": "对本地图片（png, jpg, webp 等）或扫描件 PDF 调用系统原生/离线 OCR 识别并输出结构化 Markdown。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "图片或 PDF 文件的绝对路径"}
            },
            "required": ["file_path"]
        }
    },
    {
        "name": "readmd_export_document",
        "description": "将 Markdown 文档高质量编译导出为 PDF、Word (.docx)、HTML 或 LaTeX 文件，支持 15 款专业排版风格预设。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "markdown_content": {"type": "string", "description": "Markdown 源码文本"},
                "output_path": {"type": "string", "description": "导出目标文件的绝对路径（如 /path/to/output.pdf）"},
                "output_format": {
                    "type": "string",
                    "enum": ["pdf", "docx", "html", "tex"],
                    "description": "导出格式：pdf | docx | html | tex"
                },
                "style_preset": {
                    "type": "string",
                    "enum": ["minimal", "academic", "report", "tech", "warm", "elegant", "compact"],
                    "description": "排版风格预设（默认 minimal）"
                },
                "title": {"type": "string", "description": "文档标题（可选）"}
            },
            "required": ["markdown_content", "output_path", "output_format"]
        }
    },
    {
        "name": "readmd_latex_to_md",
        "description": "将 LaTeX 论文源码或数学公式转换为干净的标准 Markdown 格式。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "latex_content": {"type": "string", "description": "LaTeX 源代码"}
            },
            "required": ["latex_content"]
        }
    },
    {
        "name": "readmd_md_to_latex",
        "description": "将 Markdown 文档转换/编译为可直接供 pdflatex / xelatex 编译的学术 LaTeX 源码。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "markdown_content": {"type": "string", "description": "Markdown 文本内容"},
                "doc_title": {"type": "string", "description": "文档标题（可选）"}
            },
            "required": ["markdown_content"]
        }
    },
    {
        "name": "readmd_parse_bibtex",
        "description": "扫描并解析 BibTeX (.bib) 参考文献文件，提取结构化论文元数据与标准引用映射。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "bib_file_path": {"type": "string", "description": ".bib 文件路径或包含 .bib 的目录路径"}
            },
            "required": ["bib_file_path"]
        }
    },
    {
        "name": "readmd_latex_to_omml",
        "description": "将 LaTeX 数学公式转换为 Word 原生 Office Math (OMML) 或 MathML XML 片段。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "latex_formula": {"type": "string", "description": "LaTeX 公式字符串（如 E = mc^2）"}
            },
            "required": ["latex_formula"]
        }
    },
    {
        "name": "readmd_ai_assistant",
        "description": "获取 ReadMD 内置的 12 种专业文档 AI 提示词流程模板（快速阅读、润色、代码审查、大纲生成等）。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "workflow_id": {
                    "type": "string",
                    "enum": list(PROMPT_WORKFLOWS.keys()),
                    "description": "工作流标识符：quick_read | polish | modify | expand | continue | translate | ask | summary | outline | weekly | to_english | code_review"
                },
                "markdown_content": {"type": "string", "description": "待处理的文档正文内容（可选）"}
            },
            "required": ["workflow_id"]
        }
    },
    {
        "name": "readmd_process_imports",
        "description": "解析并展平 Markdown 中的 @import 外部文件指令（支持子 Markdown 章节递归、CSV 转表格、代码局部行号切片）。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "markdown_content": {"type": "string", "description": "含 @import 指令的 Markdown 源码"},
                "base_dir": {"type": "string", "description": "相对路径引用的基准根目录"}
            },
            "required": ["markdown_content"]
        }
    },
    {
        "name": "readmd_generate_toc",
        "description": "扫描 Markdown 标题并生成带深度过滤与锚点跳转的层级目录树 [TOC]。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "markdown_content": {"type": "string", "description": "Markdown 源码"},
                "depth_from": {"type": "integer", "description": "起始标题层级 (默认 1)"},
                "depth_to": {"type": "integer", "description": "截止标题层级 (默认 6)"},
                "ordered_list": {"type": "boolean", "description": "是否生成带序号的有序列表 (默认 false)"}
            },
            "required": ["markdown_content"]
        }
    },
    {
        "name": "readmd_export_presentation",
        "description": "将包含 <!-- slide --> 分页的 Markdown 文档编译导出为单文件 Reveal.js 演说幻灯片 HTML。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "markdown_content": {"type": "string", "description": "Markdown 幻灯片源码"},
                "output_path": {"type": "string", "description": "导出目标 HTML 绝对路径"},
                "title": {"type": "string", "description": "演示文稿标题"},
                "theme": {"type": "string", "description": "Reveal.js 主题 (black, white, league, sky, beige, night, serif, simple, solarized)"},
                "transition": {"type": "string", "description": "转场效果 (slide, fade, zoom, convex, concave)"}
            },
            "required": ["markdown_content", "output_path"]
        }
    },
    {
        "name": "readmd_export_epub",
        "description": "将 Markdown 文档原生打包导出为符合 IDPF 规范的标准 EPUB 3.0 电子书文件 (.epub)。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "markdown_content": {"type": "string", "description": "Markdown 电子书源码"},
                "output_path": {"type": "string", "description": "导出目标 .epub 绝对路径"},
                "title": {"type": "string", "description": "电子书标题 (默认 'ReadMD 电子书')"},
                "author": {"type": "string", "description": "电子书作者 (默认 'ReadMD Author')"},
                "language": {"type": "string", "description": "语言代码 (默认 'zh-CN')"}
            },
            "required": ["markdown_content", "output_path"]
        }
    },
    {
        "name": "readmd_run_code_chunk",
        "description": "在安全沙箱环境中就地执行多语言代码块 (Python, JavaScript/Node.js, Shell, R, Rust)，捕获控制台输出并自动提取图表图像。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "code": {"type": "string", "description": "待执行的源码内容"},
                "language": {"type": "string", "description": "代码语言类型 (python, js, bash, powershell, r, rust 等，默认 python)"},
                "capture_plot": {"type": "boolean", "description": "是否自动捕获 Matplotlib 等图表 (默认 true)"}
            },
            "required": ["code"]
        }
    }
]


def handle_tool_call(name: str, args: Dict[str, Any]) -> Dict[str, Any]:
    """统一调度处理各 MCP 工具调用并返回标准响应。"""
    try:
        if name == "readmd_fix_markdown":
            content = str(args.get("content", ""))
            res = readmd_fix.fix_markdown(content)
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps({
                            "ok": True,
                            "repaired_content": res.text,
                            "fixes_count": len(res.fixes),
                            "fixes_details": res.fixes,
                            "stats": res.stats
                        }, ensure_ascii=False, indent=2)
                    }
                ]
            }

        elif name == "readmd_convert_to_markdown":
            fp = os.path.abspath(str(args.get("file_path", "")))
            if not os.path.isfile(fp):
                return {"isError": True, "content": [{"type": "text", "text": f"文件不存在: {fp}"}]}

            ext = os.path.splitext(fp)[1].lower()
            if ext == '.tex':
                with open(fp, 'r', encoding='utf-8', errors='replace') as f:
                    raw_tex = f.read()
                md = texmd.latex_to_markdown(raw_tex)
                return {"content": [{"type": "text", "text": md}]}
            elif ext == '.txt':
                with open(fp, 'r', encoding='utf-8', errors='replace') as f:
                    raw_txt = f.read()
                md, _ = txtmd.to_markdown(raw_txt)
                return {"content": [{"type": "text", "text": md}]}
            else:
                md = convert.to_markdown(fp)
                return {"content": [{"type": "text", "text": md}]}

        elif name == "readmd_web_to_markdown":
            target_url = str(args.get("url", "")).strip()
            if not target_url.startswith(('http://', 'https://')):
                return {"isError": True, "content": [{"type": "text", "text": "URL 必须以 http:// 或 https:// 开头"}]}

            doc, _ = web.fetch_document(target_url)
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps({
                            "ok": True,
                            "title": doc.get("title", ""),
                            "markdown": doc.get("markdown", ""),
                            "url": target_url,
                            "images_count": len(doc.get("images", []))
                        }, ensure_ascii=False, indent=2)
                    }
                ]
            }

        elif name == "readmd_ocr_to_markdown":
            fp = os.path.abspath(str(args.get("file_path", "")))
            if not os.path.isfile(fp):
                return {"isError": True, "content": [{"type": "text", "text": f"文件不存在: {fp}"}]}

            md = ocr.ocr_any(fp)
            return {"content": [{"type": "text", "text": md}]}

        elif name == "readmd_export_document":
            md_content = str(args.get("markdown_content", ""))
            out_path = os.path.abspath(str(args.get("output_path", "")))
            fmt = str(args.get("output_format", "pdf")).lower()
            preset = str(args.get("style_preset", "minimal"))
            title = str(args.get("title", "ReadMD Document"))

            os.makedirs(os.path.dirname(out_path), exist_ok=True)

            if fmt == 'tex':
                tex = texmd.markdown_to_latex(md_content, title=title)
                with open(out_path, 'w', encoding='utf-8') as f:
                    f.write(tex)
                return {"content": [{"type": "text", "text": f"已成功编译并导出 LaTeX 到: {out_path}"}]}

            style = mdexport.styles.preset_style(preset)
            res = mdexport.export(
                fmt=fmt,
                content=md_content,
                base_dir=os.path.dirname(out_path),
                out_path=out_path,
                options=style,
                source_name=title
            )
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps({
                            "ok": True,
                            "format": fmt,
                            "output_path": out_path,
                            "file_size": os.path.getsize(out_path) if os.path.exists(out_path) else 0,
                            "warnings": res.get("warnings", [])
                        }, ensure_ascii=False, indent=2)
                    }
                ]
            }

        elif name == "readmd_latex_to_md":
            latex_content = str(args.get("latex_content", ""))
            md = texmd.latex_to_markdown(latex_content)
            return {"content": [{"type": "text", "text": md}]}

        elif name == "readmd_md_to_latex":
            md_content = str(args.get("markdown_content", ""))
            title = str(args.get("doc_title", "ReadMD Document"))
            tex = texmd.markdown_to_latex(md_content, title=title)
            return {"content": [{"type": "text", "text": tex}]}

        elif name == "readmd_parse_bibtex":
            bp = str(args.get("bib_file_path", ""))
            res = bibtex.find_and_load_bib_for_file(bp)
            return {"content": [{"type": "text", "text": json.dumps(res, ensure_ascii=False, indent=2)}]}

        elif name == "readmd_latex_to_omml":
            formula = str(args.get("latex_formula", ""))
            omml = latex2omml.latex_to_omml(formula)
            return {"content": [{"type": "text", "text": omml}]}

        elif name == "readmd_ai_assistant":
            wf_id = str(args.get("workflow_id", "quick_read"))
            wf = PROMPT_WORKFLOWS.get(wf_id, PROMPT_WORKFLOWS["quick_read"])
            doc_content = str(args.get("markdown_content", ""))
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps({
                            "workflow_id": wf_id,
                            "workflow_name": wf["name"],
                            "system_prompt": wf["system"],
                            "user_payload": doc_content
                        }, ensure_ascii=False, indent=2)
                    }
                ]
            }
        elif name == "readmd_process_imports":
            md_content = str(args.get("markdown_content", ""))
            base_dir = str(args.get("base_dir", os.getcwd()))
            flattened = process_markdown_imports(md_content, base_dir=base_dir)
            return {"content": [{"type": "text", "text": flattened}]}

        elif name == "readmd_generate_toc":
            md_content = str(args.get("markdown_content", ""))
            depth_from = int(args.get("depth_from", 1))
            depth_to = int(args.get("depth_to", 6))
            ordered = bool(args.get("ordered_list", False))
            headings = extract_headings(md_content)
            toc_md = generate_toc_markdown(headings, depth_from=depth_from, depth_to=depth_to, ordered_list=ordered)
            return {"content": [{"type": "text", "text": toc_md}]}

        elif name == "readmd_export_presentation":
            md_content = str(args.get("markdown_content", ""))
            out_path = os.path.abspath(str(args.get("output_path", "")))
            title = str(args.get("title", "ReadMD Presentation"))
            theme = str(args.get("theme", "black"))
            transition = str(args.get("transition", "slide"))
            os.makedirs(os.path.dirname(out_path), exist_ok=True)
            html = render_presentation_html(md_content, title=title, theme=theme, transition=transition)
            with open(out_path, 'w', encoding='utf-8') as f:
                f.write(html)
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps({
                            "ok": True,
                            "output_path": out_path,
                            "file_size": os.path.getsize(out_path)
                        }, ensure_ascii=False, indent=2)
                    }
                ]
            }

        elif name == "readmd_export_epub":
            md_content = str(args.get("markdown_content", ""))
            out_path = os.path.abspath(str(args.get("output_path", "")))
            title = str(args.get("title", "ReadMD 电子书"))
            author = str(args.get("author", "ReadMD Author"))
            language = str(args.get("language", "zh-CN"))
            os.makedirs(os.path.dirname(out_path), exist_ok=True)
            export_epub(md_content, out_path, title=title, author=author, language=language)
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps({
                            "ok": True,
                            "output_path": out_path,
                            "file_size": os.path.getsize(out_path)
                        }, ensure_ascii=False, indent=2)
                    }
                ]
            }

        elif name == "readmd_run_code_chunk":
            code = str(args.get("code", ""))
            lang = str(args.get("language", "python"))
            capture_plot = bool(args.get("capture_plot", True))
            res = execute_code_chunk(code, lang=lang, capture_plot=capture_plot)
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps(res, ensure_ascii=False, indent=2)
                    }
                ]
            }

        return {"isError": True, "content": [{"type": "text", "text": f"未知的工具名称: {name}"}]}

    except Exception as e:
        logging.exception("Tool execution error: %s", e)
        return {"isError": True, "content": [{"type": "text", "text": f"工具执行异常: {str(e)}"}]}


def run_stdio_server():
    """标准 JSON-RPC 2.0 stdio 通信主循环。"""
    if sys.platform == 'win32' and hasattr(sys.stdin, 'reconfigure'):
        try:
            sys.stdin.reconfigure(encoding='utf-8', errors='replace')
            sys.stdout.reconfigure(encoding='utf-8', errors='replace')
        except Exception:
            pass

    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break
            line = line.strip()
            if not line:
                continue
            req = json.loads(line)
            req_id = req.get("id")
            method = req.get("method")

            if method == "initialize":
                res = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "serverInfo": {
                            "name": "readmd-mcp-server",
                            "version": "2.3.4"
                        },
                        "capabilities": {
                            "tools": {}
                        }
                    }
                }
            elif method == "tools/list":
                res = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "tools": TOOLS
                    }
                }
            elif method == "tools/call":
                params = req.get("params", {})
                name = params.get("name")
                arguments = params.get("arguments", {})
                tool_res = handle_tool_call(name, arguments)
                res = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": tool_res
                }
            else:
                res = {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": {
                        "code": -32601,
                        "message": f"Method not found: {method}"
                    }
                }

            sys.stdout.write(json.dumps(res, ensure_ascii=False) + '\n')
            sys.stdout.flush()
        except Exception as e:
            err_res = {
                "jsonrpc": "2.0",
                "id": None,
                "error": {
                    "code": -32603,
                    "message": str(e)
                }
            }
            sys.stdout.write(json.dumps(err_res, ensure_ascii=False) + '\n')
            sys.stdout.flush()


if __name__ == "__main__":
    run_stdio_server()
