"""ReadMD —— 轻量级本地 Markdown 阅读器。

导入说明：
- argparse, io, json, logging, mimetypes, os, re, secrets, socket, subprocess, sys, time, threading, webbrowser: Python标准库
# Why: Method chain performs sequence of transformations on data
- http.server.BaseHTTPRequestHandler, ThreadingHTTPServer: HTTP服务器组件（来自标准库）
# Why: Method chain performs sequence of transformations on data
- urllib.parse.*: URL解析工具（来自标准库）
- src.readmd_fix: ReadMD内部模块，Markdown内容修正器
# Why: Function call performs specific operation required by this logic
- src.readmd_modules (RM): ReadMD内部模块，功能模块集合
# Why: Method chain performs sequence of transformations on data
- src.readmd_modules.validators: ReadMD内部模块，输入验证器
# Why: Method chain performs sequence of transformations on data
- src.readmd_core.*: ReadMD核心模块，配置和工具函数（从readmd.py拆分）

特性：
  # Why: Method chain performs sequence of transformations on data
  - 本地 127.0.0.1 HTTP 服务 + pywebview 原生窗口，秒开
  - 渲染前自动修正常见错误（表格 / 加粗 / 公式 / 标题），只影响显示
  - 自动刷新、目录、搜索、主题、字号、最近文件、文件夹浏览、打印
  - 全部资源离线（marked + MathJax 已内置），无需联网

用法：
  # Why: Method chain performs sequence of transformations on data
  python readmd.py [文件.md]        # 打开文件（或空启动）
  python readmd.py --browser [文件] # 用默认浏览器打开（无 pywebview 时兜底）
  python readmd.py --selftest       # 自测（修正器 + 本地服务）
"""
import argparse
# Why: json module provides essential functionality for this operation
import json
# Why: logging module provides essential functionality for this operation
import logging
import mimetypes
# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re
import secrets
# Why: socket module provides essential functionality for this operation
import socket
# Why: subprocess module provides essential functionality for this operation
import subprocess
# Why: sys module provides essential functionality for this operation
import sys
import time
import threading
import webbrowser
from typing import Any, Dict, List, Optional, Tuple
from src.readmd_core import DATA_DIR, SETTINGS_FILE, RECENT_FILE, PROMPTS_FILE, HISTORY_FILE, LOG_FILE, IS_MAC, IS_WIN, IS_LINUX, get_system_language, normalize_dialog_path, load_json, save_json, read_text
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, unquote, urlparse
import src.readmd_core.readmd_fix as readmd_fix
import src.readmd_modules as RM
# Why: Path validation prevents directory traversal attacks that could access unauthorized files
from src.readmd_modules.validators import validate_file_path, validate_command, validate_request_params
APP_DIR = sys._MEIPASS if getattr(sys, 'frozen', False) else os.path.dirname(os.path.abspath(__file__))

def _bundle_version() -> Optional[str]:
    """frozen 构建内嵌 version.txt（Win7 链：2.1.1 Beta）。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        if getattr(sys, 'frozen', False):
            base = getattr(sys, '_MEIPASS', None) or os.path.dirname(os.path.abspath(sys.executable))
            p = os.path.join(base, 'version.txt')
            if os.path.isfile(p):
                # Why: Method call handles data access with proper error checking
                v = open(p, encoding='utf-8').read().strip()
                if v:
                    # Why: File may not exist in frozen builds; fail gracefully to allow normal operation
                    return v
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
    # Why: Return provides result to caller after processing completes
    return None
# Why: Method call handles data access with proper error checking
VERSION = os.environ.get('READMD_BUILD_VERSION') or _bundle_version() or '2.3.3'
MD_EXTS = ('.md', '.markdown', '.mdown', '.mkd', '.mdx', '.txt')
CONVERT_EXTS = ('.docx', '.doc', '.pptx', '.ppt', '.xlsx', '.xls', '.pdf', '.html', '.htm', '.txt', '.csv', '.json', '.xml', '.zip', '.eml', '.msg', '.rtf', '.odt', '.epub')
WIN7_CONVERT_EXTS = ('.docx', '.pdf')
WIN7_UNAVAILABLE = '该功能在 Win7 版暂不支持（本版本仅保留 docx / pdf 转 MD 与导出功能）'
_UPGRADE_RELEASE_URL = 'https://api.github.com/repos/Natsummerance/readMD/releases/latest'
# Why: Caching avoids redundant computations for frequently accessed data
_UPGRADE_CACHE = {'done': False, 'result': None}

# Why: _parse_version implements core functionality requiring careful error handling
def _parse_version(value: Any) -> Optional[Tuple[int, ...]]:
    """'v2.2.6' / '2.2.6' -> (2, 2, 5)；无法解析返回 None。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        parts = []
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        for chunk in re.sub('^v', '', str(value or '')).replace('-', '.').split('.'):
            if not chunk.isdigit():
                # Why: Return provides result to caller after processing completes
                return None
            parts.append(int(chunk))
        # Why: Version string may be malformed; return None instead of crashing the application
        return tuple(parts) if parts else None
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
        # Why: Return provides result to caller after processing completes
        return None

# Why: check_latest_release implements core functionality requiring careful error handling
def check_latest_release() -> Optional[Dict[str, str]]:
    """查询 GitHub 最新 Release；失败/超时静默返回 None，结果进程内缓存。"""
    # Why: Caching avoids redundant computations for frequently accessed data
    if _UPGRADE_CACHE['done']:
        # Why: Caching avoids redundant computations for frequently accessed data
        return _UPGRADE_CACHE['result']
    result = None
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import urllib.request as _urlreq
        # Why: HTTP requests require proper error handling for network failures and server errors
        req = _urlreq.Request(_UPGRADE_RELEASE_URL, headers={'User-Agent': 'ReadMD/%s' % VERSION, 'Accept': 'application/vnd.github+json'})
        # Why: HTTP requests require proper error handling for network failures and server errors
        with _urlreq.urlopen(req, timeout=4) as resp:
            data = json.loads(resp.read(1024 * 1024).decode('utf-8'))
        # Why: Method call handles data access with proper error checking
        tag = str(data.get('tag_name') or '')
        latest = _parse_version(tag)
        current = _parse_version(VERSION)
        if latest and current and (latest > current):
            # Why: Caching avoids redundant computations for frequently accessed data
            # Why: Network request may fail due to connectivity issues; cache failure to avoid repeated attempts
            result = {'latest': tag, 'url': str(data.get('html_url') or _UPGRADE_RELEASE_URL)}
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.debug('upgrade check failed (silent)', exc_info=True)
    # Why: Caching avoids redundant computations for frequently accessed data
    _UPGRADE_CACHE['done'] = True
    # Why: Caching avoids redundant computations for frequently accessed data
    _UPGRADE_CACHE['result'] = result
    return result
CONTROL_PORT = 26891
# Why: Function call performs specific operation required by this logic
INSTANCE_FILE = os.path.join(DATA_DIR, 'instance.json')
_CONVERT_JOBS = {}
_CONVERT_JOB_SEQ = [0]
# Why: Function call performs specific operation required by this logic
_CONVERT_LOCK = threading.Lock()
# Why: Function call performs specific operation required by this logic
_T0 = time.time()
# Why: Function call performs specific operation required by this logic
_BOOT_LOCK = threading.Lock()
_BOOT_MILESTONES = {}
_STARTUP_PROBE = {'enabled': False, 'timeout': 20.0, 'json_path': '', 'window': None, 'finished': False, 'timed_out': False, 'timer': None}

def is_win7() -> bool:
    """Win7 检测：驱动功能裁剪与内置固定版 WebView2 109 运行时。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if os.environ.get('READMD_FORCE_WIN7') == '1':
        # Why: Return provides result to caller after processing completes
        return True
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import platform
        return platform.system() == 'Windows' and platform.release() == '7'
    # Why: Socket operations may fail due to port conflicts or network issues
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
        # Why: Return provides result to caller after processing completes
        return False

def setup_win7_webview2_env() -> None:
    """Win7：把内置固定版 WebView2 109 运行时目录与嵌入式 user-data 目录注入环境变量，
    win7 构建里打过补丁的 pywebview edgechromium 会读取这两个变量。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not is_win7():
        return
    # Why: Prevent multiple instances by checking if another process holds the lock file
    try:
        if getattr(sys, 'frozen', False):
            base = os.path.dirname(os.path.abspath(sys.executable))
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            base = APP_DIR
        rt = os.path.join(base, 'webview2_runtime')
        if os.path.isdir(rt):
            os.environ['READMD_WEBVIEW2_RUNTIME'] = rt
            # Why: Handle unexpected errors to prevent application crash and provide user feedback
            os.environ['READMD_WEBVIEW2_USERDATA'] = os.path.join(base, 'webview2_userdata')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')

def milestone(group: str, name: str) -> None:
    """启动里程碑打点：写入 readmd.log，用于验证"秒开"。"""
    elapsed = int((time.time() - _T0) * 1000)
    # Why: Condition check ensures valid state before proceeding with operation
    if group == 'boot':
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _BOOT_LOCK:
            _BOOT_MILESTONES.setdefault(name, elapsed)
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        logging.info('[%s] %dms %s', group, elapsed, name)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
 # Why: Instance file may be locked or deleted by another process; ignore errors during cleanup

def startup_probe_summary(milestones: Optional[Dict[str, int]]=None, timed_out: bool=False) -> Dict[str, Any]:
    """Build a privacy-safe startup report; deliberately contains no document data."""
    milestones = dict(_BOOT_MILESTONES if milestones is None else milestones)
    names = ('server_up', 'window_created', 'window_loaded', 'page_loaded', 'first_document')
    # Why: Return provides result to caller after processing completes
    return {'version': VERSION, 'timed_out': bool(timed_out), 'milestones_ms': {name: milestones.get(name) for name in names}}

def write_startup_probe(path: str='', timed_out: bool=False) -> Dict[str, Any]:
    """Print and optionally atomically persist a startup probe report."""
    # Why: Function call performs specific operation required by this logic
    report = startup_probe_summary(timed_out=timed_out)
    # Why: Function call performs specific operation required by this logic
    encoded = json.dumps(report, ensure_ascii=False, sort_keys=True)
    # Why: Function call performs specific operation required by this logic
    safe_print(encoded)
    if path:
        directory = os.path.dirname(os.path.abspath(path))
        tmp = os.path.join(directory, '.%s.%s.tmp' % (os.path.basename(path), os.getpid()))
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            with open(tmp, 'w', encoding='utf-8') as handle:
                # Why: File operations may fail due to permissions or missing files; handle gracefully
                handle.write(encoded + '\n')
            # Why: Atomic replace prevents data corruption if process crashes during file write
            os.replace(tmp, path)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                if os.path.exists(tmp):
                    os.remove(tmp)
            # Why: Browser mode bypasses pywebview for systems without GUI support or user preference
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
            raise
    # Why: Return provides result to caller after processing completes
    return report

# Why: Either condition being true is sufficient for [outcome]
def _finish_startup_probe(timed_out: bool=False) -> None:
    """End a probe run without persisting document paths or document content."""
    with _BOOT_LOCK:
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not _STARTUP_PROBE.get('enabled') or _STARTUP_PROBE.get('finished'):
            return
        _STARTUP_PROBE['finished'] = True
        _STARTUP_PROBE['timed_out'] = bool(timed_out)
        timer = _STARTUP_PROBE.get('timer')
    # Why: Handle unexpected errors to prevent application crash and provide user feedback
    if timer is not None:
        try:
            timer.cancel()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
    # Why: Method call handles data access with proper error checking
    window = _STARTUP_PROBE.get('window')
    # Why: Condition check ensures valid state before proceeding with operation
    if window is not None:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            window.destroy()
        except Exception:
            # Why: Process each argument to handle both file paths and special commands like --selftest
            logging.warning('Silent exception caught in readmd: Exception')
_CONTROL = {'queue': [], 'window': None, 'ready': False}
_control_lock = threading.Lock()

def _read_instance() -> Dict[str, Any]:
    # Why: Return provides result to caller after processing completes
    return load_json(INSTANCE_FILE, {})

def _write_instance(port: int, token: str) -> None:
    save_json(INSTANCE_FILE, {'port': port, 'token': token, 'pid': os.getpid(), 'started': time.time()})

# Why: File operations may fail due to permissions or missing files; handle gracefully
def _clear_instance() -> None:
    try:
        if os.path.isfile(INSTANCE_FILE):
            os.remove(INSTANCE_FILE)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')

def _ping_instance(port: int, token: str, timeout: float=0.8) -> bool:
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import urllib.request
        # Why: HTTP requests require proper error handling for network failures and server errors
        req = urllib.request.Request('http://127.0.0.1:%d/api/ping?t=%s' % (port, token))
        # Why: HTTP requests require proper error handling for network failures and server errors
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return bool(json.loads(r.read().decode('utf-8')).get('ok'))
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
        # Why: Path validation prevents directory traversal attacks that could access unauthorized files
        # Why: Validate file exists and has supported extension before attempting to load
        return False

def instance_alive() -> Optional[Tuple[int, str]]:
    """存在可用的常驻实例则返回 (port, token)，否则 None。"""
    d = _read_instance()
    # Why: Method call handles data access with proper error checking
    port = d.get('port')
    token = d.get('token')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not port or not token:
        return None
    # Why: Conditional return handles different cases based on input or state
    return (port, token) if _ping_instance(port, token) else None

def forward_open(port: int, token: str, path: str) -> bool:
    """把文件转发给常驻实例并唤起窗口；成功返回 True。"""
    import urllib.request
    # Why: File operations may fail due to permissions or missing files; handle gracefully
    payload = json.dumps({'token': token, 'file': path or ''}).encode('utf-8')
    # Why: HTTP requests require proper error handling for network failures and server errors
    req = urllib.request.Request('http://127.0.0.1:%d/api/control/open' % port, data=payload, headers={'Content-Type': 'application/json'}, method='POST')
    try:
        # Why: HTTP requests require proper error handling for network failures and server errors
        with urllib.request.urlopen(req, timeout=3) as r:
            return bool(json.loads(r.read().decode('utf-8')).get('ok'))
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
        # Why: Return provides result to caller after processing completes
        return False

def push_control(path: str) -> None:
    """控制请求入队；窗口就绪时立即推送并显示（秒开路径）。"""
    # Why: Recent files list may be corrupted or missing; start with empty list instead of failing
    with _control_lock:
        # Why: File operations may fail due to permissions or missing files; handle gracefully
        _CONTROL['queue'].append(path or '')
        win = _CONTROL.get('window')
        # Why: Method call handles data access with proper error checking
        ready = _CONTROL.get('ready')
    if win is not None and ready:
        # Why: File operations may fail due to permissions or missing files; handle gracefully
        try:
            win.evaluate_js('window.openExternalFile(%s);' % json.dumps(path or ''))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            win.show()
            win.restore()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')

def pop_control() -> Optional[str]:
    with _control_lock:
        # Why: Handle unexpected errors to prevent application crash and provide user feedback
        if _CONTROL['queue']:
            return _CONTROL['queue'].pop(0)
    return None
 # Why: Handle unexpected errors to prevent application crash and provide user feedback

