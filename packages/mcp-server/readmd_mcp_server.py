# -*- coding: utf-8 -*-
"""ReadMD MCP Server —— 基于标准 Model Context Protocol (stdio) 的全功能 Markdown 与文档处理服务。

为 AI Agents (Claude Desktop, Cursor, Antigravity, VSCode 等) 提供：
1. `readmd_fix_markdown`: 自动诊断并修复 Markdown 格式错误（公式、表格、缩进、转义、HTML）；
2. `readmd_convert_to_markdown`: 将 Word (.docx), PDF (.pdf), PPT (.pptx), Excel (.xlsx), LaTeX (.tex), HTML, EPUB 等文档转为高质量 Markdown；
3. `readmd_latex_to_md`: LaTeX 源码精确转为标准 Markdown；
4. `readmd_md_to_latex`: Markdown 文档编译为标准学术 LaTeX；
5. `readmd_parse_bibtex`: 自动解析 BibTeX 文献数据库并生成学术引用映射；
6. `readmd_web_to_markdown`: 抓取网页并深度抽取清洗为 Markdown。
"""

import sys
import os
import json
import logging

# 引入 ReadMD 核心算法路径
ROOT_DIR = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

try:
    from src import readmd_fix
    from src.readmd_modules import texmd, bibtex
except ImportError:
    pass

TOOLS = [
    {
        "name": "readmd_fix_markdown",
        "description": "自动修复 Markdown 文本中的各类语法格式错误（公式断裂、表格错位、代码块未闭合、中英文标点空格等）。",
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
        "description": "将本地各种格式的文档（docx, pdf, pptx, xlsx, tex, html, epub 等）转换为高质量 Markdown。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "本地文档的绝对路径"}
            },
            "required": ["file_path"]
        }
    },
    {
        "name": "readmd_latex_to_md",
        "description": "将 LaTeX 论文或公式源码转换为干净的标准 Markdown 格式。",
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
        "description": "扫描并解析 BibTeX (.bib) 参考文献文件，提取结构化论文元数据与标准引用。",
        "inputSchema": {
            "type": "object",
            "properties": {
                "bib_file_path": {"type": "string", "description": ".bib 文件路径或包含 .bib 的目录路径"}
            },
            "required": ["bib_file_path"]
        }
    }
]


def handle_tool_call(name, args):
    if name == "readmd_fix_markdown":
        content = args.get("content", "")
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
        fp = args.get("file_path", "")
        if not os.path.isfile(fp):
            return {"isError": True, "content": [{"type": "text", "text": f"文件不存在: {fp}"}]}
        ext = os.path.splitext(fp)[1].lower()
        if ext == '.tex':
            with open(fp, 'r', encoding='utf-8', errors='replace') as f:
                raw_tex = f.read()
            md = texmd.latex_to_markdown(raw_tex)
            return {"content": [{"type": "text", "text": md}]}
        else:
            from src.readmd_modules import office
            md = office.convert_office_document(fp)
            return {"content": [{"type": "text", "text": md}]}

    elif name == "readmd_latex_to_md":
        latex_content = args.get("latex_content", "")
        md = texmd.latex_to_markdown(latex_content)
        return {"content": [{"type": "text", "text": md}]}

    elif name == "readmd_md_to_latex":
        md_content = args.get("markdown_content", "")
        title = args.get("doc_title", "ReadMD Document")
        tex = texmd.markdown_to_latex(md_content, title=title)
        return {"content": [{"type": "text", "text": tex}]}

    elif name == "readmd_parse_bibtex":
        bp = args.get("bib_file_path", "")
        res = bibtex.find_and_load_bib_for_file(bp)
        return {"content": [{"type": "text", "text": json.dumps(res, ensure_ascii=False, indent=2)}]}

    return {"isError": True, "content": [{"type": "text", "text": f"未知的工具名称: {name}"}]}


def run_stdio_server():
    """标准 JSON-RPC 2.0 stdio 通信循环。"""
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
                            "version": "2.3.0"
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
