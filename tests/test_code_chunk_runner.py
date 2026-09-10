# -*- coding: utf-8 -*-
"""Unit tests for ReadMD safe code chunk runner."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.code_chunk_runner import execute_python_chunk, execute_code_chunk


class TestCodeChunkRunner(unittest.TestCase):
    """测试安全代码块执行器。"""

    def test_simple_python_stdout(self):
        """测试基础输出捕获。"""
        code = "print('Hello from ReadMD!')\nprint(1 + 2)"
        res = execute_python_chunk(code, capture_plot=False)
        self.assertTrue(res["ok"])
        self.assertIn("Hello from ReadMD!", res["stdout"])
        self.assertIn("3", res["stdout"])

    def test_python_stderr(self):
        """测试异常捕获。"""
        code = "raise ValueError('测试异常隔离')"
        res = execute_python_chunk(code, capture_plot=False)
        self.assertFalse(res["ok"])
        self.assertIn("ValueError: 测试异常隔离", res["stderr"])

    def test_timeout_protection(self):
        """测试超时阻断保护。"""
        code = "import time\ntime.sleep(5)"
        res = execute_python_chunk(code, capture_plot=False, timeout=1)
        self.assertFalse(res["ok"])
        self.assertIn("代码执行超时", res["error"])

    def test_sql_semicolon_in_string(self):
        """测试 SQL 字符串内包含分号时不会被错误切分 (BUG-006)。"""
        sql = "SELECT 'hello;world' AS msg, \"foo;bar\" AS msg2;"
        res = execute_code_chunk(sql, lang="sql")
        self.assertTrue(res["ok"], msg=f"SQL failed: {res.get('error')}")
        self.assertIn("hello;world", res["stdout"])
        self.assertIn("foo;bar", res["stdout"])

    def test_sql_timeout_protection(self):
        """测试 SQL 无限递归查询受到超时阻断保护 (BUG-003)。"""
        sql = "WITH RECURSIVE r(i) AS (VALUES(0) UNION ALL SELECT i+1 FROM r) SELECT count(*) FROM r;"
        res = execute_code_chunk(sql, lang="sql", timeout=1)
        self.assertFalse(res["ok"])
        self.assertIn("超时", res["error"])

    def test_timeout_output_truncation(self):
        """测试超时分支依然具备输出截断与结构完整性保护 (BUG-005)。"""
        code = "import sys, time\nsys.stdout.write('early output\\n')\nsys.stdout.flush()\ntime.sleep(5)"
        res = execute_python_chunk(code, capture_plot=False, timeout=1)
        self.assertFalse(res["ok"])
        self.assertEqual(res.get("error_code"), "execution_timeout")
        self.assertIn("early output", res["stdout"])


if __name__ == '__main__':
    unittest.main()
