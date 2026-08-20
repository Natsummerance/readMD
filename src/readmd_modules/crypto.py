# -*- coding: utf-8 -*-
"""ReadMD 凭据加密模块：用于 API Key 等敏感信息的对称加密存储与优雅降级。"""

import logging
import os
from typing import Optional

from ..readmd_core.config import DATA_DIR

try:
    from cryptography.fernet import Fernet, InvalidToken
    CRYPTO_AVAILABLE = True
except ImportError:
    CRYPTO_AVAILABLE = False


def is_crypto_available() -> bool:
    """返回当前运行环境是否支持 cryptography 加密。"""
    return CRYPTO_AVAILABLE


def _default_key_path() -> str:
    return os.path.join(DATA_DIR, 'encryption.key')


def _get_or_create_key(key_path: Optional[str] = None) -> bytes:
    """获取或自动生成 Fernet 加密密钥。"""
    if not CRYPTO_AVAILABLE:
        raise RuntimeError("cryptography 库未安装，无法生成或读取加密密钥")
    path = key_path or _default_key_path()
    if os.path.exists(path):
        with open(path, 'rb') as f:
            return f.read().strip()
    key = Fernet.generate_key()
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, 'wb') as f:
        f.write(key)
    if os.name != 'nt':
        try:
            os.chmod(path, 0o600)
        except OSError:
            pass
    return key


def encrypt_api_key(api_key: str, key_path: Optional[str] = None) -> str:
    """加密 API Key。若环境缺少 cryptography 则安全优雅降级。"""
    if not api_key:
        return ''
    if not CRYPTO_AVAILABLE:
        logging.info("提示: 未安装 cryptography 库，API Key 将使用兼容方式存储。建议运行: pip install cryptography")
        return api_key
    try:
        key = _get_or_create_key(key_path)
        f = Fernet(key)
        encrypted_bytes = f.encrypt(api_key.encode('utf-8'))
        return 'enc:' + encrypted_bytes.decode('utf-8')
    except Exception as e:
        logging.warning("API Key 加密失败，降级为原样存储: %s", e)
        return api_key


def decrypt_api_key(encrypted_key: str, key_path: Optional[str] = None) -> str:
    """解密 API Key。若为明文或环境缺少 cryptography 则直接返回原字符串。"""
    if not encrypted_key:
        return ''
    if not encrypted_key.startswith('enc:'):
        # 原样明文
        return encrypted_key
    cipher_text = encrypted_key[4:]
    if not CRYPTO_AVAILABLE:
        logging.warning("已加密的 API Key 需要 cryptography 库进行解密，请安装: pip install cryptography")
        return ''
    try:
        key = _get_or_create_key(key_path)
        f = Fernet(key)
        decrypted_bytes = f.decrypt(cipher_text.encode('utf-8'))
        return decrypted_bytes.decode('utf-8')
    except InvalidToken:
        logging.warning("API Key 解密密钥不匹配或密文损坏")
        return ''
    except Exception as e:
        logging.warning("API Key 解密失败: %s", e)
        return ''
