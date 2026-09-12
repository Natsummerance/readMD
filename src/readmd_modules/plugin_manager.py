# -*- coding: utf-8 -*-
"""ReadMD 插件沙箱管理器。

负责管理轻量秒开内核之外的可选 AI 增强插件（Docling, EasyOCR, LaTeX, Whisper）。
所有插件代码和二进制隔离安装在用户数据目录下的 `plugins` 子目录中：
- Python 包：`DATA_DIR/plugins/site-packages`
- 便携二进制（如 ffmpeg）：`DATA_DIR/plugins/bin`
- 插件清单：`DATA_DIR/plugins/plugins.json`

设计原则：
1. 主进程启动时绝对不 import 任何插件中的重库，启动时间 0 毫秒增加。
2. 只有在具体功能被调用时，才按需将沙箱加入 `sys.path` 并懒加载。
3. 支持一键安装、卸载、启用/禁用开关、日志追踪及系统级降级。
"""

import importlib.metadata
import importlib.util
import json
import logging
import os
import re
import runpy
import shutil
import subprocess
import sys
import threading
from typing import Any, Dict, List, Optional, Sequence, Tuple
from src.readmd_core.config import DATA_DIR

PLUGINS_ROOT = os.path.join(DATA_DIR, 'plugins')
PLUGINS_DIR = PLUGINS_ROOT
PLUGINS_SITE_PACKAGES = os.path.join(PLUGINS_ROOT, 'site-packages')
PLUGINS_BIN = os.path.join(PLUGINS_ROOT, 'bin')
PLUGINS_MANIFEST = os.path.join(PLUGINS_ROOT, 'plugins.json')

_RE_PIP_PERCENT = re.compile(r'(?:(\d{1,3})%\s*\||Downloading\s+.*?(\d{1,3})%|\b(\d{1,3})%\b)')

_lock = threading.RLock()
# Installers share a target directory; frozen pip also modifies process globals.
# Keep this separate from _lock so status polling remains responsive.
_install_lock = threading.Lock()
_install_tasks: Dict[str, Dict[str, Any]] = {}
CONNECTED_PLUGINS = frozenset({'easyocr', 'rapidocr', 'rapid_table', 'whisper'})

# 插件定义清单（可扩展）
PLUGIN_SPECS: Dict[str, Dict[str, Any]] = {
    'easyocr': {
        'id': 'easyocr',
        'name_key': 'plugin.easyocr.name',
        'desc_key': 'plugin.easyocr.desc',
        'package': 'easyocr',
        'pip_args': ['easyocr'],
        'import_name': 'easyocr',
        'category': 'ocr',
        'weight': 'heavy',
        'approx_size': '~150MB',
        'cache_type': 'dir_has_files',
        'cache_paths': [os.path.join('~', '.EasyOCR', 'model')],
    },
    'pylatexenc': {
        'id': 'pylatexenc',
        'name_key': 'plugin.pylatexenc.name',
        'desc_key': 'plugin.pylatexenc.desc',
        'package': 'pylatexenc',
        'pip_args': ['pylatexenc'],
        'import_name': 'pylatexenc',
        'category': 'latex',
        'weight': 'light',
        'approx_size': '~1MB',
        'cache_type': 'installed',
        'cache_paths': [],
    },
    'rapidocr': {
        'id': 'rapidocr',
        'name_key': 'plugin.rapidocr.name',
        'desc_key': 'plugin.rapidocr.desc',
        'package': 'rapidocr_onnxruntime',
        'pip_args': ['rapidocr_onnxruntime'],
        'import_name': 'rapidocr_onnxruntime',
        'category': 'ocr',
        'weight': 'light',
        'approx_size': '~17MB',
        'cache_type': 'installed',
        'cache_paths': [],
    },
    'rapid_table': {
        'id': 'rapid_table',
        'name_key': 'plugin.rapid_table.name',
        'desc_key': 'plugin.rapid_table.desc',
        'package': 'rapid_table',
        'pip_args': ['rapid_table'],
        'import_name': 'rapid_table',
        'category': 'document',
        'weight': 'light',
        'approx_size': '~15MB',
        'cache_type': 'installed',
        'cache_paths': [],
    },
    'whisper': {
        'id': 'whisper',
        'name_key': 'plugin.whisper.name',
        'desc_key': 'plugin.whisper.desc',
        'package': 'openai-whisper',
        'pip_args': ['openai-whisper'],
        'import_name': 'whisper',
        'category': 'audio',
        'weight': 'heavy',
        'approx_size': '~150MB',
        'cache_type': 'dir_has_files',
        'cache_paths': [os.path.join('~', '.cache', 'whisper')],
    },
    'jieba': {
        'id': 'jieba',
        'name_key': 'plugin.jieba.name',
        'desc_key': 'plugin.jieba.desc',
        'package': 'jieba',
        'pip_args': ['jieba'],
        'import_name': 'jieba',
        'category': 'text',
        'weight': 'light',
        'approx_size': '~18MB',
        'cache_type': 'installed',
        'cache_paths': [],
    },
    'pygments': {
        'id': 'pygments',
        'name_key': 'plugin.pygments.name',
        'desc_key': 'plugin.pygments.desc',
        'package': 'pygments',
        'pip_args': ['pygments'],
        'import_name': 'pygments',
        'category': 'code',
        'weight': 'light',
        'approx_size': '~12MB',
        'cache_type': 'installed',
        'cache_paths': [],
    },
    'pandoc_bridge': {
        'id': 'pandoc_bridge',
        'name_key': 'plugin.pandoc_bridge.name',
        'desc_key': 'plugin.pandoc_bridge.desc',
        'package': 'pypandoc',
        'pip_args': ['pypandoc'],
        'import_name': 'pypandoc',
        'category': 'tools',
        'weight': 'light',
        'approx_size': '~5MB',
        'cache_type': 'installed',
        'cache_paths': [],
    },
}