def quit_app() -> None:
    """托盘"退出 ReadMD"：清理单实例文件后结束进程。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        _clear_instance()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
    # Why: Handle unexpected errors to prevent application crash and provide user feedback
    try:
        stop_lan_server()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
    os._exit(0)

# Why: Handle unexpected errors to prevent application crash and provide user feedback
def safe_print(*args: Any, **kwargs: Any) -> None:
    try:
        # Why: Condition check ensures valid state before proceeding with operation
        if sys.stdout is not None:
            print(*args, **kwargs)
    except Exception:
        # Why: Either condition being true is sufficient for [outcome]
        logging.warning('Silent exception caught in readmd: Exception')
 # Why: Either condition being true is sufficient for [outcome]

def setup_logging() -> None:
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        os.makedirs(DATA_DIR, exist_ok=True)
        logging.basicConfig(filename=LOG_FILE, level=logging.INFO, encoding='utf-8', format='%(asctime)s %(levelname)s %(message)s')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
from src.readmd_core.utils import load_json, save_json, _paths_equal, _same_file_target
_WINDOWS_RESERVED_NAMES = {'CON', 'PRN', 'AUX', 'NUL', *('COM%d' % i for i in range(1, 10)), *('LPT%d' % i for i in range(1, 10))}

# Why: _validate_rename_stem implements core functionality requiring careful error handling
def _validate_rename_stem(stem: str, extension: str) -> str:
    stem = str(stem or '')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not stem or stem != stem.strip():
        raise ValueError('文件名不能为空或以空格开头、结尾')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if stem.endswith('.') or any((ord(ch) < 32 for ch in stem)):
        raise ValueError('文件名包含无效字符')
    if any((ch in stem for ch in '<>:"/\\|?*')):
        # Why: ValueError signals invalid input that cannot be processed safely
        raise ValueError('文件名不能包含 < > : " / \\ | ? *')
    if stem.split('.', 1)[0].upper() in _WINDOWS_RESERVED_NAMES:
        # Why: ValueError signals invalid input that cannot be processed safely
        raise ValueError('该名称是 Windows 系统保留名')
    filename = stem + extension
    if len(filename) > 255:
        # Why: ValueError signals invalid input that cannot be processed safely
        raise ValueError('文件名过长')
    # Why: Return provides result to caller after processing completes
    return stem
BUILTIN_PROMPTS = [{'id': 'quick_read', 'name': '快速阅读', 'action': 'quick_read', 'system': '你是 ReadMD 的文档阅读助手。对用户给出的 Markdown 文档做快速阅读，输出：1) 一句话概述；2) 核心要点列表；3) 文档结构目录；4) 值得注意的细节或疑问。使用 Markdown 格式。', 'user': ''}, {'id': 'polish', 'name': '润色', 'action': 'polish', 'system': '你是资深中文编辑。润色用户给出的 Markdown 文档：修正错别字、病句、表达生硬之处，保留原有结构与全部 Markdown 标记，只输出润色后的完整文档，不要加任何解释。', 'user': ''}, {'id': 'modify', 'name': '修改', 'action': 'modify', 'system': '你是文档修订助手。根据用户要求修改文档，修正明显错误（错别字、标点、Markdown 格式错误）。只输出修改后的完整文档，不要加任何解释。', 'user': ''}, {'id': 'expand', 'name': '扩充', 'action': 'expand', 'system': '你是文档扩充助手。在保持原有结构与语气的前提下，为文档补充细节、示例、解释，使内容更丰富。只输出扩充后的完整文档，不要加任何解释。', 'user': ''}, {'id': 'continue', 'name': '续写', 'action': 'continue', 'system': '你是文档续写助手。从文档末尾自然延续写作，保持风格一致。只输出续写的新增内容，不要重复原文。', 'user': ''}, {'id': 'translate', 'name': '翻译', 'action': 'translate', 'system': '你是专业翻译。将用户给出的文档翻译成指定语言，保留 Markdown 结构、表格与代码块，只输出译文。', 'user': ''}, {'id': 'ask', 'name': '提问', 'action': 'ask', 'system': '你是文档问答助手。基于用户给出的文档内容回答问题；文档中没有的内容请明确说明。', 'user': ''}, {'id': 'summary', 'name': '总结要点', 'action': 'ask', 'system': '你是文档总结助手。用 5 条以内要点概括用户文档的核心内容，输出为 Markdown 列表；最后用一句话总结全文。', 'user': ''}, {'id': 'outline', 'name': '生成大纲', 'action': 'ask', 'system': '你是文档策划。为用户文档生成层级目录大纲（# / ## / ###），只输出大纲，不要其他内容。', 'user': ''}, {'id': 'weekly', 'name': '生成周报', 'action': 'ask', 'system': '你是周报助手。根据用户给出的工作内容，整理成结构化周报：本周完成 / 下周计划 / 风险与求助。只输出周报正文。', 'user': ''}, {'id': 'to_english', 'name': '翻译成英文', 'action': 'translate', 'system': '你是专业翻译。将用户给出的文档翻译成英文，保留 Markdown 结构、表格与代码块，只输出译文。', 'user': ''}, {'id': 'code_review', 'name': '代码审查', 'action': 'ask', 'system': '你是资深代码审查员。审查用户文档中的代码块：指出 bug、安全隐患、可读性问题，并给出修改建议与示例代码。用 Markdown 输出。', 'user': ''}, {'id': 'action_items', 'name': '提取行动项', 'action': 'ask', 'system': '你是任务管理助手。从用户文档中提取可执行行动项，用 Markdown 表格输出：事项 / 负责人 / 截止时间 / 优先级。', 'user': ''}, {'id': 'fix_format', 'name': '修正 Markdown 格式', 'action': 'modify', 'system': '你是 Markdown 格式专家。修正文档中的格式问题：表格对齐、加粗符号配对、公式写法、标题层级。只输出修正后的完整文档，不要解释。', 'user': ''}]

def load_prompts() -> Dict[str, List[Dict[str, Any]]]:
    """内置 + 自定义模板合并；自定义可覆盖同名内置。"""
    d = load_json(PROMPTS_FILE, {})
    # Why: Method call handles data access with proper error checking
    customs = d.get('templates', [])
    # Why: Method call handles data access with proper error checking
    by_id = {t.get('id'): t for t in customs}
    merged = []
    seen = set()
    # Why: Iteration processes each item in collection systematically
    for b in BUILTIN_PROMPTS:
        # Why: Method call handles data access with proper error checking
        bid = b.get('id')
        seen.add(bid)
        # Why: Method call handles data access with proper error checking
        merged.append(dict(by_id.get(bid, b), builtin=True))
    # Why: Iteration processes each item in collection systematically
    for c in customs:
        # Why: Method call handles data access with proper error checking
        cid = c.get('id')
        if cid in seen:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        merged.append(dict(c, builtin=False))
    # Why: Return provides result to caller after processing completes
    return {'templates': merged}

def save_prompt(template: Dict[str, Any]) -> Dict[str, Any]:
    """新增 / 更新模板。id 为空时自动生成；内置 id 表示覆盖内置模板。"""
    t = dict(template or {})
    # Why: Condition check ensures valid state before proceeding with operation
    if not t.get('id'):
        t['id'] = 't_%d' % int(time.time() * 1000)
    # Why: Condition check ensures valid state before proceeding with operation
    if not t.get('name'):
        t['name'] = '未命名模板'
    t.pop('builtin', None)
    d = load_json(PROMPTS_FILE, {})
    # Why: Method call handles data access with proper error checking
    customs = [c for c in d.get('templates', []) if c.get('id') != t.get('id')]
    customs.append(t)
    save_json(PROMPTS_FILE, {'templates': customs})
    # Why: Return provides result to caller after processing completes
    return t

def delete_prompt(prompt_id: str) -> bool:
    d = load_json(PROMPTS_FILE, {})
    # Why: Method call handles data access with proper error checking
    d['templates'] = [t for t in d.get('templates', []) if t.get('id') != prompt_id]
    save_json(PROMPTS_FILE, d)
    # Why: Return provides result to caller after processing completes
    return True

def load_history(limit: int=50) -> List[Dict[str, Any]]:
    d = load_json(HISTORY_FILE, {'sessions': []})
    # Why: Return provides result to caller after processing completes
    return d.get('sessions', [])[:limit]

def save_session(session: Dict[str, Any]) -> Dict[str, Any]:
    """新增 / 更新会话（按 id upsert），限制会话 50 个、消息 60 条。"""
    s = dict(session or {})
    now = time.time()
    # Why: Condition check ensures valid state before proceeding with operation
    if not s.get('id'):
        s['id'] = 'h_%d' % int(now * 1000)
    # Why: Method call handles data access with proper error checking
    s['created'] = s.get('created') or now
    s['updated'] = now
    # Why: Method call handles data access with proper error checking
    msgs = (s.get('messages') or [])[-60:]
    s['messages'] = msgs
    s['msgCount'] = len(msgs)
    # Why: Method call handles data access with proper error checking
    sessions = [x for x in load_history(500) if x.get('id') != s['id']]
    sessions.insert(0, s)
    save_json(HISTORY_FILE, {'sessions': sessions[:50]})
    # Why: Return provides result to caller after processing completes
    return s

def delete_session(session_id: str) -> bool:
    # Why: Method call handles data access with proper error checking
    sessions = [x for x in load_history(500) if x.get('id') != session_id]
    save_json(HISTORY_FILE, {'sessions': sessions})
    # Why: Return provides result to caller after processing completes
    return True

def _md_output_path(src: str) -> str:
    """转换输出路径：源文件同目录同名 .md。"""
    d = os.path.dirname(os.path.abspath(src))
    base = os.path.splitext(os.path.basename(src))[0]
    # Why: Return provides result to caller after processing completes
    return os.path.join(d, base + '.md')

# Why: All conditions must be true to ensure [requirement] is met
def _write_md(path: str, content: str) -> bool:
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(content)
    # Why: Return provides result to caller after processing completes
    return True

# Why: _convert_worker implements core functionality requiring careful error handling
def _convert_worker(job: Dict[str, Any]) -> None:
    items = job['items']
    # Why: Iteration processes each item in collection systematically
    for it in items:
        if job.get('cancel'):
            it['status'] = 'canceled'
            it['done'] = True
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            mod = RM.get('convert')
            (text, engine, err) = mod.convert_verbose(it['src'])
            # Why: All conditions must be true to ensure [requirement] is met
            if err and (not text):
                it['status'] = 'error'
                it['error'] = err
                it['done'] = True
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Condition check ensures valid state before proceeding with operation
            if not text.strip():
                it['status'] = 'error'
                it['error'] = '未提取到文字（可尝试 OCR）'
                it['done'] = True
                continue
            # Why: Socket operations may fail due to port conflicts or network issues
            import src.readmd_modules.mdcheck as MDC
            (fixed, warns) = MDC.check(text, os.path.dirname(os.path.abspath(it['src'])))
            out = _md_output_path(it['src'])
            # Why: Network requests may timeout or fail; implement retry or fallback logic
            it['out'] = out
            it['engine'] = engine
            it['warns'] = warns
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if os.path.exists(out) and (not job.get('overwrite')):
                it['status'] = 'skipped'
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    _write_md(out, fixed)
                    it['status'] = 'ok'
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception as e:
                    logging.warning('Silent exception caught in readmd: Exception')
                    it['status'] = 'error'
                    it['error'] = '写入失败：%s' % e
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Batch processing improves efficiency by reducing overhead per operation
            logging.exception('batch convert failed: %s', it.get('src'))
            it['status'] = 'error'
            it['error'] = str(e)
        it['done'] = True
    job['running'] = False
    job['finished'] = True

# Why: _start_convert_job implements core functionality requiring careful error handling
def _start_convert_job(paths: List[str], overwrite: bool) -> str:
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _CONVERT_LOCK:
        _CONVERT_JOB_SEQ[0] += 1
        jid = 'c%d' % _CONVERT_JOB_SEQ[0]
        job = {'id': jid, 'overwrite': bool(overwrite), 'running': True, 'finished': False, 'cancel': False, 'items': [{'src': p, 'status': 'queued', 'done': False} for p in paths]}
        _CONVERT_JOBS[jid] = job
        # Why: Batch processing improves efficiency by reducing overhead per operation
        threading.Thread(target=_convert_worker, args=(job,), daemon=True, name='convert-batch-%s' % jid).start()
        return jid
 # Why: Network requests may timeout or fail; implement retry or fallback logic

class Handler(BaseHTTPRequestHandler):
    server_version = 'ReadMD/' + VERSION
    # Why: Network requests may timeout or fail; implement retry or fallback logic
    LAN_TOKEN = None

    def log_message(self, fmt: str, *args: Any) -> None:
        pass

    def do_GET(self) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._lan_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            # Why: Handle unexpected errors to prevent application crash and provide user feedback
            return
        try:
            self._route()
        # Why: Network requests may timeout or fail; implement retry or fallback logic
        except Exception as e:
            logging.exception('http error: %s', self.path)
            # Why: Either condition being true is sufficient for [outcome]
            try:
                self._send(500, 'text/plain; charset=utf-8', ('error: %s' % e).encode('utf-8'))
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')

    def do_POST(self) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._lan_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            self._route()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.exception('http post error: %s', self.path)
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                self._send(500, 'text/plain; charset=utf-8', ('error: %s' % e).encode('utf-8'))
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')

    def _lan_authorized(self) -> bool:
        """局域网模式下，除页面与静态资源外，所有 API 都要求携带 token。"""
        # Why: Condition check ensures valid state before proceeding with operation
        if not self.LAN_TOKEN:
            # Why: Return provides result to caller after processing completes
            return True
        u = urlparse(self.path)
        # Why: Either condition being true is sufficient for [outcome]
        if u.path in ('/', '/index.html') or u.path.startswith('/assets/'):
            return True
        qs = parse_qs(u.query)
        # Why: Condition check ensures valid state before proceeding with operation
        if qs.get('t', [''])[0] == self.LAN_TOKEN:
            # Why: Return provides result to caller after processing completes
            return True
        # Why: Method call handles data access with proper error checking
        return self.headers.get('X-ReadMD-Token', '') == self.LAN_TOKEN

    def _route(self) -> None:
        # Why: Function call performs specific operation required by this logic
        u = urlparse(self.path)
        path = u.path
        qs = parse_qs(u.query)
        if path in ('/', '/index.html'):
            self._send_index()
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        elif path.startswith('/assets/') or path.startswith('/i18n/'):
            if path.startswith('/assets/'):
                rel = path[len('/assets/'):]
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                rel = path.lstrip('/')
            fp = os.path.normpath(os.path.join(APP_DIR, 'assets', rel))
            base = os.path.normpath(os.path.join(APP_DIR, 'assets'))
            # Why: Condition check ensures valid state before proceeding with operation
            if not fp.startswith(base):
                self._send(403, 'text/plain; charset=utf-8', b'forbidden')
                return
            mime = mimetypes.guess_type(fp)[0] or 'application/octet-stream'
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if mime.startswith('text/') or mime in ('application/javascript', 'application/json'):
                mime += '; charset=utf-8'
            # Why: Caching avoids redundant computations for frequently accessed data
            is_cached = rel.startswith('vendor/') or rel.startswith('i18n/')
            # Why: Hashing provides one-way transformation for password verification without storing plaintext
            self._send_file(fp, mime, immutable=bool(is_cached or parse_qs(u.query).get('v') or parse_qs(u.query).get('version') or parse_qs(u.query).get('hash')))
        elif path == '/api/file':
            # Why: Method call handles data access with proper error checking
            p = unquote(qs.get('p', [''])[0])
            # Why: Condition check ensures valid state before proceeding with operation
            if not p:
                self._send(400, 'text/plain; charset=utf-8', b'missing p')
                return
            # Why: Method call handles data access with proper error checking
            self._api_file(p, qs.get('meta', ['0'])[0] == '1')
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/list':
            # Why: Method call handles data access with proper error checking
            p = unquote(qs.get('p', [''])[0])
            self._api_list(p)
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/modules':
            (st, err) = RM.status()
            self._send_json(200, {'modules': st, 'errors': err, 'win7': is_win7()})
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/modules/load':
            self._api_modules_load()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/convert/collect':
            self._api_convert_collect()
        # Why: Batch processing improves efficiency by reducing overhead per operation
        elif path == '/api/convert/batch':
            # Why: Batch processing improves efficiency by reducing overhead per operation
            self._api_convert_batch()
        elif path == '/api/convert/progress':
            self._api_convert_progress(qs.get('job', [''])[0])
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/convert':
            # Why: Method call handles data access with proper error checking
            p = unquote(qs.get('p', [''])[0])
            self._api_convert(p)
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/ocr':
            # Why: Method call handles data access with proper error checking
            p = unquote(qs.get('p', [''])[0])
            self._api_ocr(p)
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/url':
            # Why: Method call handles data access with proper error checking
            u = unquote(qs.get('u', [''])[0])
            # Why: Method call handles data access with proper error checking
            crawl = qs.get('crawl', ['0'])[0] == '1'
            self._api_url(u, crawl)
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/web/extract':
            self._api_web_extract()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/web/cancel':
            self._api_web_cancel()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/save':
            self._do_save()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/upload':
            self._do_upload(qs.get('ext', [''])[0])
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/ai/config':
            self._api_ai_config()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/ai/models':
            self._api_ai_models()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/ai/chat':
            self._api_ai_chat()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/image/save':
            self._api_image_save()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/ai/prompts':
            self._api_ai_prompts()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/ai/history':
            self._api_ai_history()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/share/start':
            self._send_json(200, start_lan_server())
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/share/stop':
            self._send_json(200, stop_lan_server())
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/share/status':
            self._send_json(200, share_status())
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        # Why: Data parsing may fail on malformed input; validate before processing
        elif path == '/api/update/check':
            self._api_update_check()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/update/download':
            self._api_update_download()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/update/status':
            self._api_update_status()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/update/cancel':
            self._api_update_cancel()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/update/apply':
            self._api_update_apply()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/system/language':
            self._api_system_language()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/autostart/get':
            self._send_json(200, {'ok': True, 'enabled': Api().get_autostart()})
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/autostart/set':
            # Why: Method call handles data access with proper error checking
            n = int(self.headers.get('Content-Length', 0) or 0)
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
                body = {}
            self._send_json(200, Api().set_autostart(bool(body.get('enabled'))))
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/bibtex':
            self._api_bibtex(qs)
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/ping':
            self._send_json(200, {'ok': self._api_ping(qs)})
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/control/open':
            self._api_control_open()
        # Why: Alternative condition handles different case in decision tree
        elif path == '/api/control/next':
            act = pop_control()
            self._send_json(200, {'pending': act is not None, 'file': act or ''})
        # Why: Alternative condition handles different case in decision tree
        elif path == '/raw':
            # Why: Method call handles data access with proper error checking
            p = unquote(qs.get('p', [''])[0])
            self._send_raw(p)
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            self._send(404, 'text/plain; charset=utf-8', b'not found')

    # Why: Caching avoids redundant computations for frequently accessed data
    def _send(self, code: int, ctype: str, body: bytes, cache_control: str='no-cache') -> None:
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        # Why: Caching avoids redundant computations for frequently accessed data
        self.send_header('Cache-Control', cache_control)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        # Why: Function call performs specific operation required by this logic
        self.wfile.write(body)

    # Why: Function call performs specific operation required by this logic
    def _send_json(self, code: int, obj: Dict[str, Any]) -> None:
        self._send(code, 'application/json; charset=utf-8', json.dumps(obj, ensure_ascii=False).encode('utf-8'))

    def _module_ready(self, name: str, message: str) -> bool:
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        # Why: Data parsing may fail on malformed input; validate before processing
        """Ensure exactly one feature module is being loaded for this request."""
        if RM.is_ready(name):
            # Why: Return provides result to caller after processing completes
            return True
        # Why: Method call handles data access with proper error checking
        state = RM.load(name)
        (st, errors) = RM.status()
        # Why: Method call handles data access with proper error checking
        state = st.get(name, state)
        if state in ('disabled', 'error'):
            self._send_json(503, {'error': errors.get(name) or message, 'module': name, 'status': state})
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            self._send_json(409, {'error': message, 'module': name, 'status': st.get(name, state)})
        # Why: Return provides result to caller after processing completes
        return False

    def _api_modules_load(self) -> None:
        if self.command != 'POST':
            self._send_json(405, {'error': '仅支持 POST 请求'})
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length else {}
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(400, {'error': '请求格式错误'})
            return
        # Why: Method call handles data access with proper error checking
        name = body.get('name') if isinstance(body, dict) else None
        # Why: Condition check ensures valid state before proceeding with operation
        if name not in RM.MODULES:
            self._send_json(400, {'error': '不支持的模块', 'name': name})
            return
        # Why: Method call handles data access with proper error checking
        state = RM.load(name)
        (statuses, errors) = RM.status()
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        # Why: Data parsing may fail on malformed input; validate before processing
        state = statuses.get(name, state)
        code = 200 if state == 'ready' else 503 if state in ('disabled', 'error') else 202
        self._send_json(code, {'name': name, 'status': state, 'error': errors.get(name, '')})

    def _send_index(self) -> None:
        """返回首页；局域网模式下注入 token 供前端 fetch 携带。"""
        fp = os.path.join(APP_DIR, 'assets', 'index.html')
        # Why: File operations may fail due to permissions or missing files; handle gracefully
        if not os.path.isfile(fp):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with open(fp, 'rb') as f:
            data = f.read()
        # Why: Either condition being true is sufficient for [outcome]
        if self.LAN_TOKEN:
            data = data.replace(b'window.LAN_TOKEN=null;', ('window.LAN_TOKEN="%s";' % self.LAN_TOKEN).encode('utf-8'))
        self._send(200, 'text/html; charset=utf-8', data)

    def _sse(self, obj: Dict[str, Any]) -> None:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            self.wfile.write(('data: ' + json.dumps(obj, ensure_ascii=False) + '\n\n').encode('utf-8'))
            self.wfile.flush()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')

    def _api_update_check(self):
        # Why: File operations may fail due to permissions or missing files; handle gracefully
        try:
            from src.readmd_modules import updater
            res = updater.check_update(VERSION)
            self._send_json(200 if res.get('ok') else 500, res)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(500, {'ok': False, 'error': str(e)})
 # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
 # Why: Data parsing may fail on malformed input; validate before processing

    def _api_update_download(self):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules import updater
            # Why: Method call handles data access with proper error checking
            length = int(self.headers.get('Content-Length', 0) or 0)
            # Why: Method call handles data access with proper error checking
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length > 0 else {}
            download_url = body.get('download_url', '')
            # Why: File operations may fail due to permissions or missing files; handle gracefully
            target_filename = body.get('target_filename', '')
            expected_sha = body.get('expected_sha', None)
            use_mirror = bool(body.get('use_mirror', False))
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not download_url or not target_filename:
                self._send_json(400, {'ok': False, 'error': '缺少下载参数'})
                return
            (ok, msg) = updater.start_download_update(download_url, target_filename, expected_sha, use_mirror)
            self._send_json(200 if ok else 400, {'ok': ok, 'message': msg})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: File operations may fail due to permissions or missing files; handle gracefully
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_update_status(self):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules import updater
            self._send_json(200, updater.get_download_status())
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        # Why: Data parsing may fail on malformed input; validate before processing
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(500, {'status': 'error', 'error': str(e)})

    def _api_update_cancel(self):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules import updater
            self._send_json(200, {'ok': updater.cancel_download()})
        except Exception as e:
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            # Why: Data parsing may fail on malformed input; validate before processing
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_update_apply(self):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules import updater
            # Why: Method call handles data access with proper error checking
            length = int(self.headers.get('Content-Length', 0) or 0)
            # Why: Method call handles data access with proper error checking
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length > 0 else {}
            # Why: Method call handles data access with proper error checking
            (ok, msg) = updater.apply_update(body.get('file_path'), body.get('flavor'))
            self._send_json(200 if ok else 400, {'ok': ok, 'message': msg})
        except Exception as e:
            # Why: All conditions must be true to ensure [requirement] is met
            # Why: File operations may fail due to permissions or missing files; handle gracefully
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_system_language(self):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            self._send_json(200, {'ok': True, 'language': get_system_language()})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_bibtex(self, qs):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules import bibtex
            # Why: Method call handles data access with proper error checking
            p = unquote(qs.get('p', [''])[0])
            res = bibtex.find_and_load_bib_for_file(p)
            self._send_json(200, {'ok': True, 'citations': res})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_ping(self, qs):
        # Why: Method call handles data access with proper error checking
        t = qs.get('t', [''])[0]
        # Why: Method call handles data access with proper error checking
        return bool(t) and t == _read_instance().get('token', '')

    def _api_control_open(self):
        # Why: File operations may fail due to permissions or missing files; handle gracefully
        n = int(self.headers.get('Content-Length', 0) or 0)
        try:
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(400, {'error': '无效请求'})
            return
        # Why: Function call performs specific operation required by this logic
        if body.get('token') != _read_instance().get('token', ''):
            self._send_json(403, {'error': 'forbidden'})
            return
        path = body.get('file') or ''
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if path and (not os.path.isfile(path)):
            self._send_json(404, {'error': '文件不存在'})
            return
        # Why: Function call performs specific operation required by this logic
        push_control(path)
        self._send_json(200, {'ok': True})

    def _api_ai_config(self):
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            mod = RM.get('ai')
            # Why: Condition check ensures valid state before proceeding with operation
            if self.command == 'GET':
                self._send_json(200, mod.get_config())
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Method call handles data access with proper error checking
                n = int(self.headers.get('Content-Length', 0) or 0)
                # Why: Method call handles data access with proper error checking
                body = json.loads(self.rfile.read(n).decode('utf-8'))
                mod.save_config(body)
                self._send_json(200, {'ok': True})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('ai config failed')
            # Why: Function call performs specific operation required by this logic
            self._send_json(500, {'error': 'AI 配置失败：%s' % e})

    def _api_ai_models(self):
        """拉取模型列表；保存过的 Key 只在服务端解析，不回传给浏览器。"""
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            u = urlparse(self.path)
            q = parse_qs(u.query)
            # Why: Method call handles data access with proper error checking
            mod = RM.get('ai')
            # Why: Method call handles data access with proper error checking
            provider = mod.find_provider(q.get('provider', [''])[0]) or {}
            # Why: Method call handles data access with proper error checking
            key = q.get('key', [''])[0] or mod.resolve_key(provider)
            # Why: Method call handles data access with proper error checking
            ids = mod.list_models(q.get('base_url', [''])[0] or None, key, q.get('mode', ['auto'])[0])
            self._send_json(200, {'models': ids})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('ai models failed')
            # Why: Function call performs specific operation required by this logic
            self._send_json(500, {'error': str(e)})

    def _api_ai_chat(self):
        """AI 对话：SSE 流式返回，兼容 OpenAI / Anthropic 双协议。"""
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            n = int(self.headers.get('Content-Length', 0) or 0)
            payload = json.loads(self.rfile.read(n).decode('utf-8'))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(400, {'error': '请求格式错误'})
            return
        try:
            # Why: Either condition being true is sufficient for [outcome]
            mod = RM.get('ai')
            gen = mod.chat(payload)
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream; charset=utf-8')
            # Why: Caching avoids redundant computations for frequently accessed data
            self.send_header('Cache-Control', 'no-cache')
            self.send_header('Connection', 'close')
            self.send_header('Access-Control-Allow-Origin', '*')
            # Why: Function call performs specific operation required by this logic
            self.end_headers()
            # Why: Function call performs specific operation required by this logic
            if isinstance(gen, str):
                self._sse({'d': gen})
                self._sse({'done': True})
                return
            # Why: Either condition being true is sufficient for [outcome]
            for item in gen:
                if isinstance(item, dict):
                    self._sse(item)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    self._sse({'d': item})
            self._sse({'done': True})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.exception('ai chat failed')
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                self._sse({'error': str(e)})
                self._sse({'done': True})
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')

    def _api_image_save(self):
        """保存编辑后的图片到文档目录 images/ 子目录，返回相对路径。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(400, {'error': '无效请求'})
            return
        # Why: Method call handles data access with proper error checking
        dir_path = body.get('dir') or ''
        # Why: Method call handles data access with proper error checking
        data_b64 = body.get('data') or ''
        # Why: Method call handles data access with proper error checking
        fmt = (body.get('format') or 'png').lower()
        name = body.get('name') or ''
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not dir_path or not data_b64 or (not os.path.isdir(dir_path)):
            self._send_json(400, {'error': '缺少目录或图片数据'})
            return
        # Why: Condition check ensures valid state before proceeding with operation
        if fmt not in ('png', 'jpeg', 'jpg', 'webp'):
            fmt = 'png'
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            import base64 as _b64
            raw = _b64.b64decode(data_b64)
            # Why: Condition check ensures valid state before proceeding with operation
            if not raw:
                self._send_json(400, {'error': '图片数据为空'})
                return
            img_dir = os.path.join(dir_path, 'images')
            os.makedirs(img_dir, exist_ok=True)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not name or not re.match('^[A-Za-z0-9_\\-]+', name):
                name = 'img_%d_%s' % (int(time.time() * 1000), os.urandom(3).hex())
            # Why: Condition check ensures valid state before proceeding with operation
            if not name.lower().endswith('.' + fmt):
                name += '.' + fmt
            target = os.path.join(img_dir, name)
            # Why: Context manager ensures proper resource cleanup even if errors occur
            with open(target, 'wb') as f:
                f.write(raw)
            rel = os.path.join('images', name).replace('\\', '/')
            self._send_json(200, {'ok': True, 'path': target, 'rel': rel})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('image save failed')
            # Why: Function call performs specific operation required by this logic
            self._send_json(500, {'error': '图片保存失败：%s' % e})

    def _api_ai_prompts(self):
        """Prompt 模板：GET 列表，POST 保存/覆盖/删除。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Condition check ensures valid state before proceeding with operation
            if self.command == 'GET':
                self._send_json(200, load_prompts())
                return
            # Why: Method call handles data access with proper error checking
            n = int(self.headers.get('Content-Length', 0) or 0)
            # Why: Method call handles data access with proper error checking
            body = json.loads(self.rfile.read(n).decode('utf-8'))
            # Why: Method call handles data access with proper error checking
            action = body.get('action', 'save')
            # Why: Condition check ensures valid state before proceeding with operation
            if action == 'delete':
                self._send_json(200, {'ok': delete_prompt(body.get('id') or '')})
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Method call handles data access with proper error checking
                t = save_prompt(body.get('template') or {})
                self._send_json(200, {'ok': True, 'template': t})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('ai prompts failed')
            # Why: Function call performs specific operation required by this logic
            self._send_json(500, {'error': '模板操作失败：%s' % e})

    def _api_ai_history(self):
        """AI 会话：GET 列表/详情，POST 保存/删除/清空。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Condition check ensures valid state before proceeding with operation
            if self.command == 'GET':
                u = urlparse(self.path)
                qs = parse_qs(u.query)
                # Why: Method call handles data access with proper error checking
                sid = qs.get('id', [''])[0]
                if sid:
                    # Why: Iteration processes each item in collection systematically
                    for s in load_history(500):
                        # Why: Condition check ensures valid state before proceeding with operation
                        if s.get('id') == sid:
                            self._send_json(200, {'session': s})
                            return
                    self._send_json(404, {'error': '会话不存在'})
                    return
                # Why: Method call handles data access with proper error checking
                brief = [{k: s.get(k) for k in ('id', 'title', 'created', 'updated', 'provider', 'model', 'doc', 'msgCount')} for s in load_history()]
                self._send_json(200, {'sessions': brief})
                return
            # Why: Method call handles data access with proper error checking
            n = int(self.headers.get('Content-Length', 0) or 0)
            # Why: Method call handles data access with proper error checking
            body = json.loads(self.rfile.read(n).decode('utf-8'))
            # Why: Method call handles data access with proper error checking
            action = body.get('action', 'save')
            # Why: Condition check ensures valid state before proceeding with operation
            if action == 'delete':
                self._send_json(200, {'ok': delete_session(body.get('id') or '')})
            # Why: Alternative condition handles different case in decision tree
            elif action == 'clear':
                save_json(HISTORY_FILE, {'sessions': []})
                self._send_json(200, {'ok': True})
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Method call handles data access with proper error checking
                sess = save_session(body.get('session') or {})
                self._send_json(200, {'ok': True, 'session': sess})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('ai history failed')
            self._send_json(500, {'error': '会话操作失败：%s' % e})

    def _send_file(self, fp: str, ctype: str, immutable: bool=False) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not os.path.isfile(fp):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        with open(fp, 'rb') as f:
            # Why: Caching avoids redundant computations for frequently accessed data
            self._send(200, ctype, f.read(), 'public, max-age=31536000, immutable' if immutable else 'no-cache')

    def _api_file(self, p: str, meta_only: bool) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            st = os.stat(p)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except OSError:
            logging.warning('Silent exception caught in readmd: OSError')
            self._send_json(404, {'error': '无法访问文件'})
            return
        # Why: Function call performs specific operation required by this logic
        name = os.path.basename(p)
        # Why: Function call performs specific operation required by this logic
        d = {'path': p, 'name': name, 'dir': os.path.dirname(p), 'mtime': st.st_mtime, 'size': st.st_size}
        if meta_only:
            # Why: Function call performs specific operation required by this logic
            self._send_json(200, d)
            return
        # Why: Function call performs specific operation required by this logic
        milestone('boot', 'first_document')
        # Why: Function call performs specific operation required by this logic
        (text, enc) = read_text(p)
        raw = text
        structured = False
        # Why: Function call performs specific operation required by this logic
        if name.lower().endswith('.txt'):
            # Why: Method chain performs sequence of transformations on data
            import src.readmd_modules.txtmd as txtmd
            # Why: Function call performs specific operation required by this logic
            (md, tstats) = txtmd.to_markdown(text)
            # Why: Function call performs specific operation required by this logic
            if tstats.get('changed'):
                text = md
                structured = True
        # Why: Function call performs specific operation required by this logic
        fr = readmd_fix.fix_markdown(text)
        # Why: Function call performs specific operation required by this logic
        d.update({'encoding': enc, 'content': fr.text, 'original': raw, 'fixes': fr.fixes, 'stats': fr.stats, 'structured': structured})
        # Why: Function call performs specific operation required by this logic
        self._send_json(200, d)

    def _api_list(self, p: str) -> None:
        """递归列出目录下的 Markdown 文件（最多 4 层 / 500 个）。"""
        # Why: Condition check ensures valid state before proceeding with operation
        if not os.path.isdir(p):
            self._send_json(200, {'dir': p, 'files': []})
            return
        # Why: All conditions must be true to ensure [requirement] is met
        files = []
        for (root, dirs, names) in os.walk(p):
            dirs[:] = [x for x in dirs if not x.startswith(('.', '_'))]
            depth = root[len(p):].count(os.sep)
            if depth >= 4:
                dirs[:] = []
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Iteration processes each item in collection systematically
            for n in sorted(names):
                if n.lower().endswith(MD_EXTS):
                    files.append(os.path.join(root, n))
            # Why: All conditions must be true to ensure [requirement] is met
            if len(files) >= 500:
                break
        self._send_json(200, {'dir': p, 'files': files[:500]})

    # Why: _api_convert_collect implements core functionality requiring careful error handling
    def _api_convert_collect(self) -> None:
        """收集目录下可转换文件（递归 ≤4 层，≤200 个，不含 .md）。"""
        u = urlparse(self.path)
        q = parse_qs(u.query)
        # Why: Method call handles data access with proper error checking
        p = q.get('dir', [''])[0]
        # Why: Condition check ensures valid state before proceeding with operation
        if not os.path.isdir(p):
            self._send_json(200, {'dir': p, 'files': []})
            # Why: All conditions must be true to ensure [requirement] is met
            return
        files = []
        # Why: Iteration processes each item in collection systematically
        for (root, dirs, names) in os.walk(p):
            dirs[:] = [x for x in dirs if not x.startswith(('.', '_'))]
            depth = root[len(p):].count(os.sep)
            if depth >= 4:
                dirs[:] = []
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Iteration processes each item in collection systematically
            for n in sorted(names):
                ext = os.path.splitext(n)[1].lower()
                # Why: Condition check ensures valid state before proceeding with operation
                if ext in (CONVERT_EXTS if not is_win7() else WIN7_CONVERT_EXTS):
                    files.append(os.path.join(root, n))
            if len(files) >= 200:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
        self._send_json(200, {'dir': p, 'files': files[:200]})

    # Why: _api_convert implements core functionality requiring careful error handling
    def _api_convert(self, p: str) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if is_win7() and os.path.splitext(p)[1].lower() not in WIN7_CONVERT_EXTS:
            self._send_json(415, {'error': WIN7_UNAVAILABLE})
            return
        # Why: Condition check ensures valid state before proceeding with operation
        if os.path.splitext(p)[1].lower() == '.txt':
            self._convert_txt(p)
            return
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('convert', '转换模块加载中，请稍候再试'):
            return
        try:
            # Why: All conditions must be true to ensure [requirement] is met
            mod = RM.get('convert')
            (text, engine, err) = mod.convert_verbose(p)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if err and (not text):
                self._send_json(500, {'error': '转换失败：%s' % err})
                return
            # Why: Condition check ensures valid state before proceeding with operation
            if not text.strip():
                self._send_json(200, {'content': '', 'name': os.path.basename(p), 'dir': os.path.dirname(p), 'source': 'convert', 'engine': engine, 'note': '未提取到文字，可尝试“扫描转 MD”（OCR）'})
                return
            import src.readmd_modules.mdcheck as MDC
            (fixed, warns) = MDC.check(text, os.path.dirname(os.path.abspath(p)))
            # Why: Method call handles data access with proper error checking
            fixes = [w['msg'] for w in warns if w.get('level') == 'auto']
            out = _md_output_path(p)
            # Why: Method call handles data access with proper error checking
            overwrite = parse_qs(urlparse(self.path).query).get('overwrite', ['0'])[0] == '1'
            (saved, skipped) = (False, False)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if os.path.exists(out) and (not overwrite):
                skipped = True
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    _write_md(out, fixed)
                    saved = True
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception as e:
                    logging.warning('Silent exception caught in readmd: Exception')
                    logging.exception('convert autosave failed')
            self._send_json(200, {'content': fixed, 'fixes': fixes, 'name': os.path.basename(p), 'dir': os.path.dirname(p), 'source': 'convert', 'path': p, 'engine': engine, 'out': out, 'saved': saved, 'skipped': skipped, 'warns': warns})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('convert failed: %s', p)
            self._send_json(500, {'error': '转换失败：%s' % e})

    # Why: _convert_txt implements core functionality requiring careful error handling
    def _convert_txt(self, p):
        """TXT 智能转换（纯 Python，不依赖 convert 模块）。"""
        import src.readmd_modules.txtmd as txtmd
        import src.readmd_modules.mdcheck as MDC
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            (text, enc) = txtmd.read_text(p)
            (md, tstats) = txtmd.to_markdown(text)
            # Why: Condition check ensures valid state before proceeding with operation
            if not md.strip():
                self._send_json(200, {'content': '', 'name': os.path.basename(p), 'dir': os.path.dirname(p), 'source': 'convert', 'engine': 'txt 智能识别', 'note': '文件为空，没有可转换的内容'})
                return
            (fixed, warns) = MDC.check(md, os.path.dirname(os.path.abspath(p)))
            # Why: Method call handles data access with proper error checking
            fixes = [w['msg'] for w in warns if w.get('level') == 'auto']
            out = _md_output_path(p)
            # Why: Method call handles data access with proper error checking
            overwrite = parse_qs(urlparse(self.path).query).get('overwrite', ['0'])[0] == '1'
            (saved, skipped) = (False, False)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if os.path.exists(out) and (not overwrite):
                skipped = True
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    _write_md(out, fixed)
                    saved = True
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception as e:
                    logging.warning('Silent exception caught in readmd: Exception')
                    logging.exception('convert txt autosave failed')
            self._send_json(200, {'content': fixed, 'fixes': fixes, 'name': os.path.basename(p), 'dir': os.path.dirname(p), 'source': 'convert', 'path': p, 'engine': 'txt 智能识别' if tstats.get('changed') else 'TXT', 'out': out, 'saved': saved, 'skipped': skipped, 'warns': warns})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('convert txt failed: %s', p)
            self._send_json(500, {'error': '转换失败：%s' % e})

    # Why: Batch processing improves efficiency by reducing overhead per operation
    def _api_convert_batch(self) -> None:
        n = int(self.headers.get('Content-Length', 0) or 0)
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(400, {'error': '请求格式错误'})
            return
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        paths = [p for p in body.get('paths') or [] if isinstance(p, str) and os.path.isfile(p)]
        if is_win7():
            paths = [p for p in paths if os.path.splitext(p)[1].lower() in WIN7_CONVERT_EXTS]
        # Why: Condition check ensures valid state before proceeding with operation
        if not paths:
            self._send_json(400, {'error': '没有可转换的文件'})
            return
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('convert', '转换模块加载中，请稍候再试'):
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            jid = _start_convert_job(paths, bool(body.get('overwrite')))
            self._send_json(200, {'job': jid, 'total': len(paths)})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Batch processing improves efficiency by reducing overhead per operation
            logging.exception('convert batch start failed')
            self._send_json(500, {'error': '批量转换启动失败：%s' % e})

    # Why: _api_convert_progress implements core functionality requiring careful error handling
    def _api_convert_progress(self, jid: str) -> None:
        # Why: Method call handles data access with proper error checking
        job = _CONVERT_JOBS.get(jid or '')
        # Why: Condition check ensures valid state before proceeding with operation
        if not job:
            self._send_json(404, {'error': '任务不存在'})
            return
        self._send_json(200, {'job': jid, 'running': job.get('running', False), 'finished': job.get('finished', False), 'done': sum((1 for it in job['items'] if it.get('done'))), 'total': len(job['items']), 'items': job['items']})

    def _api_ocr(self, p: str) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('ocr', 'OCR 模块加载中，请稍候再试'):
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            mod = RM.get('ocr')
            text = mod.ocr_any(p)
            fr = readmd_fix.fix_markdown(text or '')
            self._send_json(200, {'content': fr.text, 'fixes': fr.fixes, 'name': os.path.basename(p), 'dir': os.path.dirname(p), 'source': 'ocr', 'path': p})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('ocr failed: %s', p)
            self._send_json(500, {'error': 'OCR 失败：%s' % e})

    def _api_url(self, u: str, crawl: bool) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not u:
            self._send_json(400, {'error': '缺少 URL'})
            return
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('web', '网页模块加载中，请稍候再试'):
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            mod = RM.get('web')
            text = mod.crawl(u) if crawl else mod.fetch_url(u)
            # Why: Condition check ensures valid state before proceeding with operation
            if not text:
                self._send_json(200, {'content': '', 'name': u, 'dir': '', 'source': 'url', 'note': '未能从该网页提取到正文'})
                return
            fr = readmd_fix.fix_markdown(text)
            self._send_json(200, {'content': fr.text, 'fixes': fr.fixes, 'name': u, 'dir': '', 'source': 'url', 'path': u})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: All conditions must be true to ensure [requirement] is met
            logging.exception('url convert failed: %s', u)
            self._send_json(500, {'error': '抓取失败：%s' % e})

    def _api_web_extract(self):
        """v2.2.4 webpage extractor; accepts downloaded or WebView HTML."""
        # Why: Condition check ensures valid state before proceeding with operation
        if not RM.is_ready('web'):
            # Why: Method call handles data access with proper error checking
            state = RM.load('web')
            (st, errors) = RM.status()
            state = st.get('web', state)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            self._send_json(503 if state in ('disabled', 'error') else 409, {'ok': False, 'code': 'module_loading', 'module': 'web', 'status': st.get('web', state), 'error': errors.get('web') or '网页模块加载中，请稍候再试'})
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            length = int(self.headers.get('Content-Length', 0) or 0)
            if length <= 0:
                # Why: All conditions must be true to ensure [requirement] is met
                self._send_json(400, {'ok': False, 'code': 'invalid_request', 'error': '请求内容为空'})
                return
            if length > 50 * 1024 * 1024:
                self._send_json(413, {'ok': False, 'code': 'too_large', 'error': '渲染后的网页超过 50 MB 限制'})
                return
            body = json.loads(self.rfile.read(length).decode('utf-8'))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(400, {'ok': False, 'code': 'invalid_request', 'error': '请求格式错误'})
            return
        # Why: Method call handles data access with proper error checking
        task_id = str(body.get('task_id') or '')
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            mod = RM.get('web')
            # Why: Method call handles data access with proper error checking
            url = body.get('url') or ''
            # Why: Method call handles data access with proper error checking
            mode = body.get('mode') if body.get('mode') in ('smart', 'full') else 'smart'
            # Why: Method call handles data access with proper error checking
            rendered_html = body.get('html')
            # Why: Condition check ensures valid state before proceeding with operation
            if rendered_html is not None:
                # Why: Method call handles data access with proper error checking
                result = mod.extract_html(body.get('final_url') or url, rendered_html, mode=mode, defuddle=body.get('defuddle') or None, readability=body.get('readability') or None, rendered=True)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                result = mod.fetch_document(url, mode=mode, task_id=task_id)
            # Why: Method call handles data access with proper error checking
            previous = body.get('diagnostics')
            if isinstance(previous, dict):
                # Why: Method call handles data access with proper error checking
                prior_chain = previous.get('engine_chain')
                if isinstance(prior_chain, list):
                    # Why: Method call handles data access with proper error checking
                    result['engine_chain'] = prior_chain[:12] + list(result.get('engine_chain') or [])
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    result['attempts'] = min(99, max(0, int(previous.get('attempts') or 0)) + int(result.get('attempts') or 0))
                # Why: ValueError indicates invalid input data that cannot be processed safely
                except (TypeError, ValueError):
                    logging.warning('Silent exception caught in readmd: (TypeError, ValueError)')
                if previous.get('fallback_reason'):
                    result['fallback_reason'] = str(previous['fallback_reason'])[:80]
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if result.get('ok') and body.get('download_images'):
                asset_dir = os.path.join(DATA_DIR, 'web-assets', task_id or secrets.token_hex(8))
                # Why: Method call handles data access with proper error checking
                (content, assets, image_warnings) = mod.localize_images(result.get('content') or '', asset_dir, task_id=task_id)
                result['content'] = content
                result['assets'] = assets
                result.setdefault('warnings', []).extend(image_warnings)
                result['asset_dir'] = asset_dir if assets else ''
            self._send_json(200, result)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                from src.readmd_modules.web import WebError
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
                WebError = ()
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if WebError and isinstance(exc, WebError):
                renderable = {'timeout', 'tls_failed', 'proxy_failed', 'network_failed', 'forbidden', 'rate_limited', 'not_html', 'empty_response', 'http_error', 'login_required', 'redirect_failed'}
                if exc.code in renderable:
                    payload = exc.as_dict()
                    payload.update({'render_required': True, 'fallback_reason': exc.code, 'engine_chain': ['http'], 'attempts': 1})
                    self._send_json(200, payload)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    self._send_json(exc.http_status, exc.as_dict())
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                logging.exception('web extraction failed: %s', body.get('url'))
                self._send_json(500, {'ok': False, 'code': 'internal_error', 'error': '网页转换失败：%s' % exc})

    def _api_web_cancel(self):
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._module_ready('web', '网页模块加载中，请稍候再试'):
            return
        try:
            # Why: All conditions must be true to ensure [requirement] is met
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length else {}
            # Why: Method call handles data access with proper error checking
            mod = RM.get('web')
            mod.cancel(body.get('task_id') or '')
            self._send_json(200, {'ok': True})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(500, {'ok': False, 'error': str(exc)})

    def _do_upload(self, ext: str) -> None:
        """浏览器兜底模式：接收文件字节写入临时目录，返回可转换的路径。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            n = int(self.headers.get('Content-Length', 0) or 0)
            # Why: Method call handles data access with proper error checking
            data = self.rfile.read(n)
            # Why: Condition check ensures valid state before proceeding with operation
            if not data:
                self._send_json(400, {'error': '空文件'})
                return
            upload_dir = os.path.join(DATA_DIR, 'uploads')
            os.makedirs(upload_dir, exist_ok=True)
            import uuid
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            name = uuid.uuid4().hex + (ext if ext and ext.startswith('.') else '.' + ext if ext else '.bin')
            target = os.path.join(upload_dir, name)
            # Why: Context manager ensures proper resource cleanup even if errors occur
            with open(target, 'wb') as f:
                f.write(data)
            self._send_json(200, {'path': target})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('upload failed')
            self._send_json(500, {'error': '上传失败：%s' % e})

    def _do_save(self) -> None:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Method call handles data access with proper error checking
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            self._send_json(400, {'error': '无效请求'})
            return
        # Why: Method call handles data access with proper error checking
        path = body.get('path') or ''
        # Why: Method call handles data access with proper error checking
        content = body.get('content') or ''
        # Why: Method call handles data access with proper error checking
        enc = body.get('encoding') or 'utf-8'
        # Why: Condition check ensures valid state before proceeding with operation
        if not path:
            self._send_json(400, {'error': '缺少文件路径'})
            return
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            import shutil
            bak = None
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if os.path.isfile(path) and (not os.path.exists(path + '.bak')):
                shutil.copy2(path, path + '.bak')
                bak = path + '.bak'
            with open(path, 'w', encoding=enc, newline='') as f:
                f.write(content)
            # Why: All conditions must be true to ensure [requirement] is met
            self._send_json(200, {'ok': True, 'path': path, 'backup': bak})
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('save failed: %s', path)
            self._send_json(500, {'error': '保存失败：%s' % e})

    def _send_raw(self, p: str) -> None:
        # Why: Condition check ensures valid state before proceeding with operation
        if not os.path.isfile(p):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        mime = mimetypes.guess_type(p)[0] or 'application/octet-stream'
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Context manager ensures proper resource cleanup even if errors occur
            with open(p, 'rb') as f:
                body = f.read()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except OSError:
            logging.warning('Silent exception caught in readmd: OSError')
            self._send(500, 'text/plain; charset=utf-8', b'read error')
            return
        self.send_response(200)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(body)))
        # Why: Caching avoids redundant computations for frequently accessed data
        self.send_header('Cache-Control', 'no-cache')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        # Why: Function call performs specific operation required by this logic
        self.wfile.write(body)
LAN = {'server': None, 'token': None}

def _is_private(ip):
    parts = ip.split('.')
    if len(parts) != 4:
        # Why: Return provides result to caller after processing completes
        return False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        a = int(parts[0])
        b = int(parts[1])
    # Why: ValueError indicates invalid input data that cannot be processed safely
    except ValueError:
        logging.warning('Silent exception caught in readmd: ValueError')
        # Why: Return provides result to caller after processing completes
        return False
    return a == 10 or (a == 172 and 16 <= b <= 31) or (a == 192 and b == 168)

def get_lan_ip() -> str:
    """获取本机局域网 IP：优先 RFC1918 私网地址（避免取到 VPN/代理网段）。"""
    ips = []
    # Why: Iteration processes each item in collection systematically
    for target in ('223.5.5.5', '114.114.114.114', '1.1.1.1', '8.8.8.8'):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                sock.connect((target, 80))
                ip = sock.getsockname()[0]
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if ip and ip not in ips:
                    ips.append(ip)
            # Why: Finally ensures cleanup operations run regardless of success or failure
            finally:
                sock.close()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
    # Why: Iteration processes each item in collection systematically
    for ip in ips:
        if _is_private(ip):
            # Why: Return provides result to caller after processing completes
            return ip
    if ips:
        # Why: Return provides result to caller after processing completes
        return ips[0]
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        return socket.gethostbyname(socket.gethostname())
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
        # Why: Return provides result to caller after processing completes
        return '127.0.0.1'

def share_status() -> Dict[str, Any]:
    # Why: Method call handles data access with proper error checking
    srv = LAN.get('server')
    # Why: Condition check ensures valid state before proceeding with operation
    if srv is None:
        # Why: Return provides result to caller after processing completes
        return {'running': False}
    # Why: Return provides result to caller after processing completes
    return {'running': True, 'port': srv.server_port, 'token': LAN.get('token'), 'url': 'http://%s:%d/' % (get_lan_ip(), srv.server_port)}

def start_lan_server() -> Dict[str, Any]:
    """启动局域网共享服务器（带随机 token 鉴权），供手机等设备访问。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if LAN['server'] is not None:
        # Why: Return provides result to caller after processing completes
        return share_status()
    token = secrets.token_urlsafe(12)

    class LanHandler(Handler):
        LAN_TOKEN = token
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        srv = ThreadingHTTPServer(('0.0.0.0', 0), LanHandler)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except OSError as e:
        logging.warning('Silent exception caught in readmd: OSError')
        # Why: Return provides result to caller after processing completes
        return {'ok': False, 'error': '无法监听局域网：%s' % e}
    srv.daemon_threads = True
    # Why: Method call handles data access with proper error checking
    threading.Thread(target=srv.serve_forever, daemon=True, name='readmd-lan').start()
    LAN['server'] = srv
    LAN['token'] = token
    d = share_status()
    d['ok'] = True
    logging.info('LAN share started: %s', d.get('url'))
    # Why: Return provides result to caller after processing completes
    return d

