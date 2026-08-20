# -*- coding: utf-8 -*-
"""ReadMD 核心工具函数模块 (src.readmd_core.utils) 单元测试。"""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core import utils


class TestReadmdCoreUtils(unittest.TestCase):
    """测试 JSON 存取、文件读取、对话框路径规范化。"""

    def test_json_save_and_load_roundtrip(self):
        """测试 JSON 写入与读取。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            json_file = os.path.join(tmpdir, 'subdir', 'test.json')
            data = {'key': 'value', 'nested': [1, 2, 3], 'chinese': '测试中文'}
            
            ok = utils.save_json(json_file, data)
            self.assertTrue(ok)
            self.assertTrue(os.path.isfile(json_file))

            loaded = utils.load_json(json_file)
            self.assertEqual(loaded, data)

    def test_load_json_fallback(self):
        """测试 JSON 读取失败时的默认值回退。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            bad_file = os.path.join(tmpdir, 'bad.json')
            with open(bad_file, 'w', encoding='utf-8') as f:
                f.write('{ invalid json content }')

            self.assertEqual(utils.load_json(bad_file, default={'default': 1}), {'default': 1})
            self.assertIsNone(utils.load_json(os.path.join(tmpdir, 'nonexistent.json')))

    def test_read_text(self):
        """测试多编码文本读取。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # UTF-8
            utf8_file = os.path.join(tmpdir, 'utf8.txt')
            with open(utf8_file, 'w', encoding='utf-8') as f:
                f.write('你好，世界！')
            self.assertEqual(utils.read_text(utf8_file), '你好，世界！')

            # GBK
            gbk_file = os.path.join(tmpdir, 'gbk.txt')
            with open(gbk_file, 'w', encoding='gbk') as f:
                f.write('简体中文GBK测试')
            self.assertEqual(utils.read_text(gbk_file), '简体中文GBK测试')

            # Nonexistent
            self.assertEqual(utils.read_text(os.path.join(tmpdir, 'no.txt'), 'default'), 'default')

    def test_normalize_dialog_path(self):
        """测试 pywebview 对话框路径规范化。"""
        self.assertIsNone(utils.normalize_dialog_path(None))
        self.assertIsNone(utils.normalize_dialog_path(''))
        self.assertIsNone(utils.normalize_dialog_path([]))

        # Windows 单元素元组
        norm = utils.normalize_dialog_path(('C:\\test\\doc.md',))
        self.assertEqual(norm, os.path.abspath('C:\\test\\doc.md'))

        # macOS 字符串
        norm_str = utils.normalize_dialog_path('/Users/test/doc')
        self.assertEqual(norm_str, os.path.abspath('/Users/test/doc'))

        # 自动补全扩展名
        norm_ext = utils.normalize_dialog_path('/Users/test/doc', extension='.md')
        self.assertTrue(norm_ext.endswith('.md'))

        # 异常路径返回校验
        with self.assertRaises(ValueError):
            utils.normalize_dialog_path(('path1', 'path2'))


if __name__ == '__main__':
    unittest.main()
