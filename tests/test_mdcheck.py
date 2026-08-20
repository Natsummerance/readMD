# -*- coding: utf-8 -*-
"""ReadMD Markdown 质量诊断模块 (src.readmd_modules.mdcheck) 单元测试。"""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.mdcheck import check


class TestMdCheck(unittest.TestCase):
    """测试 Markdown 语法自愈与警告检测。"""

    def test_check_empty_text(self):
        """测试空文本检查。"""
        fixed, issues = check("")
        self.assertEqual(fixed, "")
        self.assertEqual(issues, [])

    def test_check_unclosed_code_fence(self):
        """测试未闭合代码块自动闭合。"""
        text = "```python\nprint('hello')\n"
        fixed, issues = check(text)
        self.assertTrue(fixed.endswith("```\n") or fixed.endswith("```"))
        self.assertTrue(any(i.get('level') == 'auto' for i in issues))

    def test_check_odd_dollar_delimiters(self):
        """测试奇数个公式符号警告。"""
        text = "Here is a formula $x + y = z with missing closing dollar."
        fixed, issues = check(text)
        self.assertTrue(any(i.get('level') == 'warn' for i in issues))

    def test_check_clean_markdown(self):
        """测试规范 Markdown 零报错。"""
        text = "# 标题\n\n正文段落。\n\n- 列表1\n- 列表2\n\n$$\na^2 + b^2 = c^2\n$$\n"
        fixed, issues = check(text)
        warn_issues = [i for i in issues if i.get('level') == 'warn']
        self.assertEqual(warn_issues, [])


if __name__ == '__main__':
    unittest.main()
