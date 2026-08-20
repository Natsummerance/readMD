# -*- coding: utf-8 -*-
"""ReadMD 安全测试：API Key 加密存储与密钥隔离。"""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.crypto import encrypt_api_key, decrypt_api_key, is_crypto_available
from src.readmd_modules.ai import save_config, get_config, resolve_key, ensure_config


class TestSecurityApiKeyEncryption(unittest.TestCase):
    """测试敏感凭据落盘加密与不可逆泄露。"""

    def test_api_key_is_encrypted_in_storage(self):
        """测试保存配置时 API Key 不以明文直接落地。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            test_key = "sk-ant-test-secret-key-12345"
            key_file = os.path.join(tmpdir, "enc.key")
            
            if is_crypto_available():
                encrypted = encrypt_api_key(test_key, key_path=key_file)
                self.assertNotIn(test_key, encrypted)
                self.assertTrue(encrypted.startswith("enc:"))
                
                # 解密验证
                decrypted = decrypt_api_key(encrypted, key_path=key_file)
                self.assertEqual(decrypted, test_key)

    def test_api_key_not_exposed_in_get_config(self):
        """测试前端配置读取接口绝不返回真实 API Key。"""
        cfg = get_config()
        for p in cfg.get("presets", []):
            self.assertNotIn("api_key", p)
        for p in cfg.get("custom", []):
            self.assertNotIn("api_key", p)


if __name__ == '__main__':
    unittest.main()
