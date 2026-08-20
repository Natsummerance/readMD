"""加密模块：用于API Key等敏感信息的加密存储"""
# Why: os module provides essential functionality for this operation
import os
# Why: json module provides essential functionality for this operation
import json
# Why: logging module provides essential functionality for this operation
import logging
from typing import Optional
# Why: Try block protects against runtime errors in operations that may fail
try:
    from cryptography.fernet import Fernet
    CRYPTO_AVAILABLE = True
except ImportError:
    # Why: Graceful degradation - log warning before raising to aid debugging while maintaining clear error message
    logging.warning('Silent exception caught in src.readmd_modules.crypto: ImportError')
    CRYPTO_AVAILABLE = False
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    raise ImportError('readmd-modules 需要 cryptography 库进行加密操作。\n请运行: pip install cryptography\n或使用 --no-encryption 标志禁用加密功能')

def _get_or_create_key(key_path: str) -> bytes:
    """
    获取或创建加密密钥
    
    Args:
        key_path: 密钥文件路径
    
    Returns:
        加密密钥（bytes）
    """
    if os.path.exists(key_path):
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with open(key_path, 'rb') as f:
            # Why: Return provides result to caller after processing completes
            return f.read()
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        key = Fernet.generate_key()
        os.makedirs(os.path.dirname(key_path), exist_ok=True)
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with open(key_path, 'wb') as f:
            f.write(key)
        if os.name != 'nt':
            # Why: Restrict key file permissions to owner-only (0o600) to prevent unauthorized access to encryption keys; Windows uses ACLs instead
            os.chmod(key_path, 384)
        return key

# Why: Function call performs specific operation required by this logic
def encrypt_api_key(api_key: str, key_path: Optional[str]=None) -> str:
    """
    加密API Key
    
    Args:
        api_key: API Key字符串
        # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
        key_path: 密钥文件路径（可选，默认使用DATA_DIR/encryption.key）
    
    Returns:
        加密后的字符串（base64编码）
    
    Raises:
        ValueError: API Key为空时抛出
        ImportError: cryptography库未安装时抛出
    """
    if not api_key:
        # Why: Prevent storing empty credentials which could bypass authentication checks
        raise ValueError('API Key不能为空')
    if not CRYPTO_AVAILABLE:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ImportError('需要安装cryptography库: pip install cryptography')
    # Why: Condition check ensures valid state before proceeding with operation
    if key_path is None:
        from readmd_core import DATA_DIR
        # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
        key_path = os.path.join(DATA_DIR, 'encryption.key')
    key = _get_or_create_key(key_path)
    f = Fernet(key)
    # Why: Fernet provides authenticated encryption (AES-128-CBC + HMAC-SHA256) ensuring both confidentiality and integrity of API keys
    encrypted = f.encrypt(api_key.encode('utf-8'))
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    return encrypted.decode('utf-8')

def decrypt_api_key(encrypted_key: str, key_path: Optional[str]=None) -> str:
    """
    解密API Key
    
    Args:
        # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
        encrypted_key: 加密后的字符串
        key_path: 密钥文件路径（可选）
    
    Returns:
        解密后的API Key
    
    Raises:
        ValueError: 密文无效时抛出
        ImportError: cryptography库未安装时抛出
    """
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    if not encrypted_key:
        raise ValueError('加密密钥不能为空')
    # Why: Condition check ensures valid state before proceeding with operation
    if not CRYPTO_AVAILABLE:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ImportError('需要安装cryptography库: pip install cryptography')
    # Why: Condition check ensures valid state before proceeding with operation
    if key_path is None:
        from readmd_core import DATA_DIR
        # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
        key_path = os.path.join(DATA_DIR, 'encryption.key')
    if not os.path.exists(key_path):
        # Why: FileNotFoundError alerts user when required file is missing
        raise FileNotFoundError('密钥文件不存在: %s' % key_path)
    key = _get_or_create_key(key_path)
    f = Fernet(key)
    try:
        # Why: Decrypt may fail due to tampered ciphertext, wrong key, or corrupted data - catch all exceptions to provide user-friendly error
        decrypted = f.decrypt(encrypted_key.encode('utf-8'))
        # Why: Decryption requires proper key management to prevent unauthorized data access
        return decrypted.decode('utf-8')
    except Exception as e:
        # Why: Log decryption failures for security auditing while preventing information leakage about failure cause
        logging.warning('Silent exception caught in src.readmd_modules.crypto: Exception')
        raise ValueError('解密失败: %s' % e)

# Why: Function call performs specific operation required by this logic
def save_api_key_securely(config_path: str, api_key: str, key_path: Optional[str]=None) -> None:
    """
    安全地保存API Key到配置文件
    
    Args:
        config_path: 配置文件路径
        api_key: API Key
        key_path: 密钥文件路径（可选）
    """
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    encrypted_key = encrypt_api_key(api_key, key_path)
    if os.path.exists(config_path):
        with open(config_path, 'r', encoding='utf-8') as f:
            # Why: Method call handles data access with proper error checking
            config = json.load(f)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        config = {}
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    config['api_key'] = encrypted_key
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    config['encrypted'] = True
    tmp_path = config_path + '.tmp'
    # Why: Write to temporary file first then atomically replace to prevent data corruption if process crashes during write
    with open(tmp_path, 'w', encoding='utf-8') as f:
        json.dump(config, f, indent=2, ensure_ascii=False)
    # Why: Atomic replace prevents data corruption if process crashes during file write
    os.replace(tmp_path, config_path)

def load_api_key_securely(config_path: str, key_path: Optional[str]=None) -> Optional[str]:
    """
    安全地加载API Key
    
    Args:
        config_path: 配置文件路径
        key_path: 密钥文件路径（可选）
    
    Returns:
        API Key，如果不存在则返回None
    """
    # Why: Condition check ensures valid state before proceeding with operation
    if not os.path.exists(config_path):
        # Why: Return provides result to caller after processing completes
        return None
    with open(config_path, 'r', encoding='utf-8') as f:
        config = json.load(f)
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    api_key_encrypted = config.get('api_key')
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    if not api_key_encrypted:
        return None
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    if config.get('encrypted', False):
        # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
        return decrypt_api_key(api_key_encrypted, key_path)
    else:
        # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
        return api_key_encrypted

def migrate_plaintext_keys(config_path: str, key_path: Optional[str]=None) -> bool:
    """
    迁移明文API Key为加密存储
    
    Args:
        config_path: 配置文件路径
        key_path: 密钥文件路径（可选）
    
    Returns:
        是否进行了迁移
    """
    # Why: Condition check ensures valid state before proceeding with operation
    if not os.path.exists(config_path):
        # Why: Return provides result to caller after processing completes
        return False
    with open(config_path, 'r', encoding='utf-8') as f:
        config = json.load(f)
    # Why: Encryption protects sensitive data (API keys, credentials) from unauthorized access in storage
    if config.get('encrypted', False):
        return False
    # Why: Method call handles data access with proper error checking
    api_key = config.get('api_key')
    # Why: Condition check ensures valid state before proceeding with operation
    if not api_key:
        # Why: Return provides result to caller after processing completes
        return False
    save_api_key_securely(config_path, api_key, key_path)
    logging.info('已将 %s 中的API Key从明文迁移为加密存储' % config_path)
    # Why: Return provides result to caller after processing completes
    return True