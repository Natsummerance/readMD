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
import base64
import binascii
import gzip
import hashlib
import json
import logging
import mimetypes
import os
import re
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import threading
import webbrowser
import urllib.request
from datetime import datetime, timezone
from email.utils import formatdate, parsedate_to_datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, unquote, urlparse

from src.readmd_core import (
    normalize_dialog_path,
    load_json,
    save_json,
    read_text,
    readmd_fix,
)
from src.readmd_core.file_writer import save_text_atomic
from src.readmd_core.static_assets import resolve_asset
from src.readmd_core.safe_open import safe_external_url, safe_file_target
from src.readmd_core.versioning import compare_versions as _version_compare
from src.readmd_core.versioning import parse_version as _version_parse
from src.readmd_core.versioning import select_update_release as _select_update_release
import src.readmd_modules as RM
from src.readmd_modules.validators import validate_file_path, validate_command, paths_within
from src.readmd_modules.skills import SkillError, SkillRegistry, default_skill_roots
from src.readmd_modules import skill_import as _skill_import
from src.readmd_modules.crypto import store_credential as _store_credential, delete_credential as _delete_credential
from src.readmd_modules.pet import (
    HermesPetBridge,
    HermesPetLauncher,
    HermesPetPluginInstaller,
    PetBatchQueue,
    PetController,
    foreground_fullscreen,
    verify_model_bundle,
)
from src.readmd_core.service import ReadMDCoreService
from src.readmd_core import upstream as _upstream_sources

APP_DIR = sys._MEIPASS if getattr(sys, 'frozen', False) else os.path.dirname(os.path.abspath(__file__))
PET_MODEL_DIR = os.path.join(APP_DIR, 'assets', 'pet', 'model')

from src.readmd_core.config import (
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
    load_dotenv,
    VERSION,
)

load_dotenv()


def native_gui_required():
    """Return whether a Linux/macOS frozen build must use its native backend.

    Formal packages fail closed when the native engine is unavailable.  The
    explicit ``--browser`` mode remains available for development and
    diagnostics, but a packaged release must never silently lose the Python
    bridge by opening a regular browser window.
    """
    if not (IS_MAC or IS_LINUX):
        return False
    value = os.environ.get('READMD_REQUIRE_NATIVE_GUI')
    if value is not None:
        return value.strip().lower() in ('1', 'true', 'yes', 'on')
    return bool(getattr(sys, 'frozen', False))

MD_EXTS = ('.md', '.markdown', '.mdown', '.mkd', '.mdx', '.txt')
CODE_CONFIG_EXTS = (
    '.toml', '.yaml', '.yml', '.json', '.json5', '.jsonc', '.ini', '.cfg',
    '.conf', '.config', '.env', '.properties', '.xml', '.plist', '.inf',
    '.bat', '.cmd', '.ps1', '.psm1', '.sh', '.bash', '.zsh', '.fish', '.vbs',
    '.py', '.js', '.mjs', '.cjs', '.ts', '.tsx', '.jsx', '.c', '.cpp',
    '.h', '.hpp', '.cc', '.cxx', '.cs', '.java', '.kt', '.kts', '.rs',
    '.go', '.rb', '.php', '.swift', '.lua', '.r', '.m', '.dart', '.sql',
    '.dockerfile', '.makefile', '.gradle', '.html', '.htm', '.css', '.scss',
    '.sass', '.less', '.vue', '.svelte', '.log', '.out', '.err', '.diff',
    '.patch', '.gitignore', '.gitattributes', '.editorconfig', '.npmrc',
    '.rst', '.asciidoc', '.adoc', '.bib', '.csv', '.tsv',
)
ALL_TEXT_EXTS = MD_EXTS + CODE_CONFIG_EXTS
CONVERT_EXTS = ('.docx', '.doc', '.pptx', '.ppt', '.xlsx', '.xls', '.pdf', '.html', '.htm',
                '.txt', '.csv', '.json', '.xml', '.zip', '.eml', '.msg', '.rtf', '.odt', '.epub') + CODE_CONFIG_EXTS
WIN7_CONVERT_EXTS = ('.docx', '.pdf')
WIN7_UNAVAILABLE = '该功能在 Win7 版暂不支持（本版本仅保留 docx / pdf 转 MD 与导出功能）'

# ------------------------------------------------------------------ 升级推送（静默）

_UPGRADE_RELEASE_URL = 'https://api.github.com/repos/Natsummerance/readMD/releases/latest'
_UPGRADE_RELEASES_URL = 'https://api.github.com/repos/Natsummerance/readMD/releases?per_page=100'
_UPGRADE_CACHE = {'done': False, 'result': None}


def _parse_version(value):
    return _version_parse(value)


def _compare_versions(left, right):
    return _version_compare(left, right)


def check_latest_release():
    """查询 GitHub 最新 Release；失败/超时静默返回 None，结果进程内缓存。"""
    if _UPGRADE_CACHE['done']:
        return _UPGRADE_CACHE['result']
    result = None
    try:
        import urllib.request as _urlreq
        parsed_current = _parse_version(VERSION)
        current_is_prerelease = bool(parsed_current and parsed_current[1] == 0)
        url = _UPGRADE_RELEASES_URL if current_is_prerelease else _UPGRADE_RELEASE_URL
        req = _urlreq.Request(url, headers={
            'User-Agent': 'ReadMD/%s' % VERSION,
            'Accept': 'application/vnd.github+json',
        })
        with _urlreq.urlopen(req, timeout=4) as resp:
            payload = json.loads(resp.read(1024 * 1024).decode('utf-8'))
        releases = payload if isinstance(payload, list) else [payload]
        latest_release = _select_update_release(VERSION, releases)
        tag = str(latest_release.get('tag_name') or '') if latest_release else ''
        current = _parse_version(VERSION)
        if latest_release and current and (_compare_versions(tag, VERSION) or 0) > 0:
            result = {
                'latest': tag,
                'url': str(latest_release.get('html_url') or _UPGRADE_RELEASE_URL),
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

_CONTROL = {'queue': [], 'pet_batches': [], 'pet_menus': 0, 'window': None, 'ready': False}
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


def push_pet_batch(paths):
    """Request a user-confirmed batch through the already-loaded ReadMD UI."""
    safe_paths = [path for path in (paths or ()) if isinstance(path, str) and path]
    if not safe_paths:
        return False
    with _control_lock:
        _CONTROL['pet_batches'].append(safe_paths)
        win = _CONTROL.get('window')
        ready = _CONTROL.get('ready')
    if win is not None and ready:
        try:
            win.evaluate_js('window.receivePetBatch && window.receivePetBatch(%s);' %
                            json.dumps(safe_paths))
            with _control_lock:
                if _CONTROL['pet_batches'] and _CONTROL['pet_batches'][0] == safe_paths:
                    _CONTROL['pet_batches'].pop(0)
        except Exception:
            pass
        try:
            win.show()
            win.restore()
        except Exception:
            pass
    return True


def pop_pet_batch():
    with _control_lock:
        if _CONTROL['pet_batches']:
            return _CONTROL['pet_batches'].pop(0)
    return None


def push_pet_menu():
    """Open the existing More menu from the copied Hermes single-click event."""
    with _control_lock:
        _CONTROL['pet_menus'] += 1
        win = _CONTROL.get('window')
        ready = _CONTROL.get('ready')
    if win is not None and ready:
        try:
            win.evaluate_js('window.openPetQuickMenu && window.openPetQuickMenu();')
            with _control_lock:
                if _CONTROL['pet_menus']:
                    _CONTROL['pet_menus'] -= 1
        except Exception:
            pass
        try:
            win.show()
            win.restore()
        except Exception:
            pass
    return True


def pop_pet_menu():
    with _control_lock:
        if _CONTROL['pet_menus']:
            _CONTROL['pet_menus'] -= 1
            return True
    return False


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

# Built-in actions are metadata only; instruction text lives in assets/skills.
_BUILTIN_ACTIONS = (
    ("quick_read", "快速阅读", "quick_read", "readmd-quick-read"),
    ("polish", "润色文稿", "polish", "readmd-polish"),
    ("proofread", "语法纠错", "proofread", "readmd-proofread"),
    ("to_english", "翻译为英文", "translate_en", "readmd-translate"),
    ("to_chinese", "翻译为中文", "translate_zh", "readmd-translate"),
    ("action_items", "提取待办", "todo", "readmd-todo"),
    ("continue", "续写内容", "continue", "readmd-continue"),
    ("ask", "自由提问", "ask", "readmd-ask"),
    ("summary", "总结要点", "summary", "readmd-summary"),
    ("outline", "生成大纲", "outline", "readmd-outline"),
    ("weekly", "生成周报", "weekly", "readmd-weekly"),
    ("code_review", "代码审查", "code_review", "readmd-code-review"),
    ("fix_format", "修正格式", "modify", "readmd-format-fix"),
)


def _skill_registry(project_dir=None):
    """Return the canonical shared Skill registry used by every client."""
    if project_dir:
        return ReadMDCoreService(project_dir).skills
    global _DESKTOP_CORE_SERVICE
    if _DESKTOP_CORE_SERVICE is None:
        _DESKTOP_CORE_SERVICE = ReadMDCoreService()
    else:
        _DESKTOP_CORE_SERVICE.reload()
    return _DESKTOP_CORE_SERVICE.skills


_DESKTOP_CORE_SERVICE = None


def _builtin_prompts():
    registry = _skill_registry()
    result = []
    for template_id, name, action, skill_id in _BUILTIN_ACTIONS:
        skill = registry.get(skill_id)
        if not skill:
            continue
        result.append({
            "id": template_id,
            "skill_id": skill_id,
            "name": name,
            "action": action,
            "system": skill.instructions,
            "user": "{doc}\n\n{prompt}",
            "builtin": True,
        })
    return result


BUILTIN_PROMPTS = _builtin_prompts()


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


def _public_skill(skill, include_instructions=False):
    # Never return an absolute user path to a renderer, extension or MCP
    # client.  It is both unnecessary for the workbench and a local privacy
    # leak in exported history/screenshots.
    try:
        safe_path = os.path.relpath(skill.path, skill.root).replace('\\', '/')
    except (TypeError, ValueError):
        safe_path = skill.id
    data = {
        'id': skill.id,
        'name': skill.name,
        'description': skill.description,
        'scope': skill.scope,
        'variables': skill.variables,
        'path': safe_path,
        'metadata': dict(skill.metadata),
    }
    provenance = data['metadata'].get('provenance')
    if isinstance(provenance, dict):
        data['provenance'] = dict(provenance)
    data.setdefault('source_files', data['metadata'].get('source_files', []))
    data.setdefault('license', data['metadata'].get('license', ''))
    data.setdefault('adaptation_notes', data['metadata'].get('adaptation_notes', []))
    if include_instructions:
        data['instructions'] = skill.instructions
    return data


def load_skills(project_dir=None):
    """List Skills from builtin, user and optional project roots."""
    # The workbench needs a local preview without another round trip.  This is
    # still instruction text (never credentials) and follows the same registry
    # precedence and path checks as the read endpoint.
    return [_public_skill(skill, include_instructions=True)
            for skill in _skill_registry(project_dir).list(include_disabled=True)]


def _user_skill_folder(skill_id):
    if not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,63}', str(skill_id or '')):
        raise SkillError('Skill id must be lowercase kebab-case')
    folder = os.path.realpath(os.path.join(DATA_DIR, 'skills', str(skill_id)))
    root = os.path.realpath(os.path.join(DATA_DIR, 'skills'))
    if not paths_within(folder, root) or folder == root:
        raise SkillError('Skill path is outside the user Skill directory')
    return folder


def validate_skill_document(skill_id, content, metadata=None):
    """Validate a Skill document in memory before it is persisted."""
    if not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,63}', str(skill_id or '')):
        raise SkillError('Skill id must be lowercase kebab-case')
    if not isinstance(content, str) or not content.strip() or len(content.encode('utf-8')) > 512 * 1024:
        raise SkillError('Skill content is empty or too large')
    import tempfile
    with tempfile.TemporaryDirectory(prefix='readmd-skill-') as tmp:
        folder = os.path.join(tmp, str(skill_id))
        os.makedirs(folder, exist_ok=True)
        with open(os.path.join(folder, 'SKILL.md'), 'w', encoding='utf-8', newline='\n') as handle:
            handle.write(content)
        if metadata is not None:
            with open(os.path.join(folder, 'readmd.skill.json'), 'w', encoding='utf-8', newline='\n') as handle:
                json.dump(metadata, handle, ensure_ascii=False, indent=2)
        skill = SkillRegistry([tmp]).get(str(skill_id))
        if skill is None:
            raise SkillError('Skill was not discovered after validation')
        return _public_skill(skill, include_instructions=True)


def save_user_skill(skill_id, content, metadata=None):
    """Atomically publish a validated user Skill; scripts are never enabled."""
    validated = validate_skill_document(skill_id, content, metadata)
    folder = _user_skill_folder(skill_id)
    # Keep a local rollback snapshot before replacing an existing user Skill.
    if os.path.isfile(os.path.join(folder, 'SKILL.md')):
        import shutil
        stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
        version_dir = os.path.join(os.path.dirname(folder), '.versions', str(skill_id), stamp)
        os.makedirs(version_dir, exist_ok=True)
        shutil.copy2(os.path.join(folder, 'SKILL.md'), os.path.join(version_dir, 'SKILL.md'))
        old_meta = os.path.join(folder, 'readmd.skill.json')
        if os.path.isfile(old_meta):
            shutil.copy2(old_meta, os.path.join(version_dir, 'readmd.skill.json'))
    os.makedirs(folder, exist_ok=True)
    save_text_atomic(os.path.join(folder, 'SKILL.md'), content.strip() + '\n')
    if metadata is not None:
        safe_meta = dict(metadata)
        safe_meta['scripts_allowed'] = False
        save_text_atomic(os.path.join(folder, 'readmd.skill.json'), json.dumps(safe_meta, ensure_ascii=False, indent=2) + '\n')
    return validated


def _skill_versions(skill_id):
    """Return rollback snapshots for one user Skill, newest first."""
    _user_skill_folder(skill_id)
    root = os.path.join(DATA_DIR, 'skills', '.versions', str(skill_id))
    if not os.path.isdir(root):
        return []
    return [name for name in sorted(os.listdir(root), reverse=True)
            if re.fullmatch(r'\d{8}T\d{6}Z', name)
            and os.path.isfile(os.path.join(root, name, 'SKILL.md'))]