def _ensure_dirs():
    """确保插件目录结构就绪。"""
    try:
        os.makedirs(PLUGINS_SITE_PACKAGES, exist_ok=True)
        os.makedirs(PLUGINS_BIN, exist_ok=True)
    except Exception:
        pass


def mount_sandbox():
    """将插件沙箱目录安全挂载到 sys.path（幂等操作）。"""
    _ensure_dirs()
    with _lock:
        if PLUGINS_SITE_PACKAGES not in sys.path:
            sys.path.insert(0, PLUGINS_SITE_PACKAGES)
        # 将 bin 目录追加到 PATH，使 ffmpeg 等免配环境变量直接可用
        current_path = os.environ.get('PATH', '')
        if PLUGINS_BIN not in current_path:
            os.environ['PATH'] = PLUGINS_BIN + os.path.pathsep + current_path


def _check_model_cached(pid: str) -> bool:
    """检查重型或深度学习插件的模型权重文件是否已在本地缓存（声明式元数据驱动，消除重复分支）。"""
    spec = PLUGIN_SPECS.get(pid)
    if not spec:
        return False
    cache_type = spec.get('cache_type', 'installed')
    if cache_type == 'installed':
        return is_plugin_installed(pid)

    cache_paths = [os.path.expanduser(p) for p in spec.get('cache_paths', [])]
    if cache_type == 'dirs_any':
        return any(os.path.isdir(p) for p in cache_paths)
    elif cache_type == 'dir_has_files':
        for p in cache_paths:
            if os.path.isdir(p):
                try:
                    if bool(os.listdir(p)):
                        return True
                except OSError:
                    pass
        return False
    return False


def _read_manifest_data() -> Dict[str, Any]:
    """Safely read and parse plugins.json manifest, returning empty dict on failure."""
    if not os.path.isfile(PLUGINS_MANIFEST):
        return {}
    try:
        with open(PLUGINS_MANIFEST, 'r', encoding='utf-8') as f:
            data = json.load(f)
            return {key: value for key, value in data.items() if isinstance(value, dict)} if isinstance(data, dict) else {}
    except Exception:
        return {}


