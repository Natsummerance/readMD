# -*- coding: utf-8 -*-
"""V2.3.8 修复 #10：privacy_scan 应拦截真实文档文件名（防个人信息入库）。"""
import os
import sys
import tempfile
import unittest
import importlib.util

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

_spec = importlib.util.spec_from_file_location(
    "privacy_scan", os.path.join(ROOT_DIR, "tools", "privacy_scan.py"))
PS = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(PS)


class TestSensitiveDocumentNameDenylist(unittest.TestCase):
    """真实样本（含个人信息）一律不得进入项目，文件名命中即失败。"""

    def test_scan_file_flags_sensitive_document_name(self):
        with tempfile.TemporaryDirectory() as td:
            name = '北京交通大学软件学院毕业实习文档 （含实习记录表）.doc'
            path = os.path.join(td, name)
            with open(path, 'wb') as f:
                f.write(b'placeholder')
            failures = []
            PS.scan_file(path, name, failures)
        self.assertTrue(failures, '含真实文档名的文件必须被 privacy_scan 拦截')
        self.assertIn('sensitive real-document name', failures[0])

    def test_scan_file_allows_normal_document_name(self):
        with tempfile.TemporaryDirectory() as td:
            name = '实习记录示例.doc'
            path = os.path.join(td, name)
            with open(path, 'wb') as f:
                f.write(b'placeholder')
            failures = []
            PS.scan_file(path, name, failures)
        self.assertEqual(failures, [])


if __name__ == '__main__':
    unittest.main()
