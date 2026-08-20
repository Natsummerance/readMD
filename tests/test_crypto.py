# -*- coding: utf-8 -*-
"""Tests for src/readmd_modules/crypto.py - Encryption functionality.

Covers:
- encrypt_api_key() / decrypt_api_key() - encryption/decryption roundtrip
- migrate_plaintext_keys() - plaintext key migration
- Key management: creation, loading, validation
"""

import sys
import os
import json
import tempfile
import pytest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))

from src.readmd_modules.crypto import (
    encrypt_api_key,
    decrypt_api_key,
    save_api_key_securely,
    load_api_key_securely,
    migrate_plaintext_keys,
    _get_or_create_key,
)


@pytest.fixture
def temp_dir():
    """Create a temporary directory for test files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield tmpdir


@pytest.fixture
def key_path(temp_dir):
    """Create a path for the encryption key file."""
    return os.path.join(temp_dir, "encryption.key")


@pytest.fixture
def config_path(temp_dir):
    """Create a path for the config file."""
    return os.path.join(temp_dir, "config.json")


class TestGetOrCreateKey:
    """Test _get_or_create_key function."""

    def test_create_new_key(self, temp_dir):
        """Should create a new key file when it doesn't exist."""
        key_path = os.path.join(temp_dir, "new.key")
        key = _get_or_create_key(key_path)
        
        assert os.path.exists(key_path)
        assert len(key) > 0
        # Fernet keys are 32 bytes base64-encoded
        assert len(key) == 44  # Base64 encoded 32 bytes

    def test_load_existing_key(self, temp_dir):
        """Should load existing key file without creating a new one."""
        key_path = os.path.join(temp_dir, "existing.key")
        
        # Create key first time
        key1 = _get_or_create_key(key_path)
        mtime1 = os.path.getmtime(key_path)
        
        # Load key second time
        key2 = _get_or_create_key(key_path)
        mtime2 = os.path.getmtime(key_path)
        
        assert key1 == key2
        assert mtime1 == mtime2  # File should not be modified

    def test_key_file_permissions(self, temp_dir):
        """Key file should have restricted permissions on Unix."""
        if os.name == 'nt':
            pytest.skip("Permission test skipped on Windows")
        
        key_path = os.path.join(temp_dir, "secure.key")
        _get_or_create_key(key_path)
        
        # Check file permissions (should be 0o600)
        mode = os.stat(key_path).st_mode & 0o777
        assert mode == 0o600

    def test_creates_parent_directory(self, temp_dir):
        """Should create parent directories if they don't exist."""
        key_path = os.path.join(temp_dir, "sub", "dir", "key.file")
        key = _get_or_create_key(key_path)
        
        assert os.path.exists(key_path)
        assert len(key) > 0


class TestEncryptApiKey:
    """Test encrypt_api_key function."""

    def test_encrypt_valid_key(self, key_path):
        """Should encrypt a valid API key."""
        api_key = "sk-test123456789"
        encrypted = encrypt_api_key(api_key, key_path)
        
        assert encrypted is not None
        assert isinstance(encrypted, str)
        assert len(encrypted) > 0
        # Encrypted text should be different from original
        assert encrypted != api_key

    def test_encrypt_empty_key_raises(self, key_path):
        """Should raise ValueError for empty API key."""
        with pytest.raises(ValueError, match="API Key不能为空"):
            encrypt_api_key("", key_path)

    def test_encrypt_none_key_raises(self, key_path):
        """Should raise ValueError for None API key."""
        with pytest.raises((ValueError, TypeError)):
            encrypt_api_key(None, key_path)

    def test_encrypt_whitespace_only(self, key_path):
        """Should handle whitespace-only API key."""
        # Whitespace is technically not empty, so it should encrypt
        encrypted = encrypt_api_key("   ", key_path)
        assert encrypted is not None

    def test_encrypt_unicode_key(self, key_path):
        """Should encrypt Unicode API keys correctly."""
        api_key = "密钥-测试-🔑"
        encrypted = encrypt_api_key(api_key, key_path)
        
        assert encrypted is not None
        # Should be able to decrypt back
        decrypted = decrypt_api_key(encrypted, key_path)
        assert decrypted == api_key

    def test_encrypt_long_key(self, key_path):
        """Should handle long API keys."""
        api_key = "sk-" + "a" * 1000
        encrypted = encrypt_api_key(api_key, key_path)
        
        assert encrypted is not None
        decrypted = decrypt_api_key(encrypted, key_path)
        assert decrypted == api_key

    def test_encrypt_creates_key_file(self, temp_dir):
        """Should create key file if it doesn't exist."""
        key_path = os.path.join(temp_dir, "auto.key")
        api_key = "test-key"
        
        assert not os.path.exists(key_path)
        encrypt_api_key(api_key, key_path)
        assert os.path.exists(key_path)