def load_manifest() -> Dict[str, Dict[str, Any]]:
    """加载插件配置清单。"""
    _ensure_dirs()
    with _lock:
        data = _read_manifest_data()

        # 补齐默认状态
        result = {}
        for pid, spec in PLUGIN_SPECS.items():
            entry = data.get(pid, {})
            installed = is_plugin_installed(pid)
            task_info = _install_tasks.get(pid, {})
            result[pid] = {
                'id': pid,
                'runtime_connected': pid in CONNECTED_PLUGINS,
                'name_key': spec['name_key'],
                'desc_key': spec['desc_key'],
                'category': spec['category'],
                'weight': spec['weight'],
                'approx_size': spec['approx_size'],
                'installed': installed,
                'cached': _check_model_cached(pid),
                # 接入管线的插件安装即默认启用；轻量工具包安装后由用户开关决定。
                'enabled': (entry.get('enabled', pid in CONNECTED_PLUGINS) if installed else False),
                'uninstalled': bool(entry.get('uninstalled', False)),
                'version': entry.get('version', ''),
                'installing': task_info.get('status') == 'installing',
                'progress': task_info.get('progress', 100 if installed else 0),
                'install_error': task_info.get('error', ''),
                'install_error_code': task_info.get('error_code', ''),
                'install_error_detail': task_info.get('error_detail', ''),
                'last_log': task_info.get('last_log', ''),
            }
        # Legacy manifests and environment-provided packages may enable both.
        active_ocr = [pid for pid in ('rapidocr', 'easyocr') if result[pid]['enabled']]
        for pid in active_ocr[1:]:
            result[pid]['enabled'] = False
        return result


def save_manifest(manifest: Dict[str, Dict[str, Any]]) -> None:
    """持久化插件状态。"""
    _ensure_dirs()
    with _lock:
        to_save = {}
        for pid, info in manifest.items():
            to_save[pid] = {
                'enabled': bool(info.get('enabled', False)),
                'uninstalled': bool(info.get('uninstalled', False)),
                'version': info.get('version', ''),
            }
        tmp_file = PLUGINS_MANIFEST + '.tmp'
        try:
            with open(tmp_file, 'w', encoding='utf-8') as f:
                json.dump(to_save, f, ensure_ascii=False, indent=2)
            if os.path.isfile(PLUGINS_MANIFEST):
                os.replace(tmp_file, PLUGINS_MANIFEST)
            else:
                os.rename(tmp_file, PLUGINS_MANIFEST)
        except Exception as e:
            logging.warning('save_manifest failed: %s', e)


def is_plugin_installed(plugin_id: str) -> bool:
    """检查插件是否真正可用：沙箱内完整安装，或宿主环境自带且满足要求。"""
    spec = PLUGIN_SPECS.get(plugin_id)
    if not spec:
        return False

    _ensure_dirs()
    # 若用户在 ReadMD 中显式卸载了该插件，则返回 False
    if _read_manifest_data().get(plugin_id, {}).get('uninstalled'):
        return False

    import_name = spec['import_name']
    package = str(spec.get('package') or import_name)
    sandbox_root = os.path.normcase(PLUGINS_SITE_PACKAGES)

    # 1. 沙箱内存在包目录却没有发行版元数据 = 上次安装被中断的残骸，不算已安装
    pkg_dir = os.path.join(PLUGINS_SITE_PACKAGES, import_name)
    pkg_file = os.path.join(PLUGINS_SITE_PACKAGES, import_name + '.py')
    if os.path.isfile(pkg_file):
        return True
    if os.path.isdir(pkg_dir) and _sandbox_metadata_exists(package):
        return True

    # 2. 检查当前 Python 环境是否已经安装（支持外部环境自带，不执行顶层模块代码）
    try:
        mount_sandbox()
        spec_obj = importlib.util.find_spec(import_name)
    except Exception:
        return False
    if spec_obj is None:
        return False

    origin = os.path.normcase(str(getattr(spec_obj, 'origin', '') or ''))
    if origin.startswith(sandbox_root):
        return _sandbox_metadata_exists(package)
    return True


def is_plugin_enabled(plugin_id: str) -> bool:
    """判断插件是否处于已安装且已启用状态。"""
    if not is_plugin_installed(plugin_id):
        return False
    manifest = load_manifest()
    info = manifest.get(plugin_id, {})
    return bool(info.get('enabled', False))


