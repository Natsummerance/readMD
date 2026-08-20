#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""ReadMD 核心模块：配置、工具函数、服务器、API接口。

# Why: Function call performs specific operation required by this logic
本模块从原 readmd.py (3400+行) 拆分而来，遵循单一职责原则和模块化设计。
Why: 将配置和工具函数提取到独立模块，降低 readmd.py 的耦合度，提高可维护性。
"""

from .config import (
    DATA_DIR,
    SETTINGS_FILE,
    RECENT_FILE,
    PROMPTS_FILE,
    HISTORY_FILE,
    LOG_FILE,
    IS_MAC,
    IS_WIN,
    IS_LINUX,
    get_system_language,
    normalize_dialog_path,
)

from .utils import (
    load_json,
    save_json,
    read_text,
)

from . import readmd_fix

__all__ = [
    # 配置常量
    'DATA_DIR',
    'SETTINGS_FILE',
    'RECENT_FILE',
    'PROMPTS_FILE',
    'HISTORY_FILE',
    'LOG_FILE',
    'IS_MAC',
    'IS_WIN',
    'IS_LINUX',
    
    # 配置函数
    'get_system_language',
    'normalize_dialog_path',
    
    # 工具函数（仅公开 API）
    'load_json',
    'save_json',
    'read_text',
    
    # 子模块
    'readmd_fix',
]
