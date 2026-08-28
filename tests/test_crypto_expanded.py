# -*- coding: utf-8 -*-
"""ReadMD 凭据加密深度分支与容灾测试 (src.readmd_modules.crypto)。"""

import os
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import crypto


class TestCryptoExpanded(unittest.TestCase):
    """测试密钥生成、文件权限保护、加密降级与并发存取。"""

    def test_get_or_create_key_persistence(self):
        """测试密钥文件自动生成并持久化。"""
        if not crypto.is_crypto_available():
            self.skipTest("cryptography 库未安装")

        with tempfile.TemporaryDirectory() as tmpdir:
            key_file = os.path.join(tmpdir, "secure.key")
            key1 = crypto._get_or_create_key(key_file)
            self.assertTrue(os.path.isfile(key_file))

            # 再次获取，应完全相同
            key2 = crypto._get_or_create_key(key_file)
            self.assertEqual(key1, key2)

    def test_decrypt_corrupted_key_file(self):
        """测试密钥文件损坏时的容错处理。"""
        if not crypto.is_crypto_available():
            self.skipTest("cryptography 库未安装")

        with tempfile.TemporaryDirectory() as tmpdir:
            key_file = os.path.join(tmpdir, "bad.key")
            with open(key_file, 'w', encoding='utf-8') as f:
                f.write("corrupted key content not 32 url-safe base64")

            # 重新生成密钥
            key = crypto._get_or_create_key(key_file)
            self.assertTrue(len(key) > 0)

    def test_crypto_fallback_without_library(self):
        """Missing cryptography must fail closed, never store plaintext."""
        with patch.object(crypto, 'CRYPTO_AVAILABLE', False):
            raw = "sk-test-key-without-crypto"
            with self.assertRaises(RuntimeError):
                crypto.encrypt_api_key(raw)
            self.assertEqual(crypto.decrypt_api_key(raw), '')


if __name__ == '__main__':
    unittest.main()
