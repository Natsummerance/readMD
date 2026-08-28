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
_SERVER_DIR = os.path.dirname(os.path.abspath(__file__))
_PACKAGED_ROOT = os.path.dirname(_SERVER_DIR)
# The release ZIP keeps ``packages/mcp-server`` and ``src`` beside each other.
# Resolve from the archive layout first, then support a source checkout.
_ROOT_CANDIDATES = (
    os.path.dirname(_PACKAGED_ROOT),
    os.path.dirname(os.path.dirname(_SERVER_DIR)),
    os.getcwd(),
)
ROOT_DIR = next((candidate for candidate in _ROOT_CANDIDATES
                 if os.path.isdir(os.path.join(candidate, 'src'))),
                os.path.dirname(_SERVER_DIR))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

try:
    import src.readmd_modules as RM
    from src.readmd_core import readmd_fix
    from src.readmd_core.toc_engine import process_toc_markers, generate_toc_markdown, extract_headings
    from src.readmd_modules import bibtex, convert, latex2omml, mdexport, ocr, texmd, txtmd, web
    from src.readmd_modules.import_processor import process_markdown_imports
    from src.readmd_modules.mdexport.presentation_render import render_presentation_html
    from src.readmd_modules.mdexport.epub_render import export_epub
    from src.readmd_modules.code_chunk_runner import execute_python_chunk, execute_code_chunk
    from src.readmd_modules.skills import SkillRegistry, SkillError, default_skill_roots
    from src.readmd_core.service import ReadMDCoreService
    from src.readmd_core import upstream as upstream_sources
    from src.readmd_core.config import VERSION, HISTORY_FILE
except ImportError as e:
    logging.warning("ReadMD modules import warning in MCP server: %s", e)

# Workflow metadata is deliberately kept separate from instructions.  The
# registry below reads the same built-in/user/project Skills as the desktop
# service, so MCP cannot drift into a second prompt implementation.
# One-release compatibility aliases for clients that still send the former
# workflow IDs.  They are metadata only; prompts/list is always the live Skill
# registry and no instruction text is duplicated here.
LEGACY_WORKFLOW_ALIASES = {
    "quick_read": "readmd-quick-read", "polish": "readmd-polish",
    "modify": "readmd-format-fix", "expand": "readmd-polish",
    "continue": "readmd-continue", "translate": "readmd-translate",
    "ask": "readmd-ask", "summary": "readmd-summary",
    "outline": "readmd-outline", "weekly": "readmd-weekly",
    "to_english": "readmd-translate", "code_review": "readmd-code-review",
}
LEGACY_WORKFLOW_NAMES = {
    "quick_read": "快速阅读", "polish": "智能润色", "modify": "语法修改",
    "expand": "内容扩充", "continue": "自然续写", "translate": "学术翻译",
    "ask": "文档问答", "summary": "核心总结", "outline": "生成大纲",
    "weekly": "周报整理", "to_english": "英文翻译", "code_review": "代码审查",
}


def _resolve_skill_id(identifier):
    requested = str(identifier or '').strip()
    return LEGACY_WORKFLOW_ALIASES.get(requested, requested)

# MCP clients must make side effects explicit. Reads and analysis remain
# available by default; exports, network fetches and code execution require a
# literal ``confirm: true`` in the tool arguments.
CONFIRM_REQUIRED = {
    "readmd_web_to_markdown", "readmd_export_document",
    "readmd_export_presentation", "readmd_export_epub", "readmd_run_code_chunk",
}


def _skills_registry():
    global _CORE_SERVICE
    if _CORE_SERVICE is None:
        _CORE_SERVICE = ReadMDCoreService()
    return _CORE_SERVICE.skills


_CORE_SERVICE = None


