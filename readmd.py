#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""ReadMD —— 轻量级本地 Markdown 阅读器。

特性：
  - 本地 127.0.0.1 HTTP 服务 + pywebview 原生窗口，秒开
  - 渲染前自动修正常见错误（表格 / 加粗 / 公式 / 标题），只影响显示
  - 自动刷新、目录、搜索、主题、字号、最近文件、文件夹浏览、打印
  - 全部资源离线（marked + MathJax 已内置），无需联网

用法：
  python readmd.py [文件.md]        # 打开文件（或空启动）
  python readmd.py --browser [文件] # 用默认浏览器打开（无 pywebview 时兜底）
  python readmd.py --selftest       # 自测（修正器 + 本地服务）
"""

import argparse
import json
import logging
import mimetypes
import os
import re
import secrets
import socket
import subprocess
import sys
import time
import threading
import webbrowser
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, unquote, urlparse

from src.readmd_core import (
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
    load_json,
    save_json,
    read_text,
    readmd_fix,
)
from src.readmd_core.file_writer import save_text_atomic
import src.readmd_modules as RM
from src.readmd_modules.validators import validate_file_path, validate_command, paths_within

APP_DIR = sys._MEIPASS if getattr(sys, 'frozen', False) else os.path.dirname(os.path.abspath(__file__))


def _bundle_version():

    """frozen 构建内嵌 version.txt（Win7 链：2.1.1 Beta）。"""
    try:
        if getattr(sys, 'frozen', False):
            base = getattr(sys, '_MEIPASS', None) or os.path.dirname(os.path.abspath(sys.executable))
            p = os.path.join(base, 'version.txt')
            if os.path.isfile(p):
                v = open(p, encoding='utf-8').read().strip()
                if v:
                    return v
    except Exception:
        pass
def _env_or_bundle_version():
    v = os.environ.get('READMD_VERSION') or os.environ.get('READMD_BUILD_VERSION')
    if v:
        return v.strip()
    try:
        env_p = os.path.join(os.path.dirname(os.path.abspath(__file__)), '.env')
        if os.path.isfile(env_p):
            with open(env_p, 'r', encoding='utf-8') as f:
                for line in f:
                    if line.startswith('READMD_VERSION='):
                        val = line.split('=', 1)[1].strip().strip('\'"')
                        if val:
                            return val
    except Exception:
        pass
    return _bundle_version()


VERSION = (_env_or_bundle_version() or '2.3.7-beta.3')





MD_EXTS = ('.md', '.markdown', '.mdown', '.mkd', '.mdx', '.txt')
CONVERT_EXTS = ('.docx', '.doc', '.pptx', '.ppt', '.xlsx', '.xls', '.pdf', '.html', '.htm',
                '.txt', '.csv', '.json', '.xml', '.zip', '.eml', '.msg', '.rtf', '.odt', '.epub')
WIN7_CONVERT_EXTS = ('.docx', '.pdf')
WIN7_UNAVAILABLE = '该功能在 Win7 版暂不支持（本版本仅保留 docx / pdf 转 MD 与导出功能）'

# ------------------------------------------------------------------ 升级推送（静默）

_UPGRADE_RELEASE_URL = 'https://api.github.com/repos/Natsummerance/readMD/releases/latest'
_UPGRADE_CACHE = {'done': False, 'result': None}


def _parse_version(value):
    """'v2.2.6' / '2.2.6' -> (2, 2, 5)；无法解析返回 None。"""
    try:
        parts = []
        for chunk in re.sub(r'^v', '', str(value or '')).replace('-', '.').split('.'):
            if not chunk.isdigit():
                return None
            parts.append(int(chunk))
        return tuple(parts) if parts else None
    except Exception:
        return None


def check_latest_release():
    """查询 GitHub 最新 Release；失败/超时静默返回 None，结果进程内缓存。"""
    if _UPGRADE_CACHE['done']:
        return _UPGRADE_CACHE['result']
    result = None
    try:
        import urllib.request as _urlreq
        req = _urlreq.Request(_UPGRADE_RELEASE_URL, headers={
            'User-Agent': 'ReadMD/%s' % VERSION,
            'Accept': 'application/vnd.github+json',
        })
        with _urlreq.urlopen(req, timeout=4) as resp:
            data = json.loads(resp.read(1024 * 1024).decode('utf-8'))
        tag = str(data.get('tag_name') or '')
        latest = _parse_version(tag)
        current = _parse_version(VERSION)
        if latest and current and latest > current:
            result = {
                'latest': tag,
                'url': str(data.get('html_url') or _UPGRADE_RELEASE_URL),
            }
    except Exception:
        logging.debug('upgrade check failed (silent)', exc_info=True)
    _UPGRADE_CACHE['done'] = True
    _UPGRADE_CACHE['result'] = result
    return result

CONTROL_PORT = 26891
INSTANCE_FILE = os.path.join(DATA_DIR, 'instance.json')
_CONVERT_JOBS = {}
_CONVERT_JOB_SEQ = [0]
_CONVERT_LOCK = threading.Lock()

_T0 = time.time()
_BOOT_LOCK = threading.Lock()
_BOOT_MILESTONES = {}
_STARTUP_PROBE = {'enabled': False, 'timeout': 20.0, 'json_path': '',
                  'window': None, 'finished': False, 'timed_out': False,
                  'timer': None}


def is_win7():
    """Win7 检测：驱动功能裁剪与内置固定版 WebView2 109 运行时。"""
    if os.environ.get('READMD_FORCE_WIN7') == '1':
        return True
    try:
        import platform
        return platform.system() == 'Windows' and platform.release() == '7'
    except Exception:
        return False


def setup_win7_webview2_env():
    """Win7：把内置固定版 WebView2 109 运行时目录与嵌入式 user-data 目录注入环境变量，
    win7 构建里打过补丁的 pywebview edgechromium 会读取这两个变量。"""
    if not is_win7():
        return
    try:
        if getattr(sys, 'frozen', False):
            base = os.path.dirname(os.path.abspath(sys.executable))
        else:
            base = APP_DIR
        rt = os.path.join(base, 'webview2_runtime')
        if os.path.isdir(rt):
            os.environ['READMD_WEBVIEW2_RUNTIME'] = rt
            os.environ['READMD_WEBVIEW2_USERDATA'] = os.path.join(base, 'webview2_userdata')
    except Exception:
        pass


def milestone(group, name):
    """启动里程碑打点：写入 readmd.log，用于验证“秒开”。"""
    elapsed = int((time.time() - _T0) * 1000)
    if group == 'boot':
        with _BOOT_LOCK:
            _BOOT_MILESTONES.setdefault(name, elapsed)
    try:
        logging.info('[%s] %dms %s', group, elapsed, name)
    except Exception:
        pass


def startup_probe_summary(milestones=None, timed_out=False):
    """Build a privacy-safe startup report; deliberately contains no document data."""
    milestones = dict(_BOOT_MILESTONES if milestones is None else milestones)
    names = ('server_up', 'window_created', 'window_loaded', 'page_loaded',
             'first_document')
    return {'version': VERSION, 'timed_out': bool(timed_out),
            'milestones_ms': {name: milestones.get(name) for name in names}}


def write_startup_probe(path='', timed_out=False):
    """Print and optionally atomically persist a startup probe report."""
    report = startup_probe_summary(timed_out=timed_out)
    encoded = json.dumps(report, ensure_ascii=False, sort_keys=True)
    safe_print(encoded)
    if path:
        directory = os.path.dirname(os.path.abspath(path))
        if directory:
            os.makedirs(directory, exist_ok=True)
        tmp = os.path.join(directory, '.%s.%s.tmp' %
                           (os.path.basename(path), os.getpid()))
        try:
            with open(tmp, 'w', encoding='utf-8') as handle:
                handle.write(encoded + '\n')
            os.replace(tmp, path)
        except Exception:
            try:
                if os.path.exists(tmp):
                    os.remove(tmp)
            except Exception:
                pass
            raise
    return report


def _finish_startup_probe(timed_out=False):
    """End a probe run without persisting document paths or document content."""
    with _BOOT_LOCK:
        if not _STARTUP_PROBE.get('enabled') or _STARTUP_PROBE.get('finished'):
            return
        _STARTUP_PROBE['finished'] = True
        _STARTUP_PROBE['timed_out'] = bool(timed_out)
        timer = _STARTUP_PROBE.get('timer')
    if timer is not None:
        try:
            timer.cancel()
        except Exception:
            pass
    window = _STARTUP_PROBE.get('window')
    if window is not None:
        try:
            window.destroy()
        except Exception:
            pass


# ---------------------------------------------------------------- 单实例常驻
# 固定控制端口 + instance.json（端口/随机 token）。新进程先 ping 已有实例，
# 命中则把要打开的文件 POST 过去后立即退出，实现“双击 .md 秒开”。

_CONTROL = {'queue': [], 'window': None, 'ready': False}
_control_lock = threading.Lock()


def _read_instance():
    return load_json(INSTANCE_FILE, {})


def _write_instance(port, token):
    save_json(INSTANCE_FILE, {'port': port, 'token': token,
                              'pid': os.getpid(), 'started': time.time()})


def _clear_instance():
    try:
        if os.path.isfile(INSTANCE_FILE):
            os.remove(INSTANCE_FILE)
    except Exception:
        pass


def _ping_instance(port, token, timeout=0.8):
    try:
        import urllib.request
        req = urllib.request.Request(
            'http://127.0.0.1:%d/api/ping?t=%s' % (port, token))
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return bool(json.loads(r.read().decode('utf-8')).get('ok'))
    except Exception:
        return False


def instance_alive():
    """存在可用的常驻实例则返回 (port, token)，否则 None。"""
    d = _read_instance()
    port = d.get('port')
    token = d.get('token')
    if not port or not token:
        return None
    return (port, token) if _ping_instance(port, token) else None


def forward_open(port, token, path):
    """把文件转发给常驻实例并唤起窗口；成功返回 True。"""
    import urllib.request
    payload = json.dumps({'token': token, 'file': path or ''}).encode('utf-8')
    req = urllib.request.Request(
        'http://127.0.0.1:%d/api/control/open' % port,
        data=payload, headers={'Content-Type': 'application/json'}, method='POST')
    try:
        with urllib.request.urlopen(req, timeout=3) as r:
            return bool(json.loads(r.read().decode('utf-8')).get('ok'))
    except Exception:
        return False


def push_control(path):
    """控制请求入队；窗口就绪时立即推送并显示（秒开路径）。"""
    with _control_lock:
        _CONTROL['queue'].append(path or '')
        win = _CONTROL.get('window')
        ready = _CONTROL.get('ready')
    if win is not None and ready:
        try:
            win.evaluate_js('window.openExternalFile(%s);' % json.dumps(path or ''))
        except Exception:
            pass
        try:
            win.show()
            win.restore()
        except Exception:
            pass


def pop_control():
    with _control_lock:
        if _CONTROL['queue']:
            return _CONTROL['queue'].pop(0)
    return None


def quit_app():
    """托盘“退出 ReadMD”：清理单实例文件后结束进程。"""
    try:
        _clear_instance()
    except Exception:
        pass
    try:
        stop_lan_server()
    except Exception:
        pass
    os._exit(0)


def safe_print(*args, **kwargs):
    try:
        if sys.stdout is not None:
            print(*args, **kwargs)
    except Exception:
        pass


def setup_logging():
    try:
        os.makedirs(DATA_DIR, exist_ok=True)
        logging.basicConfig(
            filename=LOG_FILE, level=logging.INFO, encoding='utf-8',
            format='%(asctime)s %(levelname)s %(message)s')
    except Exception:
        pass





_WINDOWS_RESERVED_NAMES = {
    'CON', 'PRN', 'AUX', 'NUL',
    *('COM%d' % i for i in range(1, 10)),
    *('LPT%d' % i for i in range(1, 10)),
}


def _validate_rename_stem(stem, extension):
    stem = str(stem or '')
    if not stem or stem != stem.strip():
        raise ValueError('文件名不能为空或以空格开头、结尾')
    if stem.endswith('.') or any(ord(ch) < 32 for ch in stem):
        raise ValueError('文件名包含无效字符')
    if any(ch in stem for ch in '<>:"/\\|?*'):
        raise ValueError('文件名不能包含 < > : " / \\ | ? *')
    if stem.split('.', 1)[0].upper() in _WINDOWS_RESERVED_NAMES:
        raise ValueError('该名称是 Windows 系统保留名')
    filename = stem + extension
    if len(filename) > 255:
        raise ValueError('文件名过长')
    return stem


def _paths_equal(left, right):
    return os.path.normcase(os.path.abspath(left)) == os.path.normcase(os.path.abspath(right))


def _same_file_target(left, right):
    """Handle case-only names on case-insensitive macOS/Windows volumes."""
    if _paths_equal(left, right):
        return True
    try:
        return os.path.exists(left) and os.path.exists(right) and os.path.samefile(left, right)
    except (OSError, ValueError):
        return False



# ---------------------------------------------------------------- AI 模板 / 历史会话

# 内置 Prompt 模板（只读；可覆盖为自定义版本，或另存为自定义模板）
BUILTIN_PROMPTS = [
    {"id": "quick_read", "name": "快速阅读", "action": "quick_read",
     "system": "你是 ReadMD 的文档阅读助手。对用户给出的 Markdown 文档做快速阅读，输出：1) 一句话概述；2) 核心要点列表；3) 文档结构目录；4) 值得注意的细节或疑问。使用 Markdown 格式。",
     "user": ""},
    {"id": "polish", "name": "润色", "action": "polish",
     "system": "你是资深中文编辑。润色用户给出的 Markdown 文档：修正错别字、病句、表达生硬之处，保留原有结构与全部 Markdown 标记，只输出润色后的完整文档，不要加任何解释。",
     "user": ""},
    {"id": "modify", "name": "修改", "action": "modify",
     "system": "你是文档修订助手。根据用户要求修改文档，修正明显错误（错别字、标点、Markdown 格式错误）。只输出修改后的完整文档，不要加任何解释。",
     "user": ""},
    {"id": "expand", "name": "扩充", "action": "expand",
     "system": "你是文档扩充助手。在保持原有结构与语气的前提下，为文档补充细节、示例、解释，使内容更丰富。只输出扩充后的完整文档，不要加任何解释。",
     "user": ""},
    {"id": "continue", "name": "续写", "action": "continue",
     "system": "你是文档续写助手。从文档末尾自然延续写作，保持风格一致。只输出续写的新增内容，不要重复原文。",
     "user": ""},
    {"id": "translate", "name": "翻译", "action": "translate",
     "system": "你是专业翻译。将用户给出的文档翻译成指定语言，保留 Markdown 结构、表格与代码块，只输出译文。",
     "user": ""},
    {"id": "ask", "name": "提问", "action": "ask",
     "system": "你是文档问答助手。基于用户给出的文档内容回答问题；文档中没有的内容请明确说明。",
     "user": ""},
    {"id": "summary", "name": "总结要点", "action": "ask",
     "system": "你是文档总结助手。用 5 条以内要点概括用户文档的核心内容，输出为 Markdown 列表；最后用一句话总结全文。",
     "user": ""},
    {"id": "outline", "name": "生成大纲", "action": "ask",
     "system": "你是文档策划。为用户文档生成层级目录大纲（# / ## / ###），只输出大纲，不要其他内容。",
     "user": ""},
    {"id": "weekly", "name": "生成周报", "action": "ask",
     "system": "你是周报助手。根据用户给出的工作内容，整理成结构化周报：本周完成 / 下周计划 / 风险与求助。只输出周报正文。",
     "user": ""},
    {"id": "to_english", "name": "翻译成英文", "action": "translate",
     "system": "你是专业翻译。将用户给出的文档翻译成英文，保留 Markdown 结构、表格与代码块，只输出译文。",
     "user": ""},
    {"id": "code_review", "name": "代码审查", "action": "ask",
     "system": "你是资深代码审查员。审查用户文档中的代码块：指出 bug、安全隐患、可读性问题，并给出修改建议与示例代码。用 Markdown 输出。",
     "user": ""},
    {"id": "action_items", "name": "提取行动项", "action": "ask",
     "system": "你是任务管理助手。从用户文档中提取可执行行动项，用 Markdown 表格输出：事项 / 负责人 / 截止时间 / 优先级。",
     "user": ""},
    {"id": "fix_format", "name": "修正 Markdown 格式", "action": "modify",
     "system": "你是 Markdown 格式专家。修正文档中的格式问题：表格对齐、加粗符号配对、公式写法、标题层级。只输出修正后的完整文档，不要解释。",
     "user": ""},
]


def load_prompts():
    """内置 + 自定义模板合并；自定义可覆盖同名内置。"""
    d = load_json(PROMPTS_FILE, {})
    customs = d.get('templates', [])
    by_id = {t.get('id'): t for t in customs}
    merged = []
    seen = set()
    for b in BUILTIN_PROMPTS:
        bid = b.get('id')
        seen.add(bid)
        merged.append(dict(by_id.get(bid, b), builtin=True))
    for c in customs:
        cid = c.get('id')
        if cid in seen:
            continue
        merged.append(dict(c, builtin=False))
    return {'templates': merged}


def save_prompt(template):
    """新增 / 更新模板。id 为空时自动生成；内置 id 表示覆盖内置模板。"""
    t = dict(template or {})
    if not t.get('id'):
        t['id'] = 't_%d' % int(time.time() * 1000)
    if not t.get('name'):
        t['name'] = '未命名模板'
    t.pop('builtin', None)
    d = load_json(PROMPTS_FILE, {})
    customs = [c for c in d.get('templates', []) if c.get('id') != t.get('id')]
    customs.append(t)
    save_json(PROMPTS_FILE, {'templates': customs})
    return t


def delete_prompt(prompt_id):
    d = load_json(PROMPTS_FILE, {})
    d['templates'] = [t for t in d.get('templates', []) if t.get('id') != prompt_id]
    save_json(PROMPTS_FILE, d)
    return True


def load_history(limit=50):
    d = load_json(HISTORY_FILE, {'sessions': []})
    return d.get('sessions', [])[:limit]


def save_session(session):
    """新增 / 更新会话（按 id upsert），限制会话 50 个、消息 60 条。"""
    s = dict(session or {})
    now = time.time()
    if not s.get('id'):
        s['id'] = 'h_%d' % int(now * 1000)
    s['created'] = s.get('created') or now
    s['updated'] = now
    msgs = (s.get('messages') or [])[-60:]
    s['messages'] = msgs
    s['msgCount'] = len(msgs)
    sessions = [x for x in load_history(500) if x.get('id') != s['id']]
    sessions.insert(0, s)
    save_json(HISTORY_FILE, {'sessions': sessions[:50]})
    return s


def delete_session(session_id):
    sessions = [x for x in load_history(500) if x.get('id') != session_id]
    save_json(HISTORY_FILE, {'sessions': sessions})
    return True


def _md_output_path(src):
    """转换输出路径：源文件同目录同名 .md。"""
    d = os.path.dirname(os.path.abspath(src))
    base = os.path.splitext(os.path.basename(src))[0]
    return os.path.join(d, base + '.md')


def _write_md(path, content):
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(content)
    return True


def _convert_worker(job):
    items = job['items']
    for it in items:
        if job.get('cancel'):
            it['status'] = 'canceled'
            it['done'] = True
            continue
        try:
            mod = RM.get('convert')
            text, engine, err = mod.convert_verbose(it['src'])
            if err and not text:
                it['status'] = 'error'
                it['error'] = err
                it['done'] = True
                continue
            if not text.strip():
                it['status'] = 'error'
                it['error'] = '未提取到文字（可尝试 OCR）'
                it['done'] = True
                continue
            import src.readmd_modules.mdcheck as MDC
            fixed, warns = MDC.check(text, os.path.dirname(os.path.abspath(it['src'])))
            out = _md_output_path(it['src'])
            it['out'] = out
            it['engine'] = engine
            it['warns'] = warns
            if os.path.exists(out) and not job.get('overwrite'):
                it['status'] = 'skipped'
            else:
                try:
                    _write_md(out, fixed)
                    it['status'] = 'ok'
                except Exception as e:  # noqa: BLE001
                    it['status'] = 'error'
                    it['error'] = '写入失败：%s' % e
        except Exception as e:  # noqa: BLE001
            logging.exception('batch convert failed: %s', it.get('src'))
            it['status'] = 'error'
            it['error'] = str(e)
        it['done'] = True
    job['running'] = False
    job['finished'] = True


def _start_convert_job(paths, overwrite):
    with _CONVERT_LOCK:
        _CONVERT_JOB_SEQ[0] += 1
        jid = 'c%d' % _CONVERT_JOB_SEQ[0]
        job = {'id': jid, 'overwrite': bool(overwrite), 'running': True,
               'finished': False, 'cancel': False,
               'items': [{'src': p, 'status': 'queued', 'done': False} for p in paths]}
        _CONVERT_JOBS[jid] = job
        threading.Thread(target=_convert_worker, args=(job,), daemon=True,
                         name='convert-batch-%s' % jid).start()
        return jid


def read_text(path):
    """按编码优先级读取文本文件（UTF-8 / GB18030 / Big5 / Latin-1）。"""
    with open(path, 'rb') as f:
        data = f.read()
    if data.startswith(b'\xef\xbb\xbf'):
        return data.decode('utf-8-sig'), 'utf-8-sig'
    for enc in ('utf-8', 'gb18030', 'big5', 'latin-1'):
        try:
            return data.decode(enc), enc
        except (UnicodeDecodeError, LookupError):
            continue
    return data.decode('utf-8', errors='replace'), 'utf-8'


# ---------------------------------------------------------------- HTTP 服务

SAVE_EXTENSIONS = frozenset(('.md', '.markdown', '.mdown', '.mkd', '.mdx', '.txt'))


class ReadMDHTTPServer(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 128

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.app_token = secrets.token_urlsafe(24)
        self.authorized_save_paths = set()


class Handler(BaseHTTPRequestHandler):
    server_version = 'ReadMD/' + VERSION
    LAN_TOKEN = None

    def log_message(self, fmt, *args):
        pass  # 静默访问日志

    def do_GET(self):
        if not self._lan_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        try:
            self._route()
        except Exception as e:
            logging.exception('http error: %s', self.path)
            try:
                self._send(500, 'text/plain; charset=utf-8', ('error: %s' % e).encode('utf-8'))
            except Exception:
                pass

    def do_POST(self):
        if not self._lan_authorized() or not self._post_origin_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        u = urlparse(self.path)
        if u.path == '/api/save':
            supplied_token = self.headers.get('X-ReadMD-App-Token', '')
            if (not self.server.app_token or not supplied_token or
                    not secrets.compare_digest(supplied_token, self.server.app_token)):
                self._send(403, 'text/plain; charset=utf-8', b'forbidden')
                return
        try:
            self._route()
        except Exception as e:
            logging.exception('http post error: %s', self.path)
            try:
                self._send(500, 'text/plain; charset=utf-8', ('error: %s' % e).encode('utf-8'))
            except Exception:
                pass

    def _lan_authorized(self):
        """局域网模式下，除页面与静态资源外，所有 API 都要求携带 token。"""
        u = urlparse(self.path)
        if u.path.startswith('/api/') and not self._local_host_authorized():
            return False
        if not self.LAN_TOKEN:
            return True
        if u.path in ('/', '/index.html') or u.path.startswith('/assets/'):
            return True
        qs = parse_qs(u.query)
        if qs.get('t', [''])[0] == self.LAN_TOKEN:
            return True
        return self.headers.get('X-ReadMD-Token', '') == self.LAN_TOKEN

    def _local_host_authorized(self):
        if self.LAN_TOKEN:
            return True
        parsed = urlparse('//' + (self.headers.get('Host') or ''))
        hostname = (parsed.hostname or '').lower()
        return (hostname in ('127.0.0.1', 'localhost', '::1')
                and (parsed.port is None or parsed.port == self.server.server_port))

    def _post_origin_authorized(self):
        if not self._local_host_authorized():
            return False
        origin = self.headers.get('Origin')
        if origin:
            parsed = urlparse(origin)
            host = urlparse('//' + (self.headers.get('Host') or ''))
            return ((parsed.hostname or '').lower() == (host.hostname or '').lower()
                    and parsed.port == host.port)
        return self.headers.get('Sec-Fetch-Site', 'none') in ('none', 'same-origin')

    def _route(self):
        u = urlparse(self.path)
        path = u.path
        qs = parse_qs(u.query)
        if path in ('/', '/index.html'):
            self._send_index()
        elif path.startswith('/assets/') or path.startswith('/i18n/'):
            if path.startswith('/assets/'):
                rel = path[len('/assets/'):]
            else:
                rel = path.lstrip('/')
            fp = os.path.normpath(os.path.join(APP_DIR, 'assets', rel))
            base = os.path.normpath(os.path.join(APP_DIR, 'assets'))
            if not paths_within(fp, base):
                self._send(403, 'text/plain; charset=utf-8', b'forbidden')
                return

            mime = mimetypes.guess_type(fp)[0] or 'application/octet-stream'
            if mime.startswith('text/') or mime in ('application/javascript', 'application/json'):
                mime += '; charset=utf-8'
            is_cached = rel.startswith('vendor/') or rel.startswith('i18n/')
            self._send_file(fp, mime, immutable=bool(
                is_cached or
                parse_qs(u.query).get('v') or parse_qs(u.query).get('version') or
                parse_qs(u.query).get('hash')))

        elif path == '/api/file':
            p = unquote(qs.get('p', [''])[0])
            if not p:
                self._send(400, 'text/plain; charset=utf-8', b'missing p')
                return
            self._api_file(p, qs.get('meta', ['0'])[0] == '1')
        elif path == '/api/list':
            p = unquote(qs.get('p', [''])[0])
            self._api_list(p)
        elif path == '/api/modules':
            st, err = RM.status()
            self._send_json(200, {'modules': st, 'errors': err, 'win7': is_win7()})
        elif path == '/api/modules/load':
            self._api_modules_load()
        elif path == '/api/convert/collect':
            self._api_convert_collect()
        elif path == '/api/convert/batch':
            self._api_convert_batch()
        elif path == '/api/convert/progress':
            self._api_convert_progress(qs.get('job', [''])[0])
        elif path == '/api/convert':
            p = unquote(qs.get('p', [''])[0])
            self._api_convert(p)
        elif path == '/api/ocr':
            p = unquote(qs.get('p', [''])[0])
            self._api_ocr(p)
        elif path == '/api/url':
            u = unquote(qs.get('u', [''])[0])
            crawl = qs.get('crawl', ['0'])[0] == '1'
            self._api_url(u, crawl)
        elif path == '/api/web/extract':
            self._api_web_extract()
        elif path == '/api/web/cancel':
            self._api_web_cancel()
        elif path == '/api/save':
            self._do_save()
        elif path == '/api/upload':
            self._do_upload(qs.get('ext', [''])[0], qs.get('name', [''])[0] or qs.get('filename', [''])[0])
        elif path == '/api/ai/config':
            self._api_ai_config()
        elif path == '/api/ai/models':
            self._api_ai_models()
        elif path == '/api/ai/chat':
            self._api_ai_chat()
        elif path == '/api/image/save':
            self._api_image_save()
        elif path == '/api/ai/prompts':
            self._api_ai_prompts()
        elif path == '/api/ai/history':
            self._api_ai_history()
        elif path == '/api/share/start':
            self._send_json(200, start_lan_server())
        elif path == '/api/share/stop':
            self._send_json(200, stop_lan_server())
        elif path == '/api/share/status':
            self._send_json(200, share_status())
        elif path == '/api/update/check':
            self._api_update_check()
        elif path == '/api/update/download':
            self._api_update_download()
        elif path == '/api/update/status':
            self._api_update_status()
        elif path == '/api/update/cancel':
            self._api_update_cancel()
        elif path == '/api/update/apply':
            self._api_update_apply()
        elif path == '/api/system/language':
            self._api_system_language()
        elif path == '/api/autostart/get':
            self._send_json(200, {'ok': True, 'enabled': Api().get_autostart()})
        elif path == '/api/autostart/set':
            n = int(self.headers.get('Content-Length', 0) or 0)
            try:
                body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            except Exception:
                body = {}
            self._send_json(200, Api().set_autostart(bool(body.get('enabled'))))
        elif path == '/api/code/run':
            self._api_code_run()
        elif path == '/api/diagram/render':
            self._api_diagram_render()
        elif path == '/api/import/process':
            self._api_import_process()
        elif path == '/api/export/epub':
            self._api_export_epub()
        elif path == '/api/export/presentation':
            self._api_export_presentation()
        elif path == '/api/style/get':
            self._api_style_get()
        elif path == '/api/style/save':
            self._api_style_save()
        elif path == '/api/bibtex':
            self._api_bibtex(qs)
        elif path == '/api/ping':
            self._send_json(200, {'ok': self._api_ping(qs)})
        elif path == '/api/control/open':
            self._api_control_open()
        elif path == '/api/control/next':
            act = pop_control()
            self._send_json(200, {'pending': act is not None, 'file': act or ''})
        elif path == '/raw':
            p = unquote(qs.get('p', [''])[0])
            self._send_raw(p)
        else:
            self._send(404, 'text/plain; charset=utf-8', b'not found')

    def _send(self, code, ctype, body, cache_control='no-cache'):
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', cache_control)
        self.end_headers()
        self.wfile.write(body)

    def _send_json(self, code, obj):
        self._send(code, 'application/json; charset=utf-8',
                   json.dumps(obj, ensure_ascii=False).encode('utf-8'))

    def _module_ready(self, name, message):
        """Ensure exactly one feature module is being loaded for this request."""
        if RM.is_ready(name):
            return True
        state = RM.load(name)
        st, errors = RM.status()
        state = st.get(name, state)
        if state in ('disabled', 'error'):
            self._send_json(503, {'error': errors.get(name) or message,
                                  'module': name, 'status': state})
        else:
            # ``load`` turns an old error into a retrying loading state.
            self._send_json(409, {'error': message, 'module': name,
                                  'status': st.get(name, state)})
        return False

    def _api_modules_load(self):
        if self.command != 'POST':
            self._send_json(405, {'error': '仅支持 POST 请求'})
            return
        try:
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length else {}
        except Exception:
            self._send_json(400, {'error': '请求格式错误'})
            return
        name = body.get('name') if isinstance(body, dict) else None
        if name not in RM.MODULES:
            self._send_json(400, {'error': '不支持的模块', 'name': name})
            return
        state = RM.load(name)
        statuses, errors = RM.status()
        state = statuses.get(name, state)
        code = 200 if state == 'ready' else (503 if state in ('disabled', 'error') else 202)
        self._send_json(code, {'name': name, 'status': state,
                               'error': errors.get(name, '')})

    def _send_index(self):
        """返回首页；局域网模式下注入 token 供前端 fetch 携带。"""
        fp = os.path.join(APP_DIR, 'assets', 'index.html')
        if not os.path.isfile(fp):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        with open(fp, 'rb') as f:
            data = f.read()
        if self.LAN_TOKEN:
            data = data.replace(b'window.LAN_TOKEN=null;',
                                ('window.LAN_TOKEN="%s";' % self.LAN_TOKEN).encode('utf-8'))
        app_token = self.server.app_token
        if app_token:
            data = data.replace(b'<meta name="readmd-app-token" content="">',
                                ('<meta name="readmd-app-token" content="%s">' % app_token).encode('utf-8'))
        self._send(200, 'text/html; charset=utf-8', data, 'no-store')

    def _sse(self, obj):
        try:
            self.wfile.write(('data: ' + json.dumps(obj, ensure_ascii=False) + '\n\n').encode('utf-8'))
            self.wfile.flush()
        except Exception:
            pass

    def _api_update_check(self):
        try:
            from src.readmd_modules import updater
            res = updater.check_update(VERSION)
            self._send_json(200 if res.get('ok') else 500, res)
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_update_download(self):
        try:
            from src.readmd_modules import updater
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length > 0 else {}
            download_url = body.get('download_url', '')
            target_filename = body.get('target_filename', '')
            expected_sha = body.get('expected_sha', None)
            use_mirror = bool(body.get('use_mirror', False))
            if not download_url or not target_filename:
                self._send_json(400, {'ok': False, 'error': '缺少下载参数'})
                return
            ok, msg = updater.start_download_update(download_url, target_filename, expected_sha, use_mirror)
            self._send_json(200 if ok else 400, {'ok': ok, 'message': msg})
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_update_status(self):
        try:
            from src.readmd_modules import updater
            self._send_json(200, updater.get_download_status())
        except Exception as e:
            self._send_json(500, {'status': 'error', 'error': str(e)})

    def _api_update_cancel(self):
        try:
            from src.readmd_modules import updater
            self._send_json(200, {'ok': updater.cancel_download()})
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_update_apply(self):
        try:
            from src.readmd_modules import updater
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length > 0 else {}
            ok, msg = updater.apply_update(body.get('file_path'), body.get('flavor'))
            self._send_json(200 if ok else 400, {'ok': ok, 'message': msg})
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_system_language(self):
        try:
            self._send_json(200, {'ok': True, 'language': get_system_language()})
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_code_run(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            lang = body.get('lang', 'python')
            code = body.get('code', '')
            cwd = body.get('cwd') or None
            timeout = int(body.get('timeout', 10))
            from src.readmd_modules import code_chunk_runner
            res = code_chunk_runner.execute_code_chunk(lang, code, cwd=cwd, timeout=timeout)
            self._send_json(200, res)
        except Exception as e:
            logging.exception('api_code_run failed')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_diagram_render(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            engine = body.get('engine', 'mermaid')
            code = body.get('code', '')
            from src.readmd_modules import diagrams
            if engine in ('puml', 'plantuml'):
                svg_url = diagrams.get_plantuml_svg_url(code)
                self._send_json(200, {'ok': True, 'type': 'url', 'svg_url': svg_url})
            elif engine == 'tikz':
                html_out = diagrams.format_tikz_html(code)
                self._send_json(200, {'ok': True, 'type': 'html', 'html': html_out})
            else:
                self._send_json(200, {'ok': True, 'engine': engine, 'code': code})
        except Exception as e:
            logging.exception('api_diagram_render failed')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_import_process(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            content = body.get('content', '')
            base_dir = body.get('base_dir', '')
            current_file = body.get('current_file', '')
            from src.readmd_modules import import_processor
            processed = import_processor.process_markdown_imports(content, base_dir=base_dir, current_file=current_file)
            self._send_json(200, {'ok': True, 'content': processed})
        except Exception as e:
            logging.exception('api_import_process failed')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_export_epub(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            content = body.get('content', '')
            out_path = body.get('out_path', '')
            meta = body.get('meta') or {}
            from src.readmd_modules.mdexport import epub_render
            if not out_path:
                out_path = os.path.join(tempfile.gettempdir(), f'readmd_export_{int(time.time()*1000)}.epub')
            ok = epub_render.build_epub(content, out_path, meta=meta)
            self._send_json(200, {'ok': ok, 'path': out_path})
        except Exception as e:
            logging.exception('api_export_epub failed')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_export_presentation(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            content = body.get('content', '')
            theme = body.get('theme', 'black')
            transition = body.get('transition', 'slide')
            from src.readmd_modules.mdexport import presentation_render
            html_out = presentation_render.generate_presentation_html(content, theme=theme, transition=transition)
            self._send_json(200, {'ok': True, 'html': html_out})
        except Exception as e:
            logging.exception('api_export_presentation failed')
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_style_get(self):
        try:
            from src.readmd_core import style_injector
            data = style_injector.get_custom_styles()
            self._send_json(200, {'ok': True, 'data': data})
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_style_save(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            from src.readmd_core import style_injector
            ok = style_injector.save_custom_styles(body.get('css', ''), body.get('head', ''))
            self._send_json(200, {'ok': ok})
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_bibtex(self, qs):
        try:
            from src.readmd_modules import bibtex
            p = unquote(qs.get('p', [''])[0])
            res = bibtex.find_and_load_bib_for_file(p)
            self._send_json(200, {'ok': True, 'citations': res})
        except Exception as e:
            self._send_json(500, {'ok': False, 'error': str(e)})

    def _api_ping(self, qs):


        t = qs.get('t', [''])[0]
        return bool(t) and t == _read_instance().get('token', '')


    def _api_control_open(self):
        n = int(self.headers.get('Content-Length', 0) or 0)
        try:
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
        except Exception:
            self._send_json(400, {'error': '无效请求'})
            return
        if body.get('token') != _read_instance().get('token', ''):
            self._send_json(403, {'error': 'forbidden'})
            return
        path = body.get('file') or ''
        if path and not os.path.isfile(path):
            self._send_json(404, {'error': '文件不存在'})
            return
        push_control(path)
        self._send_json(200, {'ok': True})

    def _api_ai_config(self):
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        try:
            mod = RM.get('ai')
            if self.command == 'GET':
                self._send_json(200, mod.get_config())
            else:
                n = int(self.headers.get('Content-Length', 0) or 0)
                body = json.loads(self.rfile.read(n).decode('utf-8'))
                mod.save_config(body)
                self._send_json(200, {'ok': True})
        except Exception as e:
            logging.exception('ai config failed')
            self._send_json(500, {'error': 'AI 配置失败：%s' % e})

    def _api_ai_models(self):
        """拉取模型列表；保存过的 Key 只在服务端解析，不回传给浏览器。"""
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        try:
            u = urlparse(self.path)
            q = parse_qs(u.query)
            mod = RM.get('ai')
            provider = mod.find_provider(q.get('provider', [''])[0]) or {}
            key = q.get('key', [''])[0] or mod.resolve_key(provider)
            ids = mod.list_models(q.get('base_url', [''])[0] or None,
                                  key,
                                  q.get('mode', ['auto'])[0])
            self._send_json(200, {'models': ids})
        except Exception as e:
            logging.exception('ai models failed')
            self._send_json(500, {'error': str(e)})

    def _api_ai_chat(self):
        """AI 对话：SSE 流式返回，兼容 OpenAI / Anthropic 双协议。"""
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            payload = json.loads(self.rfile.read(n).decode('utf-8'))
        except Exception:
            self._send_json(400, {'error': '请求格式错误'})
            return
        try:
            mod = RM.get('ai')
            gen = mod.chat(payload)
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream; charset=utf-8')
            self.send_header('Cache-Control', 'no-cache')
            self.send_header('Connection', 'close')
            self.end_headers()
            if isinstance(gen, str):
                self._sse({'d': gen})
                self._sse({'done': True})
                return
            for item in gen:
                if isinstance(item, dict):
                    self._sse(item)
                else:
                    self._sse({'d': item})
            self._sse({'done': True})
        except Exception as e:
            logging.exception('ai chat failed')
            try:
                self._sse({'error': str(e)})
                self._sse({'done': True})
            except Exception:
                pass

    def _api_image_save(self):
        """保存编辑后的图片到文档目录 images/ 子目录，返回相对路径。"""
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
        except Exception:
            self._send_json(400, {'error': '无效请求'})
            return
        dir_path = body.get('dir') or ''
        data_b64 = body.get('data') or ''
        fmt = (body.get('format') or 'png').lower()
        name = body.get('name') or ''
        if not dir_path or not data_b64 or not os.path.isdir(dir_path):
            self._send_json(400, {'error': '缺少目录或图片数据'})
            return
        if fmt not in ('png', 'jpeg', 'jpg', 'webp'):
            fmt = 'png'
        try:
            import base64 as _b64
            raw = _b64.b64decode(data_b64)
            if not raw:
                self._send_json(400, {'error': '图片数据为空'})
                return
            img_dir = os.path.join(dir_path, 'images')
            os.makedirs(img_dir, exist_ok=True)
            if not name or not re.match(r'^[A-Za-z0-9_\-]+', name):
                name = 'img_%d_%s' % (int(time.time() * 1000), os.urandom(3).hex())
            if not name.lower().endswith('.' + fmt):
                name += '.' + fmt
            target = os.path.join(img_dir, name)
            with open(target, 'wb') as f:
                f.write(raw)
            rel = os.path.join('images', name).replace('\\', '/')
            self._send_json(200, {'ok': True, 'path': target, 'rel': rel})
        except Exception as e:
            logging.exception('image save failed')
            self._send_json(500, {'error': '图片保存失败：%s' % e})

    def _api_ai_prompts(self):
        """Prompt 模板：GET 列表，POST 保存/覆盖/删除。"""
        try:
            if self.command == 'GET':
                self._send_json(200, load_prompts())
                return
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
            action = body.get('action', 'save')
            if action == 'delete':
                self._send_json(200, {'ok': delete_prompt(body.get('id') or '')})
            else:
                t = save_prompt(body.get('template') or {})
                self._send_json(200, {'ok': True, 'template': t})
        except Exception as e:
            logging.exception('ai prompts failed')
            self._send_json(500, {'error': '模板操作失败：%s' % e})

    def _api_ai_history(self):
        """AI 会话：GET 列表/详情，POST 保存/删除/清空。"""
        try:
            if self.command == 'GET':
                u = urlparse(self.path)
                qs = parse_qs(u.query)
                sid = qs.get('id', [''])[0]
                if sid:
                    for s in load_history(500):
                        if s.get('id') == sid:
                            self._send_json(200, {'session': s})
                            return
                    self._send_json(404, {'error': '会话不存在'})
                    return
                brief = [{k: s.get(k) for k in ('id', 'title', 'created', 'updated',
                                                'provider', 'model', 'doc', 'msgCount')}
                         for s in load_history()]
                self._send_json(200, {'sessions': brief})
                return
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
            action = body.get('action', 'save')
            if action == 'delete':
                self._send_json(200, {'ok': delete_session(body.get('id') or '')})
            elif action == 'clear':
                save_json(HISTORY_FILE, {'sessions': []})
                self._send_json(200, {'ok': True})
            else:
                sess = save_session(body.get('session') or {})
                self._send_json(200, {'ok': True, 'session': sess})
        except Exception as e:
            logging.exception('ai history failed')
            self._send_json(500, {'error': '会话操作失败：%s' % e})
    def _send_file(self, fp, ctype, immutable=False):
        if not os.path.isfile(fp):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        with open(fp, 'rb') as f:
            self._send(200, ctype, f.read(),
                       'public, max-age=31536000, immutable' if immutable else 'no-cache')

    def _api_file(self, p, meta_only):
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        self.server.authorized_save_paths.add(os.path.realpath(p))
        try:
            st = os.stat(p)
        except OSError:
            self._send_json(404, {'error': '无法访问文件'})
            return
        name = os.path.basename(p)
        d = {
            'path': p, 'name': name, 'dir': os.path.dirname(p),
            'mtime': st.st_mtime, 'size': st.st_size,
        }
        if meta_only:
            self._send_json(200, d)
            return
        milestone('boot', 'first_document')
        text, enc = read_text(p)
        raw = text
        structured = False
        if name.lower().endswith('.txt'):
            import src.readmd_modules.txtmd as txtmd
            md, tstats = txtmd.to_markdown(text)
            if tstats.get('changed'):
                text = md
                structured = True
        fr = readmd_fix.fix_markdown(text)
        d.update({
            'encoding': enc,
            'content': fr.text,
            'original': raw,
            'fixes': fr.fixes,
            'stats': fr.stats,
            'structured': structured,
        })
        self._send_json(200, d)

    def _api_list(self, p):
        """递归列出目录下的 Markdown 文件（最多 4 层 / 500 个）。"""
        if not os.path.isdir(p):
            self._send_json(200, {'dir': p, 'files': []})
            return
        files = []
        for root, dirs, names in os.walk(p):
            dirs[:] = [x for x in dirs if not x.startswith(('.', '_'))]
            depth = root[len(p):].count(os.sep)
            if depth >= 4:
                dirs[:] = []
                continue
            for n in sorted(names):
                if n.lower().endswith(MD_EXTS):
                    files.append(os.path.join(root, n))
            if len(files) >= 500:
                break
        self._send_json(200, {'dir': p, 'files': files[:500]})

    def _api_convert_collect(self):
        """收集目录下可转换文件（递归 ≤4 层，≤200 个，不含 .md）。"""
        u = urlparse(self.path)
        q = parse_qs(u.query)
        p = q.get('dir', [''])[0]
        if not os.path.isdir(p):
            self._send_json(200, {'dir': p, 'files': []})
            return
        files = []
        for root, dirs, names in os.walk(p):
            dirs[:] = [x for x in dirs if not x.startswith(('.', '_'))]
            depth = root[len(p):].count(os.sep)
            if depth >= 4:
                dirs[:] = []
                continue
            for n in sorted(names):
                ext = os.path.splitext(n)[1].lower()
                if ext in (CONVERT_EXTS if not is_win7() else WIN7_CONVERT_EXTS):
                    files.append(os.path.join(root, n))
            if len(files) >= 200:
                break
        self._send_json(200, {'dir': p, 'files': files[:200]})

    def _api_convert(self, p):
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        if is_win7() and os.path.splitext(p)[1].lower() not in WIN7_CONVERT_EXTS:
            self._send_json(415, {'error': WIN7_UNAVAILABLE})
            return
        if os.path.splitext(p)[1].lower() == '.txt':
            self._convert_txt(p)
            return
        if not self._module_ready('convert', '转换模块加载中，请稍候再试'):
            return
        try:
            mod = RM.get('convert')
            text, engine, err = mod.convert_verbose(p)
            if err and not text:
                self._send_json(500, {'error': '转换失败：%s' % err})
                return
            if not text.strip():
                self._send_json(200, {'content': '', 'name': os.path.basename(p),
                                      'dir': os.path.dirname(p), 'source': 'convert',
                                      'engine': engine,
                                      'note': '未提取到文字，可尝试“扫描转 MD”（OCR）'})
                return
            import src.readmd_modules.mdcheck as MDC
            fixed, warns = MDC.check(text, os.path.dirname(os.path.abspath(p)))
            fixes = [w['msg'] for w in warns if w.get('level') == 'auto']
            out = _md_output_path(p)
            overwrite = parse_qs(urlparse(self.path).query).get('overwrite', ['0'])[0] == '1'
            saved, skipped = False, False
            if os.path.exists(out) and not overwrite:
                skipped = True
            else:
                try:
                    _write_md(out, fixed)
                    saved = True
                except Exception as e:  # noqa: BLE001
                    logging.exception('convert autosave failed')
            self._send_json(200, {'content': fixed, 'fixes': fixes,
                                  'name': os.path.basename(p),
                                  'dir': os.path.dirname(p), 'source': 'convert', 'path': p,
                                  'engine': engine, 'out': out, 'saved': saved,
                                  'skipped': skipped, 'warns': warns})
        except Exception as e:
            logging.exception('convert failed: %s', p)
            self._send_json(500, {'error': '转换失败：%s' % e})

    def _convert_txt(self, p):
        """TXT 智能转换（纯 Python，不依赖 convert 模块）。"""
        import src.readmd_modules.txtmd as txtmd
        import src.readmd_modules.mdcheck as MDC
        try:
            text, enc = txtmd.read_text(p)
            md, tstats = txtmd.to_markdown(text)
            if not md.strip():
                self._send_json(200, {'content': '', 'name': os.path.basename(p),
                                      'dir': os.path.dirname(p), 'source': 'convert',
                                      'engine': 'txt 智能识别',
                                      'note': '文件为空，没有可转换的内容'})
                return
            fixed, warns = MDC.check(md, os.path.dirname(os.path.abspath(p)))
            fixes = [w['msg'] for w in warns if w.get('level') == 'auto']
            out = _md_output_path(p)
            overwrite = parse_qs(urlparse(self.path).query).get('overwrite', ['0'])[0] == '1'
            saved, skipped = False, False
            if os.path.exists(out) and not overwrite:
                skipped = True
            else:
                try:
                    _write_md(out, fixed)
                    saved = True
                except Exception as e:  # noqa: BLE001
                    logging.exception('convert txt autosave failed')
            self._send_json(200, {'content': fixed, 'fixes': fixes,
                                  'name': os.path.basename(p),
                                  'dir': os.path.dirname(p), 'source': 'convert', 'path': p,
                                  'engine': 'txt 智能识别' if tstats.get('changed') else 'TXT',
                                  'out': out, 'saved': saved,
                                  'skipped': skipped, 'warns': warns})
        except Exception as e:
            logging.exception('convert txt failed: %s', p)
            self._send_json(500, {'error': '转换失败：%s' % e})

    def _api_convert_batch(self):
        n = int(self.headers.get('Content-Length', 0) or 0)
        try:
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
        except Exception:
            self._send_json(400, {'error': '请求格式错误'})
            return
        paths = [p for p in (body.get('paths') or [])
                 if isinstance(p, str) and os.path.isfile(p)]
        if is_win7():
            paths = [p for p in paths if os.path.splitext(p)[1].lower() in WIN7_CONVERT_EXTS]
        if not paths:
            self._send_json(400, {'error': '没有可转换的文件'})
            return
        if not self._module_ready('convert', '转换模块加载中，请稍候再试'):
            return
        try:
            jid = _start_convert_job(paths, bool(body.get('overwrite')))
            self._send_json(200, {'job': jid, 'total': len(paths)})
        except Exception as e:
            logging.exception('convert batch start failed')
            self._send_json(500, {'error': '批量转换启动失败：%s' % e})

    def _api_convert_progress(self, jid):
        job = _CONVERT_JOBS.get(jid or '')
        if not job:
            self._send_json(404, {'error': '任务不存在'})
            return
        self._send_json(200, {
            'job': jid, 'running': job.get('running', False),
            'finished': job.get('finished', False),
            'done': sum(1 for it in job['items'] if it.get('done')),
            'total': len(job['items']),
            'items': job['items'],
        })

    def _api_ocr(self, p):
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        if not self._module_ready('ocr', 'OCR 模块加载中，请稍候再试'):
            return
        try:
            mod = RM.get('ocr')
            text = mod.ocr_any(p)
            fr = readmd_fix.fix_markdown(text or '')
            self._send_json(200, {'content': fr.text, 'fixes': fr.fixes,
                                  'name': os.path.basename(p),
                                  'dir': os.path.dirname(p), 'source': 'ocr', 'path': p})
        except Exception as e:
            logging.exception('ocr failed: %s', p)
            self._send_json(500, {'error': 'OCR 失败：%s' % e})

    def _api_url(self, u, crawl):
        if not u:
            self._send_json(400, {'error': '缺少 URL'})
            return
        if not self._module_ready('web', '网页模块加载中，请稍候再试'):
            return
        try:
            mod = RM.get('web')
            text = mod.crawl(u) if crawl else mod.fetch_url(u)
            if not text:
                self._send_json(200, {'content': '', 'name': u, 'dir': '',
                                      'source': 'url', 'note': '未能从该网页提取到正文'})
                return
            fr = readmd_fix.fix_markdown(text)
            self._send_json(200, {'content': fr.text, 'fixes': fr.fixes,
                                  'name': u, 'dir': '', 'source': 'url', 'path': u})
        except Exception as e:
            logging.exception('url convert failed: %s', u)
            self._send_json(500, {'error': '抓取失败：%s' % e})

    def _api_web_extract(self):
        """v2.2.4 webpage extractor; accepts downloaded or WebView HTML."""
        if not RM.is_ready('web'):
            state = RM.load('web')
            st, errors = RM.status()
            state = st.get('web', state)
            self._send_json(503 if state in ('disabled', 'error') else 409,
                            {'ok': False, 'code': 'module_loading', 'module': 'web',
                             'status': st.get('web', state),
                             'error': errors.get('web') or '网页模块加载中，请稍候再试'})
            return
        try:
            length = int(self.headers.get('Content-Length', 0) or 0)
            if length <= 0:
                self._send_json(400, {'ok': False, 'code': 'invalid_request',
                                      'error': '请求内容为空'})
                return
            if length > 50 * 1024 * 1024:
                self._send_json(413, {'ok': False, 'code': 'too_large',
                                      'error': '渲染后的网页超过 50 MB 限制'})
                return
            body = json.loads(self.rfile.read(length).decode('utf-8'))
        except Exception:
            self._send_json(400, {'ok': False, 'code': 'invalid_request',
                                  'error': '请求格式错误'})
            return
        task_id = str(body.get('task_id') or '')
        try:
            mod = RM.get('web')
            url = body.get('url') or ''
            mode = body.get('mode') if body.get('mode') in ('smart', 'full') else 'smart'
            rendered_html = body.get('html')
            if rendered_html is not None:
                result = mod.extract_html(
                    body.get('final_url') or url, rendered_html, mode=mode,
                    defuddle=body.get('defuddle') or None,
                    readability=body.get('readability') or None, rendered=True)
            else:
                result = mod.fetch_document(url, mode=mode, task_id=task_id)
            previous = body.get('diagnostics')
            if isinstance(previous, dict):
                prior_chain = previous.get('engine_chain')
                if isinstance(prior_chain, list):
                    result['engine_chain'] = prior_chain[:12] + list(
                        result.get('engine_chain') or [])
                try:
                    result['attempts'] = min(99, max(0, int(previous.get('attempts') or 0)) +
                                             int(result.get('attempts') or 0))
                except (TypeError, ValueError):
                    pass
                if previous.get('fallback_reason'):
                    result['fallback_reason'] = str(previous['fallback_reason'])[:80]
            if result.get('ok') and body.get('download_images'):
                asset_dir = os.path.join(DATA_DIR, 'web-assets',
                                         task_id or secrets.token_hex(8))
                content, assets, image_warnings = mod.localize_images(
                    result.get('content') or '', asset_dir, task_id=task_id)
                result['content'] = content
                result['assets'] = assets
                result.setdefault('warnings', []).extend(image_warnings)
                result['asset_dir'] = asset_dir if assets else ''
            self._send_json(200, result)
        except Exception as exc:
            try:
                from src.readmd_modules.web import WebError
            except Exception:
                WebError = ()
            if WebError and isinstance(exc, WebError):
                renderable = {
                    'timeout', 'tls_failed', 'proxy_failed', 'network_failed',
                    'forbidden', 'rate_limited', 'not_html', 'empty_response',
                    'http_error', 'login_required', 'redirect_failed',
                }
                if exc.code in renderable:
                    payload = exc.as_dict()
                    payload.update({
                        'render_required': True,
                        'fallback_reason': exc.code,
                        'engine_chain': ['http'],
                        'attempts': 1,
                    })
                    self._send_json(200, payload)
                else:
                    self._send_json(exc.http_status, exc.as_dict())
            else:
                logging.exception('web extraction failed: %s', body.get('url'))
                self._send_json(500, {'ok': False, 'code': 'internal_error',
                                      'error': '网页转换失败：%s' % exc})

    def _api_web_cancel(self):
        if not self._module_ready('web', '网页模块加载中，请稍候再试'):
            return
        try:
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length else {}
            mod = RM.get('web')
            mod.cancel(body.get('task_id') or '')
            self._send_json(200, {'ok': True})
        except Exception as exc:
            self._send_json(500, {'ok': False, 'error': str(exc)})


    def _do_upload(self, ext, name=''):
        """浏览器兜底模式：接收文件字节写入临时目录，返回可转换的路径。"""
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            data = self.rfile.read(n)
            if not data:
                self._send_json(400, {'error': '空文件'})
                return
            upload_dir = os.path.join(DATA_DIR, 'uploads')
            os.makedirs(upload_dir, exist_ok=True)
            if name:
                import re
                safe_name = re.sub(r'[\\/*?:"<>|]', '_', name).strip()
                if not safe_name:
                    safe_name = 'document' + (ext if ext.startswith('.') else ('.' + ext if ext else '.bin'))
                target = os.path.join(upload_dir, safe_name)
            else:
                import uuid
                safe_name = uuid.uuid4().hex + (ext if ext and ext.startswith('.') else ('.' + ext if ext else '.bin'))
                target = os.path.join(upload_dir, safe_name)
            with open(target, 'wb') as f:
                f.write(data)
            self._send_json(200, {'path': target})
        except Exception as e:
            logging.exception('upload failed')
            self._send_json(500, {'error': '上传失败：%s' % e})

    def _do_save(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
        except Exception:
            self._send_json(400, {'error': '无效请求'})
            return
        path = body.get('path') or ''
        content = body.get('content') or ''
        enc = body.get('encoding') or 'utf-8'
        expected_mtime = body.get('expected_mtime')
        if not path:
            self._send_json(400, {'error': '缺少文件路径'})
            return
        safe_path = os.path.realpath(os.path.normpath(path))
        if (safe_path not in self.server.authorized_save_paths
                or os.path.splitext(safe_path)[1].lower() not in SAVE_EXTENSIONS):
            self._send_json(403, {'error': '文件未被授权保存'})
            return
        result = save_text_atomic(safe_path, content, enc, expected_mtime=expected_mtime)
        self._send_json(
            200 if result.get('ok') else (409 if result.get('conflict') else 500),
            result,
        )

    def _send_raw(self, p):
        if not os.path.isfile(p):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        mime = mimetypes.guess_type(p)[0] or 'application/octet-stream'
        try:
            with open(p, 'rb') as f:
                body = f.read()
        except OSError:
            self._send(500, 'text/plain; charset=utf-8', b'read error')
            return
        self.send_response(200)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-cache')
        self.end_headers()
        self.wfile.write(body)


LAN = {'server': None, 'token': None}


def _is_private(ip):
    parts = ip.split('.')
    if len(parts) != 4:
        return False
    try:
        a = int(parts[0]); b = int(parts[1])
    except ValueError:
        return False
    return (a == 10) or (a == 172 and 16 <= b <= 31) or (a == 192 and b == 168)


def get_lan_ip():
    """获取本机局域网 IP：优先 RFC1918 私网地址（避免取到 VPN/代理网段）。"""
    ips = []
    for target in ('223.5.5.5', '114.114.114.114', '1.1.1.1', '8.8.8.8'):
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            try:
                sock.connect((target, 80))
                ip = sock.getsockname()[0]
                if ip and ip not in ips:
                    ips.append(ip)
            finally:
                sock.close()
        except Exception:
            continue
    for ip in ips:
        if _is_private(ip):
            return ip
    if ips:
        return ips[0]
    try:
        return socket.gethostbyname(socket.gethostname())
    except Exception:
        return '127.0.0.1'


def share_status():
    srv = LAN.get('server')
    if srv is None:
        return {'running': False}
    return {'running': True, 'port': srv.server_port, 'token': LAN.get('token'),
            'url': 'http://%s:%d/' % (get_lan_ip(), srv.server_port)}


def start_lan_server():
    """启动局域网共享服务器（带随机 token 鉴权），供手机等设备访问。"""
    if LAN['server'] is not None:
        return share_status()
    token = secrets.token_urlsafe(12)

    class LanHandler(Handler):
        LAN_TOKEN = token

    try:
        srv = ReadMDHTTPServer(('0.0.0.0', 0), LanHandler)
    except OSError as e:
        return {'ok': False, 'error': '无法监听局域网：%s' % e}
    threading.Thread(target=srv.serve_forever, daemon=True, name='readmd-lan').start()
    LAN['server'] = srv
    LAN['token'] = token
    d = share_status()
    d['ok'] = True
    logging.info('LAN share started: %s', d.get('url'))
    return d


def stop_lan_server():
    srv = LAN.get('server')
    if srv is None:
        return {'ok': True, 'running': False}
    try:
        srv.shutdown()
    except Exception:
        pass
    try:
        srv.server_close()
    except Exception:
        pass
    LAN['server'] = None
    LAN['token'] = None
    logging.info('LAN share stopped')
    return {'ok': True, 'running': False}


def start_server(port=0):
    """启动本地 HTTP 服务。

    默认绑定固定控制端口（CONTROL_PORT）以支持单实例常驻；
    端口被其他程序占用时回退随机端口并禁用单实例。
    """
    if not port:
        port = CONTROL_PORT
    try:
        server = ReadMDHTTPServer(('127.0.0.1', port), Handler)
    except OSError:
        try:
            server = ReadMDHTTPServer(('127.0.0.1', 0), Handler)
        except OSError:
            raise
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    return server


# ---------------------------------------------------------------- JS 桥接 API

class Api(object):
    """暴露给前端 window.pywebview.api 的方法（浏览器模式下不可用）。"""

    def __init__(self):
        self._window = None
        self._page_ready = False
        self._ready_lock = threading.Lock()
        self._on_page_ready = None
        self._web_render_lock = threading.Lock()
        self._web_private_lock = threading.Lock()
        self._web_private_grants = {}
        self._clipboard_lock = threading.Lock()
        self._clipboard_tokens = {}

    @staticmethod
    def _web_origin(url):
        from urllib.parse import urlparse
        from src.readmd_modules import web as web_module
        parsed = urlparse(web_module.normalize_url(url))
        port = parsed.port
        default_port = 443 if parsed.scheme == 'https' else 80
        host = parsed.hostname or ''
        if ':' in host and not host.startswith('['):
            host = '[%s]' % host
        return '%s://%s:%d' % (parsed.scheme, host, port or default_port)

    @staticmethod
    def _web_origin_url_filter(url):
        """Return a WKContentRule URL regex for exactly one origin."""
        from urllib.parse import urlparse
        from src.readmd_modules import web as web_module
        parsed = urlparse(web_module.normalize_url(url))
        host = parsed.hostname or ''
        if ':' in host and not host.startswith('['):
            host = '[%s]' % host
        default_port = 443 if parsed.scheme == 'https' else 80
        port = parsed.port
        base = re.escape('%s://%s' % (parsed.scheme, host))
        if port is None or port == default_port:
            authority = base + r'(?::%d)?' % default_port
        else:
            authority = base + re.escape(':%d' % port)
        return '^' + authority + r'(?:/|$)'

    def authorize_private_web(self, url, task_id):
        """签发仅供桌面 WebView 使用的短期、任务与源站绑定授权。"""
        task_id = str(task_id or '').strip()
        if not task_id:
            return {'ok': False, 'code': 'invalid_task', 'error': '缺少网页转换任务 ID'}
        try:
            origin = self._web_origin(url)
        except Exception as exc:
            return {'ok': False, 'code': getattr(exc, 'code', 'invalid_url'),
                    'error': getattr(exc, 'message', str(exc))}
        grant = secrets.token_urlsafe(24)
        expires_at = time.time() + 600
        with self._web_private_lock:
            self._web_private_grants[task_id] = {
                'grant': grant, 'origin': origin, 'expires_at': expires_at,
            }
        return {'ok': True, 'grant': grant, 'origin': origin,
                'expires_at': int(expires_at)}

    def _private_web_allowed(self, url, task_id, grant):
        task_id, grant = str(task_id or ''), str(grant or '')
        with self._web_private_lock:
            record = self._web_private_grants.get(task_id)
            if not record or time.time() >= record.get('expires_at', 0):
                self._web_private_grants.pop(task_id, None)
                return False
            expected = record.get('grant') or ''
            origin = record.get('origin')
        try:
            return secrets.compare_digest(expected, grant) and self._web_origin(url) == origin
        except Exception:
            return False

    def _web_request_allowed(self, url, task_id='', private_grant=''):
        """Validate every WebView request before the native engine sends it."""
        try:
            mod = RM.get('web') if RM.is_ready('web') else __import__(
                'src.readmd_modules.web', fromlist=['web'])
            if self._private_web_allowed(url, task_id, private_grant):
                mod._validate_public_url(url, allow_private=True)
            else:
                mod._validate_public_url(url, allow_private=False)
            return True
        except Exception:
            return False

    def _install_webview_network_guard(self, reader_window, task_id,
                                       private_grant, allowed_url,
                                       offline=False):
        """Install a fail-closed native request guard before remote navigation."""
        if IS_WIN:
            try:
                from System import Action
                native = reader_window.native
                installed = [False]
                private_origin = (self._web_origin(allowed_url)
                                  if self._private_web_allowed(
                                      allowed_url, task_id, private_grant)
                                  else '')

                def allowed_by_mode(request_url):
                    if offline:
                        return request_url.lower().startswith(
                            ('about:blank', 'data:', 'blob:'))
                    if private_origin:
                        try:
                            return (request_url.lower().startswith(
                                        ('about:blank', 'data:', 'blob:')) or
                                    self._web_origin(request_url) == private_origin)
                        except Exception:
                            return False
                    return self._web_request_allowed(
                        request_url, task_id, private_grant)

                def install():
                    core = native.browser.webview.CoreWebView2

                    def guard(sender, args):
                        request_url = str(args.Request.Uri)
                        if offline:
                            if allowed_by_mode(request_url):
                                return
                            args.Response = sender.Environment.CreateWebResourceResponse(
                                None, 403, 'Blocked by ReadMD',
                                'Content-Type: text/plain; charset=utf-8')
                            return
                        if allowed_by_mode(request_url):
                            return
                        args.Response = sender.Environment.CreateWebResourceResponse(
                            None, 403, 'Blocked by ReadMD',
                            'Content-Type: text/plain; charset=utf-8')

                    core.WebResourceRequested += guard
                    native.browser._readmd_network_guard = guard
                    def navigation_guard(sender, args):
                        request_url = str(args.Uri or '')
                        if allowed_by_mode(request_url):
                            return
                        args.Cancel = True

                    core.NavigationStarting += navigation_guard
                    native.browser._readmd_navigation_guard = navigation_guard
                    installed[0] = True

                native.Invoke(Action(install))
                return installed[0]
            except Exception:
                logging.exception('failed to install WebView2 network guard')
                return False
        if IS_MAC:
            try:
                import WebKit
                from PyObjCTools import AppHelper
                from webview.platforms.cocoa import BrowserView
                finished = threading.Event()
                success = [False]
                if offline:
                    rules = [
                        {'trigger': {'url-filter': '.*'},
                         'action': {'type': 'block'}},
                        {'trigger': {'url-filter': r'^(?:about:blank|data:|blob:)'},
                         'action': {'type': 'ignore-previous-rules'}},
                    ]
                else:
                    allowed_origin_filter = self._web_origin_url_filter(allowed_url)
                    rules = [
                        {'trigger': {'url-filter': '.*'},
                         'action': {'type': 'block'}},
                        {'trigger': {'url-filter': allowed_origin_filter},
                         'action': {'type': 'ignore-previous-rules'}},
                        {'trigger': {'url-filter': r'^(?:about:blank|data:|blob:)'},
                         'action': {'type': 'ignore-previous-rules'}},
                    ]

                def install():
                    instance = BrowserView.get_instance('window', reader_window.native)
                    if instance is None:
                        finished.set()
                        return
                    store = WebKit.WKContentRuleListStore.defaultStore()

                    def compiled(rule_list, error):
                        if rule_list is not None and error is None:
                            instance.webview.configuration().userContentController().addContentRuleList_(rule_list)
                            instance._readmd_content_rule = rule_list
                            success[0] = True
                        finished.set()

                    store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler_(
                        'ReadMDPrivateNetworkGuard', json.dumps(rules), compiled)

                AppHelper.callAfter(install)
                finished.wait(5.0)
                return success[0]
            except Exception:
                logging.exception('failed to install WKWebView network guard')
                return False
        return False

    def revoke_private_web(self, task_id):
        with self._web_private_lock:
            self._web_private_grants.pop(str(task_id or ''), None)
        return True

    def choose_file(self):
        import webview
        if self._window is None:
            return None
        try:
            files = self._window.create_file_dialog(
                webview.OPEN_DIALOG,
                file_types=('Markdown 文件 (*.md;*.markdown;*.mdown;*.mkd;*.mdx;*.txt)',))
            return files[0] if files else None
        except Exception as e:
            logging.exception('choose_file failed')
            return None

    def authorize_clipboard_read(self):
        """Grant one short-lived clipboard read after an explicit UI action."""
        token = secrets.token_urlsafe(18)
        with self._clipboard_lock:
            self._clipboard_tokens[token] = time.time() + 30
        return {'ok': True, 'token': token, 'expires_at': int(time.time() + 30)}

    def read_clipboard(self, token=''):
        """Return clipboard data in a small, platform-neutral bridge shape.

        HTML is best-effort because pywebview backends expose different native
        clipboard APIs.  Callers can always fall back to ``text``.
        """
        with self._clipboard_lock:
            expires_at = self._clipboard_tokens.pop(str(token or ''), 0)
        if not token or time.time() > expires_at:
            return {'text': '', 'html': '', 'source_type': 'unauthorized',
                    'error': '请通过用户操作重新授权读取剪贴板'}
        text, html, image_path, file_list = '', '', '', []
        try:
            if IS_WIN:
                import win32clipboard
                win32clipboard.OpenClipboard()
                try:
                    if win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_HDROP):
                        raw_files = win32clipboard.GetClipboardData(win32clipboard.CF_HDROP)
                        if raw_files:
                            file_list = [f for f in raw_files if os.path.exists(f)]
                    if not file_list and win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_UNICODETEXT):
                        text = win32clipboard.GetClipboardData(win32clipboard.CF_UNICODETEXT) or ''
                    fmt = win32clipboard.RegisterClipboardFormat('HTML Format')
                    if not file_list and win32clipboard.IsClipboardFormatAvailable(fmt):
                        raw = win32clipboard.GetClipboardData(fmt)
                        if isinstance(raw, bytes) and len(raw) <= 10 * 1024 * 1024:
                            html_str = raw.decode('utf-8', errors='replace')
                            start = re.search(r'StartFragment:(\d+)', html_str)
                            end = re.search(r'EndFragment:(\d+)', html_str)
                            if start and end:
                                html = html_str[int(start.group(1)):int(end.group(1))]
                            else:
                                html = html_str
                finally:
                    win32clipboard.CloseClipboard()
            elif IS_MAC:
                try:
                    p = subprocess.run(['pbpaste'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
                    if p.returncode == 0:
                        text = p.stdout.decode('utf-8', errors='replace')
                except Exception:
                    pass
            else:
                try:
                    p = subprocess.run(['wl-paste', '-t', 'text/plain'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
                    if p.returncode == 0:
                        text = p.stdout.decode('utf-8', errors='replace')
                    else:
                        p = subprocess.run(['xclip', '-selection', 'clipboard', '-o'], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=2)
                        if p.returncode == 0:
                            text = p.stdout.decode('utf-8', errors='replace')
                except Exception:
                    pass

            if not text and not html and not file_list:
                try:
                    import tkinter
                    root = tkinter.Tk(); root.withdraw()
                    text = root.clipboard_get() or ''
                    root.destroy()
                except Exception:
                    pass
            if not text and not html and not file_list:
                try:
                    from PIL import ImageGrab, Image
                    img = ImageGrab.grabclipboard()
                    if isinstance(img, Image.Image):
                        tmp_img = os.path.join(tempfile.gettempdir(), 'readmd_clip_%d.png' % int(time.time() * 1000))
                        img.save(tmp_img, 'PNG')
                        image_path = tmp_img
                    elif isinstance(img, list):
                        file_list = [f for f in img if os.path.exists(f)]
                except Exception:
                    pass
        except Exception:
            pass

        if file_list:
            return {'text': '', 'html': '', 'files': file_list, 'source_type': 'files'}
        if image_path and os.path.isfile(image_path):
            return {'text': '', 'html': '', 'image': image_path, 'image_path': image_path, 'source_type': 'image'}
        if html and len(html.encode('utf-8')) <= 10 * 1024 * 1024:
            return {'text': text or '', 'html': html, 'source_type': 'html'}
        if text and len(text.encode('utf-8')) <= 10 * 1024 * 1024:
            return {'text': text, 'html': '', 'source_type': 'text'}
        if not text and not html and not image_path and not file_list:
            return {'text': '', 'html': '', 'source_type': 'empty', 'error': '剪贴板为空或不包含支持的内容'}
        return {'text': '', 'html': '', 'source_type': 'too_large',
                'error': '剪贴板内容超过 10 MB 限制'}




    def choose_folder(self):
        import webview
        if self._window is None:
            return None
        try:
            dirs = self._window.create_file_dialog(webview.FOLDER_DIALOG)
            return dirs[0] if dirs else None
        except Exception:
            return None

    def choose_any_file(self):
        """任意格式文件（用于“万物转 MD”）。Win7 版仅开放 docx / pdf。"""
        import webview
        if self._window is None:
            return None
        try:
            files = self._window.create_file_dialog(
                webview.OPEN_DIALOG,
                file_types=(
                    'Word / PDF (*.docx;*.pdf)' if is_win7() else '所有文件 (*.*)',
                    '文档 (*.docx;*.pdf)' if is_win7() else '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)',
                ) if is_win7() else (
                    '所有文件 (*.*)',
                    '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)',
                    '图片 (*.png;*.jpg;*.jpeg;*.bmp;*.webp;*.tif;*.tiff)',
                ))
            return files[0] if files else None
        except Exception:
            return None

    def choose_many_files(self):
        """批量转换：多选任意格式文件。Win7 版仅开放 docx / pdf。"""
        import webview
        if self._window is None:
            return []
        try:
            files = self._window.create_file_dialog(
                webview.OPEN_DIALOG, allow_multiple=True,
                file_types=(
                    'Word / PDF (*.docx;*.pdf)' if is_win7() else '所有文件 (*.*)',
                    '文档 (*.docx;*.pdf)' if is_win7() else '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)',
                ) if is_win7() else (
                    '所有文件 (*.*)',
                    '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)',
                    '图片 (*.png;*.jpg;*.jpeg;*.bmp;*.webp;*.tif;*.tiff)',
                ))
            return list(files or [])
        except Exception:
            return []

    def open_dir(self, path):
        """在文件管理器中打开目录。"""
        try:
            safe_path = validate_file_path(path)
            if IS_MAC:
                from src.readmd_modules import macos_native
                return macos_native.open_path(safe_path)
            elif IS_WIN:
                cmd = validate_command(['explorer', safe_path])
                subprocess.Popen(cmd)
            else:
                cmd = validate_command(['xdg-open', safe_path])
                subprocess.Popen(cmd)
            return True
        except Exception:
            return False

    def get_autostart(self):
        """检查开机自启动是否已开启。"""
        try:
            if IS_WIN:
                import winreg
                try:
                    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run", 0, winreg.KEY_READ) as key:
                        val, _ = winreg.QueryValueEx(key, "ReadMD")
                        return bool(val)
                except (FileNotFoundError, OSError):
                    return False
            elif IS_LINUX:
                autostart_file = os.path.expanduser('~/.config/autostart/io.github.natsummerance.readmd.desktop')
                return os.path.isfile(autostart_file)
            elif IS_MAC:
                plist_path = os.path.expanduser('~/Library/LaunchAgents/io.github.natsummerance.readmd.plist')
                return os.path.isfile(plist_path)
            return False
        except Exception as e:
            logging.exception('get_autostart failed: %s', e)
            return False

    def set_autostart(self, enabled: bool):
        """设置开机自启动开启或关闭。"""
        try:
            enabled = bool(enabled)
            if IS_WIN:
                import winreg
                with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Run", 0, winreg.KEY_SET_VALUE) as key:
                    if enabled:
                        exe_path = sys.executable
                        if getattr(sys, 'frozen', False):
                            cmd = f'"{exe_path}"'
                        else:
                            main_script = os.path.abspath(sys.argv[0])
                            cmd = f'"{exe_path}" "{main_script}"'
                        winreg.SetValueEx(key, "ReadMD", 0, winreg.REG_SZ, cmd)
                    else:
                        try:
                            winreg.DeleteValue(key, "ReadMD")
                        except (FileNotFoundError, OSError):
                            pass
                return {'ok': True, 'enabled': enabled}
            elif IS_LINUX:
                autostart_dir = os.path.expanduser('~/.config/autostart')
                autostart_file = os.path.join(autostart_dir, 'io.github.natsummerance.readmd.desktop')
                if enabled:
                    os.makedirs(autostart_dir, exist_ok=True)
                    desktop_src = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'scripts', 'linux', 'io.github.natsummerance.readmd.desktop')
                    if os.path.isfile(desktop_src):
                        import shutil
                        shutil.copy(desktop_src, autostart_file)
                    else:
                        with open(autostart_file, 'w', encoding='utf-8') as f:
                            f.write("[Desktop Entry]\nName=ReadMD\nExec=readmd\nType=Application\n")
                else:
                    if os.path.isfile(autostart_file):
                        os.remove(autostart_file)
                return {'ok': True, 'enabled': enabled}
            elif IS_MAC:
                plist_dir = os.path.expanduser('~/Library/LaunchAgents')
                plist_path = os.path.join(plist_dir, 'io.github.natsummerance.readmd.plist')
                if enabled:
                    os.makedirs(plist_dir, exist_ok=True)
                    exe_path = sys.executable
                    plist_content = f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.github.natsummerance.readmd</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_path}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"""
                    with open(plist_path, 'w', encoding='utf-8') as f:
                        f.write(plist_content)
                else:
                    if os.path.isfile(plist_path):
                        os.remove(plist_path)
                return {'ok': True, 'enabled': enabled}
            return {'ok': False, 'error': 'Unsupported platform'}
        except Exception as e:
            logging.exception('set_autostart failed: %s', e)
            return {'ok': False, 'error': str(e)}

    def start_modules(self):
        """Compatibility bridge: module loading is now initiated per feature."""
        return self.get_modules_status()


    def get_modules_status(self):
        st, err = RM.status()
        return {'modules': st, 'errors': err}

    def run_code_chunk(self, lang, code, cwd=None, timeout=10):
        """执行多语言代码块。"""
        try:
            from src.readmd_modules import code_chunk_runner
            return code_chunk_runner.execute_code_chunk(lang, code, cwd=cwd, timeout=int(timeout))
        except Exception as e:
            return {'ok': False, 'error': str(e), 'stdout': '', 'stderr': str(e), 'images': [], 'exit_code': 1}

    def render_diagram(self, engine, code, options=None):
        """渲染专业图表。"""
        try:
            from src.readmd_modules import diagrams
            if engine in ('puml', 'plantuml'):
                return {'ok': True, 'type': 'url', 'svg_url': diagrams.get_plantuml_svg_url(code)}
            elif engine == 'tikz':
                return {'ok': True, 'type': 'html', 'html': diagrams.format_tikz_html(code)}
            return {'ok': True, 'engine': engine, 'code': code}
        except Exception as e:
            return {'ok': False, 'error': str(e)}

    def process_imports(self, content, base_dir='', current_file=None):
        """处理 @import 指令。"""
        try:
            from src.readmd_modules import import_processor
            res = import_processor.process_markdown_imports(content, base_dir=base_dir, current_file=current_file)
            return {'ok': True, 'content': res}
        except Exception as e:
            return {'ok': False, 'error': str(e), 'content': content}

    def export_epub(self, content, output_path='', meta=None):
        """导出 EPUB 3.0 电子书。"""
        try:
            from src.readmd_modules.mdexport import epub_render
            if not output_path:
                output_path = os.path.join(tempfile.gettempdir(), f'readmd_export_{int(time.time()*1000)}.epub')
            ok = epub_render.build_epub(content, output_path, meta=meta or {})
            return {'ok': ok, 'path': output_path}
        except Exception as e:
            return {'ok': False, 'error': str(e)}

    def export_presentation(self, content, theme='black', transition='slide'):
        """生成 Reveal.js 演示文稿 HTML。"""
        try:
            from src.readmd_modules.mdexport import presentation_render
            html_out = presentation_render.generate_presentation_html(content, theme=theme, transition=transition)
            return {'ok': True, 'html': html_out}
        except Exception as e:
            return {'ok': False, 'error': str(e)}

    def get_custom_styles(self):
        """获取自定义样式与 Head。"""
        try:
            from src.readmd_core import style_injector
            return {'ok': True, 'data': style_injector.get_custom_styles()}
        except Exception as e:
            return {'ok': False, 'error': str(e)}

    def save_custom_styles(self, css='', head_html=''):
        """保存自定义样式与 Head。"""
        try:
            from src.readmd_core import style_injector
            ok = style_injector.save_custom_styles(css, head_html)
            return {'ok': ok}
        except Exception as e:
            return {'ok': False, 'error': str(e)}

    def render_web_page(self, url, task_id='', timeout_ms=25000,
                        interactive=False, private_grant='', source_html=''):
        """在无 JS bridge、无持久会话的临时系统 WebView 中渲染网页。"""
        if is_win7():
            return {'ok': False, 'code': 'render_unavailable',
                    'error': 'Win7 版不支持动态网页渲染'}
        allow_private = self._private_web_allowed(url, task_id, private_grant)
        # 仅在非公网 URL 且传入离线 source_html 时启用离线渲染模式；公网 http/https 正常联网渲染
        is_remote_http = url.lower().startswith(('http://', 'https://'))
        offline_render = bool(source_html and not is_remote_http)
        if offline_render and interactive:
            return {'ok': False, 'code': 'interactive_unavailable',
                    'error': '安全模式不允许未授权网页联网交互；可重试静态抓取或保留完整页面'}
        if offline_render and not source_html:
            return {'ok': False, 'code': 'render_source_missing',
                    'error': '安全模式缺少已验证的网页 HTML，无法动态渲染'}
        if not RM.is_ready('web'):
            return {'ok': False, 'code': 'module_loading', 'module': 'web',
                    'status': RM.load('web'), 'error': '网页模块加载中，请稍候再试'}
        try:
            mod = RM.get('web')
            safe_url = mod._validate_public_url(url, allow_private=allow_private)
        except Exception as exc:
            return {'ok': False, 'code': getattr(exc, 'code', 'invalid_url'),
                    'error': getattr(exc, 'message', str(exc))}
        try:
            timeout_ms = max(3000, min(300000 if interactive else 60000,
                                      int(timeout_ms or (300000 if interactive else 25000))))
        except Exception:
            timeout_ms = 300000 if interactive else 25000
        if not self._web_render_lock.acquire(blocking=False):
            return {'ok': False, 'code': 'renderer_busy',
                    'error': '动态网页渲染器正在处理另一个页面'}
        try:
            import webview
            target_init_url = safe_url if not offline_render else 'about:blank'
            reader_window = webview.create_window(
                'ReadMD 网页渲染提取器', target_init_url, hidden=not interactive,
                focus=bool(interactive), width=1100, height=800,
                resizable=bool(interactive), text_select=bool(interactive))
            if reader_window is None:
                return {'ok': False, 'code': 'render_unavailable',
                        'error': '无法创建系统网页渲染器'}
            if not self._install_webview_network_guard(
                    reader_window, task_id, private_grant, safe_url,
                    offline=offline_render):
                return {'ok': False, 'code': 'network_guard_unavailable',
                        'error': '无法启用网页私网访问保护，已停止动态渲染'}
            if offline_render:
                reader_window.load_html(source_html, base_uri=safe_url)
            deadline = time.time() + timeout_ms / 1000.0
            loaded = getattr(getattr(reader_window, 'events', None), 'loaded', None)
            if loaded is not None:
                loaded.wait(max(0.1, min(10.0, deadline - time.time())))
            time.sleep(0.5)
            last_length, last_resources, stable = -1, -1, 0
            while time.time() < deadline:
                if mod.is_cancelled(task_id):
                    return {'ok': False, 'code': 'cancelled', 'error': '已取消网页转换'}
                try:
                    if interactive:
                        reader_window.evaluate_js("""
                          (() => {
                            if (document.getElementById('__readmd_capture_bar')) return;
                            const bar=document.createElement('div');
                            bar.id='__readmd_capture_bar';
                            bar.style='position:fixed;z-index:2147483647;top:0;left:0;right:0;padding:10px 16px;background:#172033;color:white;font:14px system-ui;display:flex;gap:10px;align-items:center;box-shadow:0 2px 12px #0005';
                            bar.innerHTML='<strong style="margin-right:auto">完成登录或验证后，提取当前页面</strong><button id="__readmd_capture" style="min-height:40px;padding:0 16px;border:0;border-radius:8px;background:#3182f6;color:white">提取此页</button><button id="__readmd_abort" style="min-height:40px;padding:0 16px;border:1px solid #ffffff55;border-radius:8px;background:transparent;color:white">取消</button>';
                            document.documentElement.appendChild(bar);
                            document.getElementById('__readmd_capture').onclick=()=>window.__readmdCaptureAction='capture';
                            document.getElementById('__readmd_abort').onclick=()=>window.__readmdCaptureAction='cancel';
                          })()
                        """)
                    state = reader_window.evaluate_js(
                        "({ready:document.readyState,n:(document.body&&document.body.innerText||'').length,r:(performance.getEntriesByType&&performance.getEntriesByType('resource').length)||0,action:window.__readmdCaptureAction||''})") or {}
                    if isinstance(state, dict) and state.get('action') == 'cancel':
                        return {'ok': False, 'code': 'cancelled', 'error': '已取消交互式抓取'}
                    if interactive and isinstance(state, dict) and state.get('action') == 'capture':
                        break
                    length = int(state.get('n') or 0) if isinstance(state, dict) else 0
                    resources = int(state.get('r') or 0) if isinstance(state, dict) else 0
                    if length > 0 and length == last_length and resources == last_resources:
                        stable += 1
                    else:
                        stable = 0
                    last_length = length
                    last_resources = resources
                    if not interactive and state.get('ready') == 'complete' and stable >= 3:
                        break
                except Exception:
                    pass
                time.sleep(0.5)
            if time.time() >= deadline:
                return {'ok': False, 'code': 'render_timeout',
                        'error': '动态渲染超时，请重试或改用完整页面模式'}
            reader_path = os.path.join(APP_DIR, 'assets', 'vendor', 'readability.bundle.js')
            defuddle_path = os.path.join(APP_DIR, 'assets', 'vendor', 'defuddle.bundle.js')
            if not os.path.isfile(reader_path) or not os.path.isfile(defuddle_path):
                return {'ok': False, 'code': 'reader_missing',
                        'error': '网页正文提取器离线资源缺失'}
            with open(defuddle_path, encoding='utf-8') as handle:
                reader_window.evaluate_js(handle.read())
            with open(reader_path, encoding='utf-8') as handle:
                reader_window.evaluate_js(handle.read())
            result = reader_window.evaluate_js("""
                (() => {
                  const bar=document.getElementById('__readmd_capture_bar'); if(bar) bar.remove();
                  let defuddle = null;
                  try {
                    const parsed=window.ReadMDDefuddle.parse(document.cloneNode(true), location.href);
                    defuddle=parsed ? {
                      title:parsed.title||'', author:parsed.author||'',
                      published:parsed.published||parsed.publishedTime||'',
                      site:parsed.site||parsed.siteName||'',
                      contentMarkdown:parsed.contentMarkdown||parsed.markdown||'',
                      content:parsed.content||''
                    } : null;
                  } catch (e) { defuddle = null; }
                  let article = null;
                  try {
                    const clone = document.cloneNode(true);
                    article = new window.ReadMDReadability.Readability(clone, {charThreshold: 40}).parse();
                  } catch (e) { article = null; }
                  const compact = article ? {
                    title: article.title || '', byline: article.byline || '',
                    publishedTime: article.publishedTime || '', siteName: article.siteName || '',
                    excerpt: article.excerpt || '', content: article.content || '',
                    length: article.length || 0, url: location.href
                  } : null;
                  return {ok:true, final_url:location.href,
                          title:document.title || '',
                          html:document.documentElement.outerHTML,
                          defuddle:defuddle,
                          readability:compact};
                })()
            """)
            if not isinstance(result, dict):
                return {'ok': False, 'code': 'render_failed',
                        'error': '动态网页渲染没有返回可用内容'}
            try:
                # NavigateToString/loadHTMLString may expose data:/about: as
                # location.href. Offline HTML is already bound to safe_url.
                final_url = safe_url if offline_render else (
                    result.get('final_url') or safe_url)
                if allow_private and not self._private_web_allowed(
                        final_url, task_id, private_grant):
                    return {'ok': False, 'code': 'private_origin_changed',
                            'error': '内网页面跳转到了未授权的源站，请重新授权'}
                result['final_url'] = mod._validate_public_url(
                    final_url, allow_private=allow_private)
            except Exception as exc:
                return {'ok': False,
                        'code': getattr(exc, 'code', 'blocked_address'),
                        'error': getattr(exc, 'message', str(exc))}
            if len(result.get('html') or '') > 50 * 1024 * 1024:
                return {'ok': False, 'code': 'too_large',
                        'error': '动态渲染后的网页超过 50 MB 限制'}
            return result
        except Exception as exc:
            logging.exception('system WebView extraction failed: %s', safe_url)
            return {'ok': False, 'code': 'render_failed',
                    'error': '系统网页渲染失败：%s' % exc}
        finally:
            if reader_window is not None:
                try:
                    reader_window.clear_cookies()
                except Exception:
                    pass
                try:
                    reader_window.destroy()
                except Exception:
                    pass
            self._web_render_lock.release()

    def cancel_web_render(self, task_id=''):
        try:
            RM.get('web').cancel(task_id)
            return True
        except Exception:
            return False

    def rename_file(self, path, new_stem):
        """在原目录内重命名当前 Markdown 文件并同步本地引用。"""
        old_path = os.path.abspath(os.fspath(path)) if path else ''
        if not old_path or not os.path.isfile(old_path):
            return {'ok': False, 'code': 'not_found', 'error': '文件不存在或已被移动'}
        extension = os.path.splitext(old_path)[1]
        if extension.lower() not in MD_EXTS:
            return {'ok': False, 'code': 'unsupported_type',
                    'error': '只能重命名 Markdown 或文本文件'}
        try:
            stem = _validate_rename_stem(new_stem, extension)
        except ValueError as exc:
            return {'ok': False, 'code': 'invalid_name', 'error': str(exc)}

        new_path = os.path.join(os.path.dirname(old_path), stem + extension)
        if old_path == new_path:
            return {'ok': True, 'path': old_path, 'name': os.path.basename(old_path),
                    'warnings': []}
        same_normalized = _same_file_target(old_path, new_path)
        if os.path.exists(new_path) and not same_normalized:
            return {'ok': False, 'code': 'target_exists',
                    'error': '同目录下已存在同名文件'}

        try:
            if same_normalized:
                temp_path = old_path + '.readmd-rename-' + secrets.token_hex(6)
                os.rename(old_path, temp_path)
                try:
                    os.rename(temp_path, new_path)
                except Exception:
                    os.rename(temp_path, old_path)
                    raise
            else:
                os.rename(old_path, new_path)
        except Exception as exc:
            logging.exception('rename_file failed: %s', old_path)
            return {'ok': False, 'code': 'rename_failed', 'error': str(exc)}

        warnings = []
        old_backup, new_backup = old_path + '.bak', new_path + '.bak'
        if os.path.isfile(old_backup):
            if _same_file_target(old_backup, new_backup) and old_backup != new_backup:
                backup_tmp = old_backup + '.readmd-rename-' + secrets.token_hex(6)
                try:
                    os.rename(old_backup, backup_tmp)
                    try:
                        os.rename(backup_tmp, new_backup)
                    except Exception:
                        os.rename(backup_tmp, old_backup)
                        raise
                except Exception as exc:
                    logging.warning('case-only backup rename failed: %s', exc)
                    warnings.append('文件已重命名，但备份文件大小写未能同步')
            elif os.path.exists(new_backup):
                warnings.append('旧备份未移动：目标备份已存在')
            else:
                try:
                    os.rename(old_backup, new_backup)
                except Exception as exc:
                    logging.warning('rename backup failed: %s', exc)
                    warnings.append('文件已重命名，但旧备份未能同步移动')

        try:
            recent = load_json(RECENT_FILE, [])
            updated = []
            for item in recent if isinstance(recent, list) else []:
                value = new_path if _paths_equal(item, old_path) else item
                if not any(_paths_equal(value, existing) for existing in updated):
                    updated.append(value)
            if not save_json(RECENT_FILE, updated):
                warnings.append('最近文件记录未能同步')

            settings = load_json(SETTINGS_FILE, {})
            if isinstance(settings, dict) and settings.get('last') and _paths_equal(settings['last'], old_path):
                settings['last'] = new_path
                if not save_json(SETTINGS_FILE, settings):
                    warnings.append('上次打开记录未能同步')

            history = load_json(HISTORY_FILE, {'sessions': []})
            changed = False
            if isinstance(history, dict):
                for session in history.get('sessions', []):
                    if session.get('doc') and _paths_equal(session['doc'], old_path):
                        session['doc'] = new_path
                        changed = True
            if changed and not save_json(HISTORY_FILE, history):
                warnings.append('AI 历史文档引用未能同步')
        except Exception as exc:
            logging.exception('rename metadata sync failed')
            warnings.append('文件已重命名，但部分历史记录未能同步')

        return {'ok': True, 'path': new_path, 'name': os.path.basename(new_path),
                'old_path': old_path, 'warnings': warnings}

    def save_file(self, path, content, encoding, expected_mtime=None):
        """编辑保存：写回文件，首次保存自动生成 .bak 备份。"""
        result = save_text_atomic(
            validate_file_path(path), content, encoding, expected_mtime=expected_mtime
        )
        if not result.get('ok'):
            logging.warning('save_file rejected: %s', result.get('error'))
        return result

    def save_as(self, content, suggested, assets=None):
        """把转换 / 网页 / OCR 结果另存为 .md 文件。"""
        import webview
        if self._window is None:
            return None
        try:
            target = self._window.create_file_dialog(
                webview.SAVE_DIALOG, save_filename=suggested,
                file_types=('Markdown (*.md)',))
            target = normalize_dialog_path(target, '.md')
            if not target:
                return None
            assets = assets or []
            if assets:
                import shutil
                stem = os.path.splitext(os.path.basename(target))[0]
                asset_name = stem + '.assets'
                asset_dir = os.path.join(os.path.dirname(target), asset_name)
                os.makedirs(asset_dir, exist_ok=True)
                for item in assets:
                    source = item.get('path') if isinstance(item, dict) else ''
                    name = item.get('name') if isinstance(item, dict) else ''
                    if not source or not name or not os.path.isfile(source):
                        continue
                    destination = os.path.join(asset_dir, os.path.basename(name))
                    shutil.copy2(source, destination)
                    relative = asset_name + '/' + os.path.basename(name)
                    content = content.replace(source.replace('\\', '/'), relative)
                    content = content.replace(source, relative)
            result = save_text_atomic(target, content, 'utf-8')
            if not result.get('ok'):
                logging.warning('save_as rejected: %s', result.get('error'))
                return None
            return target
        except Exception as e:
            logging.exception('save_as failed')
            return None

    def export_doc(self, fmt, payload=None):
        """导出当前文档为 PDF / DOCX / HTML（js_api 入口）。

        payload: {content, baseDir, suggestedName, options}
        返回 {ok, path, size, warns, error, canceled}。
        """
        import webview
        if self._window is None:
            return {'ok': False, 'stage': 'save_dialog', 'error': '窗口未就绪'}
        payload = payload or {}
        fmt = (fmt or '').lower()
        ext_map = {'pdf': 'PDF 文档 (*.pdf)',
                   'docx': 'Word 文档 (*.docx)',
                   'epub': 'EPUB 电子书 (*.epub)',
                   'html': 'HTML 网页 (*.html)',
                   'tex': 'LaTeX 文档 (*.tex)'}

        if fmt not in ext_map:
            return {'ok': False, 'stage': 'options', 'error': '不支持的导出格式'}
        try:
            import src.readmd_modules.mdexport as MDE
            MDE.load()
        except Exception as e:
            logging.exception('mdexport import failed')
            return {'ok': False, 'stage': 'dependency',
                    'error': '导出模块加载失败：%s' % e}
        suggested = (payload.get('suggestedName') or 'export').strip() or 'export'
        if not suggested.lower().endswith('.' + fmt):
            suggested += '.' + fmt
        try:
            target = self._window.create_file_dialog(
                webview.SAVE_DIALOG, save_filename=suggested,
                file_types=(ext_map[fmt],))
        except Exception as e:
            logging.exception('save dialog failed')
            return {'ok': False, 'stage': 'save_dialog',
                    'error': '保存对话框失败：%s' % e}
        if not target:
            return {'ok': False, 'canceled': True}
        try:
            target = normalize_dialog_path(target, '.' + fmt)
        except ValueError as e:
            logging.exception('save dialog returned invalid path')
            return {'ok': False, 'stage': 'save_dialog', 'error': str(e)}
        try:
            return MDE.export(fmt, payload.get('content') or '',
                              payload.get('baseDir') or '', target,
                              options=payload.get('options') or {},
                              source_name=payload.get('suggestedName') or '')
        except Exception as e:
            logging.exception('export failed')
            return {'ok': False, 'stage': 'render', 'error': '导出失败：%s' % e}

    def reveal_path(self, path):
        """在文件管理器中选中该文件。"""
        try:
            safe_path = validate_file_path(path)
            if IS_MAC:
                from src.readmd_modules import macos_native
                return macos_native.reveal_path(safe_path)
            elif IS_WIN:
                cmd = validate_command(['explorer', '/select,', safe_path])
                subprocess.Popen(cmd)
            else:
                cmd = validate_command(['xdg-open', os.path.dirname(safe_path)])
                subprocess.Popen(cmd)
            return True
        except Exception:
            return False

    def get_export_presets(self):
        """返回导出样式默认值 / 内置预设 / 自定义预设 / 上次参数。"""
        try:
            from src.readmd_modules.mdexport import styles as _st
        except Exception:
            return {'error': '导出模块不可用'}
        cur = load_json(SETTINGS_FILE, {})
        return {'defaults': _st.DEFAULT_STYLE,
                'presets': _st.PRESETS,
                'custom': cur.get('exportPresets', {}),
                'last': cur.get('exportLast', {})}

    def save_export_presets(self, payload):
        """保存自定义导出预设与上次参数。"""
        cur = load_json(SETTINGS_FILE, {})
        payload = payload or {}
        if 'custom' in payload:
            cur['exportPresets'] = payload.get('custom') or {}
        if 'last' in payload:
            cur['exportLast'] = payload.get('last') or {}
        save_json(SETTINGS_FILE, cur)
        return True

    def open_external(self, url):
        try:
            webbrowser.open(url)
            return True
        except Exception:
            return False

    def open_path(self, path):
        """用系统默认程序打开文件（如图片、PDF 或外部文档）。"""
        try:
            if IS_MAC:
                subprocess.Popen(['open', path])
            elif IS_WIN:
                os.startfile(path)
            else:
                subprocess.Popen(['xdg-open', path])
            return True
        except Exception:
            return False

    def check_update(self):
        from src.readmd_modules import updater
        return updater.check_update(VERSION)

    def start_download_update(self, download_url, target_filename, expected_sha=None, use_mirror=False):
        from src.readmd_modules import updater
        ok, msg = updater.start_download_update(download_url, target_filename, expected_sha, use_mirror)
        return {'ok': ok, 'message': msg}

    def get_download_status(self):
        from src.readmd_modules import updater
        return updater.get_download_status()

    def cancel_download(self):
        from src.readmd_modules import updater
        return {'ok': updater.cancel_download()}

    def apply_update(self, file_path=None, flavor=None):
        from src.readmd_modules import updater
        ok, msg = updater.apply_update(file_path, flavor)
        return {'ok': ok, 'message': msg}

    def get_system_language(self):
        return get_system_language()

    def get_bibtex(self, file_path):
        try:
            from src.readmd_modules import bibtex
            return bibtex.find_and_load_bib_for_file(file_path)
        except Exception:
            return {}

    def get_settings(self):



        return load_json(SETTINGS_FILE, {})

    def save_settings(self, settings):
        cur = load_json(SETTINGS_FILE, {})
        cur.update(settings or {})
        save_json(SETTINGS_FILE, cur)
        return True

    def get_recent(self):
        return load_json(RECENT_FILE, [])

    def add_recent(self, path):
        rec = load_json(RECENT_FILE, [])
        try:
            rec = [x for x in rec if os.path.normcase(x) != os.path.normcase(path)]
        except Exception:
            rec = [x for x in rec if x != path]
        rec.insert(0, path)
        save_json(RECENT_FILE, rec[:20])
        return True

    def clear_recent(self):
        save_json(RECENT_FILE, [])
        return True

    def save_fixed(self, path, content):
        """把修正后的文本另存为新文件。"""
        try:
            base, ext = os.path.splitext(path)
            out = base + '.readmd' + (ext or '.md')
            with open(out, 'w', encoding='utf-8', newline='\n') as f:
                f.write(content)
            return out
        except Exception as e:
            logging.exception('save_fixed failed')
            return None

    def install_association(self):
        """注册 .md 文件关联（当前用户，无需管理员）。"""
        return install_association()

    def get_app_info(self):
        return {'version': VERSION, 'python': sys.version.split()[0]}

    def check_upgrade(self):
        """启动后前端调用：静默检查 GitHub 最新 Release（失败返回空结果）。"""
        try:
            return check_latest_release() or {}
        except Exception:
            return {}

    def report_ready(self):
        """前端页面加载完成（启动里程碑：page_loaded）。"""
        with self._ready_lock:
            if self._page_ready:
                return True
            self._page_ready = True
        milestone('boot', 'page_loaded')
        callback = self._on_page_ready
        if callback is not None:
            try:
                callback()
            except Exception:
                logging.exception('page-ready callback failed')
        _finish_startup_probe(False)
        return True

    def show_window(self):
        if self._window is not None:
            try:
                self._window.show()
                self._window.restore()
            except Exception:
                pass
        return True

    def request_quit(self):
        quit_app()
        return True


