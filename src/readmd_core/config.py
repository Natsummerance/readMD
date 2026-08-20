# -*- coding: utf-8 -*-
"""ReadMD 核心配置模块：平台数据目录、系统语言、文件路径等。"""

import locale
import os
import sys

IS_MAC = sys.platform == 'darwin'
IS_WIN = sys.platform == 'win32'
IS_LINUX = sys.platform.startswith('linux')


def _platform_data_dir() -> str:
    """跨平台数据目录：Windows APPDATA, macOS ~/Library/Application Support, Linux ~/.local/share."""
    if sys.platform == 'darwin':
        return os.path.join(os.path.expanduser('~'), 'Library', 'Application Support', 'ReadMD')
    if sys.platform == 'win32':
        return os.path.join(os.environ.get('APPDATA') or os.path.expanduser('~'), 'ReadMD')
    xdg = os.environ.get('XDG_DATA_HOME') or os.path.join(os.path.expanduser('~'), '.local', 'share')
    return os.path.join(xdg, 'ReadMD')


DATA_DIR = _platform_data_dir()
SETTINGS_FILE = os.path.join(DATA_DIR, 'settings.json')
RECENT_FILE = os.path.join(DATA_DIR, 'recent.json')
PROMPTS_FILE = os.path.join(DATA_DIR, 'prompts.json')
HISTORY_FILE = os.path.join(DATA_DIR, 'chat_history.json')
LOG_FILE = os.path.join(DATA_DIR, 'readmd.log')


def get_system_language() -> str:
    """获取当前系统默认语言代码，如 'zh-CN', 'en', 'ja', 'ko' 等。"""
    try:
        if sys.platform == 'win32':
            import ctypes
            lang_id = ctypes.windll.kernel32.GetUserDefaultUILanguage()
            primary = lang_id & 0x3ff
            sub = (lang_id >> 10) & 0x3f
            if primary == 0x04:  # Chinese
                if sub == 0x02:  # zh-CN (PRC)
                    return 'zh-CN'
                elif sub in (0x01, 0x04):  # zh-TW, zh-SG
                    return 'zh-TW' if sub == 0x01 else 'zh-HK'
                elif sub == 0x03:  # zh-HK
                    return 'zh-HK'
                return 'zh-CN'
            lang_map = {
                0x09: 'en', 0x11: 'ja', 0x12: 'ko', 0x0c: 'fr', 0x07: 'de',
                0x0a: 'es', 0x16: 'pt', 0x19: 'ru', 0x10: 'it', 0x01: 'ar',
                0x0d: 'he', 0x1e: 'th', 0x2a: 'vi', 0x21: 'id', 0x39: 'hi',
                0x45: 'bn', 0x55: 'my', 0x54: 'lo', 0x53: 'km', 0x3e: 'ms',
                0x06: 'da', 0x0b: 'fi', 0x14: 'no', 0x1d: 'sv', 0x13: 'nl',
                0x1a: 'hr', 0x18: 'ro', 0x61: 'ne', 0x24: 'sl', 0x1f: 'tr',
                0x22: 'uk', 0x08: 'el', 0x0e: 'hu',
            }
            if primary in lang_map:
                return lang_map[primary]
    except Exception:
        pass
    try:
        loc = locale.getdefaultlocale()[0]
        if loc:
            loc = loc.replace('_', '-')
            if loc.startswith('zh'):
                return 'zh-HK' if ('HK' in loc or 'Hant' in loc) else ('zh-TW' if 'TW' in loc else 'zh-CN')
            return loc.split('-')[0]
    except Exception:
        pass
    return 'zh-CN'


VERSION = '2.3.4'