# Keep a literal in source for older ecosystem manifest scanners; the runtime
# value remains sourced from the single VERSION file below.
# "version": "2.3.7"

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
                "url": {"type": "string", "description": "目标网页的 HTTP/HTTPS URL 地址"},
                "confirm": {"type": "boolean", "const": True, "description": "明确确认联网抓取"}
            },
            "required": ["url", "confirm"]
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
                ,"confirm": {"type": "boolean", "const": True, "description": "明确确认写入导出文件"}
            },
            "required": ["markdown_content", "output_path", "output_format", "confirm"]
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
                    "description": "旧版工作流别名（兼容字段）；优先使用 skill_id"
                },
                "skill_id": {"type": "string", "description": "ReadMD Skill 标识符"},
                "markdown_content": {"type": "string", "description": "待处理的文档正文内容（可选）"}
            },
            "required": []
        }
    },
    {
        "name": "readmd_ai_providers",
        "description": "读取 ReadMD 共享 AI 提供商目录与脱敏连接状态；不会返回 API Key。",
        "inputSchema": {"type": "object", "properties": {}}
    },
    {
        "name": "readmd_ai_chat",
        "description": "使用共享凭据和 ReadMD Skill 执行一次 AI 文档处理；凭据只能使用 credential_id。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "provider": {"type": "string", "description": "提供商 ID"},
                "credential_id": {"type": "string", "description": "已保存凭据句柄，不是 API Key"},
                "model": {"type": "string"},
                "skill_id": {"type": "string"},
                "markdown_content": {"type": "string"},
                "request": {"type": "string"},
                "language": {"type": "string"},
                "stream": {"type": "boolean", "default": False}
            },
            "required": ["provider", "model", "skill_id", "markdown_content"]
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
                ,"confirm": {"type": "boolean", "const": True, "description": "明确确认写入导出文件"}
            },
            "required": ["markdown_content", "output_path", "confirm"]
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
                ,"confirm": {"type": "boolean", "const": True, "description": "明确确认写入导出文件"}
            },
            "required": ["markdown_content", "output_path", "confirm"]
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
                ,"confirm": {"type": "boolean", "const": True, "description": "明确确认执行代码"}
            },
            "required": ["code", "confirm"]
        }
    }
]


def handle_tool_call(name: str, args: Dict[str, Any]) -> Dict[str, Any]:
    """统一调度处理各 MCP 工具调用并返回标准响应。"""
    try:
        if name in CONFIRM_REQUIRED and args.get("confirm") is not True:
            return {"isError": True, "content": [{"type": "text", "text": json.dumps({
                "ok": False, "error": "此操作会产生副作用，请在请求中明确设置 confirm=true"
            }, ensure_ascii=False)}]}
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
            wf_id = str(args.get("workflow_id", ""))
            skill_id = _resolve_skill_id(args.get("skill_id") or wf_id or "readmd-quick-read")
            doc_content = str(args.get("markdown_content", "")) or "(no document supplied)"
            skill = _skills_registry().get(skill_id)
            if skill is None:
                raise SkillError("Skill not found: " + skill_id)
            system_prompt = _skills_registry().render(skill_id, {
                "document": doc_content,
                "selection": "",
                "request": str(args.get("request", "")),
                "language": str(args.get("language", "en")),
                "context": "",
                "output_format": "Markdown",
            })
            return {
                "content": [
                    {
                        "type": "text",
                        "text": json.dumps({
                            "workflow_id": wf_id,
                            "workflow_name": LEGACY_WORKFLOW_NAMES.get(wf_id, skill.name),
                            "skill_id": skill_id,
                            "system_prompt": system_prompt,
                            "user_payload": doc_content
                        }, ensure_ascii=False, indent=2)
                    }
                ]
            }
        elif name == "readmd_ai_providers":
            cfg = RM.get('ai').get_config()
            return {"content": [{"type": "text", "text": json.dumps({
                "schema_version": cfg.get("schema_version"),
                "providers": cfg.get("presets", []) + cfg.get("custom", []),
                "current": cfg.get("current", {})
            }, ensure_ascii=False, indent=2)}]}
        elif name == "readmd_ai_chat":
            if "api_key" in args or "key" in args:
                return {"isError": True, "content": [{"type": "text", "text": "AI Chat 只接受 credential_id，不接受 API Key"}]}
            provider = str(args.get("provider") or "")
            credential_id = str(args.get("credential_id") or "")
            skill_id = str(args.get("skill_id") or "")
            document = str(args.get("markdown_content") or "")
            if not provider or not skill_id or not document:
                return {"isError": True, "content": [{"type": "text", "text": "provider、skill_id、markdown_content 均为必填"}]}
            selected_provider = RM.get('ai').find_provider(provider) or {}
            local_provider = RM.get('ai')._is_local_provider(selected_provider)
            if not credential_id and not local_provider:
                return {"isError": True, "content": [{"type": "text", "text": "云端提供商必须使用 credential_id；本地服务可省略"}]}
            gen = RM.get('ai').chat({
                "provider": provider, "credential_id": credential_id,
                "model": str(args.get("model") or ""), "skill_id": skill_id,
                "skill_variables": {"document": document, "selection": "",
                                     "request": str(args.get("request") or ""),
                                     "language": str(args.get("language") or "en"),
                                     "context": "", "output_format": "Markdown"},
                "messages": [{"role": "user", "content": document}],
                "stream": bool(args.get("stream", False)),
            })
            chunks, usage = [], None
            for item in gen:
                if isinstance(item, dict):
                    if item.get("error"):
                        return {"isError": True, "content": [{"type": "text", "text": str(item["error"])}]}
                    usage = item.get("usage") or usage
                elif item:
                    chunks.append(str(item))
            return {"content": [{"type": "text", "text": json.dumps({
                "ok": True, "content": "".join(chunks), "usage": usage, "skill_id": skill_id
            }, ensure_ascii=False)}]}
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