# ---------------------------------------------------------------- 文件关联

def _quote(s):
    return '"%s"' % s


def install_association():
    """把 .md 等扩展名关联到 ReadMD。

    Windows: HKCU 注册表（无需管理员）。
    macOS / Linux: 提示用户手动设置（系统不支持无 .app 注册）。
    """
    if not IS_WIN:
        if IS_MAC:
            return ('macOS 不支持自动注册文件关联。请右键 .md 文件 → 显示简介 →'
                    ' 打开方式 → 选择 ReadMD → 全部更改')
        return 'Linux 请使用 xdg-mime 手动设置 .md 文件关联'
    try:
        import shutil
        frozen = getattr(sys, 'frozen', False)
        icon_source = os.path.join(APP_DIR, 'assets', 'markdown-file.ico')
        icon_dir = os.path.join(DATA_DIR, 'icons')
        icon_file = os.path.join(icon_dir, 'markdown-file.ico')
        os.makedirs(icon_dir, exist_ok=True)
        shutil.copy2(icon_source, icon_file)
        icon = '%s,0' % _quote(icon_file)
        if frozen:
            pyw = sys.executable
            cmd = '%s "%%1"' % _quote(pyw)
        else:
            pyw = None
            for cand in (os.path.join(APP_DIR, '.venv', 'Scripts', 'pythonw.exe'),):
                if os.path.isfile(cand):
                    pyw = cand
            if pyw is None:
                py = sys.executable
                base = os.path.basename(py).lower()
                if base == 'python.exe':
                    cand = os.path.splitext(py)[0] + 'w.exe'
                    pyw = cand if os.path.isfile(cand) else None
                if pyw is None:
                    pyw = py  # 退化为 python（可能闪一个控制台）
            script = os.path.join(APP_DIR, 'readmd.py')
            cmd = '%s %s "%%1"' % (_quote(pyw), _quote(script))
        for ext in ('.md', '.markdown', '.mdown', '.mkd'):
            subprocess.run(['reg', 'add', r'HKCU\Software\Classes\%s' % ext, '/ve',
                            '/d', 'ReadMD.markdown', '/f'],
                           capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\ReadMD.markdown', '/ve',
                        '/d', 'ReadMD Markdown 阅读器', '/f'], capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\ReadMD.markdown\DefaultIcon',
                        '/ve', '/d', icon, '/f'], capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\ReadMD.markdown\shell\open\command',
                        '/ve', '/t', 'REG_EXPAND_SZ', '/d', cmd, '/f'], capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\Applications\readmd.py\shell\open\command',
                        '/ve', '/t', 'REG_EXPAND_SZ', '/d', cmd, '/f'], capture_output=True)
        try:
            subprocess.run(['ie4uinit.exe', '-show'], capture_output=True)
        except Exception:
            pass
        return True
    except Exception as e:
        logging.exception('install_association failed')
        return str(e)


