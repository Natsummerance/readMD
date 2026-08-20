# -*- coding: utf-8 -*-
"""ReadMD 核心配置模块 (src.readmd_core.config) 单元测试。"""

import os
import sys
import unittest
from unittest.mock import patch, MagicMock

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core import config


class TestReadmdCoreConfig(unittest.TestCase):
    """测试跨平台数据目录、文件常量与系统语言检测。"""

    def test_constants_defined(self):
        """测试核心常量定义。"""
        self.assertTrue(hasattr(config, 'DATA_DIR'))
        self.assertTrue(hasattr(config, 'SETTINGS_FILE'))
        self.assertTrue(hasattr(config, 'RECENT_FILE'))
        self.assertTrue(hasattr(config, 'PROMPTS_FILE'))
        self.assertTrue(hasattr(config, 'HISTORY_FILE'))
        self.assertTrue(hasattr(config, 'LOG_FILE'))
        self.assertTrue(config.SETTINGS_FILE.endswith('settings.json'))
        self.assertTrue(config.RECENT_FILE.endswith('recent.json'))

    def test_platform_data_dir_darwin(self):
        """测试 macOS 平台数据目录。"""
        with patch('sys.platform', 'darwin'):
            path = config._platform_data_dir()
            self.assertIn(os.path.join('Library', 'Application Support', 'ReadMD'), path)

    def test_platform_data_dir_win32(self):
        """测试 Windows 平台数据目录。"""
        with patch('sys.platform', 'win32'), patch.dict(os.environ, {'APPDATA': r'C:\Users\Test\AppData\Roaming'}):
            path = config._platform_data_dir()
            self.assertEqual(path, r'C:\Users\Test\AppData\Roaming\ReadMD')

    def test_platform_data_dir_linux(self):
        """测试 Linux XDG 数据目录。"""
        with patch('sys.platform', 'linux'), patch.dict(os.environ, {'XDG_DATA_HOME': '/home/test/.local/share'}):
            path = config._platform_data_dir()
            self.assertEqual(path, os.path.join('/home/test/.local/share', 'ReadMD'))

    def test_get_system_language_locale_fallback(self):
        """测试语言检测回退。"""
        with patch('sys.platform', 'linux'), patch('locale.getdefaultlocale', return_value=('zh_CN', 'UTF-8')):
            lang = config.get_system_language()
            self.assertEqual(lang, 'zh-CN')

        with patch('sys.platform', 'linux'), patch('locale.getdefaultlocale', return_value=('en_US', 'UTF-8')):
            lang = config.get_system_language()
            self.assertEqual(lang, 'en')

        with patch('sys.platform', 'linux'), patch('locale.getdefaultlocale', return_value=('zh_HK', 'UTF-8')):
            lang = config.get_system_language()
            self.assertEqual(lang, 'zh-HK')


if __name__ == '__main__':
    unittest.main()
