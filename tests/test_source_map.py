# -*- coding: utf-8 -*-
"""Unit tests for ReadMD AST source map line injector."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.source_map import (
    annotate_markdown_source_lines,
    inject_source_line_attributes_to_html,
)


class TestSourceMap(unittest.TestCase):
    """测试源码行号映射与属性注入。"""

    def test_annotate_markdown_source_lines(self):
        """测试 Markdown 块级行号注释生成。"""
        doc = "# 标题 (Line 1)\n\n段落文字 (Line 3)\n\n```python\ncode (Line 6)\n```"
        annotated = annotate_markdown_source_lines(doc)
        self.assertIn('<!-- data-source-line="1" -->', annotated)
        self.assertIn('<!-- data-source-line="3" -->', annotated)
        self.assertIn('<!-- data-source-line="5" -->', annotated)

    def test_inject_source_line_attributes_to_html(self):
        """测试将行号注释提升为 HTML 标签属性。"""
        raw_html = '<!-- data-source-line="10" -->\n<h2 class="section">标题</h2>\n<!-- data-source-line="15" -->\n<p>内容</p>'
        injected = inject_source_line_attributes_to_html(raw_html)
        self.assertIn('<h2 data-source-line="10" class="section">标题</h2>', injected)
        self.assertIn('<p data-source-line="15">内容</p>', injected)


if __name__ == '__main__':
    unittest.main()
