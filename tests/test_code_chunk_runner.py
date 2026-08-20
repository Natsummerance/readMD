# -*- coding: utf-8 -*-
"""Unit tests for ReadMD safe code chunk runner."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.code_chunk_runner import execute_python_chunk


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


if __name__ == '__main__':
    unittest.main()