def set_plugin_enabled(plugin_id: str, enabled: bool) -> bool:
    """切换插件的启用/停用状态。"""
    with _lock:
        if plugin_id not in PLUGIN_SPECS or not is_plugin_installed(plugin_id):
            return False
        manifest = _read_manifest_data()
        manifest.setdefault(plugin_id, {})['enabled'] = bool(enabled)
        if enabled:
            _disable_competing_plugins(manifest, plugin_id)
        save_manifest(manifest)
        saved = _read_manifest_data()
        return all(saved.get(pid, {}).get('enabled') == bool(info.get('enabled', False))
                   for pid, info in manifest.items())


def _disable_competing_plugins(manifest, plugin_id):
    # Table reconstruction is an enhancement, not an alternative OCR engine.
    if plugin_id in ('easyocr', 'rapidocr'):
        for other in ('easyocr', 'rapidocr'):
            if other != plugin_id:
                manifest.setdefault(other, {})['enabled'] = False


def _normalize_dist_name(name: str) -> str:
    return re.sub(r'[-_.]+', '-', name).lower()


def _dist_info_prefixes(package: str) -> Tuple[str, ...]:
    normalized = _normalize_dist_name(package)
    return (normalized + '-', normalized.replace('-', '_') + '-')


def _sandbox_metadata_exists(package: str) -> bool:
    """沙箱内是否存在该发行版的 dist-info/egg-info，即一次安装是否真正走完。"""
    prefixes = _dist_info_prefixes(package)
    try:
        entries = os.listdir(PLUGINS_SITE_PACKAGES)
    except OSError:
        return False
    return any(
        (lowered.endswith('.dist-info') or lowered.endswith('.egg-info'))
        and lowered.startswith(prefixes)
        for lowered in (entry.lower() for entry in entries)
    )


def _sandbox_artifacts(import_name: str, package: str) -> List[str]:
    """列出沙箱内属于该插件的全部工件，供卸载时逐项删除并如实汇报结果。"""
    artifacts: List[str] = []
    for candidate in (
        os.path.join(PLUGINS_SITE_PACKAGES, import_name),
        os.path.join(PLUGINS_SITE_PACKAGES, import_name + '.py'),
        os.path.join(PLUGINS_SITE_PACKAGES, import_name + '.egg-info'),
    ):
        if os.path.exists(candidate):
            artifacts.append(candidate)

    try:
        entries = sorted(os.listdir(PLUGINS_SITE_PACKAGES))
    except OSError:
        return artifacts
    prefixes = _dist_info_prefixes(package)
    for entry in entries:
        lowered = entry.lower()
        if not (lowered.endswith('.dist-info') or lowered.endswith('.egg-info')):
            continue
        if lowered.startswith(prefixes):
            artifacts.append(os.path.join(PLUGINS_SITE_PACKAGES, entry))
    return artifacts


def _requested_requirement(spec: Dict[str, Any]) -> str:
    for arg in spec.get('pip_args', []):
        if not arg.startswith('-'):
            return arg
    return ''


def _split_requirement(requirement: str) -> Tuple[str, List[Tuple[str, str]]]:
    """把 'openai-whisper>=1.0,<2' 拆成 (包名, [(操作符, 版本)])。"""
    match = re.match(r'^([A-Za-z0-9][A-Za-z0-9._-]*)\s*(.*)$', requirement.strip())
    if not match:
        return '', []
    clauses: List[Tuple[str, str]] = []
    for clause in match.group(2).split(','):
        parsed = re.match(r'^(===|==|~=|!=|<=|>=|<|>)\s*(.+)$', clause.strip())
        if parsed:
            clauses.append((parsed.group(1), parsed.group(2).strip()))
    return match.group(1), clauses


def _version_parts(raw: str) -> List[int]:
    return [int(part) for part in re.findall(r'\d+', raw)[:4]]