class TestDecryptApiKey:
    """Test decrypt_api_key function."""

    def test_decrypt_roundtrip(self, key_path):
        """Decryption should reverse encryption."""
        api_key = "my-secret-api-key-123"
        encrypted = encrypt_api_key(api_key, key_path)
        decrypted = decrypt_api_key(encrypted, key_path)
        
        assert decrypted == api_key

    def test_decrypt_empty_string_raises(self, key_path):
        """Should raise ValueError for empty encrypted string."""
        with pytest.raises(ValueError, match="加密密钥不能为空"):
            decrypt_api_key("", key_path)

    def test_decrypt_invalid_ciphertext_raises(self, key_path):
        """Should raise ValueError for invalid ciphertext."""
        # First create a key file
        encrypt_api_key("temp", key_path)
        
        with pytest.raises(ValueError, match="解密失败"):
            decrypt_api_key("invalid-ciphertext", key_path)

    def test_decrypt_wrong_key_raises(self, temp_dir):
        """Should fail when using wrong key."""
        key_path1 = os.path.join(temp_dir, "key1.key")
        key_path2 = os.path.join(temp_dir, "key2.key")
        
        # Create both key files first
        from src.readmd_modules.crypto import _get_or_create_key
        _get_or_create_key(key_path1)
        _get_or_create_key(key_path2)
        
        api_key = "secret"
        encrypted = encrypt_api_key(api_key, key_path1)
        
        # Try to decrypt with different key
        with pytest.raises(ValueError, match="解密失败"):
            decrypt_api_key(encrypted, key_path2)

    def test_decrypt_missing_key_file_raises(self, temp_dir):
        """Should raise FileNotFoundError when key file is missing."""
        key_path = os.path.join(temp_dir, "nonexistent.key")
        
        with pytest.raises(FileNotFoundError):
            decrypt_api_key("some-encrypted-text", key_path)

    def test_decrypt_tampered_ciphertext(self, key_path):
        """Should fail when ciphertext is tampered."""
        api_key = "original"
        encrypted = encrypt_api_key(api_key, key_path)
        
        # Tamper with the ciphertext
        tampered = encrypted[:-5] + "XXXXX"
        
        with pytest.raises(ValueError, match="解密失败"):
            decrypt_api_key(tampered, key_path)


class TestSaveApiKeySecurely:
    """Test save_api_key_securely function."""

    def test_save_new_config(self, config_path, key_path):
        """Should create new config file with encrypted key."""
        api_key = "test-api-key"
        save_api_key_securely(config_path, api_key, key_path)
        
        assert os.path.exists(config_path)
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)
        
        assert config['api_key'] != api_key  # Should be encrypted
        assert config['encrypted'] is True

    def test_update_existing_config(self, config_path, key_path):
        """Should update existing config while preserving other fields."""
        # Create initial config
        initial_config = {"other_field": "value", "api_key": "old-key"}
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(initial_config, f)
        
        # Update with encrypted key
        new_api_key = "new-api-key"
        save_api_key_securely(config_path, new_api_key, key_path)
        
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)
        
        assert config['other_field'] == "value"
        assert config['api_key'] != new_api_key  # Encrypted
        assert config['encrypted'] is True

    def test_atomic_write(self, config_path, key_path):
        """Should use atomic write (tmp file + replace)."""
        api_key = "test-key"
        save_api_key_securely(config_path, api_key, key_path)
        
        # Temp file should not exist after successful write
        tmp_path = config_path + '.tmp'
        assert not os.path.exists(tmp_path)
        
        # Config file should exist and be valid
        assert os.path.exists(config_path)
        with open(config_path, 'r', encoding='utf-8') as f:
            config = json.load(f)
        assert 'api_key' in config


class TestLoadApiKeySecurely:
    """Test load_api_key_securely function."""

    def test_load_encrypted_key(self, config_path, key_path):
        """Should load and decrypt an encrypted API key."""
        api_key = "my-secret-key"
        save_api_key_securely(config_path, api_key, key_path)
        
        loaded = load_api_key_securely(config_path, key_path)
        assert loaded == api_key

    def test_load_nonexistent_config(self, temp_dir):
        """Should return None when config file doesn't exist."""
        config_path = os.path.join(temp_dir, "nonexistent.json")
        result = load_api_key_securely(config_path)
        assert result is None

    def test_load_plaintext_key_backward_compat(self, config_path):
        """Should load plaintext key for backward compatibility."""
        config = {"api_key": "plaintext-key", "encrypted": False}
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        loaded = load_api_key_securely(config_path)
        assert loaded == "plaintext-key"

    def test_load_missing_api_key(self, config_path):
        """Should return None when api_key field is missing."""
        config = {"other_field": "value"}
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        loaded = load_api_key_securely(config_path)
        assert loaded is None

    def test_load_empty_api_key(self, config_path):
        """Should return None when api_key is empty."""
        config = {"api_key": ""}
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        loaded = load_api_key_securely(config_path)
        assert loaded is None


