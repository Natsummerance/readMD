# -*- coding: utf-8 -*-
"""ReadMD 凭据加密模块 (src.readmd_modules.crypto) 单元测试。"""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import crypto


class TestCryptoModule(unittest.TestCase):
    """测试 API Key 的加密、解密与优雅降级。"""

    def test_encrypt_and_decrypt_roundtrip(self):
        """测试加密与解密往返一致性。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            key_path = os.path.join(tmpdir, 'test.key')
            raw_key = 'sk-test-1234567890abcdef'

            if crypto.is_crypto_available():
                encrypted = crypto.encrypt_api_key(raw_key, key_path=key_path)
                self.assertTrue(encrypted.startswith('enc:'))
                self.assertNotEqual(encrypted, raw_key)
                self.assertTrue(os.path.isfile(key_path))

                decrypted = crypto.decrypt_api_key(encrypted, key_path=key_path)
                self.assertEqual(decrypted, raw_key)
            else:
                # 降级模式
                encrypted = crypto.encrypt_api_key(raw_key, key_path=key_path)
                self.assertEqual(encrypted, raw_key)
                decrypted = crypto.decrypt_api_key(encrypted, key_path=key_path)
                self.assertEqual(decrypted, raw_key)

    def test_empty_key_handling(self):
        """测试空 key 处理。"""
        self.assertEqual(crypto.encrypt_api_key(''), '')
        self.assertEqual(crypto.decrypt_api_key(''), '')

    def test_plaintext_pass_through(self):
        """测试未加密明文自动透传。"""
        plaintext = 'sk-legacy-unencrypted-key'
        self.assertEqual(crypto.decrypt_api_key(plaintext), plaintext)

    def test_invalid_token_handling(self):
        """测试损坏密文解密容错。"""
        if crypto.is_crypto_available():
            with tempfile.TemporaryDirectory() as tmpdir:
                key_path = os.path.join(tmpdir, 'test.key')
                bad_cipher = 'enc:invalid_corrupted_ciphertext'
                result = crypto.decrypt_api_key(bad_cipher, key_path=key_path)
                self.assertEqual(result, '')


if __name__ == '__main__':
    unittest.main()
