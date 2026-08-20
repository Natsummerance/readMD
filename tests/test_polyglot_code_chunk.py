# -*- coding: utf-8 -*-
"""Unit tests for ReadMD Polyglot safe code chunk execution."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.code_chunk_runner import execute_code_chunk, execute_python_chunk


class TestPolyglotCodeChunk(unittest.TestCase):
    """测试多语言代码块安全执行引擎。"""

    def test_python_dispatch(self):
        """测试 Python 代码块调度。"""
        res = execute_code_chunk("print('Python MultiLang Test')", lang="python", capture_plot=False)
        self.assertTrue(res["ok"])
        self.assertIn("Python MultiLang Test", res["stdout"])
        self.assertEqual(res["lang"], "python")

    def test_shell_dispatch(self):
        """测试 Shell 代码块调度。"""
        code = "echo Shell_Running"
        res = execute_code_chunk(code, lang="cmd" if sys.platform == "win32" else "bash")
        self.assertTrue(res["ok"])
        self.assertIn("Shell_Running", res["stdout"])

    def test_unsupported_language_fallback(self):
        """测试未知语言优雅报错。"""
        res = execute_code_chunk("some code", lang="unknown_lang_xyz")
        self.assertFalse(res["ok"])
        self.assertIn("暂不支持的代码语言", res["error"])


if __name__ == '__main__':
    unittest.main()
