# -*- coding: utf-8 -*-
"""ReadMD 输入安全校验完整分支覆盖测试 (src.readmd_modules.validators)。"""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.validators import (
    validate_file_path,
    validate_command,
    validate_url,
    ValidationError,
)


class TestValidatorsExpanded(unittest.TestCase):
    """测试路径扩展名白名单、目录限制、命令注入防护与 URL 协议校验。"""

    def test_validate_file_path_allowed_extensions(self):
        """测试扩展名白名单过滤。"""
        valid_path = validate_file_path("document.md", allowed_extensions=['.md', '.markdown'])
        self.assertTrue(valid_path.endswith('document.md'))

        with self.assertRaises(ValidationError):
            validate_file_path("script.exe", allowed_extensions=['.md'])

    def test_validate_file_path_allowed_dirs(self):
        """测试目录范围限制。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            allowed_subdir = os.path.join(tmpdir, "allowed")
            os.makedirs(allowed_subdir, exist_ok=True)

            safe_file = os.path.join(allowed_subdir, "test.md")
            validated = validate_file_path(safe_file, allowed_dirs=[allowed_subdir])
            self.assertEqual(validated, os.path.abspath(safe_file))

            # 跨目录访问拦截
            outside_file = os.path.join(tmpdir, "outside.md")
            with self.assertRaises(ValidationError):
                validate_file_path(outside_file, allowed_dirs=[allowed_subdir])

    def test_validate_file_path_rejects_prefix_only_sibling(self):
        """测试 allowed 前缀不能授权 allowed-secret 等同级目录。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            allowed_subdir = os.path.join(tmpdir, "allowed")
            prefix_sibling = os.path.join(tmpdir, "allowed-secret.md")
            os.makedirs(allowed_subdir, exist_ok=True)
            with open(prefix_sibling, "w", encoding="utf-8") as handle:
                handle.write("# sibling")

            with self.assertRaises(ValidationError):
                validate_file_path(prefix_sibling, allowed_dirs=[allowed_subdir])

    def test_validate_command_empty_and_types(self):
        """测试空命令或非列表/字符串参数报错。"""
        with self.assertRaises(ValidationError):
            validate_command("")
        with self.assertRaises(ValidationError):
            validate_command([])
        with self.assertRaises(ValidationError):
            validate_command(None)

    def test_validate_url_invalid_schemes_and_empty(self):
        """测试非 http/https 协议拦截。"""
        with self.assertRaises(ValidationError):
            validate_url("ftp://example.com/file.zip")
        with self.assertRaises(ValidationError):
            validate_url("file:///etc/passwd")
        with self.assertRaises(ValidationError):
            validate_url("javascript:alert(1)")
        with self.assertRaises(ValidationError):
            validate_url("")


if __name__ == '__main__':
    unittest.main()