# ---------------------------------------------------------------- 自测

def run_selftest():
    ok = True
    try:
        reader_asset = os.path.join(APP_DIR, 'assets', 'vendor', 'readability.bundle.js')
        reader_license = os.path.join(APP_DIR, 'assets', 'vendor', 'readability.LICENSE.md')
        defuddle_asset = os.path.join(APP_DIR, 'assets', 'vendor', 'defuddle.bundle.js')
        defuddle_license = os.path.join(APP_DIR, 'assets', 'vendor', 'defuddle.LICENSE.txt')
        file_icon = os.path.join(APP_DIR, 'assets', 'markdown-file.ico')
        app_icon = os.path.join(APP_DIR, 'assets', 'readmd.ico')
        assert os.path.isfile(reader_asset) and os.path.getsize(reader_asset) > 10000
        assert os.path.isfile(reader_license) and os.path.getsize(reader_license) > 400
        assert os.path.isfile(defuddle_asset) and os.path.getsize(defuddle_asset) > 100000
        assert os.path.isfile(defuddle_license) and os.path.getsize(defuddle_license) > 500
        assert os.path.isfile(file_icon) and os.path.getsize(file_icon) > 1000
        with open(file_icon, 'rb') as _icon_handle:
            assert _icon_handle.read(4) == b'\x00\x00\x01\x00'
        if os.path.isfile(app_icon):
            import hashlib as _hashlib
            with open(file_icon, 'rb') as _file_icon_handle:
                file_icon_hash = _hashlib.sha256(_file_icon_handle.read()).digest()
            with open(app_icon, 'rb') as _app_icon_handle:
                app_icon_hash = _hashlib.sha256(_app_icon_handle.read()).digest()
            assert file_icon_hash != app_icon_hash
        import trafilatura as _tra
        tra_cfg = os.path.join(os.path.dirname(_tra.__file__), 'settings.cfg')
        assert os.path.isfile(tra_cfg), 'trafilatura/settings.cfg missing'
        import src.readmd_modules.web as _web
        fixture = ('<html><head><title>Selftest article</title></head><body><article><p>' +
                   ('web extraction content ' * 30) + '</p></article></body></html>')
        extracted = _web.extract_html('https://example.com/selftest', fixture)
        assert extracted.get('ok') and 'web extraction content' in extracted.get('content', '')
        safe_print('web extraction and file association resources OK')
    except Exception as e:
        safe_print('web extraction resource selftest failed:', e)
        ok = False
    try:
        import re as _re
        setup_py = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'installer', 'setup_app.py')
        if os.path.isfile(setup_py):
            with open(setup_py, encoding='utf-8') as f:
                _src = f.read()
            # 常规链：APP_VERSION 与 VERSION 一致；Win7 链使用环境变量覆盖。
            m1 = _re.search(r"APP_VERSION\s*=\s*\(?[\s\S]*?or\s+'([^']+)'", _src)
            m2 = _re.search(r"APP_VERSION\s*=\s*'([^']+)'", _src)
            if os.environ.get('READMD_VERSION_OVERRIDE'):
                # 版本来自同一环境变量，两侧天然一致；确认 fallback 存在即可
                assert m1 is not None or m2 is not None, '未找到 APP_VERSION（env override 链）'
                safe_print('version consistency OK (%s, env override)' % VERSION)
            else:
                inst_ver = (m1.group(1) if m1 else (m2.group(1) if m2 else None))
                assert inst_ver == VERSION, '安装器版本 %s 与主程序 %s 不一致' % (inst_ver, VERSION)
                safe_print('version consistency OK (%s)' % VERSION)
        else:
            safe_print('installer/setup_app.py not found, skip version check')
    except Exception as e:
        safe_print('version consistency failed:', e)
        ok = False
    try:
        # Tests moved to tests/ directory
        # Skipping fix tests in selftest(quiet=True)
        pass
    except Exception as e:
        safe_print('fixer tests import failed:', e)
        ok = False
    try:
        server = start_server(0)
        port = server.server_port
        with urllib.request.urlopen('http://127.0.0.1:%d/' % port, timeout=5) as r:
            body = r.read().decode('utf-8', 'replace')
            assert r.status == 200 and 'ReadMD' in body
        if getattr(sys, 'frozen', False):
            with urllib.request.urlopen(
                    'http://127.0.0.1:%d/api/modules' % port, timeout=10) as r:
                d = json.loads(r.read().decode('utf-8'))
                assert 'modules' in d and 'ai' in d['modules']
        else:
            self_file = os.path.abspath(__file__)
            with urllib.request.urlopen(
                    'http://127.0.0.1:%d/api/file?p=%s' % (port, quote(self_file)),
                    timeout=5) as r:
                d = json.loads(r.read().decode('utf-8'))
                assert d['name'] == 'readmd.py'
        safe_print('http server OK (port %d)' % port)
    except Exception as e:
        safe_print('http selftest failed:', e)
        ok = False
    try:
        import urllib.request as _urlreq
        old_inst = _read_instance()
        srv = ReadMDHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        p = srv.server_port
        tok = 'selftest-%s' % os.urandom(4).hex()
        _write_instance(p, tok)

        def _ctl(url, data=None):
            req = _urlreq.Request('http://127.0.0.1:%d%s' % (p, url),
                                  data=data, method='POST' if data is not None else 'GET',
                                  headers={'Content-Type': 'application/json'} if data is not None else {})
            try:
                with _urlreq.urlopen(req, timeout=5) as r:
                    return json.loads(r.read().decode('utf-8'))
            except _urlreq.HTTPError as e:
                return json.loads(e.read().decode('utf-8'))

        assert _ctl('/api/ping?t=' + tok).get('ok') is True
        assert _ctl('/api/ping?t=bad').get('ok') is False
        assert _ctl('/api/control/open', json.dumps({'token': 'bad', 'file': ''}).encode('utf-8')).get('ok') is not True
        assert _ctl('/api/control/open', json.dumps({'token': tok, 'file': ''}).encode('utf-8')).get('ok') is True
        d = _ctl('/api/control/next')
        assert d.get('pending') is True and d.get('file') == ''
        d = _ctl('/api/control/next')
        assert d.get('pending') is False
        srv.shutdown()
        srv.server_close()
        save_json(INSTANCE_FILE, old_inst)
        safe_print('single-instance control OK')
    except Exception as e:
        safe_print('single-instance selftest failed:', e)
        ok = False
    try:
        t = save_prompt({'name': '_selftest', 'system': 'x', 'action': 'ask'})
        assert load_prompts()['templates']
        assert delete_prompt(t['id'])
        s = save_session({'title': '_selftest', 'provider': 'DeepSeek', 'model': 'deepseek-chat',
                         'doc': 't', 'messages': [{'role': 'user', 'content': 'hi'}]})
        assert s['id'] and load_history()[0]['id'] == s['id']
        assert delete_session(s['id'])
        safe_print('prompts/history OK')
    except Exception as e:
        safe_print('prompts/history selftest failed:', e)
        ok = False
    try:
        import tempfile, base64 as _b64
        with tempfile.TemporaryDirectory() as td:
            png = _b64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==')
            img_dir = os.path.join(td, 'images')
            os.makedirs(img_dir, exist_ok=True)
            target = os.path.join(img_dir, 't.png')
            with open(target, 'wb') as f:
                f.write(png)
            assert os.path.isfile(target)
        safe_print('image save OK')
    except Exception as e:
        safe_print('image save selftest failed:', e)
        ok = False
    try:
        import tempfile as _tf
        from src.readmd_modules.mdexport import export as _export
        demo_md = '# ReadMD 导出自测\n\n正文 **加粗** 与 `代码`，公式 $\\frac{a}{b}$。\n\n| 列A | 列B |\n| --- | --- |\n| 1 | 2 |\n'
        with _tf.TemporaryDirectory() as td:
            for _fmt, _ext in (('pdf', '.pdf'), ('docx', '.docx'), ('html', '.html')):
                out = os.path.join(td, 'smoke' + _ext)
                r = _export(_fmt, demo_md, td, out,
                            options={'meta': {'title': 'Selftest'}})
                assert r.get('ok') is True and os.path.isfile(out) and r.get('size', 0) > 0, r
        safe_print('export OK')
    except Exception as e:
        safe_print('export selftest failed:', e)
        ok = False
    try:
        import tempfile as _tf
        import time as _tm
        import urllib.request as _uq
        RM.load_forced('convert')
        _td = _tf.mkdtemp()
        from docx import Document as _Doc
        _dp = os.path.join(_td, 'smoke.docx')
        _d = _Doc()
        _d.add_heading('Selftest', level=1)
        _d.add_paragraph('hello world')
        _d.save(_dp)
        from src.readmd_modules import convert as _CV
        txt, eng, err = _CV.convert_verbose(_dp)
        assert eng == 'docx' and err is None and '# Selftest' in txt, (eng, err)
        srv3 = ReadMDHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=srv3.serve_forever, daemon=True).start()
        p3 = srv3.server_port
        req = _uq.Request('http://127.0.0.1:%d/api/convert/batch' % p3,
                          data=json.dumps({'paths': [_dp], 'overwrite': True}).encode('utf-8'),
                          method='POST', headers={'Content-Type': 'application/json'})
        with _uq.urlopen(req, timeout=30) as r:
            bd = json.loads(r.read().decode('utf-8'))
        assert bd.get('job'), bd
        jid = bd['job']
        pr = {}
        for _ in range(60):
            with _uq.urlopen('http://127.0.0.1:%d/api/convert/progress?job=%s' % (p3, jid), timeout=5) as r:
                pr = json.loads(r.read().decode('utf-8'))
            if pr.get('finished'):
                break
            _tm.sleep(0.25)
        assert pr.get('finished') and pr['items'][0].get('status') == 'ok', pr
        assert os.path.isfile(os.path.join(_td, 'smoke.md'))
        srv3.shutdown()
        srv3.server_close()
        safe_print('convert OK')
    except Exception as e:
        safe_print('convert selftest failed:', e)
        ok = False
    safe_print('selftest %s' % ('PASSED' if ok else 'FAILED'))
    return 0 if ok else 1


