# -*- coding: utf-8 -*-
"""ReadMD v2.3.5 全面功能自愈与竞品功能入口单元测试套件。"""

import os
import sys
import unittest
import json

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core import readmd_fix
from src.readmd_core.toc_engine import process_toc_markers
from src.readmd_modules.import_processor import process_markdown_imports
from src.readmd_modules.code_chunk_runner import execute_python_chunk, execute_code_chunk
from src.readmd_modules.mdexport.epub_render import export_epub
from src.readmd_modules.mdexport.presentation_render import render_presentation_html

class TestV235Features(unittest.TestCase):
    def test_cjk_bold_bracket_balancing(self):
        """测试 CJK 中文全角括号/书名号与加粗标记的自愈与平衡。"""
        # 测试包含中文括号的加粗
        raw1 = "**A. DashboardController 接口开发** 、**周一（2026-08-24）- 性能优化启动**"
        res1 = readmd_fix.fix_markdown(raw1)
        fixed1 = res1.text
        self.assertIn("**A. DashboardController 接口开发**", fixed1)
        self.assertIn("**周一（2026-08-24）- 性能优化启动**", fixed1)

        # 测试不平衡加粗修复
        raw2 = "**不平衡加粗片段"
        res2 = readmd_fix.fix_markdown(raw2)
        fixed2 = res2.text
        self.assertTrue(fixed2.endswith("**") or "**不平衡加粗片段**" in fixed2)

    def test_table_preservation_and_fixing(self):
        """测试表格语法自愈与完整性保留。"""
        raw_table = "| 列1 | 列2 |\n|---|---|\n| 值1 | 值2 |"
        res_table = readmd_fix.fix_markdown(raw_table)
        fixed_table = res_table.text
        self.assertIn("| 列1 | 列2 |", fixed_table)
        self.assertIn("| 值1 | 值2 |", fixed_table)

    def test_code_chunk_execution(self):
        """测试安全 Python 代码块沙箱执行与输出捕获。"""
        code = "print('Hello ReadMD v2.3.5!')\nfor i in range(3): print(i)"
        res = execute_python_chunk(code)
        self.assertTrue(res.get("ok"))
        self.assertIn("Hello ReadMD v2.3.5!", res.get("stdout"))
        self.assertIn("0\n1\n2", res.get("stdout"))

    def test_presentation_export(self):
        """测试 Reveal.js 演说模式渲染。"""
        slides_md = "# Slide 1\n\nContent 1\n\n<!-- slide -->\n\n# Slide 2\n\nContent 2"
        html = render_presentation_html(slides_md, title="Test Presentation")
        self.assertIn("assets/vendor/reveal/dist/readmd-boot.js", html)
        self.assertIn("Slide 1", html)
        self.assertIn("Slide 2", html)

    def test_import_processor_nested(self):
        """测试 @import 模块化引用展平。"""
        content = '# Main Doc\n\n@import "nonexistent_sub.md"'
        flattened = process_markdown_imports(content, base_dir=os.getcwd())
        self.assertIn("Main Doc", flattened)

if __name__ == "__main__":
    unittest.main()
