# -*- coding: utf-8 -*-
"""ReadMD 凭据加密模块。

API keys are never written as plaintext.  A missing/failed crypto backend is a
hard error for writes; callers may then ask the user to install the supported
security dependency instead of silently weakening storage.
"""

import logging
import json
import os
import re
import shutil
import subprocess
import sys
from typing import Optional

from ..readmd_core.config import DATA_DIR

SERVICE_NAME = "ReadMD"
_CREDENTIAL_RE = re.compile(r"^cred:[A-Za-z0-9_-]{8,128}$")

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
    """Encrypt an API key, failing closed when cryptography is unavailable."""
    if not api_key:
        return ''
    if not CRYPTO_AVAILABLE:
        raise RuntimeError("cryptography 库未安装，拒绝以明文保存 API Key")
    try:
        key = _get_or_create_key(key_path)
        f = Fernet(key)
        encrypted_bytes = f.encrypt(api_key.encode('utf-8'))
        return 'enc:' + encrypted_bytes.decode('utf-8')
    except Exception as e:
        logging.error("API Key 加密失败，拒绝保存: %s", e)
        raise RuntimeError("API Key 加密失败，拒绝保存") from e


def decrypt_api_key(encrypted_key: str, key_path: Optional[str] = None) -> str:
    """Decrypt an encrypted API key; legacy plaintext values fail closed."""
    if not encrypted_key:
        return ''
    if not encrypted_key.startswith('enc:'):
        logging.error("检测到未加密的 API Key，拒绝读取")
        return ''
    cipher_text = encrypted_key[4:]
    if not CRYPTO_AVAILABLE:
        logging.error("已加密的 API Key 需要 cryptography 库进行解密")
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


def _credential_target(credential_id: str) -> str:
    cid = str(credential_id or '').strip()
    if not _CREDENTIAL_RE.fullmatch(cid):
        raise ValueError('invalid credential id')
    return SERVICE_NAME + '/' + cid


def _vault_path() -> str:
    return os.path.join(DATA_DIR, 'credentials.vault')


def _vault_write(value: dict) -> None:
    if not CRYPTO_AVAILABLE:
        raise RuntimeError('缺少 cryptography，拒绝使用不安全的凭据存储')
    os.makedirs(DATA_DIR, exist_ok=True)
    encrypted = encrypt_api_key(json.dumps(value, ensure_ascii=False, separators=(',', ':')))
    tmp = _vault_path() + '.tmp'
    with open(tmp, 'w', encoding='utf-8', newline='\n') as handle:
        handle.write(encrypted + '\n')
    os.replace(tmp, _vault_path())


def _vault_load() -> dict:
    path = _vault_path()
    if not os.path.isfile(path):
        return {}
    with open(path, 'r', encoding='utf-8') as handle:
        value = decrypt_api_key(handle.read().strip())
    if not value:
        return {}
    try:
        data = json.loads(value)
        return data if isinstance(data, dict) else {}
    except json.JSONDecodeError:
        return {}


def _native_store(credential_id: str, secret: str) -> bool:
    target = _credential_target(credential_id)
    if os.name == 'nt':
        try:
            import win32cred
            win32cred.CredWrite({
                'Type': win32cred.CRED_TYPE_GENERIC,
                'TargetName': target,
                'UserName': SERVICE_NAME,
                'CredentialBlob': secret.encode('utf-8'),
                'Persist': win32cred.CRED_PERSIST_LOCAL_MACHINE,
                'Comment': 'ReadMD provider credential',
            }, 0)
            return True
        except Exception:
            logging.debug('Windows Credential Manager unavailable', exc_info=True)
    if sys.platform == 'darwin' and shutil.which('security'):
        result = subprocess.run(
            ['security', 'add-generic-password', '-a', credential_id, '-s', SERVICE_NAME,
             '-w', secret, '-U'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            check=False)
        return result.returncode == 0
    if sys.platform.startswith('linux') and shutil.which('secret-tool'):
        result = subprocess.run(
            ['secret-tool', 'store', '--label=ReadMD provider credential',
             'service', SERVICE_NAME, 'credential', credential_id], input=secret,
            text=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        return result.returncode == 0
    return False


def _native_load(credential_id: str) -> str:
    target = _credential_target(credential_id)
    if os.name == 'nt':
        try:
            import win32cred
            record = win32cred.CredRead(target, win32cred.CRED_TYPE_GENERIC)
            blob = record.get('CredentialBlob') or b''
            return blob.decode('utf-8') if isinstance(blob, bytes) else str(blob)
        except Exception:
            logging.debug('Windows Credential Manager lookup failed', exc_info=True)
    if sys.platform == 'darwin' and shutil.which('security'):
        result = subprocess.run(
            ['security', 'find-generic-password', '-a', credential_id, '-s', SERVICE_NAME, '-w'],
            capture_output=True, text=True, check=False)
        return result.stdout.strip() if result.returncode == 0 else ''
    if sys.platform.startswith('linux') and shutil.which('secret-tool'):
        result = subprocess.run(
            ['secret-tool', 'lookup', 'service', SERVICE_NAME, 'credential', credential_id],
            capture_output=True, text=True, check=False)
        return result.stdout.strip() if result.returncode == 0 else ''
    return ''


def _native_delete(credential_id: str) -> bool:
    target = _credential_target(credential_id)
    if os.name == 'nt':
        try:
            import win32cred
            win32cred.CredDelete(target, win32cred.CRED_TYPE_GENERIC)
            return True
        except Exception:
            pass
    if sys.platform == 'darwin' and shutil.which('security'):
        return subprocess.run(
            ['security', 'delete-generic-password', '-a', credential_id, '-s', SERVICE_NAME],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False).returncode == 0
    # secret-tool has no portable delete command across versions; overwrite is
    # avoided and the encrypted vault remains the authoritative fallback.
    return False


def store_credential(credential_id: str, secret: str) -> str:
    """Store a secret in the native keychain, or encrypted local vault."""
    if not secret:
        raise ValueError('credential secret is empty')
    if _native_store(credential_id, secret):
        return 'native'
    vault = _vault_load()
    vault[_credential_target(credential_id)] = encrypt_api_key(secret)
    _vault_write(vault)
    return 'encrypted-vault'


def load_credential(credential_id: str) -> str:
    """Resolve one credential without exposing it through a public config."""
    if not credential_id:
        return ''
    native = _native_load(credential_id)
    if native:
        return native
    value = _vault_load().get(_credential_target(credential_id), '')
    return decrypt_api_key(value) if value else ''


def delete_credential(credential_id: str) -> None:
    """Best-effort deletion from native store and encrypted fallback vault."""
    if not credential_id:
        return
    _native_delete(credential_id)
    vault = _vault_load()
    if _credential_target(credential_id) in vault:
        vault.pop(_credential_target(credential_id), None)
        _vault_write(vault)