class TestMigratePlaintextKeys:
    """Test migrate_plaintext_keys function."""

    def test_migrate_plaintext_to_encrypted(self, config_path, key_path):
        """Should migrate plaintext key to encrypted storage."""
        config = {"api_key": "plaintext-key", "encrypted": False}
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        migrated = migrate_plaintext_keys(config_path, key_path)
        
        assert migrated is True
        
        # Verify key is now encrypted
        with open(config_path, 'r', encoding='utf-8') as f:
            new_config = json.load(f)
        
        assert new_config['encrypted'] is True
        assert new_config['api_key'] != "plaintext-key"
        
        # Verify we can still retrieve the original key
        loaded = load_api_key_securely(config_path, key_path)
        assert loaded == "plaintext-key"

    def test_no_migration_needed_already_encrypted(self, config_path, key_path):
        """Should not migrate if already encrypted."""
        api_key = "secret"
        save_api_key_securely(config_path, api_key, key_path)
        
        migrated = migrate_plaintext_keys(config_path, key_path)
        
        assert migrated is False

    def test_no_migration_no_config_file(self, temp_dir):
        """Should return False when config file doesn't exist."""
        config_path = os.path.join(temp_dir, "nonexistent.json")
        migrated = migrate_plaintext_keys(config_path)
        assert migrated is False

    def test_no_migration_no_api_key(self, config_path):
        """Should return False when there's no api_key to migrate."""
        config = {"other_field": "value"}
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        migrated = migrate_plaintext_keys(config_path)
        assert migrated is False

    def test_migrate_preserves_other_fields(self, config_path, key_path):
        """Should preserve other config fields during migration."""
        config = {
            "api_key": "plaintext-key",
            "encrypted": False,
            "theme": "dark",
            "language": "zh-CN"
        }
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        migrate_plaintext_keys(config_path, key_path)
        
        with open(config_path, 'r', encoding='utf-8') as f:
            new_config = json.load(f)
        
        assert new_config['theme'] == "dark"
        assert new_config['language'] == "zh-CN"


class TestIntegration:
    """Integration tests for crypto module."""

    def test_full_workflow(self, temp_dir):
        """Test complete encryption workflow."""
        key_path = os.path.join(temp_dir, "workflow.key")
        config_path = os.path.join(temp_dir, "workflow.json")
        
        # Step 1: Save API key securely
        original_key = "sk-workflow-test-12345"
        save_api_key_securely(config_path, original_key, key_path)
        
        # Step 2: Load API key securely
        loaded_key = load_api_key_securely(config_path, key_path)
        assert loaded_key == original_key
        
        # Step 3: Migrate (should be no-op since already encrypted)
        migrated = migrate_plaintext_keys(config_path, key_path)
        assert migrated is False

    def test_multiple_keys_same_config(self, temp_dir):
        """Test handling multiple operations on same config."""
        key_path = os.path.join(temp_dir, "multi.key")
        config_path = os.path.join(temp_dir, "multi.json")
        
        # Save first key
        save_api_key_securely(config_path, "key1", key_path)
        assert load_api_key_securely(config_path, key_path) == "key1"
        
        # Update to second key
        save_api_key_securely(config_path, "key2", key_path)
        assert load_api_key_securely(config_path, key_path) == "key2"
        
        # Update to third key
        save_api_key_securely(config_path, "key3", key_path)
        assert load_api_key_securely(config_path, key_path) == "key3"

    def test_key_rotation(self, temp_dir):
        """Test key rotation scenario."""
        old_key_path = os.path.join(temp_dir, "old.key")
        new_key_path = os.path.join(temp_dir, "new.key")
        config_path = os.path.join(temp_dir, "rotate.json")
        
        api_key = "rotation-test-key"
        
        # Encrypt with old key
        encrypted_old = encrypt_api_key(api_key, old_key_path)
        
        # Save config with old encrypted key
        config = {"api_key": encrypted_old, "encrypted": True}
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        # Decrypt with old key works
        assert decrypt_api_key(encrypted_old, old_key_path) == api_key
        
        # Re-encrypt with new key
        encrypted_new = encrypt_api_key(api_key, new_key_path)
        
        # Update config
        config['api_key'] = encrypted_new
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f)
        
        # Decrypt with new key works
        assert decrypt_api_key(encrypted_new, new_key_path) == api_key
