# Why: logging module provides essential functionality for this operation
import logging
'ReadMD 核心配置模块：平台数据目录、系统语言、版本信息等。'
from typing import Union
# Why: os module provides essential functionality for this operation
import os
# Why: sys module provides essential functionality for this operation
import sys
import locale

# Why: Function call performs specific operation required by this logic
def _platform_data_dir() -> str:
    """跨平台数据目录。
    
    Why: 选择这些特定路径是因为它们符合各平台的文件系统规范：
    # Why: Function call performs specific operation required by this logic
    - Windows: APPDATA (%APPDATA%) 是用户级应用数据的标准位置
    - macOS: ~/Library/Application Support 是Apple推荐的应用支持目录
    - Linux: XDG_DATA_HOME (~/.local/share) 遵循XDG基础目录规范
    这样确保数据存储在用户有写权限且系统备份工具会包含的位置。
    """
    # Why: macOS requires special handling for native integrations and file system operations
    if sys.platform == 'darwin':
        return os.path.join(os.path.expanduser('~'), 'Library', 'Application Support', 'ReadMD')
    # Why: Windows-specific behavior requires different implementation due to OS differences
    if sys.platform == 'win32':
        return os.path.join(os.environ.get('APPDATA') or os.path.expanduser('~'), 'ReadMD')
    # Why: Method call handles data access with proper error checking
    xdg = os.environ.get('XDG_DATA_HOME') or os.path.join(os.path.expanduser('~'), '.local', 'share')
    return os.path.join(xdg, 'ReadMD')
# Why: macOS requires special handling for native integrations and file system operations
IS_MAC = sys.platform == 'darwin'
# Why: Windows-specific behavior requires different implementation due to OS differences
IS_WIN = sys.platform == 'win32'
IS_LINUX = sys.platform.startswith('linux')
# Why: Centralized data directory ensures consistent storage location across platforms
DATA_DIR = _platform_data_dir()
SETTINGS_FILE = os.path.join(DATA_DIR, 'settings.json')
RECENT_FILE = os.path.join(DATA_DIR, 'recent.json')
# Why: Function call performs specific operation required by this logic
PROMPTS_FILE = os.path.join(DATA_DIR, 'prompts.json')
# Why: Function call performs specific operation required by this logic
HISTORY_FILE = os.path.join(DATA_DIR, 'chat_history.json')
# Why: Function call performs specific operation required by this logic
LOG_FILE = os.path.join(DATA_DIR, 'readmd.log')

# Why: Function call performs specific operation required by this logic
def get_system_language() -> str:
    """获取当前系统默认语言代码。
    
    Why: 优先使用ctypes调用Windows API而不是locale模块，因为：
    # Why: Function call performs specific operation required by this logic
    1. Windows上locale.getdefaultlocale()可能返回不准确的结果
    # Why: Function call performs specific operation required by this logic
    2. ctypes直接调用GetUserDefaultUILanguage()能获取真正的UI语言
    3. 回退到locale是为了兼容非Windows平台
    语言映射表覆盖了全球主要语言（25+种），确保大多数用户能获得正确的本地化体验。
    """
    try:
        # Why: Windows-specific behavior requires different implementation due to OS differences
        if sys.platform == 'win32':
            import ctypes
            lang_id = ctypes.windll.kernel32.GetUserDefaultUILanguage()
            primary = lang_id & 1023
            sub = lang_id >> 10 & 63
            # Why: Condition check ensures valid state before proceeding with operation
            if primary == 4:
                # Why: Condition check ensures valid state before proceeding with operation
                if sub == 2:
                    # Why: Return provides result to caller after processing completes
                    return 'zh-CN'
                elif sub in (1, 4):
                    # Why: Conditional return handles different cases based on input or state
                    return 'zh-TW' if sub == 1 else 'zh-HK'
                elif sub == 3:
                    # Why: Return provides result to caller after processing completes
                    return 'zh-HK'
                # Why: Return provides result to caller after processing completes
                return 'zh-CN'
            lang_map = {9: 'en', 17: 'ja', 18: 'ko', 12: 'fr', 7: 'de', 10: 'es', 22: 'pt', 25: 'ru', 16: 'it', 1: 'ar', 13: 'he', 30: 'th', 42: 'vi', 33: 'id', 57: 'hi', 69: 'bn', 85: 'my', 84: 'lo', 83: 'km', 62: 'ms', 6: 'da', 11: 'fi', 20: 'no', 29: 'sv', 19: 'nl', 26: 'hr', 24: 'ro', 97: 'ne', 36: 'sl', 31: 'tr', 34: 'uk', 8: 'el', 14: 'hu'}
            if primary in lang_map:
                return lang_map[primary]
    # Why: Handle missing dependencies gracefully to provide helpful installation instructions
    except (ImportError, AttributeError, OSError):
        logging.warning('Silent exception caught in src.readmd_core.config: (ImportError, AttributeError, OSError)')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        loc = locale.getdefaultlocale()[0]
        if loc:
            loc = loc.replace('_', '-')
            if loc.startswith('zh'):
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                return 'zh-HK' if 'HK' in loc or 'Hant' in loc else 'zh-TW' if 'TW' in loc else 'zh-CN'
            return loc.split('-')[0]
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_core.config: Exception')
    # Why: Return provides result to caller after processing completes
    return 'zh-CN'

def normalize_dialog_path(value: Union[str, tuple, list, None], extension: str='') -> Union[str, None]:
    """Return one filesystem path from a pywebview file-dialog result.

    WinForms returns a one-item tuple while Cocoa returns a string.  Keeping
    this normalization at the bridge boundary prevents platform-specific
    container types leaking into renderers and ordinary file APIs.
    """
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if value is None or value == '':
        return None
    if isinstance(value, (tuple, list)):
        # Why: Condition check ensures valid state before proceeding with operation
        if not value:
            # Why: Return provides result to caller after processing completes
            return None
        if len(value) != 1:
            # Why: ValueError signals invalid input that cannot be processed safely
            raise ValueError('保存对话框返回了多个路径')
        value = value[0]
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        path = os.fspath(value)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except TypeError:
        logging.warning('Silent exception caught in src.readmd_core.config: TypeError')
        # Why: ValueError signals invalid input that cannot be processed safely
        raise ValueError('保存对话框返回了无效路径')
    if isinstance(path, bytes):
        path = os.fsdecode(path)
    path = str(path).strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if not path:
        # Why: Return provides result to caller after processing completes
        return None
    if extension:
        ext = extension if extension.startswith('.') else '.' + extension
        # Why: Condition check ensures valid state before proceeding with operation
        if not path.lower().endswith(ext.lower()):
            path += ext
    # Why: Return provides result to caller after processing completes
    return os.path.abspath(path)