def _version_satisfies(installed: str, clauses: Sequence[Tuple[str, str]]) -> bool:
    """核对已装版本；操作符不认识时一律判不满足，宁可重装也不误报"已安装"。"""
    have = _version_parts(installed)
    if not have:
        return False
    for operator, wanted_raw in clauses:
        if operator not in ('==', '===', '!=', '<=', '>=', '<', '>'):
            return False
        want = _version_parts(wanted_raw)
        if not want:
            return False
        width = max(len(have), len(want))
        left = have + [0] * (width - len(have))
        right = want + [0] * (width - len(want))
        if operator in ('==', '===') and left != right:
            return False
        if operator == '!=' and left == right:
            return False
        if operator == '>=' and left < right:
            return False
        if operator == '>' and left <= right:
            return False
        if operator == '<=' and left > right:
            return False
        if operator == '<' and left >= right:
            return False
    return True


def _installed_distribution_version(*names: str) -> str:
    """只查发行版元数据取版本号：绝不 import 插件模块，避免触发重库导入。"""
    mount_sandbox()
    for name in names:
        if not name:
            continue
        try:
            return importlib.metadata.version(name)
        except Exception:
            continue
    return ''


def _environment_plugin_ready(spec: Dict[str, Any]) -> Tuple[bool, str]:
    """判断既有环境是否真的满足请求的 spec，只有真满足才允许跳过 pip 安装。

    find_spec 命中不等于装好了：被中断的安装会在沙箱里留下没有 dist-info 的
    残骸，带版本要求的 spec 也必须核对实际版本，否则会出现"显示已安装、功能
    依旧缺失"的假成功。
    """
    import_name = str(spec.get('import_name') or '')
    package = str(spec.get('package') or import_name)
    if not import_name:
        return False, ''
    try:
        mount_sandbox()
        found = importlib.util.find_spec(import_name)
    except Exception:
        return False, ''
    if found is None:
        return False, ''

    origin = os.path.normcase(str(getattr(found, 'origin', '') or ''))
    if origin.startswith(os.path.normcase(PLUGINS_SITE_PACKAGES)) and not _sandbox_metadata_exists(
        package
    ):
        return False, ''

    version = _installed_distribution_version(package, import_name)
    _name, clauses = _split_requirement(_requested_requirement(spec))
    if clauses and not _version_satisfies(version, clauses):
        return False, version
    return True, version


_PIP_STAGE_PROGRESS: Tuple[Tuple[str, int], ...] = (
    ('Collecting', 10),
    ('Downloading', 30),
    ('Using cached', 55),
    ('Building wheel', 65),
    ('Installing collected packages', 85),
    ('Successfully installed', 100),
)

# 顺序即优先级：断网时 pip 也会打印 "No matching distribution found"，
# 所以网络特征必须先于发行版特征匹配，否则离线失败会被误报成"包不存在"。
_PIP_ERROR_SIGNATURES: Tuple[Tuple[str, Tuple[str, ...]], ...] = (
    (
        'pip_permission',
        (
            'Permission denied',
            'Access is denied',
            'WinError 5',
            'Errno 13',
            'Read-only file system',
        ),
    ),
    ('pip_timeout', ('Read timed out', 'read timeout=', 'Timed out while waiting')),
    (
        'pip_network',
        (
            'Could not fetch URL',
            'Failed to establish a new connection',
            'Max retries exceeded',
            'Name or service not known',
            'getaddrhost failed',
            'Temporary failure in name resolution',
            'Connection refused',
            'Network is unreachable',
            'NewConnectionError',
            'HTTPSConnectionPool',
            'HttpConnectionPool',
            'SSLCertVerificationError',
            'CERTIFICATE_VERIFY_FAILED',
        ),
    ),
    (
        'pip_no_distribution',
        (
            'No matching distribution found',
            'No versions were found',
            'Could not find a version that satisfies the requirement',
            'metadata-generation-failed',
        ),
    ),
)


def _classify_pip_failure(detail_lines: Sequence[str]) -> str:
    """把 pip 原始输出归类成稳定 error_code，前端只认 code，不渲染散文。"""
    blob = '\n'.join(detail_lines).lower()
    for code, markers in _PIP_ERROR_SIGNATURES:
        if any(marker.lower() in blob for marker in markers):
            return code
    return 'pip_unknown'


def _progress_from_line(line: str) -> Optional[int]:
    match = _RE_PIP_PERCENT.search(line)
    if match:
        raw = match.group(1) or match.group(2) or match.group(3)
        try:
            value = int(raw)
        except (TypeError, ValueError):
            value = -1
        if 0 <= value <= 100:
            return value
    for marker, percent in _PIP_STAGE_PROGRESS:
        if line.startswith(marker):
            return percent
    return None