def _skill_resources():
    """Expose read-only Skill metadata/instructions as MCP resources."""
    resources = []
    for skill in _skills_registry().list():
        resources.append({
            "uri": "readmd://skills/" + skill.id,
            "name": skill.name,
            "description": skill.description,
            "mimeType": "text/markdown",
        })
    return resources


def _all_resources():
    """Return read-only Skills plus persisted session/provider resources."""
    resources = _skill_resources()
    resources.extend([
        {
            "uri": "readmd://sessions",
            "name": "ReadMD AI sessions",
            "description": "Read-only summaries and messages from local AI history.",
            "mimeType": "application/json",
        },
        {
            "uri": "readmd://providers",
            "name": "ReadMD AI providers",
            "description": "Read-only provider catalog and credential status; secrets are omitted.",
            "mimeType": "application/json",
        },
    ])
    for source in upstream_sources.list_sources():
        source_id = source["id"]
        detail = upstream_sources.get_source(source_id)
        resources.append({
            "uri": "readmd://upstream/" + source_id,
            "name": "ReadMD upstream " + source_id.split("/")[0],
            "description": "Offline immutable source snapshot metadata and file allowlist.",
            "mimeType": "application/json",
        })
        for item in detail.get("source_files", []):
            resources.append({
                "uri": "readmd://upstream/%s/files/%s" % (source_id, item["id"]),
                "name": item.get("relative_path", item["id"]),
                "description": "Read-only vendored upstream source file (%s)." % item.get("sha256", ""),
                "mimeType": "text/plain",
            })
    return resources


def _scrub_resource(value):
    """Remove credential-like fields before exposing a JSON resource."""
    if isinstance(value, dict):
        return {k: _scrub_resource(v) for k, v in value.items()
                if str(k).lower() not in {"api_key", "key", "secret", "password", "token"}}
    if isinstance(value, list):
        return [_scrub_resource(item) for item in value]
    return value