def run_webview_selftest():
    """Exercise the native WebView network guard against a private subresource."""
    hits = {'probe': 0}

    class ProbeHandler(BaseHTTPRequestHandler):
        def do_GET(self):
            hits['probe'] += 1
            self.send_response(204)
            self.end_headers()

        def log_message(self, *_args):
            pass

    probe = ThreadingHTTPServer(('127.0.0.1', 0), ProbeHandler)
    threading.Thread(target=probe.serve_forever, daemon=True).start()

    class PageHandler(BaseHTTPRequestHandler):
        def do_GET(self):
            body = ('''<!doctype html><html><head><title>Guard selftest</title></head>
              <body><main><h1>Native WebView guard selftest</h1>
              <p>This local fixture verifies that the rendered document remains readable
              while a cross-origin private subresource request is denied before sending.</p>
              <img src="http://127.0.0.1:%d/probe"></main></body></html>'''
                    % probe.server_port).encode('utf-8')
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args):
            pass

    page = ThreadingHTTPServer(('127.0.0.1', 0), PageHandler)
    threading.Thread(target=page.serve_forever, daemon=True).start()
    outcome = {'ok': False, 'error': 'WebView callback did not run'}
    try:
        import webview
        RM.load_forced('web')
        api = Api()
        host = webview.create_window('ReadMD WebView selftest', 'about:blank',
                                     hidden=True, width=320, height=240)

        def exercise():
            task_id = 'native-guard-selftest'
            target = 'http://127.0.0.1:%d/article' % page.server_port
            try:
                grant = api.authorize_private_web(target, task_id)
                if not grant.get('ok'):
                    raise AssertionError(grant)
                rendered = api.render_web_page(
                    target, task_id, 15000, False, grant['grant'], '')
                if not rendered.get('ok'):
                    raise AssertionError(rendered)
                time.sleep(0.25)
                if hits['probe']:
                    raise AssertionError('private cross-origin request escaped guard')
                offline_html = ('''<!doctype html><html><head><title>Offline guard</title></head>
                      <body><main><h1>Offline public document</h1><p>The native renderer
                      must preserve the verified public base URL while blocking every
                      network request from untrusted page HTML.</p><a href="next">next</a>
                      <img src="http://127.0.0.1:%d/offline-probe"></main></body></html>'''
                                % probe.server_port)
                offline = api.render_web_page(
                    'https://93.184.216.34/selftest', 'offline-guard-selftest',
                    15000, False, '', offline_html)
                if not offline.get('ok'):
                    raise AssertionError(offline)
                if not str(offline.get('final_url', '')).startswith(
                        'https://93.184.216.34/'):
                    raise AssertionError('offline renderer lost verified base URL')
                if hits['probe']:
                    raise AssertionError('offline HTML escaped network guard')
                outcome.update(ok=True, error='')
            except Exception as exc:
                outcome.update(ok=False, error=str(exc))
            finally:
                api.revoke_private_web(task_id)
                try:
                    host.destroy()
                except Exception:
                    pass

        if IS_MAC:
            webview.start(exercise, gui='cocoa', private_mode=True)
        else:
            webview.start(exercise, private_mode=True)
    except Exception as exc:
        outcome.update(ok=False, error=str(exc))
    finally:
        for server in (page, probe):
            server.shutdown()
            server.server_close()
    safe_print('webview network guard %s%s' % (
        'PASSED' if outcome['ok'] else 'FAILED',
        '' if outcome['ok'] else ': ' + outcome['error']))
    return 0 if outcome['ok'] else 1


