# -*- coding: utf-8 -*-
"""ReadMD 安全测试：路径遍历、Shell 注入与 SSRF 防范。"""

import os
import sys
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


class TestSecurityCommandInjection(unittest.TestCase):
    """测试命令注入与路径遍历防范。"""

    def test_file_path_null_byte_rejection(self):
        """测试拦截包含 null byte 的恶意路径。"""
        with self.assertRaises(ValidationError):
            validate_file_path("document.md\x00.exe")

    def test_file_path_with_spaces_and_parentheses_allowed(self):
        """测试正常包含空格与括号的合法路径得到正常放行（修复 oc 的误杀）。"""
        p1 = validate_file_path(r"C:\Program Files (x86)\ReadMD\readme.md" if os.name == 'nt' else "/tmp/notes (1).md")
        self.assertTrue(len(p1) > 0)

    def test_command_injection_metacharacters_rejection(self):
        """测试拦截命令中的 Shell 元字符注入。"""
        with self.assertRaises(ValidationError):
            validate_command("notepad.exe ; calc.exe")

        with self.assertRaises(ValidationError):
            validate_command("code | rm -rf /")

    def test_safe_command_parsing(self):
        """测试合法命令参数正常切分与放行。"""
        cmd = validate_command(['explorer', 'C:\\test\\path'])
        self.assertEqual(cmd, ['explorer', 'C:\\test\\path'])

    def test_url_ssrf_blocking(self):
        """测试 SSRF 防护：当 allow_private=False 时拦截环回与私有地址。"""
        with self.assertRaises(ValidationError):
            validate_url("http://127.0.0.1:8080/admin", allow_private=False)

        with self.assertRaises(ValidationError):
            validate_url("http://localhost:3000/secret", allow_private=False)

        # 正常公网 URL 放行
        valid_public = validate_url("https://github.com/Natsummerance/readMD", allow_private=False)
        self.assertEqual(valid_public, "https://github.com/Natsummerance/readMD")

    def test_url_private_allowed_in_dev_mode(self):
        """测试本地开发与局域网共享模式下允许 127.0.0.1。"""
        local_url = validate_url("http://127.0.0.1:63528/article", allow_private=True)
        self.assertEqual(local_url, "http://127.0.0.1:63528/article")


if __name__ == '__main__':
    unittest.main()