def save_prompt(template):
    """新增 / 更新模板。id 为空时自动生成；内置 id 表示覆盖内置模板。"""
    t = dict(template or {})
    if not t.get('id'):
        t['id'] = 't_%d' % int(time.time() * 1000)
    if not t.get('name'):
        t['name'] = '未命名模板'
    t.pop('builtin', None)

    # 兼容旧版自定义模板：第一次保存时将 system 指令迁移为用户 Skill。
    # 迁移只写入 DATA_DIR/skills，且由同一套 Skill 校验器检查，避免继续
    # 在 prompts.json 中维护第二份可执行 Prompt 实现。
    if not t.get('skill_id') and str(t.get('system') or '').strip():
        raw_id = re.sub(r'[^a-z0-9-]+', '-', str(t.get('id') or '').lower()).strip('-')
        skill_id = ('prompt-' + raw_id)[:64].rstrip('-') or ('prompt-%d' % int(time.time() * 1000))
        system = str(t.get('system') or '').strip()
        skill_doc = (
            '---\n'
            'name: %s\n'
            'description: Use when running the custom ReadMD prompt %s.\n'
            '---\n\n%s\n' % (skill_id, str(t.get('name') or skill_id), system)
        )
        try:
            save_user_skill(skill_id, skill_doc, {
                'id': skill_id,
                'source': 'legacy-prompt-migration',
                'version': 1,
                'scripts_allowed': False,
                'legacy_template_id': t.get('id'),
            })
            t['skill_id'] = skill_id
            # Keep user text template only as a presentation/compatibility field;
            # the system instruction is now owned by SKILL.md.
            t.pop('system', None)
        except SkillError:
            # Do not persist an unvalidated legacy Prompt. The caller receives a
            # normal validation error instead of silently retaining raw Prompt code.
            raise
    d = load_json(PROMPTS_FILE, {})
    customs = [c for c in d.get('templates', []) if c.get('id') != t.get('id')]
    customs.append(t)
    save_json(PROMPTS_FILE, {'templates': customs})
    return t


def delete_prompt(prompt_id):
    d = load_json(PROMPTS_FILE, {})
    removed = [t for t in d.get('templates', []) if t.get('id') == prompt_id]
    d['templates'] = [t for t in d.get('templates', []) if t.get('id') != prompt_id]
    save_json(PROMPTS_FILE, d)
    # Remove only the exact migrated user Skill, never a builtin/project Skill.
    for template in removed:
        skill_id = str(template.get('skill_id') or '')
        if skill_id.startswith('prompt-'):
            try:
                folder = _user_skill_folder(skill_id)
                if os.path.isdir(folder):
                    import shutil
                    shutil.rmtree(folder)
            except (OSError, SkillError):
                logging.debug('legacy prompt skill cleanup failed', exc_info=True)
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


def _batch_output_paths(paths):
    """Plan collision-free Markdown targets without touching source files."""
    planned, used = {}, set()
    seen_sources = set()
    for src in paths:
        source_key = os.path.normcase(os.path.realpath(os.path.abspath(src)))
        if source_key in seen_sources:
            continue
        seen_sources.add(source_key)
        candidate = _md_output_path(src)
        key = os.path.normcase(os.path.abspath(candidate))
        if key not in used:
            planned[src] = candidate
            used.add(key)
            continue
        try:
            with open(src, 'rb') as handle:
                digest = hashlib.sha256(handle.read()).hexdigest()[:8]
        except OSError:
            digest = hashlib.sha256(os.path.abspath(src).encode('utf-8', errors='replace')).hexdigest()[:8]
        stem, ext = os.path.splitext(candidate)
        candidate = '%s-%s%s' % (stem, digest, ext)
        suffix = 2
        while os.path.normcase(os.path.abspath(candidate)) in used:
            candidate = '%s-%s-%d%s' % (stem, digest, suffix, ext)
            suffix += 1
        planned[src] = candidate
        used.add(os.path.normcase(os.path.abspath(candidate)))
    return planned


def _write_md(path, content):
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(content)
    return True


def _safe_export_target(path, suffix):
    """Validate a caller-supplied export target without requiring it to exist."""
    raw = os.fspath(path or '')
    if not raw or '\x00' in raw or any(ord(ch) < 32 for ch in raw):
        raise ValueError('invalid_output_path')
    candidate = os.path.realpath(os.path.abspath(raw))
    if not candidate.lower().endswith(str(suffix).lower()):
        raise ValueError('invalid_output_extension')
    parent = os.path.dirname(candidate)
    if not os.path.isdir(parent):
        raise ValueError('output_directory_not_found')
    if os.path.lexists(candidate) and not os.path.isfile(candidate):
        raise ValueError('output_target_not_regular')
    return candidate


def _convert_worker(job):
    items = job['items']
    for it in items:
        if job.get('cancel'):
            it['status'] = 'canceled'
            it['done'] = True
            continue
        it['status'] = 'running'
        try:
            mod = RM.get('convert')
            text, engine, err = mod.convert_verbose(it['src'])
            if err and not text:
                it['status'] = 'error'
                it['error'] = err
                it['error_code'] = 'conversion_failed'
                it['done'] = True
                continue
            if not text.strip():
                it['status'] = 'error'
                it['error'] = '未提取到文字（可尝试 OCR）'
                it['error_code'] = 'empty_output'
                it['done'] = True
                continue
            import src.readmd_modules.mdcheck as MDC
            fixed, warns = MDC.check(text, os.path.dirname(os.path.abspath(it['src'])))
            out = it.get('planned_out') or _md_output_path(it['src'])
            it['out'] = out
            it['engine'] = engine
            it['warns'] = warns
            if os.path.exists(out) and not job.get('overwrite'):
                it['status'] = 'skipped'
                it['error_code'] = 'output_exists'
            else:
                try:
                    _write_md(out, fixed)
                    it['status'] = 'ok'
                except Exception as e:  # noqa: BLE001
                    it['status'] = 'error'
                    it['error'] = '写入失败：%s' % e
                    it['error_code'] = 'write_failed'
        except Exception as e:  # noqa: BLE001
            logging.exception('batch convert failed: %s', it.get('src'))
            it['status'] = 'error'
            it['error'] = str(e)
            it['error_code'] = 'conversion_failed'
        it['done'] = True
    job['running'] = False
    job['finished'] = True


def _start_convert_job(paths, overwrite):
    with _CONVERT_LOCK:
        _CONVERT_JOB_SEQ[0] += 1
        jid = 'c%d' % _CONVERT_JOB_SEQ[0]
        outputs = _batch_output_paths(paths)
        job = {'id': jid, 'overwrite': bool(overwrite), 'running': True,
               'finished': False, 'cancel': False,
               'items': [{'src': p, 'planned_out': outputs.get(p),
                          'status': 'queued', 'done': False} for p in paths]}
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


_SKILL_EVALUATION_TOKENS = {}
_SKILL_EVALUATION_TTL = 10 * 60


def _skill_content_digest(skill_id, content):
    payload = ('%s\0%s' % (skill_id, content)).encode('utf-8')
    return hashlib.sha256(payload).hexdigest()


def _issue_skill_evaluation_token(skill_id, content):
    token = secrets.token_urlsafe(32)
    _SKILL_EVALUATION_TOKENS[token] = {
        'digest': _skill_content_digest(skill_id, content),
        'expires': time.time() + _SKILL_EVALUATION_TTL,
    }
    # Keep the in-memory registry bounded even if a client abandons drafts.
    now = time.time()
    for key, value in list(_SKILL_EVALUATION_TOKENS.items()):
        if value.get('expires', 0) <= now:
            _SKILL_EVALUATION_TOKENS.pop(key, None)
    return token


