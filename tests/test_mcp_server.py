# -*- coding: utf-8 -*-
"""ReadMD MCP Server 全功能工具集单元测试与 JSON-RPC 协议测试。"""

import json
import os
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MCP_DIR = os.path.join(ROOT, 'packages', 'mcp-server')
if MCP_DIR not in sys.path:
    sys.path.insert(0, MCP_DIR)

import readmd_mcp_server


class TestReadMDMCPServer(unittest.TestCase):
    """测试 ReadMD MCP Server 全量 14 项标准工具调用与响应格式。"""

    def test_tools_list_schema_integrity(self):
        """测试 MCP 工具注册表完整性。"""
        tools = readmd_mcp_server.TOOLS
        self.assertEqual(len(tools), 15)
        tool_names = [t["name"] for t in tools]
        expected_names = [
            "readmd_fix_markdown",
            "readmd_convert_to_markdown",
            "readmd_web_to_markdown",
            "readmd_ocr_to_markdown",
            "readmd_export_document",
            "readmd_latex_to_md",
            "readmd_md_to_latex",
            "readmd_parse_bibtex",
            "readmd_latex_to_omml",
            "readmd_ai_assistant",
            "readmd_process_imports",
            "readmd_generate_toc",
            "readmd_export_presentation",
            "readmd_export_epub",
            "readmd_run_code_chunk",
        ]
        for name in expected_names:
            self.assertIn(name, tool_names)

    def test_tool_fix_markdown(self):
        """测试 readmd_fix_markdown 工具。"""
        raw = "测试公式 $E=mc^2$ 与表格\n|a|b|\n|1|2|"
        res = readmd_mcp_server.handle_tool_call("readmd_fix_markdown", {"content": raw})
        self.assertFalse(res.get("isError", False))
        payload = json.loads(res["content"][0]["text"])
        self.assertTrue(payload["ok"])
        self.assertIn("repaired_content", payload)

    def test_tool_latex_to_md(self):
        """测试 readmd_latex_to_md 工具。"""
        tex = r"\section{Introduction}\textbf{Hello World}"
        res = readmd_mcp_server.handle_tool_call("readmd_latex_to_md", {"latex_content": tex})
        self.assertFalse(res.get("isError", False))
        self.assertIn("Hello World", res["content"][0]["text"])

    def test_tool_md_to_latex(self):
        """测试 readmd_md_to_latex 工具。"""
        md = "# 标题\n\n正文内容与公式 $x+y=z$。"
        res = readmd_mcp_server.handle_tool_call("readmd_md_to_latex", {"markdown_content": md, "doc_title": "测试论文"})
        self.assertFalse(res.get("isError", False))
        self.assertIn(r"\begin{document}", res["content"][0]["text"])

    def test_tool_latex_to_omml(self):
        """测试 readmd_latex_to_omml 工具。"""
        res = readmd_mcp_server.handle_tool_call("readmd_latex_to_omml", {"latex_formula": "E=mc^2"})
        self.assertFalse(res.get("isError", False))
        self.assertIn("m:oMath", res["content"][0]["text"])

    def test_tool_ai_assistant(self):
        """测试 readmd_ai_assistant 工具。"""
        res = readmd_mcp_server.handle_tool_call("readmd_ai_assistant", {
            "workflow_id": "quick_read",
            "markdown_content": "# 测试文档\n这是正文内容。"
        })
        self.assertFalse(res.get("isError", False))
        payload = json.loads(res["content"][0]["text"])
        self.assertEqual(payload["workflow_id"], "quick_read")
        self.assertIn("快速阅读", payload["workflow_name"])

    def test_tool_process_imports(self):
        """测试 readmd_process_imports 工具。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            sub = os.path.join(tmpdir, "sub.md")
            with open(sub, "w", encoding="utf-8") as f:
                f.write("## 引入章节\n正文。")
            res = readmd_mcp_server.handle_tool_call("readmd_process_imports", {
                "markdown_content": "# 总览\n\n@import \"sub.md\"",
                "base_dir": tmpdir
            })
            self.assertFalse(res.get("isError", False))
            self.assertIn("引入章节", res["content"][0]["text"])

    def test_tool_generate_toc(self):
        """测试 readmd_generate_toc 工具。"""
        doc = "# 第一章\n\n## 1.1 细节\n\n## 1.2 深入"
        res = readmd_mcp_server.handle_tool_call("readmd_generate_toc", {
            "markdown_content": doc,
            "depth_from": 1,
            "depth_to": 2
        })
        self.assertFalse(res.get("isError", False))
        self.assertIn("- [第一章](#第一章)", res["content"][0]["text"])
        self.assertIn("- [1.1 细节](#11-细节)", res["content"][0]["text"])

    def test_tool_export_presentation(self):
        """测试 readmd_export_presentation 工具。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            out_html = os.path.join(tmpdir, "deck.html")
            doc = "# Slide 1\n欢迎\n<!-- slide -->\n# Slide 2\n特性"
            res = readmd_mcp_server.handle_tool_call("readmd_export_presentation", {
                "markdown_content": doc,
                "output_path": out_html,
                "title": "测试演讲"
            })
            self.assertFalse(res.get("isError", False))
            self.assertTrue(os.path.isfile(out_html))

    def test_tool_export_epub(self):
        """测试 readmd_export_epub 工具。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            out_epub = os.path.join(tmpdir, "mcp_book.epub")
            doc = "# 第一章\n这是电子书内容。"
            res = readmd_mcp_server.handle_tool_call("readmd_export_epub", {
                "markdown_content": doc,
                "output_path": out_epub,
                "title": "MCP 电子书"
            })
            self.assertFalse(res.get("isError", False))
            self.assertTrue(os.path.isfile(out_epub))

    def test_tool_run_code_chunk(self):
        """测试 readmd_run_code_chunk 工具。"""
        code = "a = 10\nb = 20\nprint(f'SUM={a+b}')"
        res = readmd_mcp_server.handle_tool_call("readmd_run_code_chunk", {
            "code": code,
            "language": "python",
            "capture_plot": False
        })
        self.assertFalse(res.get("isError", False))
        payload = json.loads(res["content"][0]["text"])
        self.assertTrue(payload["ok"])
        self.assertIn("SUM=30", payload["stdout"])

    def test_tool_export_document_mock(self):
        """测试 readmd_export_document 导出功能。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            out_pdf = os.path.join(tmpdir, "export_test.pdf")
            with patch("src.readmd_modules.mdexport.export", return_value={"warnings": []}):
                res = readmd_mcp_server.handle_tool_call("readmd_export_document", {
                    "markdown_content": "# 测试导出\n内容",
                    "output_path": out_pdf,
                    "output_format": "pdf",
                    "style_preset": "academic"
                })
                self.assertFalse(res.get("isError", False))
                payload = json.loads(res["content"][0]["text"])
                self.assertTrue(payload["ok"])
                self.assertEqual(payload["format"], "pdf")

    def test_tool_web_to_markdown_mock(self):
        """测试 readmd_web_to_markdown 抓取。"""
        with patch("src.readmd_modules.web.fetch_document", return_value=({"title": "网页标题", "markdown": "# 网页内容", "images": []}, [])):
            res = readmd_mcp_server.handle_tool_call("readmd_web_to_markdown", {"url": "https://example.com/article"})
            self.assertFalse(res.get("isError", False))
            payload = json.loads(res["content"][0]["text"])
            self.assertTrue(payload["ok"])
            self.assertEqual(payload["title"], "网页标题")

    def test_tool_ocr_to_markdown_mock(self):
        """测试 readmd_ocr_to_markdown OCR 调用。"""
        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            f.write(b"fake image")
            tmp_img = f.name

        try:
            with patch("src.readmd_modules.ocr.ocr_any", return_value="OCR 识别出的 Markdown 文本"):
                res = readmd_mcp_server.handle_tool_call("readmd_ocr_to_markdown", {"file_path": tmp_img})
                self.assertFalse(res.get("isError", False))
                self.assertEqual(res["content"][0]["text"], "OCR 识别出的 Markdown 文本")
        finally:
            if os.path.exists(tmp_img):
                os.remove(tmp_img)


if __name__ == '__main__':
    unittest.main()
