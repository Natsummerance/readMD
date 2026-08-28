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
SKILLS_FILE = os.path.join(DATA_DIR, 'skills.json')
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


def _parse_env_file(filepath: str):
    """解析单个 env 文件并写入 os.environ（不覆盖已存在的变量）。"""
    if not filepath or not os.path.isfile(filepath):
        return
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith('#') or '=' not in line:
                    continue
                k, v = line.split('=', 1)
                k = k.strip()
                v = v.strip().strip('\'"')
                if k and k not in os.environ:
                    os.environ[k] = v
    except Exception:
        pass


def load_dotenv():
    """按优先级从运行环境、.env.local、.env 和 VERSION 加载全局配置。"""
    # 1. 寻找根目录与包目录候选
    candidates = []
    current_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = os.path.dirname(os.path.dirname(current_dir))
    candidates.extend([root_dir, os.getcwd()])
    if sys.argv and sys.argv[0]:
        candidates.append(os.path.dirname(os.path.abspath(sys.argv[0])))
    if hasattr(sys, '_MEIPASS'):
        candidates.append(getattr(sys, '_MEIPASS'))

    # 优先加载本地私有配置 .env.local
    for d in candidates:
        if d:
            _parse_env_file(os.path.join(d, '.env.local'))

    # 其次加载全局统一版本配置 .env
    for d in candidates:
        if d:
            _parse_env_file(os.path.join(d, '.env'))


def get_version() -> str:
    """严格从 .env / 环境变量或 VERSION 文件中读取全局版本，杜绝代码内硬编码 fallback。"""
    load_dotenv()
    if os.environ.get('READMD_VERSION'):
        return os.environ['READMD_VERSION']
    if os.environ.get('VERSION'):
        return os.environ['VERSION']

    current_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = os.path.dirname(os.path.dirname(current_dir))
    for d in [root_dir, os.getcwd(), getattr(sys, '_MEIPASS', '')]:
        if d:
            vfile = os.path.join(d, 'VERSION')
            if os.path.isfile(vfile):
                try:
                    with open(vfile, 'r', encoding='utf-8') as f:
                        v = f.read().strip()
                        if v:
                            return v
                except Exception:
                    pass
    return os.environ.get('READMD_VERSION', '')


VERSION = get_version()