def _consume_skill_evaluation_token(token, skill_id, content):
    if not isinstance(token, str) or not token:
        return False
    record = _SKILL_EVALUATION_TOKENS.pop(token, None)
    if not record or record.get('expires', 0) <= time.time():
        return False
    return secrets.compare_digest(
        record.get('digest', ''), _skill_content_digest(skill_id, content))


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    server_version = 'ReadMD/' + VERSION
    LAN_TOKEN = None
    LAN_BLOCKED_PATHS = frozenset({
        '/api/save', '/api/upload', '/api/image/save', '/api/code/run',
        '/api/update/download', '/api/update/apply', '/api/import/process',
        '/api/control/open', '/api/control/next', '/api/pets/import',
        '/api/pets/remove', '/api/pets/active',
    })
    LAN_SCOPED_PATHS = frozenset({
        '/api/file', '/api/list', '/api/ocr', '/api/convert', '/raw',
    })

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
                self._send(500, 'text/plain; charset=utf-8', 'internal error'.encode('utf-8'))
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
                self._send(500, 'text/plain; charset=utf-8', 'internal error'.encode('utf-8'))
            except Exception:
                pass

    def do_DELETE(self):
        """Handle the small, explicitly-scoped delete API surface."""
        if not self._lan_authorized() or not self._post_origin_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        try:
            self._route()
        except Exception as e:
            logging.exception('http delete error: %s', self.path)
            try:
                self._send(500, 'text/plain; charset=utf-8', 'internal error'.encode('utf-8'))
            except Exception:
                pass

    def _lan_authorized(self):
        """局域网模式下，除页面与静态资源外，所有 API 都要求携带 token。"""
        u = urlparse(self.path)
        if u.path.startswith('/api/') and not self._local_host_authorized():
            return False
        if not self.LAN_TOKEN:
            return True
        if u.path in ('/', '/index.html') or u.path.startswith('/assets/') or u.path.startswith('/i18n/'):
            return True
        qs = parse_qs(u.query)
        supplied = qs.get('t', [''])[0] or self.headers.get('X-ReadMD-Token', '')
        if not secrets.compare_digest(supplied, self.LAN_TOKEN):
            return False
        return self._lan_route_authorized(u.path)

    def _lan_route_authorized(self, path):
        """Restrict shared clients to the document scope and reader-only APIs."""
        # Skill imports can fetch remote archives and write the local Skill
        # registry; never expose that mutating surface through LAN sharing.
        if path.startswith('/api/skill-imports'):
            return False
        # Recent-file metadata contains local paths outside the shared root and
        # the add/remove/clear APIs mutate local state.  Keep all of it local.
        if path.startswith('/api/recent/'):
            return False
        if path in self.LAN_BLOCKED_PATHS:
            return False
        if path not in self.LAN_SCOPED_PATHS:
            return True
        root = getattr(self.server, 'shared_root', None)
        if not root:
            return False
        requested = parse_qs(urlparse(self.path).query).get('p', [''])[0]
        if not requested:
            return False
        try:
            target = os.path.realpath(unquote(requested))
        except Exception:
            return False
        return paths_within(target, root)

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
        asset = resolve_asset(APP_DIR, path, qs)
        if asset is not None:
            if asset.forbidden:
                self._send(403, 'text/plain; charset=utf-8', b'forbidden')
                return
            if asset.body is not None:
                self._send(
                    200,
                    asset.mime,
                    asset.body,
                    cache_control='public, max-age=31536000, immutable',
                )
                return
            self._send_file(asset.path, asset.mime, immutable=asset.immutable)
            return

        if path in ('/', '/index.html'):
            self._send_index()

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
        elif path == '/api/batch/extract-zip':
            self._api_batch_extract_zip()
        elif path == '/api/convert/progress':
            self._api_convert_progress(qs.get('job', [''])[0])
        elif path == '/api/convert/cancel':
            self._api_convert_cancel()
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
        elif path == '/api/skills':
            self._api_skills()
        elif path == '/api/pets':
            self._api_pets()
        elif path == '/api/pets/import':
            self._api_pet_import()
        elif path == '/api/pets/remove':
            self._api_pet_remove()
        elif path == '/api/pets/active':
            self._api_pet_active()
        elif path == '/api/pets/thumb':
            self._api_pet_thumb(qs)
        elif path == '/api/skill-imports':
            self._api_skill_imports()
        elif path == '/api/skill-imports/preview':
            self._api_skill_import_preview()
        elif path == '/api/skill-imports/apply':
            self._api_skill_import_apply()
        elif path.startswith('/api/skill-imports/'):
            self._api_skill_import_source(path)
        elif path == '/api/upstream-sources':
            self._api_upstream_sources()
        elif path.startswith('/api/upstream-sources/'):
            self._api_upstream_source_detail(path)
        elif path == '/api/share/start':
            try:
                length = int(self.headers.get('Content-Length', 0) or 0)
                body = json.loads(self.rfile.read(length).decode('utf-8')) if length else {}
                self._send_json(200, start_lan_server(body.get('current_file')))
            except Exception as e:
                logging.exception('share start failed')
                self._send_api_error(500, 'share_start_failed')
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
        elif path == '/api/diagram/capabilities':
            self._api_diagram_capabilities()
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
        elif path == '/api/control/pet-batch':
            paths = pop_pet_batch()
            self._send_json(200, {'pending': paths is not None, 'paths': paths or []})
        elif path == '/api/control/pet-menu':
            self._send_json(200, {'pending': pop_pet_menu()})
        elif path == '/api/recent/status':
            self._api_recent_status(qs)
        elif path == '/api/recent/add':
            self._api_recent_add()
        elif path == '/api/recent/clear':
            self._api_recent_clear()
        elif path == '/api/recent/remove':
            self._api_recent_remove()
        elif path == '/raw':
            p = unquote(qs.get('p', [''])[0])
            self._send_raw(p)
        else:
            self._send(404, 'text/plain; charset=utf-8', b'not found')

    @staticmethod
    def _compressible_content_type(ctype):
        return (ctype.startswith('text/') or 'javascript' in ctype or
                'json' in ctype or 'xml' in ctype)

    def _maybe_compress(self, ctype, body):
        """Use negotiated gzip for local text assets to reduce cold-start IO."""
        if (body and len(body) >= 1024 and self._compressible_content_type(ctype)
                and 'gzip' in (self.headers.get('Accept-Encoding', '') or '').lower()):
            return gzip.compress(body, compresslevel=6, mtime=0), True
        return body, False

    def _send(self, code, ctype, body, cache_control='no-cache', x_frame_options=None):
        try:
            body, compressed = self._maybe_compress(ctype, body)
            self.send_response(code)
            self.send_header('Content-Type', ctype)
            self.send_header('Content-Length', str(len(body)))
            if compressed:
                self.send_header('Content-Encoding', 'gzip')
                self.send_header('Vary', 'Accept-Encoding')
            self.send_header('Cache-Control', cache_control)
            if x_frame_options:
                self.send_header('X-Frame-Options', x_frame_options)
            self.end_headers()
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            # Browsers routinely cancel superseded polling/fetch requests while
            # navigating.  There is no peer left to receive a fallback body, so
            # treating this as an application failure only creates false errors.
            self.close_connection = True

    def _send_json(self, code, obj):
        self._send(code, 'application/json; charset=utf-8',
                   json.dumps(obj, ensure_ascii=False).encode('utf-8'))

    def _send_api_error(self, status, error_code, **extra):
        """Return a locale-neutral API failure without leaking host details.

        The UI owns wording through i18n.  Keeping exception strings out of
        responses also prevents absolute paths, provider diagnostics and local
        usernames from ending up in logs, history or screenshots.
        """
        payload = {'ok': False, 'error_code': str(error_code or 'internal_error')}
        payload.update(extra)
        self._send_json(status, payload)

    def _module_ready(self, name, message):
        """Ensure exactly one feature module is being loaded for this request."""
        if RM.is_ready(name):
            return True
        state = RM.load(name)
        st, errors = RM.status()
        state = st.get(name, state)
        if state in ('disabled', 'error'):
            logging.warning('module %s unavailable: %s', name, errors.get(name) or message)
            self._send_api_error(503, 'module_unavailable', module=name, status=state)
        else:
            # ``load`` turns an old error into a retrying loading state.
            self._send_json(409, {'ok': False, 'error_code': 'module_loading',
                                  'module': name, 'status': st.get(name, state)})
        return False

    def _api_modules_load(self):
        if self.command != 'POST':
            self._send_api_error(405, 'method_not_allowed')
            return
        try:
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length else {}
        except Exception:
            self._send_api_error(400, 'invalid_request')
            return
        name = body.get('name') if isinstance(body, dict) else None
        if name not in RM.MODULES:
            self._send_api_error(400, 'module_invalid', name=name)
            return
        state = RM.load(name)
        statuses, errors = RM.status()
        state = statuses.get(name, state)
        code = 200 if state == 'ready' else (503 if state in ('disabled', 'error') else 202)
        payload = {'ok': code == 200, 'name': name, 'status': state}
        if state in ('disabled', 'error'):
            payload['error_code'] = 'module_unavailable'
        elif state != 'ready':
            payload['error_code'] = 'module_loading'
        self._send_json(code, payload)

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
        self._send(200, 'text/html; charset=utf-8', data, 'no-store',
                   x_frame_options='DENY')

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
            logging.exception('api_update_check failed')
            self._send_api_error(500, 'update_check_failed')

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
            logging.exception('api_update_download failed')
            self._send_api_error(500, 'update_download_failed')

    def _api_update_status(self):
        try:
            from src.readmd_modules import updater
            self._send_json(200, updater.get_download_status())
        except Exception as e:
            logging.exception('api_update_status failed')
            self._send_api_error(500, 'update_status_failed', status='error')

    def _api_update_cancel(self):
        try:
            from src.readmd_modules import updater
            self._send_json(200, {'ok': updater.cancel_download()})
        except Exception as e:
            logging.exception('api_update_cancel failed')
            self._send_api_error(500, 'update_cancel_failed')

    def _api_update_apply(self):
        try:
            from src.readmd_modules import updater
            length = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(length).decode('utf-8')) if length > 0 else {}
            ok, msg = updater.apply_update(body.get('file_path'), body.get('flavor'))
            self._send_json(200 if ok else 400, {'ok': ok, 'message': msg})
        except Exception as e:
            logging.exception('api_update_apply failed')
            self._send_api_error(500, 'update_apply_failed')

    def _api_system_language(self):
        try:
            self._send_json(200, {'ok': True, 'language': get_system_language()})
        except Exception as e:
            logging.exception('api_system_language failed')
            self._send_api_error(500, 'system_language_failed')

    def _api_code_run(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            if body.get('confirm') is not True:
                self._send_json(400, {'ok': False, 'error_code': 'confirmation_required'})
                return
            lang = body.get('lang', 'python')
            code = body.get('code', '')
            cwd = body.get('cwd') or None
            timeout = int(body.get('timeout', 10))
            from src.readmd_modules import code_chunk_runner
            res = code_chunk_runner.execute_code_chunk(code=code, lang=lang, cwd=cwd, timeout=timeout)
            if not res.get('ok'):
                known = {'network_not_allowed', 'path_access_not_allowed',
                         'cwd_not_found', 'cwd_not_allowed', 'output_truncated'}
                raw_error = str(res.get('error') or '')
                res.setdefault('error_code', raw_error if raw_error in known else
                               ('execution_timeout' if '超时' in raw_error else 'execution_failed'))
                # The core runner keeps a diagnostic for local callers, but the
                # HTTP boundary must not expose tracebacks, absolute paths or
                # shell details.  Clients localize the stable error code.
                res.pop('error', None)
                res.pop('stderr', None)
            self._send_json(200, res)
        except Exception as e:
            logging.exception('api_code_run failed')
            self._send_json(500, {'ok': False, 'error_code': 'execution_failed'})

    def _api_pets(self):
        """List local Hermes-compatible pets without exposing arbitrary paths."""
        if self.command != 'GET':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed'})
            return
        try:
            from src.readmd_modules.pet import list_pets
            settings = load_json(SETTINGS_FILE, {})
            active = str(settings.get('pet_slug') or '') if isinstance(settings, dict) else ''
            self._send_json(200, {'ok': True, 'active': active,
                                  'pets': [item.as_dict() for item in list_pets(DATA_DIR)]})
        except Exception:
            logging.exception('pets list failed')
            self._send_json(500, {'ok': False, 'error_code': 'pet_list_failed'})

    def _api_pet_import(self):
        if self.command != 'POST':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed'})
            return
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            if n <= 0 or n > 24 * 1024 * 1024:
                self._send_json(413, {'ok': False, 'error_code': 'pet_spritesheet_too_large'})
                return
            body = json.loads(self.rfile.read(n).decode('utf-8'))
            if body.get('confirm') is not True:
                self._send_json(400, {'ok': False, 'error_code': 'confirmation_required'})
                return
            raw = base64.b64decode(str(body.get('image_base64') or ''), validate=True)
            from src.readmd_modules.pet import register_local_pet
            pet = register_local_pet(DATA_DIR, slug=str(body.get('slug') or ''),
                                     spritesheet=raw,
                                     display_name=str(body.get('display_name') or ''),
                                     description=str(body.get('description') or ''),
                                     replace=bool(body.get('replace')))
            self._send_json(200, {'ok': True, 'pet': pet.as_dict()})
        except binascii.Error:
            self._send_json(400, {'ok': False, 'error_code': 'pet_spritesheet_format_invalid'})
        except Exception as exc:
            code = getattr(exc, 'code', 'pet_import_failed')
            self._send_json(400, {'ok': False, 'error_code': code})

    def _api_pet_remove(self):
        if self.command not in ('POST', 'DELETE'):
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed'})
            return
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            if body.get('confirm') is not True:
                self._send_json(400, {'ok': False, 'error_code': 'confirmation_required'})
                return
            from src.readmd_modules.pet import remove_pet
            remove_pet(DATA_DIR, str(body.get('slug') or ''))
            settings = load_json(SETTINGS_FILE, {})
            if isinstance(settings, dict) and settings.get('pet_slug') == body.get('slug'):
                settings.pop('pet_slug', None)
                save_json(SETTINGS_FILE, settings)
            self._send_json(200, {'ok': True})
        except Exception as exc:
            self._send_json(400, {'ok': False, 'error_code': getattr(exc, 'code', 'pet_remove_failed')})

    def _api_pet_active(self):
        if self.command != 'POST':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed'})
            return
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            if body.get('confirm') is not True:
                self._send_json(400, {'ok': False, 'error_code': 'confirmation_required'})
                return
            slug = str(body.get('slug') or '').strip().lower()
            from src.readmd_modules.pet import list_pets
            if slug and slug not in {item.slug for item in list_pets(DATA_DIR)}:
                self._send_json(404, {'ok': False, 'error_code': 'pet_not_found'})
                return
            settings = load_json(SETTINGS_FILE, {})
            settings = settings if isinstance(settings, dict) else {}
            if slug:
                settings['pet_slug'] = slug
            else:
                settings.pop('pet_slug', None)
            save_json(SETTINGS_FILE, settings)
            self._send_json(200, {'ok': True, 'active': slug})
        except Exception as exc:
            self._send_json(400, {'ok': False, 'error_code': getattr(exc, 'code', 'pet_active_failed')})

    def _api_pet_thumb(self, qs):
        if self.command != 'GET':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed'})
            return
        try:
            slug = str(qs.get('slug', [''])[0] or '')
            from src.readmd_modules.pet import list_pets
            pet = next((item for item in list_pets(DATA_DIR) if item.slug == slug), None)
            if not pet:
                self._send_json(404, {'ok': False, 'error_code': 'pet_not_found'})
                return
            path = os.path.realpath(pet.spritesheet)
            if os.path.dirname(path) != os.path.realpath(pet.directory):
                self._send_json(403, {'ok': False, 'error_code': 'pet_path_invalid'})
                return
            mime = 'image/png' if path.lower().endswith('.png') else 'image/webp'
            with open(path, 'rb') as stream:
                body = stream.read(20 * 1024 * 1024 + 1)
            if len(body) > 20 * 1024 * 1024:
                self._send_json(413, {'ok': False, 'error_code': 'pet_spritesheet_too_large'})
                return
            self._send(200, mime, body, cache_control='private, max-age=3600')
        except OSError:
            self._send_json(404, {'ok': False, 'error_code': 'pet_not_found'})

    def _api_diagram_render(self):
        from src.readmd_modules import diagrams
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            engine = str(body.get('engine', 'mermaid') or 'mermaid').strip().lower()
            code = body.get('code', '')
            if engine in ('puml', 'plantuml'):
                # Mirror the desktop bridge: fetch the SVG server-side (which
                # honors system proxies) because the WebView CSP forbids remote
                # <img> sources, so a type:'url' reply could never display.
                if diagrams.has_local_plantuml():
                    self._send_json(200, {
                        'ok': True,
                        'type': 'svg',
                        'svg': diagrams.render_plantuml_svg(code),
                        'engine': engine,
                        'requires_network': False,
                    })
                else:
                    self._send_json(200, {
                        'ok': True,
                        'type': 'svg',
                        'svg': diagrams.fetch_plantuml_svg(code),
                        'engine': engine,
                        'requires_network': True,
                    })
            elif engine == 'tikz':
                html_out = diagrams.format_tikz_html(code)
                self._send_json(200, {'ok': True, 'type': 'html', 'html': html_out})
            elif engine in ('vega', 'vega-lite'):
                svg = diagrams.render_vega_svg(code, engine)
                self._send_json(200, {'ok': True, 'type': 'svg', 'svg': svg, 'engine': engine})
            elif engine in ('wsd', 'd2', 'ditaa'):
                self._send_json(422, {
                    'ok': False,
                    'error_code': 'diagram_engine_unavailable',
                    'engine': engine,
                })
            else:
                # Mermaid/WaveDrom/Bitfield/Viz are rendered by the browser's
                # lazy offline dispatcher.  The HTTP endpoint must not echo
                # source as a successful render: that produced a false-green
                # preview whenever the bridge was unavailable.
                self._send_json(422, {
                    'ok': False,
                    'error_code': 'diagram_client_renderer_required',
                    'engine': engine,
                })
        except diagrams.DiagramRenderError as exc:
            self._send_json(422, {'ok': False, 'error_code': exc.code})
        except Exception as e:
            logging.exception('api_diagram_render failed')
            self._send_json(500, {'ok': False, 'error_code': 'diagram_render_failed'})

    def _api_diagram_capabilities(self):
        """Expose local renderer availability without probing the network."""
        if self.command != 'GET':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed'})
            return
        try:
            from src.readmd_modules import diagrams
            self._send_json(200, {'ok': True, **diagrams.get_diagram_capabilities()})
        except Exception:
            logging.exception('api_diagram_capabilities failed')
            self._send_json(500, {'ok': False, 'error_code': 'diagram_capabilities_failed'})

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
            self._send_api_error(500, 'import_process_failed')

    def _api_export_epub(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            if body.get('confirm') is not True:
                self._send_api_error(400, 'confirmation_required')
                return
            content = body.get('content', '')
            out_path = body.get('out_path', '')
            meta = body.get('epub') or body.get('meta') or {}
            opts = body.get('options') or {}
            if isinstance(opts, dict) and 'epub' not in opts:
                opts['epub'] = meta
            from src.readmd_modules.mdexport import epub_render
            if not out_path:
                # Keep generated files in the app data export area instead of
                # leaking untracked temporary EPUBs into the system temp dir.
                out_dir = os.path.join(DATA_DIR, 'exports')
                os.makedirs(out_dir, exist_ok=True)
                out_path = os.path.join(out_dir, f'readmd_export_{int(time.time()*1000)}.epub')
            else:
                out_path = _safe_export_target(out_path, '.epub')
                if os.path.exists(out_path) and not body.get('overwrite'):
                    self._send_api_error(409, 'output_exists')
                    return
            epub_dict = opts.get('epub') if isinstance(opts.get('epub'), dict) else meta
            ok = epub_render.build_epub(
                content,
                out_path,
                title=str(epub_dict.get('title') or meta.get('title') or 'ReadMD Document'),
                author=str(epub_dict.get('author') or meta.get('author') or 'ReadMD'),
                language=str(epub_dict.get('language') or meta.get('language') or 'zh-CN'),
                options=opts or {'epub': epub_dict},
            )
            self._send_json(200, {'ok': bool(ok), 'path': out_path})
        except Exception as e:
            logging.exception('api_export_epub failed')
            raw_error = str(e)
            known = {'invalid_output_path', 'invalid_output_extension',
                     'output_directory_not_found', 'output_target_not_regular'}
            self._send_json(400 if raw_error in known else 500,
                            {'ok': False,
                             'error_code': raw_error if raw_error in known else 'export_failed',
                             })

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
            self._send_api_error(500, 'presentation_export_failed')

    def _api_style_get(self):
        try:
            from src.readmd_core import style_injector
            data = style_injector.get_custom_styles()
            self._send_json(200, {'ok': True, 'data': data})
        except Exception as e:
            logging.exception('api_style_get failed')
            self._send_api_error(500, 'style_read_failed')

    def _api_style_save(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            from src.readmd_core import style_injector
            ok = style_injector.save_custom_styles(body.get('css', ''), body.get('head', ''))
            self._send_json(200, {'ok': ok})
        except Exception as e:
            logging.exception('api_style_save failed')
            self._send_api_error(500, 'style_save_failed')

    def _api_bibtex(self, qs):
        try:
            from src.readmd_modules import bibtex
            p = unquote(qs.get('p', [''])[0])
            res = bibtex.find_and_load_bib_for_file(p)
            self._send_json(200, {'ok': True, 'citations': res})
        except Exception as e:
            logging.exception('api_bibtex failed')
            self._send_api_error(500, 'bibtex_failed')

    def _api_ping(self, qs):


        t = qs.get('t', [''])[0]
        return bool(t) and t == _read_instance().get('token', '')


    def _api_recent_status(self, qs):
        try:
            paths = None
            if self.command == 'POST':
                n = int(self.headers.get('Content-Length', 0) or 0)
                body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
                paths = body.get('paths')
            elif qs.get('p'):
                paths = [unquote(qs.get('p', [''])[0])]
            res = Api().check_recent_status(paths)
            self._send_json(200 if res.get('ok') else 400, res)
        except ValueError:
            self._send_json(400, {'ok': False, 'code': 'invalid_recent_paths'})
        except Exception as e:
            logging.exception('recent status failed')
            self._send_json(500, {'ok': False, 'code': 'recent_status_failed'})

    def _api_recent_add(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            path = body.get('path')
            if not isinstance(path, str) or not path or len(path) > Api.MAX_RECENT_PATH_LENGTH:
                raise ValueError('invalid path')
            self._send_json(200, {'ok': bool(Api().add_recent(path))})
        except ValueError:
            self._send_json(400, {'ok': False, 'code': 'invalid_recent_path'})
        except Exception:
            logging.exception('recent add failed')
            self._send_json(500, {'ok': False, 'code': 'recent_add_failed'})

    def _api_recent_clear(self):
        try:
            self._send_json(200, {'ok': bool(Api().clear_recent())})
        except Exception:
            logging.exception('recent clear failed')
            self._send_json(500, {'ok': False, 'code': 'recent_clear_failed'})

    def _api_recent_remove(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            path = body.get('path')
            if not isinstance(path, str) or not path or len(path) > Api.MAX_RECENT_PATH_LENGTH:
                raise ValueError('invalid path')
            ok = Api().remove_recent(path)
            self._send_json(200, {'ok': ok})
        except ValueError:
            self._send_json(400, {'ok': False, 'code': 'invalid_recent_path'})
        except Exception as e:
            logging.exception('recent remove failed')
            self._send_json(500, {'ok': False, 'code': 'recent_remove_failed'})

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
            self._send_json(500, {'ok': False, 'error_code': 'ai_config_failed'})

    def _api_ai_models(self):
        """拉取模型列表。

        Credentials are accepted only in a POST body (or resolved server-side
        from a configured provider).  The legacy query-string form is rejected
        so a reverse proxy, browser history, or access log cannot capture a key.
        """
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        try:
            u = urlparse(self.path)
            q = parse_qs(u.query)
            mod = RM.get('ai')
            provider = mod.find_provider(q.get('provider', [''])[0]) or {}
            if q.get('key'):
                self._send_json(400, {'error': 'API Key 不得出现在 URL，请使用 POST 请求体'})
                return
            body = {}
            if self.command == 'POST':
                n = int(self.headers.get('Content-Length', 0) or 0)
                body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
                if not isinstance(body, dict):
                    raise ValueError('请求体必须是 JSON 对象')
                provider = mod.find_provider(body.get('provider') or body.get('provider_id') or '') or provider
            credential_id = str(body.get('credential_id') or '').strip()
            if credential_id:
                if provider.get('credential_id') and provider.get('credential_id') != credential_id:
                    self._send_json(403, {'error': '凭据与提供商不匹配'})
                    return
                if not provider.get('credential_id'):
                    provider = mod.find_provider_by_credential(credential_id) or provider
            # api_key is retained only for one-version local compatibility and
            # is accepted in a POST body, never in a query string or response.
            key = str(body.get('api_key') or body.get('key') or '').strip() or mod.resolve_key(provider)
            base_url = str(body.get('base_url') or provider.get('base_url') or '').strip()
            mode = str(body.get('mode') or provider.get('mode') or q.get('mode', ['auto'])[0])
            endpoint_mode = str(body.get('endpoint_mode') or provider.get('endpoint_mode', 'prefix'))
            headers = body.get('headers') if isinstance(body.get('headers'), dict) else provider.get('headers')
            ids = mod.list_models(base_url,
                                  key,
                                  mode,
                                  endpoint_mode,
                                  headers)
            self._send_json(200, {'models': ids})
        except Exception as e:
            logging.exception('ai models failed')
            self._send_json(400, {'ok': False, 'error_code': 'model_list_failed'})

    def _api_ai_chat(self):
        """AI 对话：兼容 SSE 流式与标准 JSON 双模式返回。"""
        if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
            return
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            payload = json.loads(self.rfile.read(n).decode('utf-8'))
        except Exception:
            self._send_json(400, {'ok': False, 'error_code': 'invalid_request', 'error': '请求格式错误'})
            return

        if not isinstance(payload, dict):
            self._send_json(400, {'ok': False, 'error_code': 'invalid_request', 'error': '请求体必须是 JSON 对象'})
            return
        # Runtime AI requests must resolve through the shared Skill registry;
        # accepting an ad-hoc system prompt here would reintroduce the second
        # prompt implementation that the v2.3.8 contract removes.
        if not str(payload.get('skill_id') or '').strip():
            self._send_json(400, {'ok': False, 'error_code': 'skill_required'})
            return
        raw_stream = payload.get('stream', True)
        is_stream = raw_stream not in (False, 0, '0', 'false', 'False', 'no', 'off')
        try:
            mod = RM.get('ai')
            gen = mod.chat(payload)
            if not is_stream:
                # 非流式模式：组装并返回标准 JSON
                full_content = []
                usage_info = None
                if isinstance(gen, str):
                    full_content.append(gen)
                else:
                    for item in gen:
                        if isinstance(item, dict):
                            if 'usage' in item:
                                usage_info = item['usage']
                            elif 'error' in item:
                                self._send_json(502, {'ok': False, 'error_code': item.get('error_code') or 'provider_error'})
                                return
                        elif isinstance(item, str):
                            full_content.append(item)
                res_payload = {'ok': True, 'content': ''.join(full_content)}
                if usage_info:
                    res_payload['usage'] = usage_info
                self._send_json(200, res_payload)
                return

            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream; charset=utf-8')
            self.send_header('Cache-Control', 'no-cache')
            self.send_header('Connection', 'close')
            self.end_headers()
            self._sse({'type': 'meta', 'provider': payload.get('provider') or '',
                       'model': payload.get('model') or '', 'skill_id': payload.get('skill_id') or ''})
            if isinstance(gen, str):
                self._sse({'type': 'delta', 'delta': gen})
                self._sse({'type': 'done'})
                return
            for item in gen:
                if isinstance(item, dict):
                    if 'usage' in item:
                        self._sse({'type': 'usage', 'usage': item.get('usage') or {}})
                    elif 'error' in item:
                        self._sse({'type': 'error', 'error_code': item.get('error_code') or 'provider_error'})
                    else:
                        self._sse(item)
                else:
                    self._sse({'type': 'delta', 'delta': item})
            self._sse({'type': 'done'})
        except Exception as e:
            logging.exception('ai chat failed')
            if not is_stream:
                self._send_json(502, {'ok': False, 'error_code': 'provider_error'})
                return
            try:
                self._sse({'type': 'error', 'error_code': 'provider_error'})
                self._sse({'type': 'done'})
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
            from io import BytesIO

            from PIL import Image

            raw = _b64.b64decode(data_b64)
            if not raw:
                self._send_json(400, {'error': '图片数据为空'})
                return
            if len(raw) > 25 * 1024 * 1024:
                self._send_json(413, {'error': '图片超过 25 MB 限制'})
                return

            try:
                image = Image.open(BytesIO(raw))
                actual_format = (image.format or '').lower()
                image.verify()
            except Exception:
                self._send_json(400, {'error': '无效图片数据'})
                return
            expected_format = 'jpeg' if fmt == 'jpg' else fmt
            if actual_format != expected_format:
                self._send_json(400, {'error': '图片内容与格式不一致'})
                return

            img_dir = os.path.join(dir_path, 'images')
            os.makedirs(img_dir, exist_ok=True)
            safe_name = os.path.basename(name.replace('\\', '/'))
            if ('..' in safe_name or '..' in name
                    or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]{0,180}', safe_name)):
                name = 'img_%d_%s' % (int(time.time() * 1000), os.urandom(3).hex())
            else:
                name = safe_name.rsplit('.', 1)[0]
            filename = '%s.%s' % (name, fmt)
            img_dir = os.path.realpath(img_dir)
            target = os.path.realpath(os.path.join(img_dir, filename))
            if not paths_within(target, img_dir):
                self._send_json(403, {'error': '图片路径不受信任'})
                return
            if not name or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]{0,180}', name):
                self._send_json(400, {'error': '图片文件名无效'})
                return
            with open(target, 'wb') as f:
                f.write(raw)
            rel = os.path.join('images', os.path.basename(filename)).replace('\\', '/')
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
            elif action == 'batch_save':
                templates = body.get('templates', [])
                results = []
                for t in templates:
                    if isinstance(t, dict):
                        results.append(save_prompt(t))
                self._send_json(200, {'ok': True, 'count': len(results), 'templates': results})
            else:
                t = save_prompt(body.get('template') or {})
                self._send_json(200, {'ok': True, 'template': t})
        except Exception as e:
            logging.exception('ai prompts failed')
            self._send_json(500, {'error': '模板操作失败：%s' % e})

    def _api_skills(self):
        """Skills registry API; user writes are explicit and validated first."""
        try:
            if self.command == 'GET':
                q = parse_qs(urlparse(self.path).query)
                project_dir = q.get('project_dir', [''])[0] or None
                skill_id = q.get('id', [''])[0]
                registry = _skill_registry(project_dir)
                if skill_id:
                    skill = registry.get(skill_id)
                    if not skill:
                        self._send_json(404, {'error': 'Skill 不存在'})
                        return
                    self._send_json(200, {'skill': _public_skill(skill, include_instructions=True)})
                else:
                    payload = {'skills': load_skills(project_dir)}
                    if q.get('versions', [''])[0] == '1' and q.get('id', [''])[0]:
                        payload['versions'] = _skill_versions(q['id'][0])
                    self._send_json(200, payload)
                return
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
            if not isinstance(body, dict):
                self._send_json(400, {'error': '请求体必须是 JSON 对象'})
                return
            action = str(body.get('action') or 'validate').lower()
            if action == 'generate':
                # AI generation is intentionally draft-only.  The generated
                # document is validated in memory and never published until
                # the caller explicitly submits a separate publish request.
                provider = str(body.get('provider') or '').strip()
                credential_id = str(body.get('credential_id') or '').strip()
                request = str(body.get('request') or '').strip()
                if not provider or not request:
                    self._send_json(400, {'error': '生成 Skill 需要 provider 和 request'})
                    return
                if not self._module_ready('ai', 'AI 模块加载中，请稍候再试'):
                    return
                ai_mod = RM.get('ai')
                selected_provider = ai_mod.find_provider(provider) or {}
                if not credential_id and not ai_mod._is_local_provider(selected_provider):
                    self._send_json(400, {'error': '云端提供商必须使用 credential_id；本地服务可省略'})
                    return
                document = str(body.get('document') or '')[:120000]
                gen = ai_mod.chat({
                    'provider': provider,
                    'credential_id': credential_id,
                    'model': str(body.get('model') or ''),
                    'skill_id': 'readmd-skill-creator',
                    'skill_variables': {
                        'document': document,
                        'selection': '',
                        'request': request,
                        'language': str(body.get('language') or 'en'),
                        'context': 'ReadMD Skill workbench; return a disabled draft only.',
                        'output_format': 'SKILL.md plus readmd.skill.json',
                    },
                    'messages': [{'role': 'user', 'content': request}],
                    'stream': False,
                })
                chunks, usage = [], None
                for item in gen:
                    if isinstance(item, dict):
                        if item.get('error'):
                            raise ValueError(str(item['error']))
                        usage = item.get('usage') or usage
                    elif item:
                        chunks.append(str(item))
                generated = ''.join(chunks).strip()
                if not generated:
                    raise SkillError('AI 未生成可用 Skill 草稿')
                # The creator Skill is asked to return a portable SKILL.md;
                # accept fenced markdown but do not try to execute or persist
                # arbitrary response text.
                fenced = re.search(r'```(?:markdown|md)?\s*\n(.*?)```', generated, re.S | re.I)
                candidate = fenced.group(1).strip() if fenced else generated
                skill_id = str(body.get('id') or '').strip()
                if not skill_id:
                    name_match = re.search(r'^name:\s*([a-z0-9][a-z0-9-]{0,63})\s*$', candidate, re.M)
                    skill_id = name_match.group(1) if name_match else 'draft-skill'
                validated = validate_skill_document(skill_id, candidate, {
                    'id': skill_id, 'source': 'ai-generated-draft', 'version': 1,
                    'enabled': False, 'scripts_allowed': False,
                })
                self._send_json(200, {'ok': True, 'draft': validated, 'usage': usage, 'published': False})
                return
            if action in ('validate', 'draft'):
                result = validate_skill_document(body.get('id') or '', body.get('content') or '', body.get('metadata'))
                self._send_json(200, {'ok': True, 'skill': result, 'published': False})
                return
            if action == 'evaluate':
                skill_id = body.get('id') or ''
                content = body.get('content') or ''
                result = validate_skill_document(skill_id, content, body.get('metadata'))
                variables = body.get('variables') or {}
                rendered = result.get('instructions', '')
                for name, value in variables.items():
                    if re.fullmatch(r'(document|selection|request|language|context|output_format)', str(name)):
                        # ``value`` is user/document content.  Passing it as a
                        # replacement string makes backslashes such as
                        # ``C:\\Users`` be interpreted as regex escapes on
                        # Windows.  A callable replacement inserts it
                        # verbatim and also handles newlines safely.
                        replacement = str(value or '')
                        rendered = re.sub(
                            r'\{\{\s*' + re.escape(str(name)) + r'\s*\}\}',
                            lambda _match, replacement=replacement: replacement,
                            rendered,
                        )
                evaluation_token = _issue_skill_evaluation_token(skill_id, content)
                self._send_json(200, {'ok': True, 'skill': result, 'rendered': rendered,
                                      'published': False, 'baseline': 'no-skill baseline required',
                                      'evaluation_token': evaluation_token,
                                      'evaluation_expires_in': _SKILL_EVALUATION_TTL})
                return
            if action in ('save', 'publish'):
                if action == 'publish' and body.get('confirm') is not True:
                    self._send_json(400, {'error': '发布 Skill 需要 confirm=true'})
                    return
                skill_id = body.get('id') or ''
                content = body.get('content') or ''
                if action == 'publish' and not _consume_skill_evaluation_token(
                        body.get('evaluation_token'), skill_id, content):
                    self._send_json(400, {'ok': False, 'code': 'skill_evaluation_required',
                                          'error': '发布前必须完成当前内容的试跑评估'})
                    return
                result = save_user_skill(skill_id, content, body.get('metadata'))
                self._send_json(200, {'ok': True, 'skill': result, 'published': True})
                return
            if action in ('disable', 'enable'):
                skill_id = body.get('id') or ''
                folder = _user_skill_folder(skill_id)
                skill_file = os.path.join(folder, 'SKILL.md')
                if not os.path.isfile(skill_file):
                    self._send_json(404, {'error': '仅可管理已发布的用户 Skill'})
                    return
                meta_file = os.path.join(folder, 'readmd.skill.json')
                meta = load_json(meta_file, {}) if os.path.isfile(meta_file) else {}
                meta['id'] = skill_id
                meta['enabled'] = action == 'enable'
                meta['scripts_allowed'] = False
                save_text_atomic(meta_file, json.dumps(meta, ensure_ascii=False, indent=2) + '\n')
                self._send_json(200, {'ok': True, 'enabled': meta['enabled']})
                return
            if action == 'export':
                skill_id = body.get('id') or ''
                folder = _user_skill_folder(skill_id)
                skill_file = os.path.join(folder, 'SKILL.md')
                if not os.path.isfile(skill_file):
                    self._send_json(404, {'error': '用户 Skill 不存在'})
                    return
                meta_file = os.path.join(folder, 'readmd.skill.json')
                self._send_json(200, {'ok': True, 'id': skill_id,
                                      'content': read_text(skill_file),
                                      'metadata': load_json(meta_file, {}) if os.path.isfile(meta_file) else {}})
                return
            if action == 'rollback':
                skill_id = body.get('id') or ''
                versions = _skill_versions(skill_id)
                version = str(body.get('version') or (versions[0] if versions else ''))
                if version not in versions:
                    self._send_json(404, {'error': '没有可回退的 Skill 版本'})
                    return
                if body.get('confirm') is not True:
                    self._send_json(400, {'error': '回退 Skill 需要 confirm=true'})
                    return
                root = os.path.join(DATA_DIR, 'skills', '.versions', skill_id, version)
                content = read_text(os.path.join(root, 'SKILL.md'))
                metadata = load_json(os.path.join(root, 'readmd.skill.json'), {})
                result = save_user_skill(skill_id, content, metadata)
                self._send_json(200, {'ok': True, 'skill': result, 'version': version})
                return
            if action == 'delete':
                folder = _user_skill_folder(body.get('id') or '')
                if os.path.isdir(folder):
                    import shutil
                    shutil.rmtree(folder)
                self._send_json(200, {'ok': True})
                return
            self._send_json(400, {'error': '不支持的 Skill 操作'})
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            # A browser may close a short-lived fetch while its test/context is
            # shutting down; there is no response left to write and this is
            # not an application error.
            return
        except (SkillError, ValueError, json.JSONDecodeError):
            # Keep transport responses locale-neutral.  The UI maps stable
            # error codes to the active locale; raw server-language strings
            # must never leak into the workbench or API clients.
            self._send_json(400, {'ok': False, 'error_code': 'skill_request_invalid'})
        except Exception:
            logging.exception('skills api failed')
            self._send_json(500, {'ok': False, 'error_code': 'skill_operation_failed'})

    @staticmethod
    def _skill_import_body(handler):
        """Read a bounded JSON object for Skill import endpoints."""
        try:
            length = int(handler.headers.get('Content-Length', 0) or 0)
        except (TypeError, ValueError):
            raise _skill_import.SkillImportError('request_invalid', '请求体大小无效')
        if length < 0 or length > 2 * 1024 * 1024:
            raise _skill_import.SkillImportError('request_too_large', '请求体超过安全大小限制')
        try:
            body = json.loads(handler.rfile.read(length).decode('utf-8')) if length else {}
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise _skill_import.SkillImportError('request_invalid', '请求体必须是有效 JSON') from exc
        if not isinstance(body, dict):
            raise _skill_import.SkillImportError('request_invalid', '请求体必须是 JSON 对象')
        return body

    def _skill_import_error(self, exc, status=400):
        # Import errors may contain local paths or archive member names.  Keep
        # the response machine-readable and let the UI map the code to i18n.
        self._send_json(status, {'ok': False, 'error_code': exc.code})

    def _api_skill_import_preview(self):
        if self.command != 'POST':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed',
                                  'error': '仅支持 POST 请求'})
            return
        try:
            body = self._skill_import_body(self)
            source_type = str(body.get('source_type') or ('github' if body.get('url') else '')).strip().lower()
            source = str(body.get('source') or body.get('url') or '').strip()
            credential_id = str(body.get('credential_id') or '').strip()
            github_token = str(body.get('github_token') or '').strip()
            if not source:
                code = 'github_url_required' if source_type == 'github' else 'source_required'
                raise _skill_import.SkillImportError(code, '缺少 Skill 来源')
            created_credential = ''
            if github_token:
                if source_type != 'github':
                    raise _skill_import.SkillImportError('credential_not_allowed', '该来源不接受 GitHub 凭据')
                created_credential = 'cred:github:' + secrets.token_urlsafe(18)
                _store_credential(created_credential, github_token)
                credential_id = created_credential
            try:
                preview = _skill_import.preview_source(source_type, source, credential_id)
            except Exception:
                if created_credential:
                    _delete_credential(created_credential)
                raise
            self._send_json(200, {'ok': True, 'preview': preview,
                                  'credential_id': credential_id if created_credential else ''})
        except _skill_import.SkillImportError as exc:
            self._skill_import_error(exc)
        except Exception:
            logging.exception('skill import preview failed')
            self._send_json(500, {'ok': False, 'error_code': 'internal_error',
                                  'error': 'Skill 导入预览失败'})

    def _api_skill_import_apply(self):
        if self.command != 'POST':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed',
                                  'error': '仅支持 POST 请求'})
            return
        try:
            body = self._skill_import_body(self)
            preview = body.get('preview')
            selections = body.get('selections')
            if not isinstance(preview, dict) or not isinstance(selections, list):
                raise _skill_import.SkillImportError('request_invalid', '缺少预览或 Skill 选择列表')
            result = _skill_import.apply_source_import(
                preview, selections, str(body.get('credential_id') or '').strip(),
                confirm=body.get('confirm') is True,
            )
            self._send_json(200, result)
        except _skill_import.SkillImportError as exc:
            self._skill_import_error(exc)
        except Exception:
            logging.exception('skill import apply failed')
            self._send_json(500, {'ok': False, 'error_code': 'internal_error',
                                  'error': 'Skill 导入失败'})

    def _api_skill_imports(self):
        if self.command == 'GET':
            self._send_json(200, {'schema_version': 2, 'sources': _skill_import.list_sources()})
            return
        if self.command == 'DELETE':
            try:
                body = self._skill_import_body(self)
                source_id = str(body.get('source_id') or '').strip()
                if not source_id:
                    raise _skill_import.SkillImportError('source_required', '缺少来源 ID')
                if body.get('confirm') is not True:
                    raise _skill_import.SkillImportError('confirmation_required', '移除来源需要明确确认')
                if not _skill_import.remove_source(source_id):
                    raise _skill_import.SkillImportError('source_not_found', '来源不存在')
                self._send_json(200, {'ok': True, 'source_id': source_id, 'skills_removed': False})
            except _skill_import.SkillImportError as exc:
                self._skill_import_error(exc)
            except Exception:
                logging.exception('skill import source delete failed')
                self._send_json(500, {'ok': False, 'error_code': 'internal_error',
                                      'error': '移除 Skill 来源失败'})
            return
        self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed',
                              'error': '仅支持 GET 或 DELETE 请求'})

    def _api_skill_import_source(self, path):
        """Check or manually update one pinned Skill source."""
        rest = path[len('/api/skill-imports/'):].strip('/')
        parts = [unquote(p) for p in rest.split('/') if p]
        if len(parts) != 2 or parts[1] not in ('check', 'update'):
            self._send_json(404, {'ok': False, 'error_code': 'source_not_found',
                                  'error': 'Skill 来源不存在'})
            return
        source_id, action = parts
        source = _skill_import.find_source(source_id)
        if not source:
            self._send_json(404, {'ok': False, 'error_code': 'source_not_found',
                                  'error': 'Skill 来源不存在'})
            return
        if self.command != 'POST':
            self._send_json(405, {'ok': False, 'error_code': 'method_not_allowed',
                                  'error': '仅支持 POST 请求'})
            return
        try:
            body = self._skill_import_body(self)
            credential_id = str(body.get('credential_id') or source.get('credential_id') or '').strip()
            preview = _skill_import.preview_saved_source(source, credential_id)
            changed = _skill_import.source_preview_changed(source, preview)
            if action == 'check':
                self._send_json(200, {'ok': True, 'source_id': source_id, 'changed': changed,
                                      'current_commit': source.get('resolved_commit', ''),
                                      'preview': preview})
                return
            if body.get('confirm') is not True:
                raise _skill_import.SkillImportError('confirmation_required', '更新 Skill 来源需要明确确认')
            selections = body.get('selections')
            if not isinstance(selections, list) or not selections:
                raise _skill_import.SkillImportError('selection_required', '更新前请先选择要导入的 Skill')
            result = _skill_import.apply_source_import(preview, selections, credential_id, confirm=True)
            self._send_json(200, {'ok': True, 'source_id': source_id, 'changed': changed, **result})
        except _skill_import.SkillImportError as exc:
            self._skill_import_error(exc)
        except Exception:
            logging.exception('skill import source action failed')
            self._send_json(500, {'ok': False, 'error_code': 'internal_error',
                                  'error': 'Skill 来源检查失败'})

    def _api_upstream_sources(self):
        """List immutable, offline upstream snapshots without exposing paths."""
        if self.command != 'GET':
            self._send_api_error(405, 'method_not_allowed')
            return
        try:
            self._send_json(200, {
                'schema_version': 1,
                'offline': True,
                'sources': _upstream_sources.list_sources(),
            })
        except _upstream_sources.UpstreamSourceError as exc:
            logging.warning('upstream source list failed: %s', exc)
            self._send_api_error(503, 'upstream_source_unavailable')

    def _api_upstream_source_detail(self, path):
        """Read a manifest-allowlisted source/file by opaque IDs only."""
        if self.command != 'GET':
            self._send_api_error(405, 'method_not_allowed')
            return
        prefix = '/api/upstream-sources/'
        rest = path[len(prefix):]
        try:
            if '/files/' in rest:
                source_raw, file_raw = rest.rsplit('/files/', 1)
                source_id, file_id = unquote(source_raw).strip('/'), unquote(file_raw).strip('/')
                if not source_id or not file_id or '/' in file_id:
                    raise _upstream_sources.UpstreamSourceError('invalid upstream file id')
                self._send_json(200, _upstream_sources.get_file(source_id, file_id))
                return
            source_id = unquote(rest).strip('/')
            if source_id:
                self._send_json(200, _upstream_sources.get_source(source_id))
                return
            self._send_api_error(404, 'upstream_source_not_found')
        except _upstream_sources.UpstreamSourceError as exc:
            logging.info('upstream source detail unavailable: %s', exc)
            self._send_api_error(404, 'upstream_source_not_found')

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
            self._send_api_error(500, 'ai_history_failed')
    def _send_file(self, fp, ctype, immutable=False):
        if not os.path.isfile(fp):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        stat = os.stat(fp)
        etag = '"%x-%x-%x"' % (stat.st_ino, stat.st_size, stat.st_mtime_ns)
        cache_control = 'public, max-age=31536000, immutable' if immutable else 'no-cache'
        if self._resource_not_modified(etag, stat):
            self.send_response(304)
            self.send_header('Cache-Control', cache_control)
            self.send_header('ETag', etag)
            self.end_headers()
            return
        with open(fp, 'rb') as f:
            body = f.read()
        body, compressed = self._maybe_compress(ctype, body)
        self.send_response(200)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        if compressed:
            self.send_header('Content-Encoding', 'gzip')
            self.send_header('Vary', 'Accept-Encoding')
        self.send_header('Cache-Control', cache_control)
        self.send_header('ETag', etag)
        self.send_header('Last-Modified', formatdate(stat.st_mtime, usegmt=True))
        self.end_headers()
        self.wfile.write(body)

    def _resource_not_modified(self, etag, stat):
        none_match = self.headers.get('If-None-Match')
        if none_match is not None:
            requested = {item.strip().removeprefix('W/') for item in none_match.split(',')}
            return etag.removeprefix('W/') in requested or '*' in requested
        modified_since = self.headers.get('If-Modified-Since')
        if not modified_since:
            return False
        try:
            requested_at = parsedate_to_datetime(modified_since).timestamp()
            return int(requested_at) >= int(stat.st_mtime)
        except (TypeError, ValueError, OverflowError):
            return False

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
        ext = os.path.splitext(name)[1].lower()
        is_code = ext in CODE_CONFIG_EXTS
        code_lang = ''
        if is_code:
            try:
                from src.readmd_modules.convert import EXT_TO_LANG
                code_lang = EXT_TO_LANG.get(ext, '')
            except Exception:
                code_lang = ''

        d = {
            'path': p, 'name': name, 'dir': os.path.dirname(p),
            'mtime': st.st_mtime, 'size': st.st_size,
            'is_code': is_code,
            'code_lang': code_lang,
            'ext': ext,
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

        if is_code:
            fixes = []
            stats = {'lines': len(text.splitlines()), 'chars': len(text)}
            fixed_text = text
        else:
            fr = readmd_fix.fix_markdown(text)
            fixes = fr.fixes
            stats = fr.stats
            fixed_text = fr.text

        d.update({
            'encoding': enc,
            'content': fixed_text,
            'original': raw,
            'fixes': fixes,
            'stats': stats,
            'structured': structured,
            'is_code': is_code,
            'code_lang': code_lang,
            'ext': ext,
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
            self._send_api_error(404, 'file_not_found')
            return
        if is_win7() and os.path.splitext(p)[1].lower() not in WIN7_CONVERT_EXTS:
            self._send_api_error(415, 'unsupported_on_legacy_windows')
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
                self._send_api_error(422, 'conversion_failed', engine=engine or '')
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
            self._send_api_error(500, 'conversion_failed')

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
            self._send_api_error(500, 'conversion_failed')

    def _api_convert_batch(self):
        n = int(self.headers.get('Content-Length', 0) or 0)
        try:
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
        except Exception:
            self._send_json(400, {'ok': False, 'error_code': 'invalid_request', 'error': '请求格式错误'})
            return
        if body.get('confirm') is not True:
            self._send_json(400, {'ok': False, 'code': 'confirmation_required'})
            return
        paths = [p for p in (body.get('paths') or [])
                 if isinstance(p, str) and os.path.isfile(p)]
        # A repeated selection should represent one work item, otherwise two
        # workers could race on the same output and report a false success.
        unique_paths, seen_paths = [], set()
        for path in paths:
            key = os.path.normcase(os.path.realpath(os.path.abspath(path)))
            if key in seen_paths:
                continue
            seen_paths.add(key)
            unique_paths.append(path)
        paths = unique_paths
        if is_win7():
            paths = [p for p in paths if os.path.splitext(p)[1].lower() in WIN7_CONVERT_EXTS]
        if not paths:
            self._send_json(400, {'ok': False, 'error_code': 'no_convertible_files', 'error': '没有可转换的文件'})
            return
        if not self._module_ready('convert', '转换模块加载中，请稍候再试'):
            return
        try:
            jid = _start_convert_job(paths, bool(body.get('overwrite')))
            self._send_json(200, {'job': jid, 'total': len(paths)})
        except Exception as e:
            logging.exception('convert batch start failed')
            self._send_json(500, {'ok': False, 'error_code': 'batch_start_failed',
                                  'error': '批量转换启动失败：%s' % e})

    def _api_batch_extract_zip(self):
        try:
            ctype = self.headers.get('Content-Type', '')
            n = int(self.headers.get('Content-Length', 0) or 0)
            from src.readmd_modules.convert import extract_zip_archive
            dest_dir = os.path.join(DATA_DIR, 'temp_zip')
            if 'application/zip' in ctype or 'octet-stream' in ctype:
                data = self.rfile.read(n) if n else b''
                res = extract_zip_archive(data, base_temp_dir=dest_dir)
            else:
                body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
                zip_path = body.get('path', '')
                if not zip_path or not os.path.isfile(zip_path):
                    self._send_json(400, {'ok': False, 'error_code': 'invalid_zip_path', 'error': '无效的 ZIP 文件路径'})
                    return
                res = extract_zip_archive(zip_path, base_temp_dir=dest_dir)
            self._send_json(200, res)
        except Exception as e:
            logging.exception('api_batch_extract_zip failed')
            self._send_json(500, {'ok': False, 'error_code': 'zip_extract_failed', 'error': str(e), 'paths': [], 'skipped': 0, 'total': 0})

    def _api_convert_progress(self, jid):
        job = _CONVERT_JOBS.get(jid or '')
        if not job:
            self._send_json(404, {'ok': False, 'error_code': 'job_not_found', 'error': '任务不存在'})
            return
        self._send_json(200, {
            'job': jid, 'running': job.get('running', False),
            'finished': job.get('finished', False),
            'done': sum(1 for it in job['items'] if it.get('done')),
            'total': len(job['items']),
            'items': job['items'],
        })

    def _api_convert_cancel(self):
        n = int(self.headers.get('Content-Length', 0) or 0)
        try:
            body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}
        except Exception:
            self._send_json(400, {'ok': False, 'error_code': 'invalid_request', 'error': '请求格式错误'})
            return
        job = _CONVERT_JOBS.get((body.get('job') or '') if isinstance(body, dict) else '')
        if not job:
            self._send_json(404, {'ok': False, 'error_code': 'job_not_found', 'error': '任务不存在'})
            return
        job['cancel'] = True
        self._send_json(200, {'ok': True, 'job': job.get('id')})

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
            self._send_api_error(500, 'ocr_failed')

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
            self._send_api_error(500, 'url_fetch_failed')

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
                self._send_api_error(500, 'web_extraction_failed', code='internal_error')

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
            logging.exception('web cancel failed')
            self._send_api_error(500, 'web_cancel_failed')


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


LAN = {'server': None, 'token': None, 'shared_file': None}


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


def configure_lan_server(server, shared_file=None):
    """Bind a share server to one document directory and remove the local control token."""
    server.app_token = None
    server.shared_file = ''
    server.shared_root = ''
    if shared_file and os.path.isfile(shared_file):
        real_file = os.path.realpath(shared_file)
        server.shared_file = real_file
        server.shared_root = os.path.dirname(real_file)


def start_lan_server(shared_file=None):
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
    configure_lan_server(srv, shared_file)
    threading.Thread(target=srv.serve_forever, daemon=True, name='readmd-lan').start()
    LAN['server'] = srv
    LAN['token'] = token
    LAN['shared_file'] = srv.shared_file
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
    LAN['shared_file'] = None
    logging.info('LAN share stopped')
    return {'ok': True, 'running': False}


def start_server(port=0, host='127.0.0.1'):
    """启动本地 HTTP 服务。

    默认绑定固定控制端口（CONTROL_PORT）以支持单实例常驻；
    端口被其他程序占用时回退随机端口并禁用单实例。
    """
    if not port:
        port = CONTROL_PORT
    bind_host = str(host or '127.0.0.1').strip()
    try:
        server = ReadMDHTTPServer((bind_host, port), Handler)
    except OSError:
        try:
            server = ReadMDHTTPServer((bind_host, 0), Handler)
        except OSError:
            raise
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    return server


# ---------------------------------------------------------------- JS 桥接 API

class Api(object):
    MAX_RECENT_ENTRIES = 24
    MAX_RECENT_PATH_LENGTH = 4096
    MAX_RECENT_SCAN_ENTRIES = 512
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
        # The optional Hermes-derived pet is disabled by default. Its external
        # Electron runtime lives in user data so it never affects reader start.
        self._pet_controller = PetController()
        self._pet_queue = PetBatchQueue()
        self._pet_bridge = HermesPetBridge(DATA_DIR)
        self._pet_installer = HermesPetPluginInstaller(DATA_DIR)
        self._pet_launcher = HermesPetLauncher(
            APP_DIR, self._pet_bridge,
            adapter_dir=os.path.join(DATA_DIR, 'pet', 'hermes-adapter'),
        )
        self._pet_command_stop = threading.Event()
        self._pet_command_thread = None
        self._pet_fullscreen_thread = None

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

    def choose_skill_source(self, source_type):
        """Choose a local Skill directory or ZIP through the native dialog."""
        import webview
        if self._window is None:
            return None
        kind = str(source_type or '').strip().lower()
        try:
            if kind in ('directory', 'folder'):
                files = self._window.create_file_dialog(webview.FOLDER_DIALOG)
            elif kind in ('zip', 'archive'):
                files = self._window.create_file_dialog(
                    webview.OPEN_DIALOG,
                    file_types=('ZIP Skill bundle (*.zip)',),
                )
            else:
                return None
            return files[0] if files else None
        except Exception:
            logging.exception('choose Skill source failed: %s', kind)
            return None

    def choose_pet_plugin(self):
        """Choose an explicit optional desktop-pet package through the native dialog.

        This returns a path only; :meth:`install_pet_plugin` still requires the
        caller's separate confirmation and verifies the package manifest.
        """
        import webview
        if self._window is None:
            return None
        try:
            files = self._window.create_file_dialog(
                webview.OPEN_DIALOG,
                file_types=('ReadMD desktop pet (*.zip)',),
            )
            return files[0] if files else None
        except Exception:
            logging.exception('choose desktop pet plugin failed')
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

    def run_code_chunk(self, lang, code, cwd=None, timeout=10, confirm=False):
        """执行多语言代码块。"""
        if confirm is not True:
            return {'ok': False, 'error_code': 'confirmation_required',
                    'stdout': '', 'stderr': '', 'images': [], 'exit_code': 1}
        try:
            from src.readmd_modules import code_chunk_runner
            result = code_chunk_runner.execute_code_chunk(code=code, lang=lang, cwd=cwd, timeout=int(timeout))
            if not result.get('ok'):
                raw_error = str(result.get('error') or '')
                known = {'network_not_allowed', 'path_access_not_allowed',
                         'cwd_not_found', 'cwd_not_allowed', 'output_truncated'}
                result['error_code'] = raw_error if raw_error in known else (
                    'execution_timeout' if '超时' in raw_error else 'execution_failed')
                result.pop('error', None)
                result.pop('stderr', None)
            return result
        except Exception as e:
            logging.exception('run_code_chunk failed')
            return {'ok': False, 'error_code': 'execution_failed',
                    'stdout': '', 'stderr': '', 'images': [], 'exit_code': 1}

    def render_diagram(self, engine, code, options=None):
        """渲染专业图表。"""
        from src.readmd_modules import diagrams
        try:
            engine = str(engine or 'mermaid').strip().lower()
            if engine in ('puml', 'plantuml'):
                # Prefer an explicitly installed local PlantUML runtime.  The
                # online URL is only a transparent fallback when local Java/
                # PlantUML is unavailable; callers can inspect ``type`` and
                # ``requires_network`` instead of mistaking it for offline.
                if diagrams.has_local_plantuml():
                    return {
                        'ok': True,
                        'type': 'svg',
                        'svg': diagrams.render_plantuml_svg(code),
                        'engine': engine,
                        'requires_network': False,
                    }
                # The WebView CSP forbids remote <img> sources, so fetch the
                # SVG server-side (honors system proxies) and return markup
                # like any other server-rendered diagram.
                return {
                    'ok': True,
                    'type': 'svg',
                    'svg': diagrams.fetch_plantuml_svg(code),
                    'engine': engine,
                    'requires_network': True,
                }
            elif engine in ('wsd', 'd2', 'ditaa'):
                # No pinned, redistributable offline renderer in this release;
                # fail closed with explicit reason rather than falling through
                # to a misleading client-renderer message.
                return {'ok': False, 'error_code': 'diagram_engine_unavailable', 'engine': engine}
            elif engine == 'tikz':
                return {'ok': True, 'type': 'html', 'html': diagrams.format_tikz_html(code)}
            elif engine in ('vega', 'vega-lite'):
                return {'ok': True, 'type': 'svg', 'svg': diagrams.render_vega_svg(code, engine), 'engine': engine}
            return {'ok': False, 'error_code': 'diagram_client_renderer_required', 'engine': engine}
        except diagrams.DiagramRenderError as exc:
            return {'ok': False, 'error_code': exc.code}
        except Exception as e:
            logging.exception('render_diagram failed')
            return {'ok': False, 'error_code': 'diagram_render_failed'}

    def get_diagram_capabilities(self):
        """Return the renderer capability snapshot used by the desktop UI."""
        try:
            from src.readmd_modules import diagrams
            return {'ok': True, **diagrams.get_diagram_capabilities()}
        except Exception:
            logging.exception('get_diagram_capabilities failed')
            return {'ok': False, 'error_code': 'diagram_capabilities_failed'}

    def process_imports(self, content, base_dir='', current_file=None):
        """处理 @import 指令。"""
        try:
            from src.readmd_modules import import_processor
            res = import_processor.process_markdown_imports(content, base_dir=base_dir, current_file=current_file)
            return {'ok': True, 'content': res}
        except Exception as e:
            logging.exception('process_imports failed')
            return {'ok': False, 'error_code': 'import_process_failed', 'content': content}

    def export_epub(self, content, output_path='', meta=None, confirm=False):
        """导出 EPUB 3.0 电子书。"""
        if confirm is not True:
            return {'ok': False, 'error_code': 'confirmation_required'}
        try:
            from src.readmd_modules.mdexport import epub_render
            if not output_path:
                out_dir = os.path.join(DATA_DIR, 'exports')
                os.makedirs(out_dir, exist_ok=True)
                output_path = os.path.join(out_dir, f'readmd_export_{int(time.time()*1000)}.epub')
            else:
                output_path = _safe_export_target(output_path, '.epub')
                if os.path.exists(output_path):
                    return {'ok': False, 'error_code': 'output_exists'}
            meta_dict = meta if isinstance(meta, dict) else {}
            epub_dict = meta_dict.get('epub') if isinstance(meta_dict.get('epub'), dict) else meta_dict
            ok = epub_render.build_epub(
                content,
                output_path,
                title=str(epub_dict.get('title') or meta_dict.get('title') or 'ReadMD Document'),
                author=str(epub_dict.get('author') or meta_dict.get('author') or 'ReadMD'),
                language=str(epub_dict.get('language') or meta_dict.get('language') or 'zh-CN'),
                options=meta_dict if 'epub' in meta_dict else {'epub': epub_dict},
            )
            return {'ok': bool(ok), 'path': output_path}
        except Exception as e:
            logging.exception('export_epub failed')
            return {'ok': False, 'error_code': 'export_failed'}

    def extract_zip_batch(self, zip_path):
        """解压 ZIP 归档并返回提取出的文件列表与跳过统计。"""
        try:
            from src.readmd_modules.convert import extract_zip_archive
            dest_dir = os.path.join(DATA_DIR, 'temp_zip')
            return extract_zip_archive(zip_path, base_temp_dir=dest_dir)
        except Exception as e:
            logging.exception('extract_zip_batch failed')
            return {'ok': False, 'error_code': 'zip_extract_failed', 'error': str(e), 'paths': [], 'skipped': 0, 'total': 0}

    def export_presentation(self, content, theme='black', transition='slide', save=False):
        """生成 Reveal.js 演示文稿 HTML。

        save=False（默认）：返回 html 供应用内预览 iframe 使用（行为与旧版一致）。
        save=True：弹出保存对话框，写入自包含单文件 HTML，返回 {ok, path}。
        """
        try:
            from src.readmd_modules.mdexport import presentation_render
            if save:
                import webview
                if self._window is None:
                    return {'ok': False, 'error_code': 'window_not_ready'}
                try:
                    target = self._window.create_file_dialog(
                        webview.SAVE_DIALOG, save_filename='presentation.html',
                        file_types=('HTML 网页 (*.html)',))
                except Exception as e:
                    logging.exception('presentation save dialog failed')
                    return {'ok': False, 'error_code': 'save_dialog_failed'}
                if not target:
                    return {'ok': False, 'canceled': True}
                try:
                    target = normalize_dialog_path(target, '.html')
                except ValueError as e:
                    return {'ok': False, 'error_code': 'invalid_output_path'}
                html_out = presentation_render.generate_presentation_html(
                    content, theme=theme, transition=transition, standalone=True)
                with open(target, 'w', encoding='utf-8') as handle:
                    handle.write(html_out)
                return {'ok': True, 'path': target}
            html_out = presentation_render.generate_presentation_html(content, theme=theme, transition=transition)
            return {'ok': True, 'html': html_out}
        except Exception as e:
            logging.exception('export_presentation failed')
            return {'ok': False, 'error_code': 'presentation_export_failed'}

    def get_custom_styles(self):
        """获取自定义样式与 Head。"""
        try:
            from src.readmd_core import style_injector
            return {'ok': True, 'data': style_injector.get_custom_styles()}
        except Exception as e:
            logging.exception('get_custom_styles failed')
            return {'ok': False, 'error_code': 'style_read_failed'}

    def save_custom_styles(self, css='', head_html=''):
        """保存自定义样式与 Head。"""
        try:
            from src.readmd_core import style_injector
            ok = style_injector.save_custom_styles(css, head_html)
            return {'ok': ok}
        except Exception as e:
            logging.exception('save_custom_styles failed')
            return {'ok': False, 'error_code': 'style_save_failed'}

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
        if extension.lower() not in ALL_TEXT_EXTS:
            return {'ok': False, 'code': 'unsupported_type',
                    'error': '只能重命名 Markdown 或文本/代码文件'}
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
            url = safe_external_url(url)
        except Exception as exc:
            logging.warning('Blocked unsafe external URL: %s', exc)
            return False
        try:
            webbrowser.open(url)
            return True
        except Exception:
            return False

    def open_path(self, path):
        """用系统默认程序打开文件（如图片、PDF 或外部文档）。"""
        try:
            path = safe_file_target(path)
        except Exception as exc:
            logging.warning('Blocked unsafe file open: %s', exc)
            return False
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

    def _pet_model_status(self):
        try:
            return verify_model_bundle(PET_MODEL_DIR)
        except Exception:
            return {'ready': False, 'code': 'model_validation_failed'}

    def _pet_preferences(self):
        settings = load_json(SETTINGS_FILE, {})
        settings = settings if isinstance(settings, dict) else {}
        bounds = settings.get('pet_bounds')
        if not isinstance(bounds, dict):
            bounds = None
        try:
            # Match the copied Hermes overlay's fallback scale.  The fallback
            # asset is a 384x512 cell (not the old, incorrectly treated
            # 1536x1024 whole sheet), so 0.33 is compact while remaining easy
            # to grab on a high-DPI desktop.
            scale = round(float(settings.get('pet_scale', 0.33)), 2)
        except (TypeError, ValueError):
            scale = 0.33
        try:
            opacity = round(float(settings.get('pet_opacity', 1.0)), 2)
        except (TypeError, ValueError):
            opacity = 1.0
        renderer = settings.get('pet_renderer')
        if renderer not in ('hermes-sprite', 'live2d'):
            renderer = 'hermes-sprite'
        return {
            'bounds': bounds,
            'renderer': renderer,
            'info': {
                'scale': max(0.18, min(0.72, scale)),
                'opacity': max(0.35, min(1.0, opacity)),
            },
        }

    def _publish_pet_runtime(self, runtime=None):
        runtime = runtime if isinstance(runtime, dict) else self._pet_controller.snapshot()
        prefs = self._pet_preferences()
        return self._pet_bridge.publish(
            runtime, info=prefs['info'], bounds=prefs['bounds'],
            renderer=prefs['renderer'], fullscreen=foreground_fullscreen(),
        )

    def _record_pet_event(self, event):
        try:
            return self._publish_pet_runtime(self._pet_controller.handle_event(event))
        except ValueError:
            return self._publish_pet_runtime()

    def get_pet_runtime_status(self):
        status = self._pet_controller.snapshot()
        status['model'] = self._pet_model_status()
        status['adapter'] = self._pet_launcher.status()
        status['adapter_dir'] = os.path.join(DATA_DIR, 'pet', 'hermes-adapter')
        prefs = self._pet_preferences()
        status['preferences'] = dict(prefs['info'], renderer=prefs['renderer'])
        return status

    def list_local_pets(self):
        """Return the local Hermes-compatible gallery entries."""
        from src.readmd_modules.pet import list_pets
        settings = load_json(SETTINGS_FILE, {})
        active = str(settings.get('pet_slug') or '') if isinstance(settings, dict) else ''
        return {'ok': True, 'active': active,
                'pets': [item.as_dict() for item in list_pets(DATA_DIR)]}

    def import_local_pet(self, image_path, slug, display_name='', description='', replace=False, confirm=False):
        """Import one user-selected PNG/WebP pet into ``DATA_DIR/pets``."""
        if confirm is not True:
            return {'ok': False, 'error_code': 'confirmation_required'}
        if not isinstance(image_path, str) or len(image_path) > 32768:
            return {'ok': False, 'error_code': 'pet_source_invalid'}
        try:
            source = os.path.realpath(image_path)
            if not os.path.isfile(source):
                return {'ok': False, 'error_code': 'pet_source_not_found'}
            if os.path.getsize(source) > 20 * 1024 * 1024:
                return {'ok': False, 'error_code': 'pet_spritesheet_too_large'}
            from src.readmd_modules.pet import register_local_pet
            with open(source, 'rb') as stream:
                spritesheet = stream.read()
            pet = register_local_pet(DATA_DIR, slug=slug, spritesheet=spritesheet,
                                     display_name=display_name, description=description,
                                     replace=bool(replace))
            return {'ok': True, 'pet': pet.as_dict()}
        except Exception as exc:
            return {'ok': False, 'error_code': getattr(exc, 'code', 'pet_import_failed')}

    def remove_local_pet(self, slug, confirm=False):
        if confirm is not True:
            return {'ok': False, 'error_code': 'confirmation_required'}
        try:
            from src.readmd_modules.pet import remove_pet
            remove_pet(DATA_DIR, slug)
            settings = load_json(SETTINGS_FILE, {})
            if isinstance(settings, dict) and settings.get('pet_slug') == slug:
                settings.pop('pet_slug', None)
                save_json(SETTINGS_FILE, settings)
            return {'ok': True}
        except Exception as exc:
            return {'ok': False, 'error_code': getattr(exc, 'code', 'pet_remove_failed')}

    def set_active_pet(self, slug, confirm=False):
        if confirm is not True:
            return {'ok': False, 'error_code': 'confirmation_required'}
        try:
            from src.readmd_modules.pet import list_pets
            slug = str(slug or '').strip().lower()
            if slug and slug not in {item.slug for item in list_pets(DATA_DIR)}:
                return {'ok': False, 'error_code': 'pet_not_found'}
            self.save_settings({'pet_slug': slug} if slug else {'pet_slug': None})
            return {'ok': True, 'active': slug}
        except Exception:
            return {'ok': False, 'error_code': 'pet_active_failed'}

    def install_pet_plugin(self, archive_path, confirm=False):
        """Install an explicit local desktop-pet package into user data only."""
        if self._pet_launcher.status().get('running'):
            return {'ok': False, 'code': 'pet_plugin_stop_before_install'}
        if not isinstance(archive_path, str) or len(archive_path) > 32768:
            return {'ok': False, 'code': 'invalid_pet_plugin_archive'}
        return self._pet_installer.install_archive(archive_path, confirm=bool(confirm))

    def configure_pet(self, settings):
        if not isinstance(settings, dict):
            return {'ok': False, 'code': 'invalid_pet_settings'}
        renderer = str(settings.get('renderer') or 'hermes-sprite')
        if renderer not in ('hermes-sprite', 'live2d'):
            return {'ok': False, 'code': 'invalid_pet_renderer'}
        preference_updates = {}
        if 'scale' in settings:
            try:
                scale = round(float(settings['scale']), 2)
            except (TypeError, ValueError):
                return {'ok': False, 'code': 'invalid_pet_scale'}
            if not 0.18 <= scale <= 0.72:
                return {'ok': False, 'code': 'invalid_pet_scale'}
            preference_updates['pet_scale'] = scale
        if 'opacity' in settings:
            try:
                opacity = round(float(settings['opacity']), 2)
            except (TypeError, ValueError):
                return {'ok': False, 'code': 'invalid_pet_opacity'}
            if not 0.35 <= opacity <= 1.0:
                return {'ok': False, 'code': 'invalid_pet_opacity'}
            preference_updates['pet_opacity'] = opacity
        if 'renderer' in settings:
            preference_updates['pet_renderer'] = renderer
        if preference_updates:
            self.save_settings(preference_updates)
        if 'reduced_motion' in settings:
            self._pet_controller.set_reduced_motion(bool(settings['reduced_motion']))
        if settings.get('enabled') is False:
            runtime = self._pet_controller.disable()
            self._publish_pet_runtime(runtime)
            self._pet_command_stop.set()
            self._pet_launcher.stop()
            return {'ok': True, 'runtime': runtime}
        if settings.get('enabled') is True:
            model = self._pet_model_status()
            # Hermes's copied sprite overlay is an independent, MIT-licensed
            # fallback.  Only the optional Cubism renderer requires a verified
            # Live2D rights chain; otherwise a missing model would wrongly make
            # the already bundled Hermes plugin impossible to start.
            if renderer == 'live2d' and not model.get('ready'):
                return {'ok': False, 'code': model.get('code', 'model_not_ready')}
            # Updating a slider while the pet is already open must only
            # republish preferences. Re-enabling here would reset a working
            # animation back to idle and needlessly restart its command loop.
            if self._pet_controller.snapshot().get('enabled'):
                runtime = self._pet_controller.snapshot()
                self._publish_pet_runtime(runtime)
                return {'ok': True, 'runtime': runtime, 'renderer': renderer, 'model': model,
                        'adapter': self._pet_launcher.status()}
            launched = self._pet_launcher.start()
            if not launched.get('ok'):
                return launched
            runtime = self._pet_controller.enable()
            self._publish_pet_runtime(runtime)
            self._start_pet_command_loop()
            self._start_pet_fullscreen_loop()
            return {'ok': True, 'runtime': runtime, 'renderer': renderer, 'model': model,
                    'adapter': launched.get('runtime')}
        runtime = self._pet_controller.snapshot()
        self._publish_pet_runtime(runtime)
        return {'ok': True, 'runtime': runtime, 'adapter': self._pet_launcher.status()}

    def enqueue_pet_files(self, paths):
        if not isinstance(paths, (list, tuple)):
            return {'ok': False, 'code': 'invalid_pet_batch'}
        if len(paths) > 128 or any(not isinstance(path, str) or not path for path in paths):
            return {'ok': False, 'code': 'invalid_pet_batch'}
        return {'ok': True, 'tasks': self._pet_queue.submit(paths)}

    def get_pet_batch(self):
        self._drain_pet_command()
        return self._pet_queue.grouped_snapshot()

    def _drain_pet_command(self):
        """Consume one external-pet command without granting it shell access."""
        command = self._pet_bridge.take_command()
        if not command:
            return None
        if command.get('type') == 'open-menu':
            push_pet_menu()
            return {'type': 'open-menu', 'ok': True}
        if command.get('type') == 'bounds':
            self.save_settings({'pet_bounds': command['bounds']})
            return {'type': 'bounds', 'ok': True}
        if command.get('type') == 'scale':
            self.save_settings({'pet_scale': command['scale']})
            return {'type': 'scale', 'ok': True}
        if command.get('type') == 'clipboard':
            return self._open_pet_clipboard(command)
        if command.get('type') != 'drop':
            return command
        return self._queue_pet_drop(command.get('paths', []))

    def _queue_pet_drop(self, raw_paths):
        """Classify local file paths and request the shared confirmation UI.

        Both a physical drop and a clipboard file-list use this helper.  It
        deliberately queues rather than opening or converting anything, so a
        mixed clipboard cannot make one category bypass the user's decision.
        """
        paths = []
        for raw_path in raw_paths or ():
            try:
                path = os.path.abspath(os.fspath(raw_path))
            except (TypeError, ValueError):
                continue
            if os.path.isfile(path):
                paths.append(path)
        if not paths:
            return {'type': 'drop', 'accepted': 0}
        tasks = self._pet_queue.submit(paths)
        self._record_pet_event('work_started')
        # A pet drop is deliberately non-destructive.  The unified batch
        # workbench owns the explicit open/convert confirmation, so a stray
        # drop can never launch documents or start a conversion job by itself.
        grouped = {}
        for task in tasks:
            grouped[task['kind']] = grouped.get(task['kind'], 0) + 1
        push_pet_batch(paths)
        return {'type': 'drop', 'accepted': len(tasks), 'queued': grouped,
                'requires_confirmation': True}

    def _start_pet_command_loop(self):
        thread = self._pet_command_thread
        if thread is not None and thread.is_alive():
            return
        self._pet_command_stop.clear()

        def run():
            while not self._pet_command_stop.wait(0.25):
                try:
                    self._drain_pet_command()
                except Exception:
                    logging.exception('pet command bridge failed')

        self._pet_command_thread = threading.Thread(target=run, name='readmd-pet-bridge', daemon=True)
        self._pet_command_thread.start()

    def _start_pet_fullscreen_loop(self):
        thread = self._pet_fullscreen_thread
        if thread is not None and thread.is_alive():
            return
        self._pet_command_stop.clear()

        def run():
            last = None
            while not self._pet_command_stop.wait(2.0):
                try:
                    enabled = bool(self._pet_controller.snapshot().get('enabled'))
                except Exception:
                    continue
                if not enabled:
                    last = None
                    continue
                try:
                    current = foreground_fullscreen()
                except Exception:
                    current = False
                if current == last:
                    continue
                last = current
                try:
                    self._publish_pet_runtime()
                except Exception:
                    logging.exception('pet fullscreen publish failed')

        self._pet_fullscreen_thread = threading.Thread(target=run, name='readmd-pet-fullscreen', daemon=True)
        self._pet_fullscreen_thread.start()

    def _open_pet_clipboard(self, command):
        """Split an explicit clipboard action without discarding any type.

        A Windows clipboard can carry CF_HDROP plus text and an image at the
        same time. File entries enter the user-confirmed batch queue while the
        textual/image portion becomes one local Markdown note. Neither branch
        overwrites the other, and a bad image does not suppress valid files.
        """
        text = command.get('text') or ''
        image = command.get('image_png') or ''
        paths = command.get('paths') or []
        drop_result = self._queue_pet_drop(paths) if paths else {'type': 'drop', 'accepted': 0}
        if not text and not image:
            if drop_result.get('accepted'):
                return {'type': 'clipboard', 'accepted': drop_result['accepted'], 'batch': drop_result}
            return {'type': 'clipboard', 'accepted': 0}
        inbox = os.path.join(DATA_DIR, 'pet', 'clipboard')
        os.makedirs(inbox, exist_ok=True)
        stamp = datetime.now().strftime('%Y%m%d-%H%M%S-%f')
        content = text
        if image:
            try:
                image_path = os.path.join(inbox, 'clipboard-%s.png' % stamp)
                raw = base64.b64decode(image.encode('ascii'), validate=True)
                if not raw.startswith(b'\x89PNG\r\n\x1a\n'):
                    return {'type': 'clipboard', 'accepted': 0, 'code': 'invalid_clipboard_image'}
                with open(image_path, 'wb') as handle:
                    handle.write(raw)
                relative = os.path.basename(image_path)
                content = (content + '\n\n' if content else '') + '![' + relative + '](' + relative + ')\n'
            except (OSError, ValueError, UnicodeError, binascii.Error):
                if drop_result.get('accepted'):
                    return {
                        'type': 'clipboard', 'accepted': drop_result['accepted'],
                        'batch': drop_result, 'code': 'clipboard_image_write_failed',
                    }
                return {'type': 'clipboard', 'accepted': 0, 'code': 'clipboard_image_write_failed'}
        markdown_path = os.path.join(inbox, 'clipboard-%s.md' % stamp)
        _write_md(markdown_path, content)
        push_control(markdown_path)
        self._record_pet_event('work_succeeded')
        return {
            'type': 'clipboard', 'accepted': 1 + drop_result.get('accepted', 0),
            'path': markdown_path, 'batch': drop_result if drop_result.get('accepted') else None,
        }

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

    def remove_recent(self, path):
        """从最近打开记录中单条移除指定文件。"""
        if not path:
            return False
        rec = load_json(RECENT_FILE, [])
        try:
            target = os.path.normcase(os.path.normpath(path))
            rec = [x for x in rec if os.path.normcase(os.path.normpath(x)) != target]
        except Exception:
            rec = [x for x in rec if x != path]
        save_json(RECENT_FILE, rec)
        return True

    def check_recent_status(self, paths=None):
        """检查最近文件列表的存在状态与迁移/删除情况。"""
        if paths is None:
            paths = load_json(RECENT_FILE, [])
        if isinstance(paths, str):
            paths = [paths]
        if not isinstance(paths, (list, tuple)):
            raise ValueError('recent_paths_must_be_list')
        paths = list(paths[:self.MAX_RECENT_ENTRIES])
        results = []
        for p in paths:
            if not isinstance(p, str) or not p or len(p) > self.MAX_RECENT_PATH_LENGTH:
                raise ValueError('invalid recent path')
            name = os.path.basename(p)
            dirname = os.path.dirname(p)
            # 1. 检查原路径是否存在
            if os.path.isfile(p):
                results.append({
                    'path': p,
                    'status': 'exists',
                    'resolved_path': p,
                    'name': name,
                    'dir': dirname,
                })
                continue

            # 2. 原路径不存在，尝试有界的邻近目录探测。不要递归扫描整棵
            # 用户目录/网络盘；最近文件检查必须保持轻量且可预测。
            moved_path = None
            try:
                def find_in(directory, include_children=False):
                    if not directory or not os.path.isdir(directory):
                        return None
                    children = []
                    checked = 0
                    try:
                        with os.scandir(directory) as entries:
                            for entry in entries:
                                checked += 1
                                if checked > self.MAX_RECENT_SCAN_ENTRIES:
                                    break
                                if entry.name == name and entry.is_file(follow_symlinks=False):
                                    return entry.path
                                if include_children and entry.is_dir(follow_symlinks=False):
                                    children.append(entry.path)
                        if include_children:
                            for child in children[:64]:
                                try:
                                    with os.scandir(child) as entries:
                                        for entry in entries:
                                            if entry.name == name and entry.is_file(follow_symlinks=False):
                                                return entry.path
                                except OSError:
                                    continue
                    except OSError:
                        return None
                    return None

                # 2.1 检查同级目录与一层子目录
                moved_path = find_in(dirname, include_children=True)

                # 2.2 检查父目录的直接子项
                if not moved_path and dirname:
                    moved_path = find_in(os.path.dirname(dirname))

                # 2.3 仅检查常见目录的直接路径，不遍历其内容
                if not moved_path:
                    user_home = os.path.expanduser('~')
                    common_dirs = [
                        os.path.join(user_home, 'Desktop'),
                        os.path.join(user_home, 'Documents'),
                        os.path.join(user_home, 'Downloads'),
                    ]
                    for cd in common_dirs:
                        candidate = os.path.join(cd, name)
                        if os.path.isfile(candidate):
                            moved_path = candidate
                            break
            except Exception:
                moved_path = None

            if moved_path:
                results.append({
                    'path': p,
                    'status': 'moved',
                    'resolved_path': moved_path,
                    'name': name,
                    'dir': os.path.dirname(moved_path),
                })
            else:
                results.append({
                    'path': p,
                    'status': 'deleted',
                    'resolved_path': p,
                    'name': name,
                    'dir': dirname,
                })

        return {'ok': True, 'items': results}

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

    def toggle_native_fullscreen(self):
        """Toggle the host window's native fullscreen state when supported."""
        if self._window is None:
            return {'ok': False, 'code': 'window_not_ready', 'supported': False}
        toggle = getattr(self._window, 'toggle_fullscreen', None)
        if not callable(toggle):
            return {'ok': False, 'code': 'native_fullscreen_unavailable', 'supported': False}
        try:
            result = toggle()
            return {'ok': True, 'supported': True, 'fullscreen': bool(result) if isinstance(result, bool) else None}
        except Exception:
            logging.exception('native fullscreen toggle failed')
            return {'ok': False, 'code': 'native_fullscreen_failed', 'supported': False}

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
            # 安装器使用同一动态解析器（环境覆盖 → .env/VERSION → 内嵌版本）。
            # 不要用跨文件贪婪正则抓“or '...''”：安装器还会校验字符串中的
            # NUL 字节，旧正则会把那个校验字面量误报成版本号。
            assignment = _re.search(r"(?m)^\s*APP_VERSION\s*=\s*([^\r\n]+)", _src)
            assert assignment and '_env_or_bundle_version' in assignment.group(1), '安装器未绑定统一版本解析器'
            assert 'READMD_VERSION' in _src and 'VERSION' in _src, '安装器缺少 VERSION/环境回退链'
            safe_print('version consistency OK (%s, dynamic resolver)' % VERSION)
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
                          data=json.dumps({'paths': [_dp], 'overwrite': True,
                                           'confirm': True}).encode('utf-8'),
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
    parser.add_argument('--host', default='127.0.0.1',
                        help='绑定地址（默认仅本机；容器/局域网部署可显式使用 0.0.0.0）')
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
    parser.add_argument('--check-linux', action='store_true',
                        help='诊断 Linux / 银河麒麟 / 统信 UOS 系统环境与图形引擎兼容性')
    parser.add_argument('--check-windows', action='store_true',
                        help='诊断 Windows 系统环境与 Edge WebView2 运行时兼容性')
    parser.add_argument('--check-macos', action='store_true',
                        help='诊断 macOS 系统环境与 Cocoa WKWebView 兼容性')
    parser.add_argument('--diagnose', '--check-system', dest='diagnose', action='store_true',
                        help='全平台系统与图形引擎自适应原生运行环境综合诊断')
    args = parser.parse_args()

    if getattr(args, 'diagnose', False):
        from src.readmd_modules import system_native
        print(system_native.format_unified_report())
        return 0
    if getattr(args, 'check_linux', False):
        from src.readmd_modules import linux_native
        print(linux_native.format_diagnosis_report())
        return 0
    if getattr(args, 'check_windows', False):
        from src.readmd_modules import windows_native
        print(windows_native.format_diagnosis_report())
        return 0
    if getattr(args, 'check_macos', False):
        from src.readmd_modules import macos_native
        print(macos_native.format_diagnosis_report())
        return 0

    if args.startup_probe_json and not args.startup_probe:
        parser.error('--startup-probe-json 需要 --startup-probe')
    if args.startup_probe and args.browser:
        parser.error('--startup-probe 不能与 --browser 同时使用')
    if args.startup_probe_timeout <= 0:
        parser.error('--startup-probe-timeout 必须大于 0')

    if IS_LINUX:
        try:
            from src.readmd_modules import linux_native
            linux_native.setup_linux_env()
        except Exception:
            logging.exception('Linux compatibility setup failed')
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

    server = start_server(args.port, args.host)
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

    display_host = '127.0.0.1' if args.host in ('0.0.0.0', '::') else args.host
    url = 'http://%s:%d/' % (display_host, server.server_port)
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

    try:
        if IS_MAC:
            try:
                webview.start(gui='cocoa')
            except Exception as exc:
                if native_gui_required():
                    raise RuntimeError('macOS Cocoa WKWebView is unavailable; install the native release dependencies') from exc
                logging.warning('macOS Cocoa WKWebView start failed (%s), falling back to browser-app in development mode...', exc)
                from src.readmd_modules import macos_native
                proc = macos_native.launch_browser_app(url)
                if proc is not None:
                    try:
                        proc.wait()
                    except KeyboardInterrupt:
                        pass
                else:
                    webbrowser.open(url)
        elif IS_LINUX:
            from src.readmd_modules import linux_native
            backend_info = linux_native.probe_gui_backends()
            started = False
            if backend_info.get('gtk_webkit'):
                try:
                    webview.start(gui='gtk')
                    started = True
                except Exception as exc:
                    logging.warning('WebKitGTK start failed (%s), falling back to other backends...', exc)
            if not started and backend_info.get('qt_webengine'):
                try:
                    webview.start(gui='qt')
                    started = True
                except Exception as exc:
                    logging.warning('QtWebEngine start failed (%s), falling back to browser-app...', exc)
            if not started:
                if native_gui_required():
                    raise RuntimeError('Linux WebKitGTK/QtWebEngine is unavailable; install the native release dependencies')
                # Tier 3 & 4 Universal Fallback: Launch standalone native Browser App Mode
                logging.info('Activating universal Linux Browser App Mode fallback...')
                proc = linux_native.launch_browser_app(url)
                if proc is not None:
                    try:
                        proc.wait()
                    except KeyboardInterrupt:
                        pass
                    started = True
                else:
                    try:
                        webview.start()
                        started = True
                    except Exception:
                        webbrowser.open(url)
                        started = True
        else:
            try:
                webview.start()
            except Exception as exc:
                logging.warning('Windows WebView2 start failed (%s), falling back to Edge/Chrome App Mode...', exc)
                from src.readmd_modules import windows_native
                proc = windows_native.launch_browser_app(url)
                if proc is not None:
                    try:
                        proc.wait()
                    except KeyboardInterrupt:
                        pass
                else:
                    webbrowser.open(url)
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


def _tray_labels():
    """Resolve tray menu labels from the current locale (en/zh fallback)."""
    try:
        lang = get_system_language() or ''
    except Exception:
        lang = ''
    is_zh = lang.lower().startswith('zh')
    defaults = {
        'tray.show': '显示 ReadMD' if is_zh else 'Show ReadMD',
        'menu.open': '打开文件…' if is_zh else 'Open File…',
        'tray.quit': '退出 ReadMD' if is_zh else 'Quit ReadMD',
    }
    strings = {}
    for code in (lang, 'zh-CN' if is_zh else 'en'):
        try:
            path = os.path.join(APP_DIR, 'assets', 'i18n', f'{code}.json')
            with open(path, encoding='utf-8') as f:
                strings = json.load(f)
            break
        except Exception:
            continue
    return {key: strings.get(key) or default for key, default in defaults.items()}


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
        labels = _tray_labels()
        menu = pystray.Menu(
            pystray.MenuItem(labels['tray.show'], act_show, default=True),
            pystray.MenuItem(labels['menu.open'], act_open),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem(labels['tray.quit'], act_quit),
        )
        icon = pystray.Icon('readmd', img, 'ReadMD', menu=menu)
        icon.run_detached()
        _tray_icon['icon'] = icon
        logging.info('tray started')
        return icon
    except Exception as e:
        logging.exception('tray start failed: %s', e)
        return None


if __name__ == '__main__':
    import multiprocessing
    multiprocessing.freeze_support()
    sys.exit(main())
