# -*- coding: utf-8 -*-
"""ReadMD 文件对话框与路径规范化模块 (src.readmd_core.dialogs)。

负责：
1. 跨平台 pywebview 与原生文件对话框返回值规范化；
2. 常用文件类型过滤器与扩展名映射；
3. 安全路径检测与扩展名补全。
"""

import os
from typing import Any, Optional

# 常用对话框文件类型过滤器定义
MARKDOWN_FILTER = 'Markdown Files (*.md;*.markdown;*.mdown);;All Files (*.*)'
IMAGE_FILTER = 'Image Files (*.png;*.jpg;*.jpeg;*.gif;*.webp;*.svg);;All Files (*.*)'
PDF_FILTER = 'PDF Files (*.pdf);;All Files (*.*)'
WORD_FILTER = 'Word Documents (*.docx);;All Files (*.*)'
ALL_FILES_FILTER = 'All Files (*.*)'

CONVERT_FILE_FILTER = (
    'All Supported Files (*.docx;*.pdf;*.pptx;*.xlsx;*.html;*.htm;*.csv;*.json;*.xml;*.txt;*.zip;*.tex);;'
    'Word Documents (*.docx);;'
    'PDF Files (*.pdf);;'
    'LaTeX Files (*.tex);;'
    'PowerPoint (*.pptx);;'
    'Excel (*.xlsx);;'
    'Text Files (*.txt);;'
    'All Files (*.*)'
)


def normalize_dialog_path(value: Any, extension: str = '') -> Optional[str]:
    """将 pywebview 等跨平台文件对话框返回值统一转为单一绝对路径字符串。

    Windows Forms 返回一元元组 ('path',)，Cocoa/macOS 返回字符串，取消操作返回 None 或空列表。
    保持在边界处规整化，避免平台特定数据类型渗透至业务逻辑中。
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


def format_save_filename(stem: str, extension: str) -> str:
    """根据给定的主文件名与扩展名构造标准输出文件名。"""
    stem = stem.strip() or 'untitled'
    ext = extension if extension.startswith('.') else '.' + extension
    if not stem.lower().endswith(ext.lower()):
        return stem + ext
    return stem