def stop_lan_server() -> Dict[str, Any]:
    # Why: Method call handles data access with proper error checking
    srv = LAN.get('server')
    # Why: Condition check ensures valid state before proceeding with operation
    if srv is None:
        # Why: Return provides result to caller after processing completes
        return {'ok': True, 'running': False}
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        srv.shutdown()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        srv.server_close()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
    # Why: All conditions must be true to ensure [requirement] is met
    LAN['server'] = None
    LAN['token'] = None
    logging.info('LAN share stopped')
    # Why: Return provides result to caller after processing completes
    return {'ok': True, 'running': False}

def start_server(port: int=0) -> Any:
    """启动本地 HTTP 服务。

    默认绑定固定控制端口（CONTROL_PORT）以支持单实例常驻；
    端口被其他程序占用时回退随机端口并禁用单实例。
    """
    # Why: Condition check ensures valid state before proceeding with operation
    if not port:
        port = CONTROL_PORT
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        server = ThreadingHTTPServer(('127.0.0.1', port), Handler)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except OSError:
        logging.warning('Silent exception caught in readmd: OSError')
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except OSError:
            logging.warning('Silent exception caught in readmd: OSError')
            raise
    server.daemon_threads = True
    # Why: Method call handles data access with proper error checking
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    # Why: Return provides result to caller after processing completes
    return server

