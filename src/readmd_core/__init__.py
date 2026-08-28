# -*- coding: utf-8 -*-
"""ReadMD 核心包 (readmd_core)：跨平台配置、核心工具、HTTP 服务与语法自愈引擎。"""

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
    VERSION,
)

from .utils import (
    load_json,
    save_json,
    read_text,
    normalize_dialog_path,
)

from . import readmd_fix
from .readmd_fix import fix_markdown, FixResult

from . import server
from .server import start_server, is_port_in_use, find_available_port, ReadMDHTTPHandler, ThreadedReadMDServer

from . import dialogs
from .dialogs import MARKDOWN_FILTER, IMAGE_FILTER, PDF_FILTER, WORD_FILTER, ALL_FILES_FILTER, CONVERT_FILE_FILTER

from . import window_state
from .window_state import WindowStateManager
from .service import ReadMDCoreService

__all__ = [
    # 配置与路径
    'DATA_DIR',
    'SETTINGS_FILE',
    'RECENT_FILE',
    'PROMPTS_FILE',
    'HISTORY_FILE',
    'LOG_FILE',
    'IS_MAC',
    'IS_WIN',
    'IS_LINUX',
    'get_system_language',
    'VERSION',
    
    # 核心工具
    'load_json',
    'save_json',
    'read_text',
    'normalize_dialog_path',
    
    # 语法自愈
    'readmd_fix',
    'fix_markdown',
    'FixResult',

    # HTTP 服务
    'server',
    'start_server',
    'is_port_in_use',
    'find_available_port',
    'ReadMDHTTPHandler',
    'ThreadedReadMDServer',
    'ReadMDCoreService',

    # 对话框与过滤
    'dialogs',
    'MARKDOWN_FILTER',
    'IMAGE_FILTER',
    'PDF_FILTER',
    'WORD_FILTER',
    'ALL_FILES_FILTER',
    'CONVERT_FILE_FILTER',

    # 窗口与状态
    'window_state',
    'WindowStateManager',
]