class _LineEmitter:
    """把 pip 写出的流按行转发给回调，用于进度解析与失败分类。"""

    encoding = 'utf-8'
    errors = 'replace'

    def __init__(self, on_line) -> None:
        self._on_line = on_line
        self._buffer = ''

    def write(self, text: str) -> int:
        self._buffer += text
        while '\n' in self._buffer:
            line, self._buffer = self._buffer.split('\n', 1)
            self._on_line(line)
        return len(text)

    def flush(self) -> None:
        if self._buffer:
            line, self._buffer = self._buffer, ''
            self._on_line(line)

    def writable(self) -> bool:
        return True

    def isatty(self) -> bool:
        return False

    def fileno(self) -> int:
        raise OSError('captured pip output has no file descriptor')


def _run_pip_subprocess(args: Sequence[str], on_line) -> int:
    process = subprocess.Popen(
        [sys.executable, '-m', 'pip', *args],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding='utf-8',
        errors='replace',
    )
    if process.stdout is not None:
        for line in process.stdout:
            on_line(line)
    return process.wait()


def _run_pip_inprocess(args: Sequence[str], on_line) -> int:
    stdout, stderr, argv = sys.stdout, sys.stderr, sys.argv
    saved_main = sys.modules.get('__main__')
    saved_env = {key: os.environ.get(key) for key in ('PIP_PROGRESS_BAR', 'NO_COLOR')}
    # 关掉进度条才有稳定的按行输出；冻结态无法分配 tty，动画进度条也无处可画。
    os.environ['PIP_PROGRESS_BAR'] = 'off'
    os.environ['NO_COLOR'] = '1'

    # 冻结态 PyInstaller 下，pip._vendor.distlib.resources 的 _finder_registry 缺少
    # 对 PyInstaller 自定义模块加载器（FrozenImporter）的映射，导致 wheel 安装控制台脚本时
    # 报 DistlibException: Unable to locate finder for 'pip._vendor.distlib'。
    # 补充映射并为 ScriptMaker.make 提供容错，确保第三方库纯 Python 导入正常就绪。
    try:
        import pip._vendor.distlib as _distlib
        import pip._vendor.distlib.resources as _distlib_res
        _loader = getattr(_distlib, '__loader__', None)
        if _loader is not None and type(_loader) not in _distlib_res._finder_registry:
            _distlib_res._finder_registry[type(_loader)] = _distlib_res.ResourceFinder
    except Exception:
        pass
    _orig_make = None
    try:
        import pip._vendor.distlib.scripts as _distlib_scripts
        _orig_make = _distlib_scripts.ScriptMaker.make
        def _safe_make(self, specification, options=None):
            try:
                return _orig_make(self, specification, options)
            except Exception:
                return []
        _distlib_scripts.ScriptMaker.make = _safe_make
    except Exception:
        pass

    sys.argv = ['pip', *args]
    emitter = _LineEmitter(on_line)
    sys.stdout = emitter
    sys.stderr = emitter
    try:
        runpy.run_module('pip', run_name='__main__', alter_sys=True)
        code = 0
    except SystemExit as exc:
        if exc.code is None:
            code = 0
        elif isinstance(exc.code, int):
            code = exc.code
        else:
            code = 1
    finally:
        try:
            emitter.flush()
        finally:
            sys.stdout, sys.stderr, sys.argv = stdout, stderr, argv
            if saved_main is not None:
                sys.modules['__main__'] = saved_main
            else:
                sys.modules.pop('__main__', None)
        if _orig_make is not None:
            _distlib_scripts.ScriptMaker.make = _orig_make
        for key, value in saved_env.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
    return code


def _run_pip(args: Sequence[str], on_line) -> int:
    """在当前线程内执行 pip：冻结态用 runpy，开发态保留子进程。"""
    if getattr(sys, 'frozen', False):
        return _run_pip_inprocess(args, on_line)
    return _run_pip_subprocess(args, on_line)


