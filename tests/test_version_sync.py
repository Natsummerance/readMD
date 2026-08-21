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


if __name__ == '__main__':
    unittest.main()
