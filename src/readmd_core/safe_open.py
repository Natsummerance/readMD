# -*- coding: utf-8 -*-
"""Validation for files and URLs handed to operating-system launchers."""

import os
from pathlib import Path
from urllib.parse import urlparse

from src.readmd_modules.validators import ValidationError, validate_file_path, validate_url


_SAFE_FILE_SUFFIXES = frozenset({
    '.csv',
    '.docx',
    '.epub',
    '.gif',
    '.htm',
    '.html',
    '.jpeg',
    '.jpg',
    '.json',
    '.md',
    '.markdown',
    '.pdf',
    '.png',
    '.ppt',
    '.pptx',
    '.tex',
    '.tif',
    '.tiff',
    '.txt',
    '.webp',
    '.xls',
    '.xlsx',
})


def safe_file_target(path):
    """Return an existing, regular, non-executable document or media file."""
    if not path:
        raise ValueError('文件路径不能为空')
    raw = os.fspath(path)
    if '\x00' in raw or raw.rstrip() != raw:
        raise ValueError('文件路径包含非法结尾字符')

    candidate = Path(validate_file_path(raw))
    if not candidate.is_file():
        raise ValueError('目标文件不存在')

    suffix = candidate.suffix.lower()
    if suffix not in _SAFE_FILE_SUFFIXES:
        raise ValueError(f'不允许通过系统默认程序打开 {suffix or "无后缀"} 文件')
    return str(candidate)


def safe_external_url(url):
    """Return a browser-safe HTTP(S) or simple mailto URL."""
    if not url or isinstance(url, bytes):
        raise ValueError('外部链接不能为空')
    value = str(url).strip()
    if not value or len(value) > 2048 or any(ord(char) < 32 for char in value):
        raise ValueError('外部链接包含非法字符')

    parsed = urlparse(value)
    if parsed.scheme == 'mailto':
        if '@' not in parsed.path or ' ' in value or '\t' in value or '\r' in value or '\n' in value:
            raise ValueError('无效的邮件链接')
        return value
    try:
        return validate_url(value, allow_private=True)
    except ValidationError as exc:
        raise ValueError(str(exc)) from exc
