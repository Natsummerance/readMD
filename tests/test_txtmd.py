# -*- coding: utf-8 -*-
"""ReadMD TXT 智能转 Markdown (src.readmd_modules.txtmd) 单元测试。"""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.txtmd import to_markdown, _heading_level, _table_cells


class TestTxtMdConversion(unittest.TestCase):
    """测试纯文本结构化转 Markdown。"""

    def test_chinese_chapter_headings(self):
        """测试中文章节编号识别为标题。"""
        text = "第一章 绪论\n这里是正文段落。\n\n第二节 相关工作\n这是第二节内容。"
        md, stats = to_markdown(text)
        self.assertIn("# 第一章 绪论", md)
        self.assertIn("## 第二节 相关工作", md)
        self.assertTrue(stats['headings'] >= 2)

    def test_numbered_headings(self):
        """测试数字分级编号识别。"""
        text = "1. 背景介绍\n内容一。\n\n1.1 技术架构\n架构说明。\n\n1.1.1 核心算法\n算法细节。"
        md, stats = to_markdown(text)
        self.assertIn("## 1. 背景介绍", md)
        self.assertIn("### 1.1 技术架构", md)
        self.assertIn("#### 1.1.1 核心算法", md)

    def test_bullet_list_conversion(self):
        """测试不同项目符号统一转换为标准列表。"""
        text = "• 第一项\n· 第二项\n▪ 第三项\n◦ 第四项"
        md, stats = to_markdown(text)
        self.assertIn("- 第一项", md)
        self.assertIn("- 第二项", md)
        self.assertIn("- 第三项", md)
        self.assertIn("- 第四项", md)

    def test_tab_and_space_tables(self):
        """测试 Tab 与多空格对齐转 Markdown 表格。"""
        text = "名称\t数量\t价格\n苹果\t10\t5.5\n香蕉\t20\t3.0"
        md, stats = to_markdown(text)
        self.assertIn("| 名称 | 数量 | 价格 |", md)
        self.assertIn("| --- | --- | --- |", md)
        self.assertIn("| 苹果 | 10 | 5.5 |", md)
        self.assertTrue(stats['tables'] >= 1)

    def test_table_cell_splitting(self):
        """测试单行表格单元格切分。"""
        cells_tab = _table_cells("A\tB\tC")
        self.assertEqual(cells_tab, ['A', 'B', 'C'])

        cells_space = _table_cells("Col1    Col2    Col3")
        self.assertEqual(cells_space, ['Col1', 'Col2', 'Col3'])


if __name__ == '__main__':
    unittest.main()
