#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""安全测试：API Key加密存储"""

import os
import sys
import pytest
import tempfile
import json

# 添加项目根目录到路径
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


class TestAPIKeyEncryption:
    """测试API Key加密功能"""
    
    def test_encrypt_decrypt_roundtrip(self):
        """测试加密解密往返"""
        from src.readmd_modules.crypto import encrypt_api_key, decrypt_api_key
        
        original_key = "sk-test-1234567890abcdef1234567890abcdef"
        
        # 加密
        encrypted = encrypt_api_key(original_key)
        
        # 验证加密后的值与原始值不同
        assert encrypted != original_key
        assert len(encrypted) > len(original_key)
        
        # 解密
        decrypted = decrypt_api_key(encrypted)
        
        # 验证解密后的值与原始值相同
        assert decrypted == original_key
    
    def test_different_keys_produce_different_ciphertext(self):
        """测试相同的明文产生不同的密文（使用随机IV）"""
        from src.readmd_modules.crypto import encrypt_api_key
        
        key = "sk-test-1234567890abcdef1234567890abcdef"
        
        # 加密两次
        encrypted1 = encrypt_api_key(key)
        encrypted2 = encrypt_api_key(key)
        
        # 验证两次加密结果不同（因为使用了随机IV）
        assert encrypted1 != encrypted2
    
    def test_invalid_ciphertext_raises_error(self):
        """测试无效密文抛出异常"""
        from src.readmd_modules.crypto import decrypt_api_key
        
        with pytest.raises(Exception):
            decrypt_api_key("invalid_ciphertext")
    
    def test_empty_key_handling(self):
        """测试空密钥处理"""
        from src.readmd_modules.crypto import encrypt_api_key, decrypt_api_key
        
        with pytest.raises(ValueError):
            encrypt_api_key("")
        
        with pytest.raises(ValueError):
            encrypt_api_key(None)


class TestSecureStorage:
    """测试安全存储功能"""
    
    def test_save_and_load_encrypted_key(self):
        """测试保存和加载加密的API Key"""
        from src.readmd_modules.crypto import save_api_key_securely, load_api_key_securely
        
        # 创建临时配置文件（先创建空文件）
        with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
            config_path = f.name
            json.dump({}, f)  # 写入空JSON对象
        
        try:
            # 保存API Key
            api_key = "sk-test-1234567890abcdef1234567890abcdef"
            save_api_key_securely(config_path, api_key)
            
            # 验证文件中存储的是加密后的值
            with open(config_path, 'r') as f:
                config = json.load(f)
            
            assert 'api_key' in config
            assert config['api_key'] != api_key  # 应该是加密后的值
            
            # 加载API Key
            loaded_key = load_api_key_securely(config_path)
            
            # 验证加载的值与原始值相同
            assert loaded_key == api_key
            
        finally:
            if os.path.exists(config_path):
                os.unlink(config_path)
    
    def test_migrate_plaintext_to_encrypted(self):
        """测试从明文迁移到加密"""
        from src.readmd_modules.crypto import migrate_plaintext_keys
        
        # 创建包含明文Key的配置文件
        with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
            config_path = f.name
            json.dump({'api_key': 'sk-plaintext-key-12345'}, f)
        
        try:
            # 迁移
            migrated = migrate_plaintext_keys(config_path)
            
            # 验证已迁移
            assert migrated is True
            
            # 验证文件中现在是加密的
            with open(config_path, 'r') as f:
                config = json.load(f)
            
            assert config['api_key'] != 'sk-plaintext-key-12345'
            
        finally:
            if os.path.exists(config_path):
                os.unlink(config_path)


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