def _read_skill_resource(uri):
    prefix = "readmd://skills/"
    skill_id = str(uri or "")[len(prefix):] if str(uri or "").startswith(prefix) else ""
    if not skill_id or "/" in skill_id or "\\" in skill_id:
        raise SkillError("invalid Skill resource URI")
    skill = _skills_registry().get(skill_id)
    if not skill:
        raise SkillError("Skill not found: " + skill_id)
    return {"contents": [{"uri": uri, "mimeType": "text/markdown", "text": skill.instructions}]}


def _read_resource(uri):
    if uri == "readmd://sessions":
        try:
            with open(HISTORY_FILE, encoding="utf-8") as handle:
                value = json.load(handle)
        except (OSError, ValueError, json.JSONDecodeError):
            value = {"sessions": []}
        return {"contents": [{"uri": uri, "mimeType": "application/json",
                              "text": json.dumps(_scrub_resource(value), ensure_ascii=False)}]}
    if uri == "readmd://providers":
        cfg = RM.get('ai').get_config()
        value = {"schema_version": cfg.get("schema_version"),
                 "providers": cfg.get("presets", []) + cfg.get("custom", []),
                 "current": cfg.get("current", {})}
        return {"contents": [{"uri": uri, "mimeType": "application/json",
                              "text": json.dumps(_scrub_resource(value), ensure_ascii=False)}]}
    prefix = "readmd://upstream/"
    if str(uri or "").startswith(prefix):
        ident = str(uri)[len(prefix):]
        marker = "/files/"
        if marker in ident:
            source_id, file_id = ident.rsplit(marker, 1)
            value = upstream_sources.get_file(source_id, file_id)
            return {"contents": [{"uri": uri, "mimeType": value.get("mime", "text/plain"),
                                  "text": value.get("content", "")}]}
        value = upstream_sources.get_source(ident)
        value.pop("source_files", None)
        return {"contents": [{"uri": uri, "mimeType": "application/json",
                              "text": json.dumps(_scrub_resource(value), ensure_ascii=False)}]}
    return _read_skill_resource(uri)


def _prompt_descriptors():
    descriptors = []
    # Every discoverable Skill is a first-class MCP prompt.  The legacy aliases
    # are accepted only by prompts/get and are deliberately not advertised.
    for skill in _skills_registry().list():
        descriptors.append({"name": skill.id, "description": skill.description,
                            "arguments": [
                                {"name": "markdown_content", "description": "Document text", "required": False},
                                {"name": "request", "description": "User request", "required": False},
                                {"name": "language", "description": "Output language", "required": False},
                            ], "skill_id": skill.id})
    return descriptors


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
                            "version": VERSION
                        },
                        "capabilities": {"tools": {}, "resources": {}, "prompts": {}}
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
            elif method == "resources/list":
                res = {"jsonrpc": "2.0", "id": req_id, "result": {"resources": _all_resources()}}
            elif method == "resources/read":
                params = req.get("params", {})
                res = {"jsonrpc": "2.0", "id": req_id, "result": _read_resource(params.get("uri"))}
            elif method == "prompts/list":
                res = {"jsonrpc": "2.0", "id": req_id, "result": {"prompts": _prompt_descriptors()}}
            elif method == "prompts/get":
                params = req.get("params", {})
                requested = str(params.get("name") or "readmd-quick-read")
                skill_id = _resolve_skill_id(requested)
                skill = _skills_registry().get(skill_id)
                if skill is None:
                    raise SkillError("Skill not found: " + skill_id)
                name = skill.name
                arguments = params.get("arguments") or {}
                doc = str(arguments.get("markdown_content") or "(no document supplied)")
                prompt = _skills_registry().render(skill_id, {
                    "document": doc, "selection": "", "request": str(arguments.get("request") or ""),
                    "language": str(arguments.get("language") or "en"), "context": "", "output_format": "Markdown",
                })
                res = {"jsonrpc": "2.0", "id": req_id, "result": {"description": name,
                    "messages": [{"role": "user", "content": {"type": "text", "text": prompt}}]}}
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