# ---------------------------------------------------------------- 启动

def main():
    global _T0
    parser = argparse.ArgumentParser(description='ReadMD - 轻量级 Markdown 阅读器')
    parser.add_argument('file', nargs='?', help='要打开的 .md 文件')
    parser.add_argument('--browser', action='store_true', help='用默认浏览器打开（兜底模式）')
    parser.add_argument('--port', type=int, default=0, help='本地服务端口（默认随机）')
    parser.add_argument('--selftest', action='store_true', help='运行自测')
    parser.add_argument('--webview-selftest', action='store_true',
                        help='运行原生 WebView 私网隔离自测')
    parser.add_argument('--mods', action='store_true', help='加载全部扩展模块并报告状态')
    parser.add_argument('--share', action='store_true', help='启动后自动开启局域网共享（手机扫码访问）')
    parser.add_argument('--assoc', action='store_true', help='注册 .md 默认打开方式后退出')
    parser.add_argument('--startup-probe', action='store_true',
                        help='记录启动里程碑并在页面就绪后自动退出')
    parser.add_argument('--startup-probe-json', metavar='PATH',
                        help='把 --startup-probe 的 JSON 报告原子写入 PATH')
    parser.add_argument('--startup-probe-timeout', type=float, default=20.0, metavar='SECONDS',
                        help='启动 probe 超时秒数（默认 20）')
    args = parser.parse_args()

    if args.startup_probe_json and not args.startup_probe:
        parser.error('--startup-probe-json 需要 --startup-probe')
    if args.startup_probe and args.browser:
        parser.error('--startup-probe 不能与 --browser 同时使用')
    if args.startup_probe_timeout <= 0:
        parser.error('--startup-probe-timeout 必须大于 0')
    if args.startup_probe:
        _T0 = time.time()
        with _BOOT_LOCK:
            _BOOT_MILESTONES.clear()
            _STARTUP_PROBE.update({'enabled': True, 'timeout': args.startup_probe_timeout,
                                   'json_path': args.startup_probe_json or '', 'window': None,
                                   'finished': False, 'timed_out': False, 'timer': None})

    if args.assoc:
        r = install_association()
        safe_print('association: %s' % ('OK' if r is True else r))
        return 0 if r is True else 1

    if args.selftest:
        sys.exit(run_selftest())

    if args.webview_selftest:
        sys.exit(run_webview_selftest())

    if args.mods:
        ok = True
        for m in RM.MODULES:
            good = RM.load_forced(m)
            st, err = RM.status()
            safe_print('%s: %s%s' % (m, st.get(m), (' - ' + err.get(m, '')) if err.get(m) else ''))
            ok = ok and good
        return 0 if ok else 1

    setup_logging()
    milestone('boot', 'start')

    # Win7 版：OCR / AI / 网页转 MD 依赖 WinRT 或未打包，标记为不可用
    if is_win7():
        RM.set_disabled(('ocr', 'web', 'ai'), WIN7_UNAVAILABLE)

    # 单实例：已有常驻实例 → 转发文件 / 唤起窗口后立即退出（秒开）
    alive = None if args.startup_probe else instance_alive()
    if alive is not None:
        port, token = alive
        if not args.file or forward_open(port, token, os.path.abspath(args.file)):
            return 0

    server = start_server(args.port)
    if server.server_port == CONTROL_PORT:
        _write_instance(CONTROL_PORT, secrets.token_urlsafe(16))
    milestone('boot', 'server_up')
    try:
        from src.readmd_modules import updater
        updater.clean_old_update_artifacts()
    except Exception:
        pass
    if args.share:

        d = start_lan_server()
        if d.get('ok'):
            safe_print('局域网共享已开启：%s' % d.get('url'))
        else:
            safe_print('局域网共享失败：%s' % d.get('error'))
    initial = None
    if args.file:
        p = os.path.abspath(args.file)
        if os.path.isfile(p):
            initial = p
        else:
            safe_print('文件不存在: %s' % args.file)

    url = 'http://127.0.0.1:%d/' % server.server_port
    if initial:
        url += '?file=' + quote(initial)

    if args.browser:
        webbrowser.open(url)
        safe_print('ReadMD 服务运行于 %s（Ctrl+C 退出）' % url)
        try:
            while True:
                threading.Event().wait(3600)
        except KeyboardInterrupt:
            pass
        _clear_instance()
        return 0

    try:
        import webview
    except ImportError:
        safe_print('未安装 pywebview。请先运行 install%s，或用 --browser 模式。' %
                   ('.sh' if sys.platform != 'win32' else '.bat'))
        safe_print('快速兜底：python readmd.py --browser "%s"' % (initial or ''))
        if args.startup_probe:
            write_startup_probe(args.startup_probe_json, timed_out=False)
        return 1

    api = Api()
    milestone('boot', 'webview_imported')
    try:
        window = webview.create_window(
            'ReadMD', url, js_api=api,
            width=1160, height=820, min_size=(720, 480),
            text_select=True, zoomable=True, background_color='#f7f7f5')
    except Exception as e:
        safe_print('创建窗口失败：%s' % e)
        if args.startup_probe:
            write_startup_probe(args.startup_probe_json, timed_out=False)
        return 1
    api._window = window
    api._on_page_ready = lambda: _start_tray_once(window)
    with _control_lock:
        _CONTROL['window'] = window
        _CONTROL['ready'] = True
    milestone('boot', 'window_created')

    if args.startup_probe:
        with _BOOT_LOCK:
            _STARTUP_PROBE['window'] = window
            timer = threading.Timer(args.startup_probe_timeout, _finish_startup_probe,
                                    kwargs={'timed_out': True})
            timer.daemon = True
            _STARTUP_PROBE['timer'] = timer
        timer.start()

    # 页面加载完成（Python 侧兜底；JS report_ready 为精确 page_loaded 打点）
    def _on_loaded():
        milestone('boot', 'window_loaded')

    try:
        window.events.loaded += _on_loaded
    except Exception:
        pass

    # 关闭按钮 → 隐藏到托盘（真正退出走托盘“退出”）
    def _on_closing():
        # Startup probes must destroy the window so the frozen process can
        # flush its JSON report and exit.  Hiding it would leave CI waiting on
        # an unreachable tray process.
        if args.startup_probe:
            return True
        # Before the UI reports ready there is no tray affordance. Let the
        # native close proceed instead of creating a hidden, unreachable app.
        if not api._page_ready:
            return True
        try:
            window.hide()
        except Exception:
            pass
        return False

    try:
        window.events.closing += _on_closing
    except Exception:
        pass

    setup_win7_webview2_env()
    if IS_LINUX:
        try:
            from src.readmd_modules import linux_native
            linux_native.setup_linux_env()
        except Exception:
            pass

    try:
        if IS_MAC:
            webview.start(gui='cocoa')
        elif IS_LINUX:
            try:
                webview.start(gui='gtk')
            except Exception:
                webview.start()
        else:
            webview.start()
    except Exception as e:
        logging.exception('webview start failed')
        safe_print('启动失败：%s' % e)
        try:
            if IS_WIN:
                from src.readmd_modules import windows_native
                windows_native.show_error('ReadMD', '启动失败：%s' % e)
            elif IS_MAC:
                from src.readmd_modules import macos_native
                macos_native.show_error('ReadMD', '启动失败，请查看日志。')
            elif IS_LINUX:
                from src.readmd_modules import linux_native
                linux_native.show_notification('ReadMD 启动失败', str(e))
        except Exception:
            pass
        if args.startup_probe:
            write_startup_probe(args.startup_probe_json,
                                timed_out=bool(_STARTUP_PROBE.get('timed_out')))
        return 1


    if args.startup_probe:
        timed_out = bool(_STARTUP_PROBE.get('timed_out'))
        try:
            write_startup_probe(args.startup_probe_json, timed_out=timed_out)
        except Exception as exc:
            safe_print('startup probe write failed: %s' % exc)
            return 1
        _clear_instance()
        return 1 if timed_out else 0

    # 窗口真正被销毁时（托盘退出走 os._exit，通常到不了这里）
    _clear_instance()
    return 0