def _set_task(
    plugin_id: str,
    *,
    status: str,
    progress: int,
    last_log: str,
    error_code: str = '',
    error_detail: Sequence[str] = (),
) -> None:
    detail = '\n'.join(error_detail)
    with _lock:
        _install_tasks[plugin_id] = {
            'status': status,
            'error': error_code,
            'error_code': error_code,
            'error_detail': detail[-4000:],
            'last_log': last_log,
            'progress': progress,
        }


def _mark_installed(plugin_id: str, version: str) -> None:
    """落盘"已安装且启用"终态，并把解析到的真实版本写进清单。"""
    with _lock:
        manifest_data = _read_manifest_data()
        entry = dict(manifest_data.get(plugin_id) or {})
        entry['enabled'] = True
        entry['uninstalled'] = False
        if version:
            entry['version'] = version
        manifest_data[plugin_id] = entry
        _disable_competing_plugins(manifest_data, plugin_id)
        save_manifest(manifest_data)


def _verify_plugin_import(plugin_id: str, spec: Dict[str, Any]) -> bool:
    """An importable spec alone cannot detect missing DLLs or dependencies."""
    try:
        mount_sandbox()
        importlib.import_module(spec['import_name'])
        return True
    except Exception as error:
        _set_task(plugin_id, status='error', progress=0,
                  last_log='Plugin import failed.', error_code='pip_unknown',
                  error_detail=[str(error)])
        return False


def install_plugin_async(plugin_id: str) -> bool:
    """在后台线程中异步安装插件至沙箱目录。"""
    spec = PLUGIN_SPECS.get(plugin_id)
    if not spec:
        return False

    with _lock:
        task = _install_tasks.get(plugin_id)
        if task and task.get('status') == 'installing':
            return True  # 已经在安装中

        _set_task(
            plugin_id,
            status='installing',
            progress=0,
            last_log='Starting installation...',
        )

        # 清理 uninstalled 标记
        manifest_data = _read_manifest_data()
        if plugin_id in manifest_data and manifest_data[plugin_id].get('uninstalled'):
            manifest_data[plugin_id]['uninstalled'] = False
            save_manifest(manifest_data)

    def _worker():
        _ensure_dirs()

        ready, ready_version = _environment_plugin_ready(spec)
        if ready:
            if not _verify_plugin_import(plugin_id, spec):
                return
            _set_task(
                plugin_id,
                status='success',
                progress=100,
                last_log='Plugin ready in environment.',
            )
            _mark_installed(plugin_id, ready_version)
            logging.info(
                'Plugin %s ready from environment (version=%s)',
                plugin_id,
                ready_version or 'unknown',
            )
            return

        pip_args = [
            'install',
            '--cache-dir',
            os.path.join(PLUGINS_ROOT, 'pip-cache'),
            '--target',
            PLUGINS_SITE_PACKAGES,
            '--no-warn-script-location',
            '--prefer-binary',
            '--upgrade',
            '--timeout',
            '60',
            *spec['pip_args'],
        ]
        seen_lines: List[str] = []

        def on_line(raw: str) -> None:
            clean = raw.strip()
            if not clean:
                return
            seen_lines.append(clean)
            with _lock:
                task = _install_tasks.get(plugin_id)
                if task is None or task.get('status') != 'installing':
                    return
                task['last_log'] = clean
                percent = _progress_from_line(clean)
                if percent is not None and percent > task.get('progress', 0):
                    task['progress'] = percent

        logging.info(
            'Installing plugin %s via pip %s (in_process=%s)',
            plugin_id,
            ' '.join(pip_args),
            bool(getattr(sys, 'frozen', False)),
        )
        try:
            return_code = _run_pip(pip_args, on_line)
        except ImportError:
            _set_task(
                plugin_id,
                status='error',
                progress=0,
                last_log='pip is unavailable in this build.',
                error_code='pip_unavailable',
                error_detail=['pip module not found in this build'],
            )
            logging.error('Plugin %s install failed: pip is unavailable', plugin_id)
            return
        except Exception as exc:
            _set_task(
                plugin_id,
                status='error',
                progress=0,
                last_log=str(exc),
                error_code='pip_unknown',
                error_detail=[str(exc)],
            )
            logging.exception('Plugin %s install exception', plugin_id)
            return

        if return_code == 0:
            importlib.invalidate_caches()
            ready, installed_version = _environment_plugin_ready(spec)
            if not ready:
                _set_task(
                    plugin_id, status='error', progress=0,
                    last_log='Installed package could not be verified.',
                    error_code='pip_unknown',
                    error_detail=['pip exited successfully but the requested package is unavailable or has an incompatible version.'],
                )
                return
            if not _verify_plugin_import(plugin_id, spec):
                return
            _mark_installed(plugin_id, installed_version)
            _set_task(
                plugin_id,
                status='success',
                progress=100,
                last_log='Installation completed successfully.',
            )
            logging.info(
                'Plugin %s installed successfully (version=%s)',
                plugin_id,
                installed_version or 'unknown',
            )
            return

        tail = seen_lines[-8:] or ['Pip install failed']
        _set_task(
            plugin_id,
            status='error',
            progress=0,
            last_log=tail[-1],
            error_code=_classify_pip_failure(tail),
            error_detail=tail,
        )
        logging.warning('Plugin %s install failed with rc %d', plugin_id, return_code)

    def _serialized_worker():
        with _install_lock:
            try:
                _worker()
            except Exception as exc:
                _set_task(plugin_id, status='error', progress=0, last_log=str(exc),
                          error_code='pip_unknown', error_detail=[str(exc)])
                logging.exception('Plugin %s install exception', plugin_id)

    t = threading.Thread(target=_serialized_worker, name=f'readmd-plugin-install-{plugin_id}', daemon=True)
    t.start()
    return True


