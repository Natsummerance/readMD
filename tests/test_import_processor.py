# -*- coding: utf-8 -*-
"""Unit tests for ReadMD @import modular file import processor."""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.import_processor import (
    ImportProcessor,
    csv_to_markdown_table,
    parse_attributes,
    process_markdown_imports,
    slice_code_lines,
)


class TestImportProcessor(unittest.TestCase):
    """测试 @import 模块化处理器的各项能力。"""

    def test_parse_attributes(self):
        """测试属性键值解析。"""
        attrs = parse_attributes("line_begin=10 line_end=20 highlight=[15, 18] as_code=true lang=\"python\"")
        self.assertEqual(attrs["line_begin"], 10)
        self.assertEqual(attrs["line_end"], 20)
        self.assertEqual(attrs["highlight"], [15, 18])
        self.assertTrue(attrs["as_code"])
        self.assertEqual(attrs["lang"], "python")

    def test_csv_to_markdown_table(self):
        """测试 CSV 字符串转 Markdown 表格。"""
        raw_csv = "Name,Age,Role\nAlice,30,Engineer\nBob,25,Designer\n"
        table = csv_to_markdown_table(raw_csv)
        self.assertIn("| Name | Age | Role |", table)
        self.assertIn("| Alice | 30 | Engineer |", table)
        self.assertIn("| Bob | 25 | Designer |", table)

    def test_slice_code_lines(self):
        """测试源码行号切片。"""
        code = "line1\nline2\nline3\nline4\nline5"
        sliced = slice_code_lines(code, line_begin=2, line_end=4, lang="py")
        self.assertIn("```py\nline2\nline3\nline4\n```", sliced)

    def test_recursive_markdown_import(self):
        """测试嵌套子 Markdown 文件导入。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            sub1_path = os.path.join(tmpdir, "sub1.md")
            sub2_path = os.path.join(tmpdir, "sub2.md")
            main_path = os.path.join(tmpdir, "main.md")

            with open(sub2_path, "w", encoding="utf-8") as f:
                f.write("### Sub Chapter 2\nContent from sub2.")

            with open(sub1_path, "w", encoding="utf-8") as f:
                f.write("## Sub Chapter 1\nContent from sub1.\n\n@import \"sub2.md\"")

            with open(main_path, "w", encoding="utf-8") as f:
                f.write("# Main Book\n\n@import \"sub1.md\"")

            with open(main_path, "r", encoding="utf-8") as f:
                content = f.read()

            result = process_markdown_imports(content, base_dir=tmpdir, current_file=main_path)
            self.assertIn("Sub Chapter 1", result)
            self.assertIn("Sub Chapter 2", result)
            self.assertIn("Content from sub2.", result)

    def test_circular_import_defense(self):
        """测试循环依赖防御。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            file_a = os.path.join(tmpdir, "a.md")
            file_b = os.path.join(tmpdir, "b.md")

            with open(file_a, "w", encoding="utf-8") as f:
                f.write("# A\n@import \"b.md\"")

            with open(file_b, "w", encoding="utf-8") as f:
                f.write("# B\n@import \"a.md\"")

            with open(file_a, "r", encoding="utf-8") as f:
                content = f.read()

            result = process_markdown_imports(content, base_dir=tmpdir, current_file=file_a)
            self.assertIn("循环引用", result)

    def test_import_csv_and_diagrams(self):
        """测试导入外部 CSV 与 PUML 图表。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            csv_path = os.path.join(tmpdir, "stats.csv")
            puml_path = os.path.join(tmpdir, "arch.puml")
            main_path = os.path.join(tmpdir, "main.md")

            with open(csv_path, "w", encoding="utf-8") as f:
                f.write("Metric,Score\nSpeed,95\nQuality,99\n")

            with open(puml_path, "w", encoding="utf-8") as f:
                f.write("@startuml\nA -> B: Message\n@enduml")

            with open(main_path, "w", encoding="utf-8") as f:
                f.write("# System Overview\n\n@import \"stats.csv\"\n\n@import \"arch.puml\"")

            with open(main_path, "r", encoding="utf-8") as f:
                content = f.read()

            result = process_markdown_imports(content, base_dir=tmpdir, current_file=main_path)
            self.assertIn("| Metric | Score |", result)
            self.assertIn("```puml", result)


if __name__ == '__main__':
    unittest.main()
