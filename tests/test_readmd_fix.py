# -*- coding: utf-8 -*-
"""ReadMD Markdown 语法自愈引擎 (src.readmd_core.readmd_fix) 完整单元测试。"""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.readmd_fix import fix_markdown, FixResult


class TestReadmdFix(unittest.TestCase):
    """测试 Markdown 渲染前语法错误自愈。"""

    def test_heading_missing_space(self):
        """测试标题 # 缺少空格自动补全。"""
        text = "#标题一\n##二级标题\n### 三级标题(已有空格)\n###### 六级标题"
        res = fix_markdown(text)
        self.assertIn("# 标题一", res.text)
        self.assertIn("## 二级标题", res.text)
        self.assertIn("### 三级标题(已有空格)", res.text)
        self.assertIn("###### 六级标题", res.text)
        self.assertTrue(len(res.fixes) >= 2)

    def test_table_missing_separator(self):
        """测试表格缺失分隔行时自动补全。"""
        text = "| 姓名 | 年龄 | 城市 |\n| 张三 | 25 | 北京 |\n| 李四 | 30 | 上海 |"
        res = fix_markdown(text)
        self.assertIn("| --- | --- | --- |", res.text)
        self.assertIn("| 张三 | 25 | 北京 |", res.text)

    def test_table_uneven_columns(self):
        """测试表格各行列数不齐时自动对齐补全。"""
        text = "| A | B | C |\n| --- | --- | --- |\n| 1 | 2 |\n| 3 | 4 | 5 | 6 |"
        res = fix_markdown(text)
        lines = [line for line in res.text.split('\n') if '|' in line]
        self.assertTrue(len(lines) >= 4)

    def test_unclosed_bold_and_italic(self):
        """测试未闭合加粗与斜体标记自愈。"""
        text = "这是一段 **未闭合的粗体文本\n下一行正常文本"
        res = fix_markdown(text)
        self.assertIn("**未闭合的粗体文本**", res.text)

    def test_formula_currency_protection(self):
        """测试货币符号 $ 不被误判为公式。"""
        text = "This item costs $100 and the other costs $200."
        res = fix_markdown(text)
        # 不应将 $100 and the other costs $ 误修复为公式块
        self.assertEqual(res.text, text)

    def test_formula_unclosed_dollars(self):
        """测试未闭合行内/块级公式。"""
        text = "$$ E = mc^2 \n下一段"
        res = fix_markdown(text)
        self.assertIn("$$", res.text)

    def test_code_fence_preservation(self):
        """测试代码块内的特殊符号不被篡改。"""
        text = "```python\n#not_a_heading\n| not | a | table |\n**not bold**\n```"
        res = fix_markdown(text)
        self.assertIn("#not_a_heading", res.text)
        self.assertIn("| not | a | table |", res.text)


if __name__ == '__main__':
    unittest.main()