def uninstall_plugin(plugin_id: str) -> bool:
    """卸载沙箱中的插件，并在配置清单中标记为已卸载。

    返回真实结果：只要还有工件删不掉就不写"已卸载"标记，而是回传
    uninstall_locked，避免前端显示"已卸载"、后端却仍能 import 到残骸。
    """
    spec = PLUGIN_SPECS.get(plugin_id)
    if not spec:
        return False

    import_name = spec['import_name']
    package = str(spec.get('package') or import_name)
    with _lock:
        if _install_tasks.get(plugin_id, {}).get('status') == 'installing':
            return False
        failures: List[str] = []
        for artifact in _sandbox_artifacts(import_name, package):
            try:
                if os.path.isdir(artifact) and not os.path.islink(artifact):
                    shutil.rmtree(artifact)
                else:
                    os.remove(artifact)
            except OSError as exc:
                failures.append(f'{artifact}: {exc}')
                logging.warning('uninstall_plugin %s could not remove %s: %s', plugin_id, artifact, exc)

        # 从 sys.modules 清理已缓存模块
        for mod_name in list(sys.modules.keys()):
            if mod_name == import_name or mod_name.startswith(import_name + '.'):
                sys.modules.pop(mod_name, None)
        importlib.invalidate_caches()

        if failures:
            _set_task(
                plugin_id,
                status='error',
                progress=0,
                last_log=failures[0],
                error_code='uninstall_locked',
                error_detail=failures,
            )
            return False

        # 更新清单标记 uninstalled=True, enabled=False
        manifest_data = _read_manifest_data()
        if plugin_id not in manifest_data:
            manifest_data[plugin_id] = {}
        manifest_data[plugin_id]['uninstalled'] = True
        manifest_data[plugin_id]['enabled'] = False
        save_manifest(manifest_data)

        # 卸载即清空安装进度与错误态，卡片回到未安装
        _install_tasks.pop(plugin_id, None)
    return True


def get_ffmpeg_path() -> Optional[str]:
    """探测系统环境变量或沙箱中的 ffmpeg 路径。"""
    mount_sandbox()
    # 1. 优先检查沙箱 bin 目录中的便携可执行文件
    bin_name = 'ffmpeg.exe' if sys.platform == 'win32' else 'ffmpeg'
    sandbox_ffmpeg = os.path.join(PLUGINS_BIN, bin_name)
    if os.path.isfile(sandbox_ffmpeg) and os.access(sandbox_ffmpeg, os.X_OK):
        return sandbox_ffmpeg

    # 2. 检查系统 PATH
    found = shutil.which('ffmpeg')
    if found:
        return found
    return None
