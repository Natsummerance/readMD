# -*- coding: utf-8 -*-
"""Unit tests for ReadMD @import PDF page slicing and LESS/TikZ import."""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.import_processor import process_markdown_imports


class TestPdfImportSlicing(unittest.TestCase):
    """测试 PDF 页码切片与 LESS/TikZ 样式文件导入。"""

    def test_import_less_file(self):
        """测试导入 LESS 样式文件。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            less_path = os.path.join(tmpdir, "theme.less")
            main_path = os.path.join(tmpdir, "main.md")

            with open(less_path, "w", encoding="utf-8") as f:
                f.write("@primary: #2563eb;\nbody { color: @primary; }")

            with open(main_path, "w", encoding="utf-8") as f:
                f.write("# 文档\n\n@import \"theme.less\"")

            with open(main_path, "r", encoding="utf-8") as f:
                content = f.read()

            res = process_markdown_imports(content, base_dir=tmpdir, current_file=main_path)
            self.assertIn('<style type="text/less">', res)
            self.assertIn('@primary: #2563eb;', res)

    def test_import_tikz_file(self):
        """测试导入 TikZ 矢量图源码。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            tikz_path = os.path.join(tmpdir, "circle.tikz")
            main_path = os.path.join(tmpdir, "main.md")

            with open(tikz_path, "w", encoding="utf-8") as f:
                f.write("\\draw (0,0) circle (2);")

            with open(main_path, "w", encoding="utf-8") as f:
                f.write("# 物理图\n\n@import \"circle.tikz\"")

            with open(main_path, "r", encoding="utf-8") as f:
                content = f.read()

            res = process_markdown_imports(content, base_dir=tmpdir, current_file=main_path)
            self.assertIn('<script type="text/tikz">', res)
            self.assertIn('\\draw (0,0) circle (2);', res)


if __name__ == '__main__':
    unittest.main()
