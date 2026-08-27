# -*- coding: utf-8 -*-
"""Unit test to ensure all platform descriptors are 100% in sync with .env version."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from tools.sync_version import load_env_version, sync_all


class TestVersionSync(unittest.TestCase):
    """测试统一版本号配置体系与跨平台描述文件一致性。"""

    def test_version_files_in_sync(self):
        ver = load_env_version()
        self.assertTrue(ver, "READMD_VERSION must not be empty in .env or environment")
        in_sync = sync_all(ver, check_only=True)
        self.assertTrue(in_sync, f"All platform version descriptors must be in sync with {ver}")

    def test_env_and_example_matrix_variables(self):
        """测试 .env 与 .env.example 包含完整的全平台衍生版本号变量。"""
        for fn in ['.env', '.env.example']:
            fpath = os.path.join(ROOT, fn)
            self.assertTrue(os.path.isfile(fpath), f"{fn} must exist")
            with open(fpath, 'r', encoding='utf-8') as f:
                content = f.read()
            self.assertIn('READMD_VERSION=', content)
            self.assertIn('READMD_VERSION_TAG=', content)
            self.assertIn('READMD_VERSION_SEMVER=', content)
            self.assertIn('READMD_VERSION_TRIPLET=', content)
            self.assertIn('READMD_VERSION_LINGLONG=', content)
            self.assertIn('READMD_VERSION_HARMONY_CODE=', content)
            self.assertIn('READMD_VERSION_HARMONY_NAME=', content)
            self.assertIn('READMD_VERSION_WINDOWS_FILEVER=', content)
            self.assertIn('READMD_VERSION_VSCODE=', content)

    def test_mcp_server_and_frontend_version_sync(self):
        """测试 MCP Server 与前端 HTML/JS 版本与配置保持一致。"""
        ver = load_env_version()
        
        mcp_path = os.path.join(ROOT, 'packages', 'mcp-server', 'readmd_mcp_server.py')
        with open(mcp_path, 'r', encoding='utf-8') as f:
            mcp_src = f.read()
        self.assertIn(f'"version": "{ver}"', mcp_src)

        index_path = os.path.join(ROOT, 'assets', 'index.html')
        with open(index_path, 'r', encoding='utf-8') as f:
            index_src = f.read()
        self.assertIn(f'data-version="{ver}"', index_src)
        self.assertIn(f'v{ver}', index_src)


if __name__ == '__main__':
    unittest.main()
