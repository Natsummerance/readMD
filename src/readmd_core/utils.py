# -*- coding: utf-8 -*-
"""ReadMD 核心工具函数：JSON持久化、文本读取、文件对话框路径规整等。"""

import json
import logging
import os
import tempfile
import time
from typing import Any, Optional


def normalize_dialog_path(value: Any, extension: str = '') -> Optional[str]:
    """Return one filesystem path from a pywebview file-dialog result.

    WinForms returns a one-item tuple while Cocoa returns a string. Keeping
    this normalization at the bridge boundary prevents platform-specific
    container types leaking into renderers and ordinary file APIs.
    """
    if value is None or value == '':
        return None
    if isinstance(value, (tuple, list)):
        if not value:
            return None
        if len(value) != 1:
            raise ValueError('保存对话框返回了多个路径')
        value = value[0]
    try:
        path = os.fspath(value)
    except TypeError:
        raise ValueError('保存对话框返回了无效路径')
    if isinstance(path, bytes):
        path = os.fsdecode(path)
    path = str(path).strip()
    if not path:
        return None
    if extension:
        ext = extension if extension.startswith('.') else '.' + extension
        if not path.lower().endswith(ext.lower()):
            path += ext
    return os.path.abspath(path)


def load_json(path: str, default: Any = None) -> Any:
    """安全读取 JSON 文件，若不存在或损坏则返回 default。"""
    if not os.path.isfile(path):
        return default
    try:
        with open(path, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception as e:
        logging.warning("读取 JSON 失败 %s: %s", path, e)
        return default


def save_json(path: str, data: Any) -> bool:
    """安全写入 JSON 文件，自动创建上级目录。"""
    tmp_path = None
    try:
        target = os.path.abspath(path)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        # A fixed ``.tmp`` name lets a reader/AV scanner retain the old
        # handle while another writer truncates the same file.  Use a unique
        # sibling and retry the final replace briefly on Windows, where an
        # antivirus/indexer can transiently deny the rename.
        fd, tmp_path = tempfile.mkstemp(prefix=os.path.basename(target) + '.', suffix='.tmp', dir=os.path.dirname(target))
        with os.fdopen(fd, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
            f.flush()
            os.fsync(f.fileno())
        last_error = None
        for attempt in range(6):
            try:
                os.replace(tmp_path, target)
                tmp_path = None
                break
            except PermissionError as exc:
                last_error = exc
                if attempt == 5:
                    raise
                time.sleep(0.03 * (attempt + 1))
        if last_error is not None and tmp_path is not None:
            raise last_error
        return True
    except Exception as e:
        logging.error("保存 JSON 失败 %s: %s", path, e)
        return False
    finally:
        if tmp_path:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass


def read_text(path: str, default: str = '') -> str:
    """读取纯文本文件，支持 UTF-8 / GBK 自动回退。"""
    if not os.path.isfile(path):
        return default
    for enc in ('utf-8', 'utf-8-sig', 'gbk', 'cp1252', 'latin-1'):
        try:
            with open(path, 'r', encoding=enc) as f:
                return f.read()
        except (UnicodeDecodeError, UnicodeError):
            continue
        except Exception:
            break
    return default