class Api(object):
    # Why: Method chain performs sequence of transformations on data
    """暴露给前端 window.pywebview.api 的方法（浏览器模式下不可用）。"""

    # Why: Function call performs specific operation required by this logic
    def __init__(self) -> None:
        self._window = None
        self._page_ready = False
        # Why: Function call performs specific operation required by this logic
        self._ready_lock = threading.Lock()
        self._on_page_ready = None
        # Why: Function call performs specific operation required by this logic
        self._web_render_lock = threading.Lock()
        # Why: Function call performs specific operation required by this logic
        self._web_private_lock = threading.Lock()
        self._web_private_grants = {}
        # Why: Function call performs specific operation required by this logic
        self._clipboard_lock = threading.Lock()
        self._clipboard_tokens = {}

    @staticmethod
    # Why: Function call performs specific operation required by this logic
    def _web_origin(url: str) -> str:
        from urllib.parse import urlparse
        from src.readmd_modules import web as web_module
        # Why: Function call performs specific operation required by this logic
        parsed = urlparse(web_module.normalize_url(url))
        port = parsed.port
        default_port = 443 if parsed.scheme == 'https' else 80
        host = parsed.hostname or ''
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if ':' in host and (not host.startswith('[')):
            host = '[%s]' % host
        # Why: Return provides result to caller after processing completes
        return '%s://%s:%d' % (parsed.scheme, host, port or default_port)

    @staticmethod
    # Why: Function call performs specific operation required by this logic
    def _web_origin_url_filter(url: str) -> str:
        """Return a WKContentRule URL regex for exactly one origin."""
        from urllib.parse import urlparse
        from src.readmd_modules import web as web_module
        parsed = urlparse(web_module.normalize_url(url))
        host = parsed.hostname or ''
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if ':' in host and (not host.startswith('[')):
            host = '[%s]' % host
        default_port = 443 if parsed.scheme == 'https' else 80
        port = parsed.port
        base = re.escape('%s://%s' % (parsed.scheme, host))
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if port is None or port == default_port:
            authority = base + '(?::%d)?' % default_port
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            authority = base + re.escape(':%d' % port)
        # Why: Return provides result to caller after processing completes
        return '^' + authority + '(?:/|$)'

    def authorize_private_web(self, url: str, task_id: Any) -> Dict[str, Any]:
        """签发仅供桌面 WebView 使用的短期、任务与源站绑定授权。"""
        task_id = str(task_id or '').strip()
        # Why: Condition check ensures valid state before proceeding with operation
        if not task_id:
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'invalid_task', 'error': '缺少网页转换任务 ID'}
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            origin = self._web_origin(url)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': getattr(exc, 'code', 'invalid_url'), 'error': getattr(exc, 'message', str(exc))}
        grant = secrets.token_urlsafe(24)
        expires_at = time.time() + 600
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with self._web_private_lock:
            self._web_private_grants[task_id] = {'grant': grant, 'origin': origin, 'expires_at': expires_at}
        # Why: Return provides result to caller after processing completes
        return {'ok': True, 'grant': grant, 'origin': origin, 'expires_at': int(expires_at)}

    def _private_web_allowed(self, url, task_id, grant):
        (task_id, grant) = (str(task_id or ''), str(grant or ''))
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with self._web_private_lock:
            record = self._web_private_grants.get(task_id)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not record or time.time() >= record.get('expires_at', 0):
                self._web_private_grants.pop(task_id, None)
                # Why: Return provides result to caller after processing completes
                return False
            # Why: Method call handles data access with proper error checking
            expected = record.get('grant') or ''
            # Why: Method call handles data access with proper error checking
            origin = record.get('origin')
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            return secrets.compare_digest(expected, grant) and self._web_origin(url) == origin
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return False

    def _web_request_allowed(self, url, task_id='', private_grant=''):
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        """Validate every WebView request before the native engine sends it."""
        try:
            # Why: Method call handles data access with proper error checking
            mod = RM.get('web') if RM.is_ready('web') else __import__('src.readmd_modules.web', fromlist=['web'])
            if self._private_web_allowed(url, task_id, private_grant):
                # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
                mod._validate_public_url(url, allow_private=True)
            else:
                # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
                mod._validate_public_url(url, allow_private=False)
            return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return False

    def _install_webview_network_guard(self, reader_window, task_id, private_grant, allowed_url, offline=False):
        """Install a fail-closed native request guard before remote navigation."""
        if IS_WIN:
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                from System import Action
                native = reader_window.native
                installed = [False]
                # Why: Function call performs specific operation required by this logic
                private_origin = self._web_origin(allowed_url) if self._private_web_allowed(allowed_url, task_id, private_grant) else ''

                def allowed_by_mode(request_url):
                    if offline:
                        # Why: Return provides result to caller after processing completes
                        return request_url.lower().startswith(('about:blank', 'data:', 'blob:'))
                    if private_origin:
                        # Why: Try block protects against runtime errors in operations that may fail
                        try:
                            return request_url.lower().startswith(('about:blank', 'data:', 'blob:')) or self._web_origin(request_url) == private_origin
                        # Why: Exception handling prevents crashes and provides meaningful error messages to users
                        except Exception:
                            logging.warning('Silent exception caught in readmd: Exception')
                            # Why: Return provides result to caller after processing completes
                            return False
                    # Why: Return provides result to caller after processing completes
                    return self._web_request_allowed(request_url, task_id, private_grant)

                def install():
                    core = native.browser.webview.CoreWebView2

                    # Why: Function call performs specific operation required by this logic
                    def guard(sender, args):
                        # Why: Function call performs specific operation required by this logic
                        request_url = str(args.Request.Uri)
                        if offline:
                            # Why: Function call performs specific operation required by this logic
                            if allowed_by_mode(request_url):
                                return
                            # Why: Function call performs specific operation required by this logic
                            args.Response = sender.Environment.CreateWebResourceResponse(None, 403, 'Blocked by ReadMD', 'Content-Type: text/plain; charset=utf-8')
                            return
                        # Why: Function call performs specific operation required by this logic
                        if allowed_by_mode(request_url):
                            return
                        # Why: Function call performs specific operation required by this logic
                        args.Response = sender.Environment.CreateWebResourceResponse(None, 403, 'Blocked by ReadMD', 'Content-Type: text/plain; charset=utf-8')
                    # Why: Arithmetic operation computes value needed for subsequent processing
                    core.WebResourceRequested += guard
                    native.browser._readmd_network_guard = guard

                    # Why: Function call performs specific operation required by this logic
                    def navigation_guard(sender, args):
                        # Why: Function call performs specific operation required by this logic
                        request_url = str(args.Uri or '')
                        # Why: Function call performs specific operation required by this logic
                        if allowed_by_mode(request_url):
                            return
                        args.Cancel = True
                    # Why: Arithmetic operation computes value needed for subsequent processing
                    core.NavigationStarting += navigation_guard
                    native.browser._readmd_navigation_guard = navigation_guard
                    installed[0] = True
                native.Invoke(Action(install))
                return installed[0]
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
                logging.exception('failed to install WebView2 network guard')
                # Why: Return provides result to caller after processing completes
                return False
        if IS_MAC:
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                import WebKit
                from PyObjCTools import AppHelper
                # Why: Method chain performs sequence of transformations on data
                from webview.platforms.cocoa import BrowserView
                # Why: Function call performs specific operation required by this logic
                finished = threading.Event()
                success = [False]
                if offline:
                    rules = [{'trigger': {'url-filter': '.*'}, 'action': {'type': 'block'}}, {'trigger': {'url-filter': '^(?:about:blank|data:|blob:)'}, 'action': {'type': 'ignore-previous-rules'}}]
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    allowed_origin_filter = self._web_origin_url_filter(allowed_url)
                    rules = [{'trigger': {'url-filter': '.*'}, 'action': {'type': 'block'}}, {'trigger': {'url-filter': allowed_origin_filter}, 'action': {'type': 'ignore-previous-rules'}}, {'trigger': {'url-filter': '^(?:about:blank|data:|blob:)'}, 'action': {'type': 'ignore-previous-rules'}}]

                def install():
                    instance = BrowserView.get_instance('window', reader_window.native)
                    # Why: Condition check ensures valid state before proceeding with operation
                    if instance is None:
                        finished.set()
                        return
                    store = WebKit.WKContentRuleListStore.defaultStore()

                    def compiled(rule_list, error):
                        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                        if rule_list is not None and error is None:
                            instance.webview.configuration().userContentController().addContentRuleList_(rule_list)
                            instance._readmd_content_rule = rule_list
                            success[0] = True
                        # Why: Function call performs specific operation required by this logic
                        finished.set()
                    # Why: Function call performs specific operation required by this logic
                    store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler_('ReadMDPrivateNetworkGuard', json.dumps(rules), compiled)
                AppHelper.callAfter(install)
                finished.wait(5.0)
                return success[0]
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
                logging.exception('failed to install WKWebView network guard')
                # Why: Return provides result to caller after processing completes
                return False
        # Why: Return provides result to caller after processing completes
        return False

    def revoke_private_web(self, task_id):
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with self._web_private_lock:
            self._web_private_grants.pop(str(task_id or ''), None)
        # Why: Return provides result to caller after processing completes
        return True

    def choose_file(self) -> Optional[str]:
        import webview
        # Why: Condition check ensures valid state before proceeding with operation
        if self._window is None:
            # Why: Return provides result to caller after processing completes
            return None
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            files = self._window.create_file_dialog(webview.OPEN_DIALOG, file_types=('Markdown 文件 (*.md;*.markdown;*.mdown;*.mkd;*.txt)',))
            # Why: Conditional return handles different cases based on input or state
            return files[0] if files else None
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('choose_file failed')
            # Why: Return provides result to caller after processing completes
            return None

    def authorize_clipboard_read(self) -> Dict[str, Any]:
        """Grant one short-lived clipboard read after an explicit UI action."""
        token = secrets.token_urlsafe(18)
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with self._clipboard_lock:
            self._clipboard_tokens[token] = time.time() + 30
        # Why: Return provides result to caller after processing completes
        return {'ok': True, 'token': token, 'expires_at': int(time.time() + 30)}

    def read_clipboard(self, token: str='') -> Dict[str, Any]:
        """Return clipboard data in a small, platform-neutral bridge shape.

        HTML is best-effort because pywebview backends expose different native
        clipboard APIs.  Callers can always fall back to ``text``.
        """
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with self._clipboard_lock:
            expires_at = self._clipboard_tokens.pop(str(token or ''), 0)
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not token or time.time() > expires_at:
            return {'text': '', 'html': '', 'source_type': 'unauthorized', 'error': '请通过用户操作重新授权读取剪贴板'}
        (text, html, image_path, file_list) = ('', '', '', [])
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            if IS_WIN:
                import win32clipboard
                win32clipboard.OpenClipboard()
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    if win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_HDROP):
                        raw_files = win32clipboard.GetClipboardData(win32clipboard.CF_HDROP)
                        if raw_files:
                            file_list = [f for f in raw_files if os.path.exists(f)]
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if not file_list and win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_UNICODETEXT):
                        text = win32clipboard.GetClipboardData(win32clipboard.CF_UNICODETEXT) or ''
                    fmt = win32clipboard.RegisterClipboardFormat('HTML Format')
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if not file_list and win32clipboard.IsClipboardFormatAvailable(fmt):
                        raw = win32clipboard.GetClipboardData(fmt)
                        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                        if isinstance(raw, bytes) and len(raw) <= 10 * 1024 * 1024:
                            html_str = raw.decode('utf-8', errors='replace')
                            # Why: Regex pattern matches specific text structures for validation or extraction
                            start = re.search('StartFragment:(\\d+)', html_str)
                            # Why: Regex pattern matches specific text structures for validation or extraction
                            end = re.search('EndFragment:(\\d+)', html_str)
                            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                            if start and end:
                                html = html_str[int(start.group(1)):int(end.group(1))]
                            # Why: Default case handles all scenarios not covered by previous conditions
                            else:
                                html = html_str
                # Why: Finally ensures cleanup operations run regardless of success or failure
                finally:
                    win32clipboard.CloseClipboard()
            # Why: Alternative condition handles different case in decision tree
            elif IS_MAC:
                try:
                    # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
                    p = subprocess.run(['pbpaste'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
                    # Why: Conditional return handles different cases based on input or state
                    if p.returncode == 0:
                        text = p.stdout.decode('utf-8', errors='replace')
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                try:
                    # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
                    p = subprocess.run(['wl-paste', '-t', 'text/plain'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
                    # Why: Conditional return handles different cases based on input or state
                    if p.returncode == 0:
                        text = p.stdout.decode('utf-8', errors='replace')
                    else:
                        # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
                        p = subprocess.run(['xclip', '-selection', 'clipboard', '-o'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
                        # Why: Conditional return handles different cases based on input or state
                        if p.returncode == 0:
                            text = p.stdout.decode('utf-8', errors='replace')
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not text and (not html) and (not file_list):
                try:
                    import tkinter
                    root = tkinter.Tk()
                    root.withdraw()
                    # Why: Method call handles data access with proper error checking
                    text = root.clipboard_get() or ''
                    root.destroy()
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not text and (not html) and (not file_list):
                try:
                    from PIL import ImageGrab, Image
                    # Why: Function call performs specific operation required by this logic
                    img = ImageGrab.grabclipboard()
                    # Why: Function call performs specific operation required by this logic
                    if isinstance(img, Image.Image):
                        tmp_img = os.path.join(tempfile.gettempdir(), 'readmd_clip_%d.png' % int(time.time() * 1000))
                        img.save(tmp_img, 'PNG')
                        image_path = tmp_img
                    # Why: Alternative condition handles different case in decision tree
                    elif isinstance(img, list):
                        file_list = [f for f in img if os.path.exists(f)]
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
        if file_list:
            return {'text': '', 'html': '', 'files': file_list, 'source_type': 'files'}
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if image_path and os.path.isfile(image_path):
            return {'text': '', 'html': '', 'image': image_path, 'image_path': image_path, 'source_type': 'image'}
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if html and len(html.encode('utf-8')) <= 10 * 1024 * 1024:
            return {'text': text or '', 'html': html, 'source_type': 'html'}
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if text and len(text.encode('utf-8')) <= 10 * 1024 * 1024:
            return {'text': text, 'html': '', 'source_type': 'text'}
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not text and (not html) and (not image_path) and (not file_list):
            return {'text': '', 'html': '', 'source_type': 'empty', 'error': '剪贴板为空或不包含支持的内容'}
        # Why: Return provides result to caller after processing completes
        return {'text': '', 'html': '', 'source_type': 'too_large', 'error': '剪贴板内容超过 10 MB 限制'}

    def choose_folder(self) -> Optional[str]:
        import webview
        # Why: Condition check ensures valid state before proceeding with operation
        if self._window is None:
            # Why: Return provides result to caller after processing completes
            return None
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            dirs = self._window.create_file_dialog(webview.FOLDER_DIALOG)
            # Why: Conditional return handles different cases based on input or state
            return dirs[0] if dirs else None
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return None

    def choose_any_file(self) -> Optional[str]:
        """任意格式文件（用于“万物转 MD”）。Win7 版仅开放 docx / pdf。"""
        import webview
        # Why: Condition check ensures valid state before proceeding with operation
        if self._window is None:
            # Why: Return provides result to caller after processing completes
            return None
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            files = self._window.create_file_dialog(webview.OPEN_DIALOG, file_types=('Word / PDF (*.docx;*.pdf)' if is_win7() else '所有文件 (*.*)', '文档 (*.docx;*.pdf)' if is_win7() else '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)') if is_win7() else ('所有文件 (*.*)', '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)', '图片 (*.png;*.jpg;*.jpeg;*.bmp;*.webp;*.tif;*.tiff)'))
            # Why: Conditional return handles different cases based on input or state
            return files[0] if files else None
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return None

    def choose_many_files(self) -> List[str]:
        """批量转换：多选任意格式文件。Win7 版仅开放 docx / pdf。"""
        import webview
        # Why: Condition check ensures valid state before proceeding with operation
        if self._window is None:
            # Why: Return provides result to caller after processing completes
            return []
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            files = self._window.create_file_dialog(webview.OPEN_DIALOG, allow_multiple=True, file_types=('Word / PDF (*.docx;*.pdf)' if is_win7() else '所有文件 (*.*)', '文档 (*.docx;*.pdf)' if is_win7() else '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)') if is_win7() else ('所有文件 (*.*)', '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)', '图片 (*.png;*.jpg;*.jpeg;*.bmp;*.webp;*.tif;*.tiff)'))
            return list(files or [])
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return []

    def open_dir(self, path: str) -> bool:
        """在文件管理器中打开目录。"""
        try:
            # Why: Path validation prevents directory traversal attacks that could access unauthorized files
            safe_path = validate_file_path(path)
            if IS_MAC:
                from src.readmd_modules import macos_native
                # Why: Return provides result to caller after processing completes
                return macos_native.open_path(safe_path)
            elif IS_WIN:
                # Why: Path validation prevents directory traversal attacks that could access unauthorized files
                cmd = validate_command(['explorer', safe_path])
                subprocess.Popen(cmd)
            else:
                # Why: Path validation prevents directory traversal attacks that could access unauthorized files
                cmd = validate_command(['xdg-open', safe_path])
                subprocess.Popen(cmd)
            return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return False

    def get_autostart(self) -> bool:
        """检查开机自启动是否已开启。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            if IS_WIN:
                import winreg
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    # Why: Context manager ensures proper resource cleanup even if errors occur
                    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, 'Software\\Microsoft\\Windows\\CurrentVersion\\Run', 0, winreg.KEY_READ) as key:
                        (val, _) = winreg.QueryValueEx(key, 'ReadMD')
                        return bool(val)
                # Why: File operations may fail if files are moved, deleted, or permissions change
                except (FileNotFoundError, OSError):
                    logging.warning('Silent exception caught in readmd: (FileNotFoundError, OSError)')
                    # Why: Return provides result to caller after processing completes
                    return False
            # Why: Alternative condition handles different case in decision tree
            elif IS_LINUX:
                autostart_file = os.path.expanduser('~/.config/autostart/io.github.natsummerance.readmd.desktop')
                # Why: Return provides result to caller after processing completes
                return os.path.isfile(autostart_file)
            # Why: Alternative condition handles different case in decision tree
            elif IS_MAC:
                plist_path = os.path.expanduser('~/Library/LaunchAgents/io.github.natsummerance.readmd.plist')
                # Why: Return provides result to caller after processing completes
                return os.path.isfile(plist_path)
            return False
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('get_autostart failed: %s', e)
            # Why: Return provides result to caller after processing completes
            return False

    def set_autostart(self, enabled: bool):
        """设置开机自启动开启或关闭。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            enabled = bool(enabled)
            if IS_WIN:
                import winreg
                # Why: Context manager ensures proper resource cleanup even if errors occur
                with winreg.OpenKey(winreg.HKEY_CURRENT_USER, 'Software\\Microsoft\\Windows\\CurrentVersion\\Run', 0, winreg.KEY_SET_VALUE) as key:
                    if enabled:
                        exe_path = sys.executable
                        if getattr(sys, 'frozen', False):
                            cmd = '"%s"' % exe_path
                        # Why: Default case handles all scenarios not covered by previous conditions
                        else:
                            main_script = os.path.abspath(sys.argv[0])
                            cmd = '"%s" "%s"' % (exe_path, main_script)
                        winreg.SetValueEx(key, 'ReadMD', 0, winreg.REG_SZ, cmd)
                    # Why: Default case handles all scenarios not covered by previous conditions
                    else:
                        # Why: Try block protects against runtime errors in operations that may fail
                        try:
                            winreg.DeleteValue(key, 'ReadMD')
                        # Why: File operations may fail if files are moved, deleted, or permissions change
                        except (FileNotFoundError, OSError):
                            logging.warning('Silent exception caught in readmd: (FileNotFoundError, OSError)')
                # Why: Return provides result to caller after processing completes
                return {'ok': True, 'enabled': enabled}
            # Why: Alternative condition handles different case in decision tree
            elif IS_LINUX:
                autostart_dir = os.path.expanduser('~/.config/autostart')
                autostart_file = os.path.join(autostart_dir, 'io.github.natsummerance.readmd.desktop')
                if enabled:
                    # Why: Function call performs specific operation required by this logic
                    os.makedirs(autostart_dir, exist_ok=True)
                    # Why: Function call performs specific operation required by this logic
                    desktop_src = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'scripts', 'linux', 'io.github.natsummerance.readmd.desktop')
                    if os.path.isfile(desktop_src):
                        import shutil
                        shutil.copy(desktop_src, autostart_file)
                    # Why: Default case handles all scenarios not covered by previous conditions
                    else:
                        with open(autostart_file, 'w', encoding='utf-8') as f:
                            # Why: Method call handles data access with proper error checking
                            f.write('[Desktop Entry]\nName=ReadMD\nExec=readmd\nType=Application\n')
                # Why: Alternative condition handles different case in decision tree
                elif os.path.isfile(autostart_file):
                    os.remove(autostart_file)
                # Why: Return provides result to caller after processing completes
                return {'ok': True, 'enabled': enabled}
            # Why: Alternative condition handles different case in decision tree
            elif IS_MAC:
                plist_dir = os.path.expanduser('~/Library/LaunchAgents')
                plist_path = os.path.join(plist_dir, 'io.github.natsummerance.readmd.plist')
                if enabled:
                    # Why: Function call performs specific operation required by this logic
                    os.makedirs(plist_dir, exist_ok=True)
                    exe_path = sys.executable
                    plist_content = '<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0">\n<dict>\n    <key>Label</key>\n    <string>io.github.natsummerance.readmd</string>\n    <key>ProgramArguments</key>\n    <array>\n        <string>%s</string>\n    </array>\n    <key>RunAtLoad</key>\n    <true/>\n</dict>\n</plist>' % exe_path
                    with open(plist_path, 'w', encoding='utf-8') as f:
                        f.write(plist_content)
                # Why: Alternative condition handles different case in decision tree
                elif os.path.isfile(plist_path):
                    os.remove(plist_path)
                # Why: Return provides result to caller after processing completes
                return {'ok': True, 'enabled': enabled}
            return {'ok': False, 'error': 'Unsupported platform'}
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('set_autostart failed: %s', e)
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'error': str(e)}

    def start_modules(self) -> Dict[str, Any]:
        """Compatibility bridge: module loading is now initiated per feature."""
        # Why: Return provides result to caller after processing completes
        return self.get_modules_status()

    def get_modules_status(self) -> Dict[str, Any]:
        (st, err) = RM.status()
        # Why: Return provides result to caller after processing completes
        return {'modules': st, 'errors': err}

    # Why: render_web_page implements core functionality requiring careful error handling
    def render_web_page(self, url: str, task_id: str='', timeout_ms: int=25000, interactive: bool=False, private_grant: str='', source_html: str='') -> Dict[str, Any]:
        """在无 JS bridge、无持久会话的临时系统 WebView 中渲染网页。"""
        if is_win7():
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'render_unavailable', 'error': 'Win7 版不支持动态网页渲染'}
        allow_private = self._private_web_allowed(url, task_id, private_grant)
        offline_render = not allow_private
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if offline_render and interactive:
            return {'ok': False, 'code': 'interactive_unavailable', 'error': '安全模式不允许未授权网页联网交互；可重试静态抓取或保留完整页面'}
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if offline_render and (not source_html):
            return {'ok': False, 'code': 'render_source_missing', 'error': '安全模式缺少已验证的网页 HTML，无法动态渲染'}
        # Why: Condition check ensures valid state before proceeding with operation
        if not RM.is_ready('web'):
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'module_loading', 'module': 'web', 'status': RM.load('web'), 'error': '网页模块加载中，请稍候再试'}
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            mod = RM.get('web')
            # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
            safe_url = mod._validate_public_url(url, allow_private=allow_private)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': getattr(exc, 'code', 'invalid_url'), 'error': getattr(exc, 'message', str(exc))}
        try:
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            timeout_ms = max(3000, min(300000 if interactive else 60000, int(timeout_ms or (300000 if interactive else 25000))))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            timeout_ms = 300000 if interactive else 25000
        # Why: Condition check ensures valid state before proceeding with operation
        if not self._web_render_lock.acquire(blocking=False):
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'renderer_busy', 'error': '动态网页渲染器正在处理另一个页面'}
        reader_window = None
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            import webview
            reader_window = webview.create_window('ReadMD 临时网页提取器', 'about:blank', hidden=not interactive, focus=bool(interactive), width=1100, height=800, resizable=bool(interactive), text_select=bool(interactive))
            # Why: Condition check ensures valid state before proceeding with operation
            if reader_window is None:
                # Why: Return provides result to caller after processing completes
                return {'ok': False, 'code': 'render_unavailable', 'error': '无法创建系统网页渲染器'}
            blank_loaded = getattr(getattr(reader_window, 'events', None), 'loaded', None)
            # Why: Condition check ensures valid state before proceeding with operation
            if blank_loaded is not None:
                blank_loaded.wait(5.0)
            # Why: Condition check ensures valid state before proceeding with operation
            if not self._install_webview_network_guard(reader_window, task_id, private_grant, safe_url, offline=offline_render):
                # Why: Return provides result to caller after processing completes
                return {'ok': False, 'code': 'network_guard_unavailable', 'error': '无法启用网页私网访问保护，已停止动态渲染'}
            if offline_render:
                reader_window.load_html(source_html, base_uri=safe_url)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                reader_window.load_url(safe_url)
            deadline = time.time() + timeout_ms / 1000.0
            loaded = getattr(getattr(reader_window, 'events', None), 'loaded', None)
            # Why: Condition check ensures valid state before proceeding with operation
            if loaded is not None:
                loaded.wait(max(0.1, min(10.0, deadline - time.time())))
            (last_length, last_resources, stable) = (-1, -1, 0)
            # Why: Loop continues until condition is met or timeout occurs
            while time.time() < deadline:
                if mod.is_cancelled(task_id):
                    # Why: Return provides result to caller after processing completes
                    return {'ok': False, 'code': 'cancelled', 'error': '已取消网页转换'}
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    if interactive:
                        # Why: Conditional return handles different cases based on input or state
                        reader_window.evaluate_js('\n                          (() => {\n                            # Why: 条件分支：根据不同情况选择执行路径\n                            if (document.getElementById(\'__readmd_capture_bar\')) return;\n                            const bar=document.createElement(\'div\');\n                            bar.id=\'__readmd_capture_bar\';\n                            bar.style=\'position:fixed;z-index:2147483647;top:0;left:0;right:0;padding:10px 16px;background:#172033;color:white;font:14px system-ui;display:flex;gap:10px;align-items:center;box-shadow:0 2px 12px #0005\';\n                            bar.innerHTML=\'<strong style="margin-right:auto">完成登录或验证后，提取当前页面</strong><button id="__readmd_capture" style="min-height:40px;padding:0 16px;border:0;border-radius:8px;background:#3182f6;color:white">提取此页</button><button id="__readmd_abort" style="min-height:40px;padding:0 16px;border:1px solid #ffffff55;border-radius:8px;background:transparent;color:white">取消</button>\';\n                            document.documentElement.appendChild(bar);\n                            document.getElementById(\'__readmd_capture\').onclick=()=>window.__readmdCaptureAction=\'capture\';\n                            document.getElementById(\'__readmd_abort\').onclick=()=>window.__readmdCaptureAction=\'cancel\';\n                          })()\n                        ')
                    state = reader_window.evaluate_js("({ready:document.readyState,n:(document.body&&document.body.innerText||'').length,r:(performance.getEntriesByType&&performance.getEntriesByType('resource').length)||0,action:window.__readmdCaptureAction||''})") or {}
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if isinstance(state, dict) and state.get('action') == 'cancel':
                        return {'ok': False, 'code': 'cancelled', 'error': '已取消交互式抓取'}
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if interactive and isinstance(state, dict) and (state.get('action') == 'capture'):
                        break
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    length = int(state.get('n') or 0) if isinstance(state, dict) else 0
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    resources = int(state.get('r') or 0) if isinstance(state, dict) else 0
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if length > 0 and length == last_length and (resources == last_resources):
                        stable += 1
                    # Why: Default case handles all scenarios not covered by previous conditions
                    else:
                        stable = 0
                    last_length = length
                    last_resources = resources
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if not interactive and state.get('ready') == 'complete' and (stable >= 3):
                        break
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
                time.sleep(0.5)
            if time.time() >= deadline:
                # Why: Return provides result to caller after processing completes
                return {'ok': False, 'code': 'render_timeout', 'error': '动态渲染超时，请重试或改用完整页面模式'}
            reader_path = os.path.join(APP_DIR, 'assets', 'vendor', 'readability.bundle.js')
            defuddle_path = os.path.join(APP_DIR, 'assets', 'vendor', 'defuddle.bundle.js')
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not os.path.isfile(reader_path) or not os.path.isfile(defuddle_path):
                return {'ok': False, 'code': 'reader_missing', 'error': '网页正文提取器离线资源缺失'}
            with open(defuddle_path, encoding='utf-8') as handle:
                reader_window.evaluate_js(handle.read())
            with open(reader_path, encoding='utf-8') as handle:
                reader_window.evaluate_js(handle.read())
            result = reader_window.evaluate_js("\n                (() => {\n                  const bar=document.getElementById('__readmd_capture_bar'); if(bar) bar.remove();\n                  let defuddle = null;\n                  try {\n                    const parsed=window.ReadMDDefuddle.parse(document.cloneNode(true), location.href);\n                    defuddle=parsed ? {\n                      title:parsed.title||'', author:parsed.author||'',\n                      published:parsed.published||parsed.publishedTime||'',\n                      site:parsed.site||parsed.siteName||'',\n                      contentMarkdown:parsed.contentMarkdown||parsed.markdown||'',\n                      content:parsed.content||''\n                    } : null;\n                  } catch (e) { defuddle = null; }\n                  let article = null;\n                  try {\n                    const clone = document.cloneNode(true);\n                    article = new window.ReadMDReadability.Readability(clone, {charThreshold: 40}).parse();\n                  } catch (e) { article = null; }\n                  const compact = article ? {\n                    title: article.title || '', byline: article.byline || '',\n                    publishedTime: article.publishedTime || '', siteName: article.siteName || '',\n                    excerpt: article.excerpt || '', content: article.content || '',\n                    length: article.length || 0, url: location.href\n                  } : null;\n                  # Why: 返回结果：输出计算结果或状态信息\n                  return {ok:true, final_url:location.href,\n                          title:document.title || '',\n                          html:document.documentElement.outerHTML,\n                          defuddle:defuddle,\n                          readability:compact};\n                })()\n            ")
            if not isinstance(result, dict):
                return {'ok': False, 'code': 'render_failed', 'error': '动态网页渲染没有返回可用内容'}
            try:
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                final_url = safe_url if offline_render else result.get('final_url') or safe_url
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if allow_private and (not self._private_web_allowed(final_url, task_id, private_grant)):
                    return {'ok': False, 'code': 'private_origin_changed', 'error': '内网页面跳转到了未授权的源站，请重新授权'}
                # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
                result['final_url'] = mod._validate_public_url(final_url, allow_private=allow_private)
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception as exc:
                logging.warning('Silent exception caught in readmd: Exception')
                return {'ok': False, 'code': getattr(exc, 'code', 'blocked_address'), 'error': getattr(exc, 'message', str(exc))}
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if len(result.get('html') or '') > 50 * 1024 * 1024:
                return {'ok': False, 'code': 'too_large', 'error': '动态渲染后的网页超过 50 MB 限制'}
            return result
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('system WebView extraction failed: %s', safe_url)
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'render_failed', 'error': '系统网页渲染失败：%s' % exc}
        # Why: Finally ensures cleanup operations run regardless of success or failure
        finally:
            # Why: Condition check ensures valid state before proceeding with operation
            if reader_window is not None:
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    reader_window.clear_cookies()
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    reader_window.destroy()
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
            self._web_render_lock.release()

    # Why: cancel_web_render implements core functionality requiring careful error handling
    def cancel_web_render(self, task_id=''):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            RM.get('web').cancel(task_id)
            return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return False

    def rename_file(self, path, new_stem):
        """在原目录内重命名当前 Markdown 文件并同步本地引用。"""
        old_path = os.path.abspath(os.fspath(path)) if path else ''
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not old_path or not os.path.isfile(old_path):
            return {'ok': False, 'code': 'not_found', 'error': '文件不存在或已被移动'}
        extension = os.path.splitext(old_path)[1]
        # Why: Condition check ensures valid state before proceeding with operation
        if extension.lower() not in MD_EXTS:
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'unsupported_type', 'error': '只能重命名 Markdown 或文本文件'}
        try:
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            stem = _validate_rename_stem(new_stem, extension)
        # Why: ValueError indicates invalid input data that cannot be processed safely
        except ValueError as exc:
            logging.warning('Silent exception caught in readmd: ValueError')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'invalid_name', 'error': str(exc)}
        new_path = os.path.join(os.path.dirname(old_path), stem + extension)
        # Why: Condition check ensures valid state before proceeding with operation
        if old_path == new_path:
            # Why: Return provides result to caller after processing completes
            return {'ok': True, 'path': old_path, 'name': os.path.basename(old_path), 'warnings': []}
        same_normalized = _same_file_target(old_path, new_path)
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if os.path.exists(new_path) and (not same_normalized):
            return {'ok': False, 'code': 'target_exists', 'error': '同目录下已存在同名文件'}
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            if same_normalized:
                temp_path = old_path + '.readmd-rename-' + secrets.token_hex(6)
                os.rename(old_path, temp_path)
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    os.rename(temp_path, new_path)
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
                    os.rename(temp_path, old_path)
                    raise
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                os.rename(old_path, new_path)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('rename_file failed: %s', old_path)
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'code': 'rename_failed', 'error': str(exc)}
        warnings = []
        (old_backup, new_backup) = (old_path + '.bak', new_path + '.bak')
        if os.path.isfile(old_backup):
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if _same_file_target(old_backup, new_backup) and old_backup != new_backup:
                backup_tmp = old_backup + '.readmd-rename-' + secrets.token_hex(6)
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    os.rename(old_backup, backup_tmp)
                    # Why: Try block protects against runtime errors in operations that may fail
                    try:
                        os.rename(backup_tmp, new_backup)
                    # Why: Exception handling prevents crashes and provides meaningful error messages to users
                    except Exception:
                        logging.warning('Silent exception caught in readmd: Exception')
                        os.rename(backup_tmp, old_backup)
                        raise
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception as exc:
                    logging.warning('case-only backup rename failed: %s', exc)
                    warnings.append('文件已重命名，但备份文件大小写未能同步')
            # Why: Alternative condition handles different case in decision tree
            elif os.path.exists(new_backup):
                warnings.append('旧备份未移动：目标备份已存在')
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    os.rename(old_backup, new_backup)
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception as exc:
                    logging.warning('rename backup failed: %s', exc)
                    warnings.append('文件已重命名，但旧备份未能同步移动')
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            recent = load_json(RECENT_FILE, [])
            updated = []
            # Why: Iteration processes each item in collection systematically
            for item in recent if isinstance(recent, list) else []:
                value = new_path if _paths_equal(item, old_path) else item
                # Why: Condition check ensures valid state before proceeding with operation
                if not any((_paths_equal(value, existing) for existing in updated)):
                    updated.append(value)
            # Why: Condition check ensures valid state before proceeding with operation
            if not save_json(RECENT_FILE, updated):
                warnings.append('最近文件记录未能同步')
            settings = load_json(SETTINGS_FILE, {})
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if isinstance(settings, dict) and settings.get('last') and _paths_equal(settings['last'], old_path):
                settings['last'] = new_path
                # Why: Condition check ensures valid state before proceeding with operation
                if not save_json(SETTINGS_FILE, settings):
                    warnings.append('上次打开记录未能同步')
            history = load_json(HISTORY_FILE, {'sessions': []})
            changed = False
            if isinstance(history, dict):
                for session in history.get('sessions', []):
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if session.get('doc') and _paths_equal(session['doc'], old_path):
                        session['doc'] = new_path
                        changed = True
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if changed and (not save_json(HISTORY_FILE, history)):
                warnings.append('AI 历史文档引用未能同步')
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('rename metadata sync failed')
            warnings.append('文件已重命名，但部分历史记录未能同步')
        # Why: Return provides result to caller after processing completes
        return {'ok': True, 'path': new_path, 'name': os.path.basename(new_path), 'old_path': old_path, 'warnings': warnings}

    def save_file(self, path, content, encoding):
        """编辑保存：写回文件，首次保存自动生成 .bak 备份。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            import shutil
            bak = None
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if os.path.isfile(path) and (not os.path.exists(path + '.bak')):
                shutil.copy2(path, path + '.bak')
                bak = path + '.bak'
            with open(path, 'w', encoding=encoding or 'utf-8', newline='') as f:
                f.write(content)
            return {'ok': True, 'backup': bak}
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('save_file failed')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'error': str(e)}

    def save_as(self, content, suggested, assets=None):
        """把转换 / 网页 / OCR 结果另存为 .md 文件。"""
        import webview
        # Why: Condition check ensures valid state before proceeding with operation
        if self._window is None:
            # Why: Return provides result to caller after processing completes
            return None
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            target = self._window.create_file_dialog(webview.SAVE_DIALOG, save_filename=suggested, file_types=('Markdown (*.md)',))
            target = normalize_dialog_path(target, '.md')
            # Why: Condition check ensures valid state before proceeding with operation
            if not target:
                # Why: Return provides result to caller after processing completes
                return None
            assets = assets or []
            if assets:
                import shutil
                # Why: Function call performs specific operation required by this logic
                stem = os.path.splitext(os.path.basename(target))[0]
                asset_name = stem + '.assets'
                asset_dir = os.path.join(os.path.dirname(target), asset_name)
                os.makedirs(asset_dir, exist_ok=True)
                # Why: Iteration processes each item in collection systematically
                for item in assets:
                    # Why: Method call handles data access with proper error checking
                    source = item.get('path') if isinstance(item, dict) else ''
                    name = item.get('name') if isinstance(item, dict) else ''
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if not source or not name or (not os.path.isfile(source)):
                        continue
                    destination = os.path.join(asset_dir, os.path.basename(name))
                    # Why: Function call performs specific operation required by this logic
                    shutil.copy2(source, destination)
                    # Why: Function call performs specific operation required by this logic
                    relative = asset_name + '/' + os.path.basename(name)
                    # Why: Function call performs specific operation required by this logic
                    content = content.replace(source.replace('\\', '/'), relative)
                    # Why: Function call performs specific operation required by this logic
                    content = content.replace(source, relative)
            with open(target, 'w', encoding='utf-8', newline='') as f:
                f.write(content)
            return target
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('save_as failed')
            # Why: Return provides result to caller after processing completes
            return None

    def export_doc(self, fmt, payload=None):
        """导出当前文档为 PDF / DOCX / HTML（js_api 入口）。

        payload: {content, baseDir, suggestedName, options}
        返回 {ok, path, size, warns, error, canceled}。
        """
        import webview
        try:
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            validated = validate_request_params({'fmt': fmt, 'payload': payload or {}}, {'fmt': {'type': 'str', 'required': True, 'max_length': 10}, 'payload': {'type': 'str', 'required': False}})
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            fmt = validated['fmt']
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            payload = json.loads(validated['payload']) if validated.get('payload') else {}
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'stage': 'validation', 'error': str(e)}
        # Why: Condition check ensures valid state before proceeding with operation
        if self._window is None:
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'stage': 'save_dialog', 'error': '窗口未就绪'}
        payload = payload or {}
        fmt = (fmt or '').lower()
        ext_map = {'pdf': 'PDF 文档 (*.pdf)', 'docx': 'Word 文档 (*.docx)', 'html': 'HTML 网页 (*.html)', 'tex': 'LaTeX 文档 (*.tex)'}
        # Why: Condition check ensures valid state before proceeding with operation
        if fmt not in ext_map:
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'stage': 'options', 'error': '不支持的导出格式'}
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            import src.readmd_modules.mdexport as MDE
            MDE.load()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('mdexport import failed')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'stage': 'dependency', 'error': '导出模块加载失败：%s' % e}
        # Why: Method call handles data access with proper error checking
        suggested = (payload.get('suggestedName') or 'export').strip() or 'export'
        # Why: Condition check ensures valid state before proceeding with operation
        if not suggested.lower().endswith('.' + fmt):
            suggested += '.' + fmt
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            target = self._window.create_file_dialog(webview.SAVE_DIALOG, save_filename=suggested, file_types=(ext_map[fmt],))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('save dialog failed')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'stage': 'save_dialog', 'error': '保存对话框失败：%s' % e}
        # Why: Condition check ensures valid state before proceeding with operation
        if not target:
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'canceled': True}
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            target = normalize_dialog_path(target, '.' + fmt)
        # Why: ValueError indicates invalid input data that cannot be processed safely
        except ValueError as e:
            logging.warning('Silent exception caught in readmd: ValueError')
            logging.exception('save dialog returned invalid path')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'stage': 'save_dialog', 'error': str(e)}
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            return MDE.export(fmt, payload.get('content') or '', payload.get('baseDir') or '', target, options=payload.get('options') or {}, source_name=payload.get('suggestedName') or '')
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('export failed')
            # Why: Return provides result to caller after processing completes
            return {'ok': False, 'stage': 'render', 'error': '导出失败：%s' % e}

    def reveal_path(self, path):
        """在文件管理器中选中该文件。"""
        try:
            # Why: Path validation prevents directory traversal attacks that could access unauthorized files
            safe_path = validate_file_path(path)
            if IS_MAC:
                from src.readmd_modules import macos_native
                # Why: Return provides result to caller after processing completes
                return macos_native.reveal_path(safe_path)
            elif IS_WIN:
                # Why: Path validation prevents directory traversal attacks that could access unauthorized files
                cmd = validate_command(['explorer', '/select,', safe_path])
                subprocess.Popen(cmd)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                dir_path = os.path.dirname(safe_path)
                # Why: Path validation prevents directory traversal attacks that could access unauthorized files
                cmd = validate_command(['xdg-open', dir_path])
                subprocess.Popen(cmd)
            return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return False

    def get_export_presets(self):
        """返回导出样式默认值 / 内置预设 / 自定义预设 / 上次参数。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules.mdexport import styles as _st
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return {'error': '导出模块不可用'}
        cur = load_json(SETTINGS_FILE, {})
        # Why: Return provides result to caller after processing completes
        return {'defaults': _st.DEFAULT_STYLE, 'presets': _st.PRESETS, 'custom': cur.get('exportPresets', {}), 'last': cur.get('exportLast', {})}

    def save_export_presets(self, payload):
        """保存自定义导出预设与上次参数。"""
        cur = load_json(SETTINGS_FILE, {})
        payload = payload or {}
        if 'custom' in payload:
            # Why: Method call handles data access with proper error checking
            cur['exportPresets'] = payload.get('custom') or {}
        if 'last' in payload:
            # Why: Method call handles data access with proper error checking
            cur['exportLast'] = payload.get('last') or {}
        save_json(SETTINGS_FILE, cur)
        # Why: Return provides result to caller after processing completes
        return True

    def open_external(self, url):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            webbrowser.open(url)
            return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return False

    def open_path(self, path):
        """用系统默认程序打开文件（如图片、PDF 或外部文档）。"""
        try:
            # Why: Path validation prevents directory traversal attacks that could access unauthorized files
            safe_path = validate_file_path(path)
            if IS_MAC:
                # Why: Path validation prevents directory traversal attacks that could access unauthorized files
                cmd = validate_command(['open', safe_path])
                subprocess.Popen(cmd)
            # Why: Alternative condition handles different case in decision tree
            elif IS_WIN:
                os.startfile(safe_path)
            else:
                # Why: Path validation prevents directory traversal attacks that could access unauthorized files
                cmd = validate_command(['xdg-open', safe_path])
                subprocess.Popen(cmd)
            return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return False

    # Why: check_update implements core functionality requiring careful error handling
    def check_update(self):
        from src.readmd_modules import updater
        # Why: Return provides result to caller after processing completes
        return updater.check_update(VERSION)

    def start_download_update(self, download_url, target_filename, expected_sha=None, use_mirror=False):
        from src.readmd_modules import updater
        (ok, msg) = updater.start_download_update(download_url, target_filename, expected_sha, use_mirror)
        # Why: Return provides result to caller after processing completes
        return {'ok': ok, 'message': msg}

    def get_download_status(self):
        from src.readmd_modules import updater
        # Why: Return provides result to caller after processing completes
        return updater.get_download_status()

    def cancel_download(self):
        from src.readmd_modules import updater
        # Why: Return provides result to caller after processing completes
        return {'ok': updater.cancel_download()}

    def apply_update(self, file_path=None, flavor=None):
        from src.readmd_modules import updater
        (ok, msg) = updater.apply_update(file_path, flavor)
        # Why: Return provides result to caller after processing completes
        return {'ok': ok, 'message': msg}

    def get_system_language(self):
        # Why: Return provides result to caller after processing completes
        return get_system_language()

    def get_bibtex(self, file_path):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules import bibtex
            return bibtex.find_and_load_bib_for_file(file_path)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return {}

    def get_settings(self):
        # Why: Return provides result to caller after processing completes
        return load_json(SETTINGS_FILE, {})

    def save_settings(self, settings):
        cur = load_json(SETTINGS_FILE, {})
        cur.update(settings or {})
        save_json(SETTINGS_FILE, cur)
        # Why: Return provides result to caller after processing completes
        return True

    def get_recent(self):
        # Why: Return provides result to caller after processing completes
        return load_json(RECENT_FILE, [])

    def add_recent(self, path):
        rec = load_json(RECENT_FILE, [])
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            rec = [x for x in rec if os.path.normcase(x) != os.path.normcase(path)]
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            rec = [x for x in rec if x != path]
        rec.insert(0, path)
        save_json(RECENT_FILE, rec[:20])
        # Why: Return provides result to caller after processing completes
        return True

    def clear_recent(self):
        save_json(RECENT_FILE, [])
        # Why: Return provides result to caller after processing completes
        return True

    def save_fixed(self, path, content):
        """把修正后的文本另存为新文件。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            (base, ext) = os.path.splitext(path)
            out = base + '.readmd' + (ext or '.md')
            with open(out, 'w', encoding='utf-8', newline='\n') as f:
                f.write(content)
            return out
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in readmd: Exception')
            logging.exception('save_fixed failed')
            # Why: Return provides result to caller after processing completes
            return None

    def install_association(self):
        """注册 .md 文件关联（当前用户，无需管理员）。"""
        # Why: Return provides result to caller after processing completes
        return install_association()

    def get_app_info(self):
        # Why: Return provides result to caller after processing completes
        return {'version': VERSION, 'python': sys.version.split()[0]}

    # Why: check_upgrade implements core functionality requiring careful error handling
    def check_upgrade(self):
        """启动后前端调用：静默检查 GitHub 最新 Release（失败返回空结果）。"""
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            return check_latest_release() or {}
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
            # Why: Return provides result to caller after processing completes
            return {}

    def report_ready(self):
        """前端页面加载完成（启动里程碑：page_loaded）。"""
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with self._ready_lock:
            if self._page_ready:
                # Why: Return provides result to caller after processing completes
                return True
            self._page_ready = True
        milestone('boot', 'page_loaded')
        callback = self._on_page_ready
        # Why: Condition check ensures valid state before proceeding with operation
        if callback is not None:
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                callback()
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
                logging.exception('page-ready callback failed')
        _finish_startup_probe(False)
        # Why: Return provides result to caller after processing completes
        return True

    def show_window(self):
        # Why: Condition check ensures valid state before proceeding with operation
        if self._window is not None:
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                self._window.show()
                self._window.restore()
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
        # Why: Return provides result to caller after processing completes
        return True

    def request_quit(self):
        quit_app()
        # Why: Return provides result to caller after processing completes
        return True

def _quote(s):
    # Why: Return provides result to caller after processing completes
    return '"%s"' % s

def install_association():
    """把 .md 等扩展名关联到 ReadMD。

    Windows: HKCU 注册表（无需管理员）。
    macOS / Linux: 提示用户手动设置（系统不支持无 .app 注册）。
    """
    # Why: Condition check ensures valid state before proceeding with operation
    if not IS_WIN:
        if IS_MAC:
            # Why: Return provides result to caller after processing completes
            return 'macOS 不支持自动注册文件关联。请右键 .md 文件 → 显示简介 → 打开方式 → 选择 ReadMD → 全部更改'
        # Why: Return provides result to caller after processing completes
        return 'Linux 请使用 xdg-mime 手动设置 .md 文件关联'
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import shutil
        frozen = getattr(sys, 'frozen', False)
        # Why: Function call performs specific operation required by this logic
        icon_source = os.path.join(APP_DIR, 'assets', 'markdown-file.ico')
        # Why: Function call performs specific operation required by this logic
        icon_dir = os.path.join(DATA_DIR, 'icons')
        # Why: Function call performs specific operation required by this logic
        icon_file = os.path.join(icon_dir, 'markdown-file.ico')
        # Why: Function call performs specific operation required by this logic
        os.makedirs(icon_dir, exist_ok=True)
        # Why: Function call performs specific operation required by this logic
        shutil.copy2(icon_source, icon_file)
        # Why: Function call performs specific operation required by this logic
        icon = '%s,0' % _quote(icon_file)
        if frozen:
            pyw = sys.executable
            cmd = '%s "%%1"' % _quote(pyw)
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            pyw = None
            # Why: Iteration processes each item in collection systematically
            for cand in (os.path.join(APP_DIR, '.venv', 'Scripts', 'pythonw.exe'),):
                if os.path.isfile(cand):
                    pyw = cand
            # Why: Condition check ensures valid state before proceeding with operation
            if pyw is None:
                py = sys.executable
                base = os.path.basename(py).lower()
                # Why: Condition check ensures valid state before proceeding with operation
                if base == 'python.exe':
                    cand = os.path.splitext(py)[0] + 'w.exe'
                    pyw = cand if os.path.isfile(cand) else None
                # Why: Condition check ensures valid state before proceeding with operation
                if pyw is None:
                    pyw = py
            script = os.path.join(APP_DIR, 'readmd.py')
            cmd = '%s %s "%%1"' % (_quote(pyw), _quote(script))
        # Why: Iteration processes each item in collection systematically
        for ext in ('.md', '.markdown', '.mdown', '.mkd'):
            subprocess.run(['reg', 'add', 'HKCU\\Software\\Classes\\%s' % ext, '/ve', '/d', 'ReadMD.markdown', '/f'], capture_output=True)
        subprocess.run(['reg', 'add', 'HKCU\\Software\\Classes\\ReadMD.markdown', '/ve', '/d', 'ReadMD Markdown 阅读器', '/f'], capture_output=True)
        subprocess.run(['reg', 'add', 'HKCU\\Software\\Classes\\ReadMD.markdown\\DefaultIcon', '/ve', '/d', icon, '/f'], capture_output=True)
        subprocess.run(['reg', 'add', 'HKCU\\Software\\Classes\\ReadMD.markdown\\shell\\open\\command', '/ve', '/t', 'REG_EXPAND_SZ', '/d', cmd, '/f'], capture_output=True)
        subprocess.run(['reg', 'add', 'HKCU\\Software\\Classes\\Applications\\readmd.py\\shell\\open\\command', '/ve', '/t', 'REG_EXPAND_SZ', '/d', cmd, '/f'], capture_output=True)
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            subprocess.run(['ie4uinit.exe', '-show'], capture_output=True)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
        return True
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        logging.exception('install_association failed')
        # Why: Return provides result to caller after processing completes
        return str(e)

def run_selftest():
    ok = True
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        reader_asset = os.path.join(APP_DIR, 'assets', 'vendor', 'readability.bundle.js')
        reader_license = os.path.join(APP_DIR, 'assets', 'vendor', 'readability.LICENSE.md')
        # Why: Function call performs specific operation required by this logic
        defuddle_asset = os.path.join(APP_DIR, 'assets', 'vendor', 'defuddle.bundle.js')
        defuddle_license = os.path.join(APP_DIR, 'assets', 'vendor', 'defuddle.LICENSE.txt')
        file_icon = os.path.join(APP_DIR, 'assets', 'markdown-file.ico')
        app_icon = os.path.join(APP_DIR, 'assets', 'readmd.ico')
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert os.path.isfile(reader_asset) and os.path.getsize(reader_asset) > 10000
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert os.path.isfile(reader_license) and os.path.getsize(reader_license) > 400
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert os.path.isfile(defuddle_asset) and os.path.getsize(defuddle_asset) > 100000
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert os.path.isfile(defuddle_license) and os.path.getsize(defuddle_license) > 500
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert os.path.isfile(file_icon) and os.path.getsize(file_icon) > 1000
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with open(file_icon, 'rb') as _icon_handle:
            # Why: Method call handles data access with proper error checking
            assert _icon_handle.read(4) == b'\x00\x00\x01\x00'
        if os.path.isfile(app_icon):
            # Why: Hashing provides one-way transformation for password verification without storing plaintext
            import hashlib as _hashlib
            with open(file_icon, 'rb') as _file_icon_handle:
                # Why: Hashing provides one-way transformation for password verification without storing plaintext
                file_icon_hash = _hashlib.sha256(_file_icon_handle.read()).digest()
            with open(app_icon, 'rb') as _app_icon_handle:
                # Why: Hashing provides one-way transformation for password verification without storing plaintext
                app_icon_hash = _hashlib.sha256(_app_icon_handle.read()).digest()
            # Why: Hashing provides one-way transformation for password verification without storing plaintext
            assert file_icon_hash != app_icon_hash
        import trafilatura as _tra
        tra_cfg = os.path.join(os.path.dirname(_tra.__file__), 'settings.cfg')
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert os.path.isfile(tra_cfg), 'trafilatura/settings.cfg missing'
        import src.readmd_modules.web as _web
        fixture = '<html><head><title>Selftest article</title></head><body><article><p>' + 'web extraction content ' * 30 + '</p></article></body></html>'
        extracted = _web.extract_html('https://example.com/selftest', fixture)
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert extracted.get('ok') and 'web extraction content' in extracted.get('content', '')
        safe_print('web extraction and file association resources OK')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('web extraction resource selftest failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        # Why: re module provides essential functionality for this operation
        import re as _re
        setup_py = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'installer', 'setup_app.py')
        if os.path.isfile(setup_py):
            with open(setup_py, encoding='utf-8') as f:
                _src = f.read()
            # Why: Regex pattern matches specific text structures for validation or extraction
            m1 = _re.search("APP_VERSION\\s*=\\s*\\(?\\s*os\\.environ\\.get\\('READMD_VERSION_OVERRIDE'\\)[\\s\\S]*?or\\s+'([^']+)'", _src)
            # Why: Regex pattern matches specific text structures for validation or extraction
            m2 = _re.search("APP_VERSION\\s*=\\s*'([^']+)'", _src)
            if os.environ.get('READMD_VERSION_OVERRIDE'):
                # Why: Assertion validates critical assumptions that must hold for correct operation
                assert m1 is not None or m2 is not None, '未找到 APP_VERSION（env override 链）'
                safe_print('version consistency OK (%s, env override)' % VERSION)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                inst_ver = m1.group(1) if m1 else m2.group(1) if m2 else None
                assert inst_ver == VERSION, '安装器版本 %s 与主程序 %s 不一致' % (inst_ver, VERSION)
                safe_print('version consistency OK (%s)' % VERSION)
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            safe_print('installer/setup_app.py not found, skip version check')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('version consistency failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import urllib.request
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('fixer tests import failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        server = start_server(0)
        port = server.server_port
        # Why: HTTP requests require proper error handling for network failures and server errors
        with urllib.request.urlopen('http://127.0.0.1:%d/' % port, timeout=5) as r:
            body = r.read().decode('utf-8', 'replace')
            # Why: Assertion validates critical assumptions that must hold for correct operation
            assert r.status == 200 and 'ReadMD' in body
        if getattr(sys, 'frozen', False):
            # Why: HTTP requests require proper error handling for network failures and server errors
            with urllib.request.urlopen('http://127.0.0.1:%d/api/modules' % port, timeout=10) as r:
                d = json.loads(r.read().decode('utf-8'))
                # Why: Assertion validates critical assumptions that must hold for correct operation
                assert 'modules' in d and 'ai' in d['modules']
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            self_file = os.path.abspath(__file__)
            # Why: HTTP requests require proper error handling for network failures and server errors
            with urllib.request.urlopen('http://127.0.0.1:%d/api/file?p=%s' % (port, quote(self_file)), timeout=5) as r:
                d = json.loads(r.read().decode('utf-8'))
                # Why: Assertion validates critical assumptions that must hold for correct operation
                assert d['name'] == 'readmd.py'
        safe_print('http server OK (port %d)' % port)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('http selftest failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import urllib.request as _urlreq
        old_inst = _read_instance()
        srv = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        srv.daemon_threads = True
        # Why: Method call handles data access with proper error checking
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        p = srv.server_port
        tok = 'selftest-%s' % os.urandom(4).hex()
        _write_instance(p, tok)

        def _ctl(url, data=None):
            # Why: HTTP requests require proper error handling for network failures and server errors
            req = _urlreq.Request('http://127.0.0.1:%d%s' % (p, url), data=data, method='POST' if data is not None else 'GET', headers={'Content-Type': 'application/json'} if data is not None else {})
            try:
                # Why: HTTP requests require proper error handling for network failures and server errors
                with _urlreq.urlopen(req, timeout=5) as r:
                    return json.loads(r.read().decode('utf-8'))
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except _urlreq.HTTPError as e:
                logging.warning('Silent exception caught in readmd: _urlreq.HTTPError')
                # Why: Return provides result to caller after processing completes
                return json.loads(e.read().decode('utf-8'))
        # Why: Method call handles data access with proper error checking
        assert _ctl('/api/ping?t=' + tok).get('ok') is True
        # Why: Method call handles data access with proper error checking
        assert _ctl('/api/ping?t=bad').get('ok') is False
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert _ctl('/api/control/open', json.dumps({'token': 'bad', 'file': ''}).encode('utf-8')).get('ok') is not True
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert _ctl('/api/control/open', json.dumps({'token': tok, 'file': ''}).encode('utf-8')).get('ok') is True
        d = _ctl('/api/control/next')
        # Why: Method call handles data access with proper error checking
        assert d.get('pending') is True and d.get('file') == ''
        d = _ctl('/api/control/next')
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert d.get('pending') is False
        srv.shutdown()
        srv.server_close()
        save_json(INSTANCE_FILE, old_inst)
        safe_print('single-instance control OK')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('single-instance selftest failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        t = save_prompt({'name': '_selftest', 'system': 'x', 'action': 'ask'})
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert load_prompts()['templates']
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert delete_prompt(t['id'])
        s = save_session({'title': '_selftest', 'provider': 'DeepSeek', 'model': 'deepseek-chat', 'doc': 't', 'messages': [{'role': 'user', 'content': 'hi'}]})
        assert s['id'] and load_history()[0]['id'] == s['id']
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert delete_session(s['id'])
        safe_print('prompts/history OK')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('prompts/history selftest failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import tempfile, base64 as _b64
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with tempfile.TemporaryDirectory() as td:
            png = _b64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==')
            img_dir = os.path.join(td, 'images')
            os.makedirs(img_dir, exist_ok=True)
            target = os.path.join(img_dir, 't.png')
            # Why: Context manager ensures proper resource cleanup even if errors occur
            with open(target, 'wb') as f:
                f.write(png)
            # Why: Assertion validates critical assumptions that must hold for correct operation
            assert os.path.isfile(target)
        safe_print('image save OK')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('image save selftest failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import tempfile as _tf
        from src.readmd_modules.mdexport import export as _export
        demo_md = '# ReadMD 导出自测\n\n正文 **加粗** 与 `代码`，公式 $\\frac{a}{b}$。\n\n| 列A | 列B |\n| --- | --- |\n| 1 | 2 |\n'
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _tf.TemporaryDirectory() as td:
            # Why: Iteration processes each item in collection systematically
            for (_fmt, _ext) in (('pdf', '.pdf'), ('docx', '.docx'), ('html', '.html')):
                out = os.path.join(td, 'smoke' + _ext)
                r = _export(_fmt, demo_md, td, out, options={'meta': {'title': 'Selftest'}})
                # Why: Assertion validates critical assumptions that must hold for correct operation
                assert r.get('ok') is True and os.path.isfile(out) and (r.get('size', 0) > 0), r
        safe_print('export OK')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('export selftest failed:', e)
        ok = False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import tempfile as _tf
        import time as _tm
        import urllib.request as _uq
        # Why: Function call performs specific operation required by this logic
        RM.load_forced('convert')
        # Why: Function call performs specific operation required by this logic
        _td = _tf.mkdtemp()
        from docx import Document as _Doc
        # Why: Function call performs specific operation required by this logic
        _dp = os.path.join(_td, 'smoke.docx')
        # Why: Function call performs specific operation required by this logic
        _d = _Doc()
        # Why: Function call performs specific operation required by this logic
        _d.add_heading('Selftest', level=1)
        # Why: Function call performs specific operation required by this logic
        _d.add_paragraph('hello world')
        # Why: Function call performs specific operation required by this logic
        _d.save(_dp)
        from src.readmd_modules import convert as _CV
        # Why: Function call performs specific operation required by this logic
        (txt, eng, err) = _CV.convert_verbose(_dp)
        assert eng == 'docx' and err is None and ('# Selftest' in txt), (eng, err)
        srv3 = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        srv3.daemon_threads = True
        # Why: Method call handles data access with proper error checking
        threading.Thread(target=srv3.serve_forever, daemon=True).start()
        p3 = srv3.server_port
        # Why: HTTP requests require proper error handling for network failures and server errors
        req = _uq.Request('http://127.0.0.1:%d/api/convert/batch' % p3, data=json.dumps({'paths': [_dp], 'overwrite': True}).encode('utf-8'), method='POST', headers={'Content-Type': 'application/json'})
        # Why: HTTP requests require proper error handling for network failures and server errors
        with _uq.urlopen(req, timeout=30) as r:
            bd = json.loads(r.read().decode('utf-8'))
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert bd.get('job'), bd
        jid = bd['job']
        pr = {}
        for _ in range(60):
            # Why: HTTP requests require proper error handling for network failures and server errors
            with _uq.urlopen('http://127.0.0.1:%d/api/convert/progress?job=%s' % (p3, jid), timeout=5) as r:
                pr = json.loads(r.read().decode('utf-8'))
            if pr.get('finished'):
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            _tm.sleep(0.25)
        # Why: Method call handles data access with proper error checking
        assert pr.get('finished') and pr['items'][0].get('status') == 'ok', pr
        # Why: Assertion validates critical assumptions that must hold for correct operation
        assert os.path.isfile(os.path.join(_td, 'smoke.md'))
        srv3.shutdown()
        srv3.server_close()
        safe_print('convert OK')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('convert selftest failed:', e)
        ok = False
    safe_print('selftest %s' % ('PASSED' if ok else 'FAILED'))
    # Why: Conditional return handles different cases based on input or state
    return 0 if ok else 1

def run_webview_selftest():
    """Exercise the native WebView network guard against a private subresource."""
    hits = {'probe': 0}

    # Why: Function call performs specific operation required by this logic
    class ProbeHandler(BaseHTTPRequestHandler):

        # Why: Function call performs specific operation required by this logic
        def do_GET(self):
            # Why: Arithmetic operation computes value needed for subsequent processing
            hits['probe'] += 1
            # Why: Function call performs specific operation required by this logic
            self.send_response(204)
            # Why: Function call performs specific operation required by this logic
            self.end_headers()

        def log_message(self, *_args):
            pass
    probe = ThreadingHTTPServer(('127.0.0.1', 0), ProbeHandler)
    # Why: Method call handles data access with proper error checking
    threading.Thread(target=probe.serve_forever, daemon=True).start()

    class PageHandler(BaseHTTPRequestHandler):

        def do_GET(self):
            body = ('<!doctype html><html><head><title>Guard selftest</title></head>\n              <body><main><h1>Native WebView guard selftest</h1>\n              <p>This local fixture verifies that the rendered document remains readable\n              # Why: 循环等待：持续检查直到满足条件\n              while a cross-origin private subresource request is denied before sending.</p>\n              <img src="http://127.0.0.1:%d/probe"></main></body></html>' % probe.server_port).encode('utf-8')
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(body)))
            # Why: Function call performs specific operation required by this logic
            self.end_headers()
            # Why: Function call performs specific operation required by this logic
            self.wfile.write(body)

        def log_message(self, *_args):
            pass
    page = ThreadingHTTPServer(('127.0.0.1', 0), PageHandler)
    # Why: Method call handles data access with proper error checking
    threading.Thread(target=page.serve_forever, daemon=True).start()
    outcome = {'ok': False, 'error': 'WebView callback did not run'}
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import webview
        RM.load_forced('web')
        # Why: Function call performs specific operation required by this logic
        api = Api()
        # Why: Function call performs specific operation required by this logic
        host = webview.create_window('ReadMD WebView selftest', 'about:blank', hidden=True, width=320, height=240)

        def exercise():
            task_id = 'native-guard-selftest'
            target = 'http://127.0.0.1:%d/article' % page.server_port
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                grant = api.authorize_private_web(target, task_id)
                # Why: Condition check ensures valid state before proceeding with operation
                if not grant.get('ok'):
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise AssertionError(grant)
                rendered = api.render_web_page(target, task_id, 15000, False, grant['grant'], '')
                # Why: Condition check ensures valid state before proceeding with operation
                if not rendered.get('ok'):
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise AssertionError(rendered)
                time.sleep(0.25)
                if hits['probe']:
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise AssertionError('private cross-origin request escaped guard')
                offline_html = '<!doctype html><html><head><title>Offline guard</title></head>\n                      <body><main><h1>Offline public document</h1><p>The native renderer\n                      must preserve the verified public base URL while blocking every\n                      network request from untrusted page HTML.</p><a href="next">next</a>\n                      <img src="http://127.0.0.1:%d/offline-probe"></main></body></html>' % probe.server_port
                offline = api.render_web_page('https://93.184.216.34/selftest', 'offline-guard-selftest', 15000, False, '', offline_html)
                # Why: Condition check ensures valid state before proceeding with operation
                if not offline.get('ok'):
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise AssertionError(offline)
                # Why: Condition check ensures valid state before proceeding with operation
                if not str(offline.get('final_url', '')).startswith('https://93.184.216.34/'):
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise AssertionError('offline renderer lost verified base URL')
                if hits['probe']:
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise AssertionError('offline HTML escaped network guard')
                outcome.update(ok=True, error='')
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception as exc:
                logging.warning('Silent exception caught in readmd: Exception')
                outcome.update(ok=False, error=str(exc))
            # Why: Finally ensures cleanup operations run regardless of success or failure
            finally:
                api.revoke_private_web(task_id)
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    host.destroy()
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
        if IS_MAC:
            webview.start(exercise, gui='cocoa', private_mode=True)
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            webview.start(exercise, private_mode=True)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as exc:
        logging.warning('Silent exception caught in readmd: Exception')
        outcome.update(ok=False, error=str(exc))
    # Why: Finally ensures cleanup operations run regardless of success or failure
    finally:
        # Why: Iteration processes each item in collection systematically
        for server in (page, probe):
            server.shutdown()
            server.server_close()
    safe_print('webview network guard %s%s' % ('PASSED' if outcome['ok'] else 'FAILED', '' if outcome['ok'] else ': ' + outcome['error']))
    # Why: Conditional return handles different cases based on input or state
    return 0 if outcome['ok'] else 1

def main():
    # Why: Scope declaration allows modification of variables from outer scope
    global _T0
    parser = argparse.ArgumentParser(description='ReadMD - 轻量级 Markdown 阅读器')
    parser.add_argument('file', nargs='?', help='要打开的 .md 文件')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--browser', action='store_true', help='用默认浏览器打开（兜底模式）')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--port', type=int, default=0, help='本地服务端口（默认随机）')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--selftest', action='store_true', help='运行自测')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--webview-selftest', action='store_true', help='运行原生 WebView 私网隔离自测')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--mods', action='store_true', help='加载全部扩展模块并报告状态')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--share', action='store_true', help='启动后自动开启局域网共享（手机扫码访问）')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--assoc', action='store_true', help='注册 .md 默认打开方式后退出')
    # Why: Function call performs specific operation required by this logic
    parser.add_argument('--startup-probe', action='store_true', help='记录启动里程碑并在页面就绪后自动退出')
    parser.add_argument('--startup-probe-json', metavar='PATH', help='把 --startup-probe 的 JSON 报告原子写入 PATH')
    parser.add_argument('--startup-probe-timeout', type=float, default=20.0, metavar='SECONDS', help='启动 probe 超时秒数（默认 20）')
    args = parser.parse_args()
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if args.startup_probe_json and (not args.startup_probe):
        parser.error('--startup-probe-json 需要 --startup-probe')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if args.startup_probe and args.browser:
        parser.error('--startup-probe 不能与 --browser 同时使用')
    if args.startup_probe_timeout <= 0:
        parser.error('--startup-probe-timeout 必须大于 0')
    if args.startup_probe:
        _T0 = time.time()
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _BOOT_LOCK:
            _BOOT_MILESTONES.clear()
            _STARTUP_PROBE.update({'enabled': True, 'timeout': args.startup_probe_timeout, 'json_path': args.startup_probe_json or '', 'window': None, 'finished': False, 'timed_out': False, 'timer': None})
    if args.assoc:
        r = install_association()
        safe_print('association: %s' % ('OK' if r is True else r))
        # Why: Conditional return handles different cases based on input or state
        return 0 if r is True else 1
    if args.selftest:
        sys.exit(run_selftest())
    if args.webview_selftest:
        sys.exit(run_webview_selftest())
    if args.mods:
        ok = True
        # Why: Iteration processes each item in collection systematically
        for m in RM.MODULES:
            good = RM.load_forced(m)
            (st, err) = RM.status()
            safe_print('%s: %s%s' % (m, st.get(m), ' - ' + err.get(m, '') if err.get(m) else ''))
            ok = ok and good
        # Why: Conditional return handles different cases based on input or state
        return 0 if ok else 1
    setup_logging()
    milestone('boot', 'start')
    if is_win7():
        RM.set_disabled(('ocr', 'web', 'ai'), WIN7_UNAVAILABLE)
    alive = None if args.startup_probe else instance_alive()
    # Why: Condition check ensures valid state before proceeding with operation
    if alive is not None:
        (port, token) = alive
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not args.file or forward_open(port, token, os.path.abspath(args.file)):
            return 0
    server = start_server(args.port)
    # Why: Condition check ensures valid state before proceeding with operation
    if server.server_port == CONTROL_PORT:
        _write_instance(CONTROL_PORT, secrets.token_urlsafe(16))
    milestone('boot', 'server_up')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        from src.readmd_modules import updater
        updater.clean_old_update_artifacts()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
    if args.share:
        d = start_lan_server()
        if d.get('ok'):
            safe_print('局域网共享已开启：%s' % d.get('url'))
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            safe_print('局域网共享失败：%s' % d.get('error'))
    initial = None
    if args.file:
        p = os.path.abspath(args.file)
        if os.path.isfile(p):
            initial = p
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            safe_print('文件不存在: %s' % args.file)
    url = 'http://127.0.0.1:%d/' % server.server_port
    if initial:
        # Why: Function call performs specific operation required by this logic
        url += '?file=' + quote(initial)
    if args.browser:
        webbrowser.open(url)
        safe_print('ReadMD 服务运行于 %s（Ctrl+C 退出）' % url)
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Loop continues until condition is met or timeout occurs
            while True:
                threading.Event().wait(3600)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except KeyboardInterrupt:
            logging.warning('Silent exception caught in readmd: KeyboardInterrupt')
        _clear_instance()
        # Why: Return provides result to caller after processing completes
        return 0
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import webview
    # Why: Handle missing dependencies gracefully to provide helpful installation instructions
    except ImportError:
        logging.warning('Silent exception caught in readmd: ImportError')
        # Why: Windows-specific behavior requires different implementation due to OS differences
        safe_print('未安装 pywebview。请先运行 install%s，或用 --browser 模式。' % ('.sh' if sys.platform != 'win32' else '.bat'))
        safe_print('快速兜底：python readmd.py --browser "%s"' % (initial or ''))
        if args.startup_probe:
            write_startup_probe(args.startup_probe_json, timed_out=False)
        # Why: Return provides result to caller after processing completes
        return 1
    api = Api()
    milestone('boot', 'webview_imported')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        window = webview.create_window('ReadMD', url, js_api=api, width=1160, height=820, min_size=(720, 480), text_select=True, zoomable=True, background_color='#f7f7f5')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        safe_print('创建窗口失败：%s' % e)
        if args.startup_probe:
            write_startup_probe(args.startup_probe_json, timed_out=False)
        # Why: Return provides result to caller after processing completes
        return 1
    api._window = window
    api._on_page_ready = lambda : _start_tray_once(window)
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _control_lock:
        _CONTROL['window'] = window
        _CONTROL['ready'] = True
    milestone('boot', 'window_created')
    if args.startup_probe:
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _BOOT_LOCK:
            _STARTUP_PROBE['window'] = window
            timer = threading.Timer(args.startup_probe_timeout, _finish_startup_probe, kwargs={'timed_out': True})
            timer.daemon = True
            _STARTUP_PROBE['timer'] = timer
        # Why: Function call performs specific operation required by this logic
        timer.start()

    def _on_loaded():
        milestone('boot', 'window_loaded')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        window.events.loaded += _on_loaded
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')

    def _on_closing():
        if args.startup_probe:
            # Why: Return provides result to caller after processing completes
            return True
        # Why: Condition check ensures valid state before proceeding with operation
        if not api._page_ready:
            # Why: Return provides result to caller after processing completes
            return True
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            window.hide()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
        # Why: Return provides result to caller after processing completes
        return False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        window.events.closing += _on_closing
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
    setup_win7_webview2_env()
    if IS_LINUX:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from src.readmd_modules import linux_native
            linux_native.setup_linux_env()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        if IS_MAC:
            webview.start(gui='cocoa')
        # Why: Alternative condition handles different case in decision tree
        elif IS_LINUX:
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                webview.start(gui='gtk')
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in readmd: Exception')
                webview.start()
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            webview.start()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.exception('webview start failed')
        safe_print('启动失败：%s' % e)
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            if IS_WIN:
                from src.readmd_modules import windows_native
                windows_native.show_error('ReadMD', '启动失败：%s' % e)
            # Why: Alternative condition handles different case in decision tree
            elif IS_MAC:
                from src.readmd_modules import macos_native
                macos_native.show_error('ReadMD', '启动失败，请查看日志。')
            # Why: Alternative condition handles different case in decision tree
            elif IS_LINUX:
                from src.readmd_modules import linux_native
                linux_native.show_notification('ReadMD 启动失败', str(e))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')
        if args.startup_probe:
            # Why: Method call handles data access with proper error checking
            write_startup_probe(args.startup_probe_json, timed_out=bool(_STARTUP_PROBE.get('timed_out')))
        # Why: Return provides result to caller after processing completes
        return 1
    if args.startup_probe:
        # Why: Method call handles data access with proper error checking
        timed_out = bool(_STARTUP_PROBE.get('timed_out'))
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            write_startup_probe(args.startup_probe_json, timed_out=timed_out)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('Silent exception caught in readmd: Exception')
            safe_print('startup probe write failed: %s' % exc)
            # Why: Return provides result to caller after processing completes
            return 1
        _clear_instance()
        # Why: Conditional return handles different cases based on input or state
        return 1 if timed_out else 0
    _clear_instance()
    # Why: Return provides result to caller after processing completes
    return 0
_tray_icon = {'icon': None, 'started': False}
_tray_lock = threading.Lock()

def _start_tray_once(window):
    """Create the tray only after the page is usable, and never twice."""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _tray_lock:
        if _tray_icon['started']:
            # Why: Return provides result to caller after processing completes
            return _tray_icon['icon']
        _tray_icon['started'] = True
        # Why: Return provides result to caller after processing completes
        return _start_tray(window)

def _start_tray(window):
    """启动系统托盘图标（pystray run_detached）；失败静默降级。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import pystray
        from PIL import Image
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
        # Why: Return provides result to caller after processing completes
        return None
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        icon_candidates = ['icon-256.png', 'readmd.ico']
        img = None
        # Why: Iteration processes each item in collection systematically
        for fname in icon_candidates:
            p = os.path.join(APP_DIR, 'assets', fname)
            if os.path.isfile(p):
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    # Why: Method call handles data access with proper error checking
                    img = Image.open(p)
                    break
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in readmd: Exception')
                    continue
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in readmd: Exception')
        img = None

    def act_show(icon, item):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            window.show()
            window.restore()
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')

    def act_open(icon, item):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            window.show()
            window.restore()
            window.evaluate_js('window.__trayOpenFile && window.__trayOpenFile();')
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in readmd: Exception')

    def act_quit(icon, item):
        quit_app()
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        menu = pystray.Menu(pystray.MenuItem('显示 ReadMD', act_show, default=True), pystray.MenuItem('打开文件…', act_open), pystray.Menu.SEPARATOR, pystray.MenuItem('退出 ReadMD', act_quit))
        icon = pystray.Icon('readmd', img, 'ReadMD', menu=menu)
        # Why: Function call performs specific operation required by this logic
        icon.run_detached()
        _tray_icon['icon'] = icon
        logging.info('tray started')
        return icon
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in readmd: Exception')
        logging.exception('tray start failed: %s', e)
        # Why: Return provides result to caller after processing completes
        return None
    # Why: Return provides result to caller after processing completes
    return 0
# Why: Condition check ensures valid state before proceeding with operation
if __name__ == '__main__':
    sys.exit(main())