_tray_icon = {'icon': None, 'started': False}
_tray_lock = threading.Lock()


def _start_tray_once(window):
    """Create the tray only after the page is usable, and never twice."""
    with _tray_lock:
        if _tray_icon['started']:
            return _tray_icon['icon']
        _tray_icon['started'] = True
        return _start_tray(window)


def _start_tray(window):
    """启动系统托盘图标（pystray run_detached）；失败静默降级。"""
    try:
        import pystray
        from PIL import Image
    except Exception:
        return None
    try:
        # macOS pystray 需要 PNG；优先 .png 兜底 .ico
        icon_candidates = ['icon-256.png', 'readmd.ico']
        img = None
        for fname in icon_candidates:
            p = os.path.join(APP_DIR, 'assets', fname)
            if os.path.isfile(p):
                try:
                    img = Image.open(p)
                    break
                except Exception:
                    continue
    except Exception:
        img = None

    def act_show(icon, item):
        try:
            window.show()
            window.restore()
        except Exception:
            pass

    def act_open(icon, item):
        try:
            window.show()
            window.restore()
            window.evaluate_js('window.__trayOpenFile && window.__trayOpenFile();')
        except Exception:
            pass

    def act_quit(icon, item):
        quit_app()

    try:
        menu = pystray.Menu(
            pystray.MenuItem('显示 ReadMD', act_show, default=True),
            pystray.MenuItem('打开文件…', act_open),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem('退出 ReadMD', act_quit),
        )
        icon = pystray.Icon('readmd', img, 'ReadMD', menu=menu)
        icon.run_detached()
        _tray_icon['icon'] = icon
        logging.info('tray started')
        return icon
    except Exception as e:
        logging.exception('tray start failed: %s', e)
        return None
    return 0


if __name__ == '__main__':
    sys.exit(main())
