# -*- coding: utf-8 -*-
"""ReadMD 自动检查更新与资产下载模块 (src.readmd_modules.updater) 单元测试。"""

import json
import os
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import updater


class TestUpdaterModule(unittest.TestCase):
    """测试语义化版本比对、Release 资产匹配、SHA256 校验与更新状态机。"""

    def test_parse_semver(self):
        """测试语义化版本解析。"""
        self.assertEqual(updater.parse_semver('v2.3.3'), (2, 3, 3))
        self.assertEqual(updater.parse_semver('2.3.0'), (2, 3, 0))
        self.assertEqual(updater.parse_semver('v3.0.0-beta'), (3, 0, 0))
        self.assertEqual(updater.parse_semver(''), (0, 0, 0))
        self.assertEqual(updater.parse_semver('invalid'), (0, 0, 0))

    def test_is_newer_version(self):
        """测试新旧版本比较。"""
        self.assertTrue(updater.is_newer_version('v2.3.4', 'v2.3.3'))
        self.assertTrue(updater.is_newer_version('v3.0.0', 'v2.9.9'))
        self.assertFalse(updater.is_newer_version('v2.3.3', 'v2.3.3'))
        self.assertFalse(updater.is_newer_version('v2.3.2', 'v2.3.3'))

    def test_detect_app_flavor(self):
        """测试各操作系统平台打包形态识别。"""
        with patch('sys.platform', 'darwin'):
            self.assertEqual(updater.detect_app_flavor(), 'macos')

        with patch('sys.platform', 'linux'):
            self.assertEqual(updater.detect_app_flavor(), 'linux')

        with patch('sys.platform', 'win32'), patch.object(sys, 'frozen', True, create=True), patch('sys.executable', r'C:\ReadMD\ReadMD-Portable.exe'):
            self.assertEqual(updater.detect_app_flavor(), 'win_portable')

        with patch('sys.platform', 'win32'), patch.object(sys, 'frozen', True, create=True), patch('sys.executable', r'C:\ReadMD\ReadMD.exe'):
            self.assertEqual(updater.detect_app_flavor(), 'win_installer')

    def test_match_release_asset(self):
        """测试根据不同平台自动匹配对应架构的安装包。"""
        assets = [
            {'name': 'ReadMD-Setup-2.3.4.exe', 'browser_download_url': 'http://example.com/setup.exe', 'size': 50000000},
            {'name': 'ReadMD-Portable-2.3.4.zip', 'browser_download_url': 'http://example.com/portable.zip', 'size': 45000000},
            {'name': 'ReadMD-2.3.4-macOS-arm64.dmg', 'browser_download_url': 'http://example.com/mac-arm64.dmg', 'size': 40000000},
            {'name': 'ReadMD-2.3.4-macOS-x64.dmg', 'browser_download_url': 'http://example.com/mac-x64.dmg', 'size': 40000000},
            {'name': 'SHA256SUMS.txt', 'browser_download_url': 'http://example.com/sha256.txt', 'size': 500},
        ]

        # Windows 安装版
        asset_win, sha_win = updater.match_release_asset(assets, flavor='win_installer')
        self.assertIsNotNone(asset_win)
        self.assertEqual(asset_win['name'], 'ReadMD-Setup-2.3.4.exe')
        self.assertIsNotNone(sha_win)

        # Windows 便携版
        asset_port, _ = updater.match_release_asset(assets, flavor='win_portable')
        self.assertEqual(asset_port['name'], 'ReadMD-Portable-2.3.4.zip')

        # macOS ARM
        with patch('platform.machine', return_value='arm64'):
            asset_mac, _ = updater.match_release_asset(assets, flavor='macos')
            self.assertEqual(asset_mac['name'], 'ReadMD-2.3.4-macOS-arm64.dmg')

    def test_compute_file_sha256(self):
        """测试本地文件 SHA256 哈希计算。"""
        with tempfile.NamedTemporaryFile(suffix='.bin', mode='wb', delete=False) as f:
            f.write(b'ReadMD Test Binary Data 12345')
            tmp_path = f.name

        try:
            import hashlib
            expected = hashlib.sha256(b'ReadMD Test Binary Data 12345').hexdigest().lower()
            actual = updater.compute_file_sha256(tmp_path)
            self.assertEqual(actual, expected)
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_check_update_mock(self):
        """测试通过 Mock GitHub API 模拟检查新版本。"""
        mock_release = {
            'tag_name': 'v2.4.0',
            'name': 'ReadMD 2.4.0 正式版',
            'body': '更新日志：修复已知问题并提升渲染速度',
            'published_at': '2026-08-20T12:00:00Z',
            'html_url': 'https://github.com/Natsummerance/readMD/releases/tag/v2.4.0',
            'assets': [
                {'name': 'ReadMD-Setup-2.4.0.exe', 'browser_download_url': 'http://example.com/setup.exe', 'size': 123456}
            ]
        }

        with patch('src.readmd_modules.updater._fetch_release_json', return_value=mock_release):
            res = updater.check_update(current_version='2.3.3')
            self.assertTrue(res.get('ok'))
            self.assertTrue(res.get('has_update'))
            self.assertEqual(res.get('latest_version'), 'v2.4.0')

    def test_resolve_expected_sha_parses_checksum_manifest(self):
        """The matching SHA256SUMS row is used as the download integrity hash."""
        manifest = (
            '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  ReadMD-portable-v1.exe\n'
            '*0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  ReadMDSetup-v1.exe\n'
        )
        with patch('src.readmd_modules.updater._fetch_text', return_value=manifest):
            actual = updater.resolve_expected_sha(
                'http://example.com/SHA256SUMS.txt', 'ReadMDSetup-v1.exe'
            )
        self.assertEqual(actual, '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

    def test_check_update_embeds_expected_sha(self):
        """Update checks expose the matched binary's expected SHA-256."""
        mock_release = {
            'tag_name': 'v2.4.0',
            'html_url': 'https://example.com/release',
            'assets': [
                {'name': 'ReadMDSetup-v2.4.0.exe', 'browser_download_url': 'http://example.com/setup.exe', 'size': 123456},
                {'name': 'SHA256SUMS.txt', 'browser_download_url': 'http://example.com/SHA256SUMS.txt', 'size': 500},
            ],
        }
        manifest = (
            '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  ReadMDSetup-v2.4.0.exe\n'
            'other  ReadMDPortable.zip\n'
        )

        def fake_fetch(url, timeout=5):
            if url == 'http://example.com/SHA256SUMS.txt':
                return manifest
            return mock_release

        with patch('src.readmd_modules.updater._fetch_release_json', side_effect=fake_fetch), \
             patch('src.readmd_modules.updater._fetch_text', return_value=manifest):
            res = updater.check_update(current_version='2.3.3')
        self.assertEqual(res['asset']['expected_sha'], '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

    def test_check_update_already_latest(self):
        """测试已是最新版。"""
        mock_release = {
            'tag_name': 'v2.3.3',
            'name': 'ReadMD 2.3.3',
            'body': '当前版本',
            'published_at': '2026-08-20T12:00:00Z',
            'html_url': 'https://github.com/Natsummerance/readMD/releases/tag/v2.3.3',
            'assets': []
        }
        with patch('src.readmd_modules.updater._fetch_release_json', return_value=mock_release):
            res = updater.check_update(current_version='2.3.3')
            self.assertTrue(res.get('ok'))
            self.assertFalse(res.get('has_update'))


if __name__ == '__main__':
    unittest.main()
