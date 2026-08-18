# -*- coding: utf-8 -*-
"""ReadMD 安装器 —— 苹果风动画安装/卸载程序（纯本地，无需管理员权限）。

用法：
    ReadMDSetup.exe                    图形安装（未安装时）/ 升级（已安装时）
    ReadMDSetup.exe --uninstall        图形卸载
    ReadMDSetup.exe --install-silent [目录]   静默安装（脚本 / 自测用）
    ReadMDSetup.exe --uninstall-silent        静默卸载（脚本 / 自测用）
    ReadMDSetup.exe --version
"""

import argparse
import ctypes
import json
import mimetypes
import os
import secrets
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import uuid
try:
    import winreg
except ImportError:
    class _MockWinreg:
        REG_SZ = 1
        REG_EXPAND_SZ = 2
        REG_DWORD = 4
        HKEY_CURRENT_USER = None
        HKEY_LOCAL_MACHINE = None
        KEY_SET_VALUE = 0
    winreg = _MockWinreg()
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


from urllib.parse import urlparse


# PyInstaller --splash 启动画面不会自动关闭，且无关闭按钮、置顶显示。
# v2.0.0 曾因未调用 pyi_splash.close() 导致黑屏弹窗卡死安装界面，
# 这里在 Python 启动后立即关闭，作为防御。仅当 bootloader 注入了
# _PYI_SPLASH_IPC 环境变量（即构建时启用了 --splash）才导入，
# 避免在无 splash 的构建里触发 pyi_splash 模块自身的报错。
try:
    if os.environ.get('_PYI_SPLASH_IPC'):
        import pyi_splash  # type: ignore
        pyi_splash.close()
except Exception:
    pass

APP_NAME = 'ReadMD'
APP_EXE = 'ReadMD.exe'
UNINST_EXE = 'ReadMDUninstall.exe'
def _bundle_version():
    """frozen 安装器内嵌 version.txt（Win7 链：2.1.1 Beta）。"""
    try:
        if getattr(sys, '_MEIPASS', None):
            p = os.path.join(sys._MEIPASS, 'version.txt')
            if os.path.isfile(p):
                v = open(p, encoding='utf-8').read().strip()
                if v:
                    return v
    except Exception:
        pass
    return None


APP_VERSION = (os.environ.get('READMD_VERSION_OVERRIDE')
               or _bundle_version() or '2.2.8')



PUBLISHER = 'Natsummerance'
PROG_ID = 'ReadMD.markdown'
EXTENSIONS = ['.md', '.markdown', '.mdown', '.mkd']
RELEASE_URL = 'https://github.com/Natsummerance/readMD/releases'

INSTALL_STEPS = [
    ('prepare', '准备安装目录'),
    ('copy', '复制程序文件'),
    ('runtime', '安装 WebView2 运行时'),
    ('assoc', '注册文件关联'),
    ('shortcut', '创建快捷方式'),
    ('uninst', '写入卸载信息'),
]
UNINSTALL_STEPS = [
    ('stop', '关闭运行中的 ReadMD'),
    ('assoc', '移除文件关联'),
    ('shortcut', '删除快捷方式'),
    ('uninst', '移除卸载信息'),
    ('delete', '清理安装目录'),
]
UNINSTALL_KEY = r'Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD'
ELEVATION_SCHEMA = 1
ELEVATION_TTL_SECONDS = 300


class InstallError(RuntimeError):
    """An install failure which is safe to expose to the installer UI."""

    def __init__(self, code, message, path='', actions=None):
        super().__init__(message)
        self.code = code
        self.path = path or ''
        self.actions = list(actions or [])

    def as_dict(self):
        return {'ok': False, 'code': self.code, 'message': str(self),
                'path': self.path, 'actions': self.actions}


def _result(ok=True, code='ok', path='', message='', actions=None, **extra):
    result = {'ok': ok, 'code': code, 'path': path, 'message': message,
              'actions': list(actions or [])}
    result.update(extra)
    return result


def resource_path(name):
    base = getattr(sys, '_MEIPASS', None) or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    return os.path.join(base, name)


def asset_root():
    if getattr(sys, '_MEIPASS', None):
        return sys._MEIPASS
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def app_data_dir():
    return os.path.join(os.environ.get('APPDATA') or os.path.expanduser('~'), 'ReadMD')


def default_install_dir():
    base = os.environ.get('LOCALAPPDATA') or os.path.expanduser('~')
    return os.path.join(base, 'Programs', 'ReadMD')


WEBVIEW2_CLIENT_GUID = r'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'


def is_win7():
    """Win7 检测：决定是否内置 / 默认安装固定版 WebView2 109 运行时。"""
    if os.environ.get('READMD_FORCE_WIN7') == '1':
        return True
    try:
        import platform
        return platform.system() == 'Windows' and platform.release() == '7'
    except Exception:
        return False


def system_webview2_installed():
    """系统级 Evergreen WebView2 运行时是否已安装（HKCU / HKLM）。"""
    for root in (winreg.HKEY_CURRENT_USER, winreg.HKEY_LOCAL_MACHINE):
        try:
            with winreg.OpenKey(root, WEBVIEW2_CLIENT_GUID) as k:
                v, _ = winreg.QueryValueEx(k, 'pv')
                if v:
                    return True
        except OSError:
            pass
    return False


def bundled_webview2_runtime_dir():
    """内置固定版 WebView2 运行时目录（含 msedgewebview2.exe）。

    候选顺序：安装器自身 _MEIPASS 内嵌 → 与可执行文件同目录（卸载器位于
    安装目录，与 webview2_runtime 平级）→ 源码目录 installer\webview2_runtime。
    """
    cands = []
    if getattr(sys, '_MEIPASS', None):
        cands.append(os.path.join(sys._MEIPASS, 'webview2_runtime'))
    if getattr(sys, 'frozen', False):
        cands.append(os.path.join(os.path.dirname(os.path.abspath(sys.executable)), 'webview2_runtime'))
    cands.append(os.path.join(asset_root(), 'installer', 'webview2_runtime'))
    for cand in cands:
        if cand and os.path.isdir(cand) and os.path.isfile(os.path.join(cand, 'msedgewebview2.exe')):
            return cand
    return None


def is_uninstaller():
    if getattr(sys, 'frozen', False):
        name = os.path.basename(sys.executable).lower()
    else:
        name = os.path.basename(sys.argv[0]).lower()
    return name.startswith('readmduninstall')


def bundled_exe(name):
    """内置可执行文件；Win7 构建的卸载器以 -win7 后缀命名，一并兜底。"""
    names = [name]
    if name.lower().endswith('.exe'):
        names.append(name[:-4] + '-win7.exe')
    if getattr(sys, '_MEIPASS', None):
        for n in names:
            p = os.path.join(sys._MEIPASS, n)
            if os.path.isfile(p):
                return p
    for n in names:
        for cand in (resource_path(n), os.path.join(asset_root(), 'dist', n)):
            if os.path.isfile(cand):
                return cand
    return None


def bundled_app_dir():
    """onedir 应用目录（ReadMD.exe + _internal）：优先安装器内嵌，回退本地 dist。"""
    for cand in (
        os.path.join(sys._MEIPASS, 'ReadMD') if getattr(sys, '_MEIPASS', None) else None,
        os.path.join(asset_root(), 'dist', 'ReadMD'),
    ):
        if cand and os.path.isfile(os.path.join(cand, APP_EXE)):
            return cand
    return None


def detect_install_dir():
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY) as k:
            loc, _ = winreg.QueryValueEx(k, 'InstallLocation')
        if loc and os.path.isdir(loc):
            return loc
    except OSError:
        pass
    d = default_install_dir()
    if os.path.isfile(os.path.join(d, APP_EXE)):
        return d
    return None


def is_installed():
    inst = detect_install_dir()
    if not inst or not os.path.isfile(os.path.join(inst, APP_EXE)):
        return None
    ver = '?'
    try:
        with open(os.path.join(inst, 'install.json'), encoding='utf-8') as f:
            ver = json.load(f).get('version', '?')
    except Exception:
        # 旧版（如 v1.4）可能没有 install.json：回退读注册表 DisplayVersion
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY) as k:
                ver, _ = winreg.QueryValueEx(k, 'DisplayVersion')
        except OSError:
            pass
    return {'dir': inst, 'version': ver}


def app_running():
    try:
        r = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq %s' % APP_EXE],
                           capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
        return APP_EXE.lower() in r.stdout.decode('utf-8', 'replace').lower()
    except Exception:
        return False


def stop_app():
    try:
        subprocess.run(['taskkill', '/IM', APP_EXE, '/F'],
                       capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
        time.sleep(0.6)
    except Exception:
        pass


# ---------------------------------------------------------------- 安装前检查 / 提权
def _normal_install_dir(value):
    """Return an absolute installation directory, never a volume root."""
    if not isinstance(value, str) or not value.strip() or '\x00' in value:
        raise InstallError('invalid_dir', '安装目录无效', str(value or ''))
    path = os.path.abspath(os.path.normpath(os.path.expandvars(os.path.expanduser(value.strip()))))
    drive, tail = os.path.splitdrive(path)
    if path == os.path.dirname(path) or tail in ('', os.sep, '/', '\\'):
        raise InstallError('invalid_dir', '不能将程序安装到磁盘根目录', path)
    return path


def _is_child(path, parent):
    try:
        return os.path.commonpath([os.path.abspath(path), os.path.abspath(parent)]) == os.path.abspath(parent)
    except ValueError:
        return False


def _is_admin():
    if os.name != 'nt':
        return True
    try:
        return bool(ctypes.windll.shell32.IsUserAnAdmin())
    except Exception:
        return False


def _protected_install_path(path):
    """System locations that normally require elevation. LocalAppData stays user-writable."""
    path = os.path.normcase(os.path.abspath(path))
    local = os.environ.get('LOCALAPPDATA')
    if local and _is_child(path, local):
        return False
    protected = [os.environ.get('ProgramFiles'), os.environ.get('ProgramFiles(x86)'),
                 os.environ.get('WINDIR'), os.environ.get('SystemRoot')]
    return any(root and _is_child(path, root) for root in protected)


def _bundle_size():
    """Conservative source-size estimate used by the disk-space preflight."""
    root = bundled_app_dir()
    if root:
        try:
            return sum(os.path.getsize(os.path.join(base, f))
                       for base, _dirs, files in os.walk(root) for f in files)
        except OSError:
            pass
    exe = bundled_exe(APP_EXE)
    try:
        return os.path.getsize(exe) if exe else 110 * 1024 * 1024
    except OSError:
        return 110 * 1024 * 1024


def preflight_install(directory, options=None, *, disk_usage=shutil.disk_usage,
                      running_check=app_running, admin_check=_is_admin,
                      write_probe=True):
    """Check an install target without changing an existing ReadMD installation.

    The returned dictionary is deliberately stable for the web UI and for callers
    which need to decide whether to retry, close ReadMD, or request elevation.
    """
    options = options or {}
    try:
        path = _normal_install_dir(directory)
    except InstallError as exc:
        return exc.as_dict()
    parent = os.path.dirname(path)
    if not _is_child(path, parent):  # defensive, also protects future path changes
        return _result(False, 'invalid_dir', path, '安装目录不在其父目录中')
    if _protected_install_path(path) and not admin_check():
        return _result(False, 'requires_admin', path, '此目录受 Windows 保护，需要管理员权限。',
                       ['elevate', 'change_dir'])
    try:
        os.makedirs(parent, exist_ok=True)
    except PermissionError:
        return _result(False, 'permission_denied', parent, '无法创建或写入安装目录。',
                       ['elevate', 'change_dir'])
    except OSError as exc:
        return _result(False, 'invalid_dir', parent, '无法使用安装目录：%s' % exc, ['change_dir'])
    try:
        free = disk_usage(parent).free
        required = _bundle_size() + 32 * 1024 * 1024
        if options.get('webview2'):
            required += 160 * 1024 * 1024
        if free < required:
            return _result(False, 'no_space', path, '可用空间不足。', ['change_dir'],
                           free_bytes=free, required_bytes=required)
    except OSError:
        # A network filesystem may not report space; the real write probe below is
        # still authoritative.
        pass
    if write_probe:
        probe = None
        try:
            probe = os.path.join(parent, '.readmd-write-probe-%s' % uuid.uuid4().hex)
            with open(probe, 'xb') as f:
                f.write(b'ReadMD')
            os.remove(probe)
        except PermissionError:
            return _result(False, 'permission_denied', parent, '目录不可写。', ['elevate', 'change_dir'])
        except OSError as exc:
            return _result(False, 'permission_denied', parent, '目录写入测试失败：%s' % exc,
                           ['elevate', 'change_dir'])
        finally:
            if probe and os.path.isfile(probe):
                try:
                    os.remove(probe)
                except OSError:
                    pass
    old = None
    if os.path.exists(path) and not os.path.isdir(path):
        return _result(False, 'invalid_dir', path, '安装位置已被同名文件占用。', ['change_dir'])
    if os.path.isdir(path):
        try:
            with open(os.path.join(path, 'install.json'), encoding='utf-8') as f:
                old = json.load(f).get('version')
        except Exception:
            old = 'unknown' if os.path.isfile(os.path.join(path, APP_EXE)) else None
    running = bool(running_check())
    if running and not options.get('force'):
        return _result(False, 'file_in_use', path, 'ReadMD 正在运行，请关闭后再升级。',
                       ['close_app_retry', 'change_dir'], old_version=old, running=True)
    return _result(True, 'ok', path, '可以安装。', ['install'], old_version=old, running=running)


def _elevation_payload_dir(temp_dir=None):
    return os.path.abspath(temp_dir or tempfile.gettempdir())


def create_elevation_payload(options, *, temp_dir=None, now=None, token=None):
    """Create a short-lived, single-use handoff file for UAC relaunch."""
    now = time.time() if now is None else now
    target = _normal_install_dir((options or {}).get('dir', ''))
    directory = _elevation_payload_dir(temp_dir)
    os.makedirs(directory, exist_ok=True)
    token = token or secrets.token_hex(16)  # 128 bits; passed only to the child process.
    payload = {'schema': ELEVATION_SCHEMA, 'token': token, 'expires_at': now + ELEVATION_TTL_SECONDS,
               'target_dir': target, 'options': dict(options or {}, dir=target)}
    path = os.path.join(directory, 'readmd-elevation-%s.json' % uuid.uuid4().hex)
    # x prevents an attacker from substituting a pre-existing filename.
    with open(path, 'x', encoding='utf-8') as f:
        json.dump(payload, f, ensure_ascii=False)
    return path, token


def _payload_owner_is_current_user(path):
    """Validate the payload owner. UAC keeps the same user SID in normal use."""
    if os.name != 'nt':
        try:
            return os.stat(path).st_uid == os.getuid()
        except (AttributeError, OSError):
            return False
    try:
        if os.path.islink(path):
            return False
        script = ('$acl = Get-Acl -LiteralPath $args[0]; '
                  '$me = [Security.Principal.WindowsIdentity]::GetCurrent().Name; '
                  'Write-Output $acl.Owner; Write-Output $me')
        result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', script, path],
                                capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW, timeout=5)
        owners = result.stdout.decode('utf-8', 'replace').splitlines()
        return result.returncode == 0 and len(owners) >= 2 and owners[0].strip().lower() == owners[1].strip().lower()
    except (OSError, subprocess.SubprocessError):
        return False


def consume_elevation_payload(path, token, *, temp_dir=None, now=None, owner_check=_payload_owner_is_current_user):
    """Validate then delete an elevated handoff file before using its contents."""
    expected_dir = _elevation_payload_dir(temp_dir)
    real = os.path.abspath(path or '')
    if not _is_child(real, expected_dir) or os.path.dirname(real) != expected_dir:
        raise InstallError('invalid_elevation_payload', '提权请求来源无效。', real)
    if not owner_check(real):
        raise InstallError('invalid_elevation_payload', '提权请求所有者无效。', real)
    try:
        with open(real, encoding='utf-8') as f:
            payload = json.load(f)
    except (OSError, ValueError) as exc:
        raise InstallError('invalid_elevation_payload', '提权请求无法读取。', real) from exc
    finally:
        # Single use even if validation fails; never leave a reusable command.
        try:
            os.remove(real)
        except OSError:
            pass
    now = time.time() if now is None else now
    if (not isinstance(payload, dict) or payload.get('schema') != ELEVATION_SCHEMA
            or not secrets.compare_digest(str(payload.get('token', '')), str(token or ''))
            or not isinstance(payload.get('expires_at'), (int, float))
            or payload['expires_at'] < now):
        raise InstallError('invalid_elevation_payload', '提权请求已失效或被篡改。', real)
    options = payload.get('options')
    if not isinstance(options, dict) or payload.get('target_dir') != options.get('dir'):
        raise InstallError('invalid_elevation_payload', '提权请求目标目录无效。', real)
    options['dir'] = _normal_install_dir(options['dir'])
    return options


def request_elevation(options, *, payload_writer=create_elevation_payload, shell_execute=None):
    """Restart this installer via ShellExecuteW(runas) using a one-shot payload."""
    path, token = payload_writer(options)
    if shell_execute is None:
        if os.name != 'nt':
            return _result(False, 'requires_admin', options.get('dir', ''), '当前系统不支持 UAC。')
        shell_execute = ctypes.windll.shell32.ShellExecuteW
    executable = sys.executable
    prefix = '' if getattr(sys, 'frozen', False) else '"%s" ' % os.path.abspath(__file__)
    args = '%s--elevated-payload "%s" --elevation-token "%s"' % (prefix, path, token)
    try:
        result = shell_execute(None, 'runas', executable, args, None, 1)
    except Exception as exc:
        try:
            os.remove(path)
        except OSError:
            pass
        return _result(False, 'elevation_failed', options.get('dir', ''), '无法请求管理员权限：%s' % exc)
    if isinstance(result, int) and result <= 32:
        try:
            os.remove(path)
        except OSError:
            pass
        return _result(False, 'elevation_failed', options.get('dir', ''), '管理员权限请求被取消或失败。')
    return _result(True, 'elevation_started', options.get('dir', ''), '已请求管理员权限。', ['close'])


# ---------------------------------------------------------------- 注册表
def reg_set(path, name, value, typ=winreg.REG_SZ):
    with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, path, 0, winreg.KEY_SET_VALUE) as k:
        winreg.SetValueEx(k, name, 0, typ, value)


def reg_get(path, name=''):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path) as k:
            if name:
                v, _ = winreg.QueryValueEx(k, name)
            else:
                v = winreg.QueryValue(k, '')
            return v
    except OSError:
        return None


def reg_del_tree(path):
    try:
        winreg.DeleteKey(winreg.HKEY_CURRENT_USER, path)
    except OSError:
        pass


# ---------------------------------------------------------------- 文件关联
def backup_assoc():
    bak_dir = os.path.join(app_data_dir(), 'backup')
    os.makedirs(bak_dir, exist_ok=True)
    bak = os.path.join(bak_dir, '.md.reg.bak')
    if reg_get(r'Software\Classes\.md') is not None:
        subprocess.run(['reg', 'export', r'HKCU\Software\Classes\.md', bak, '/y'],
                       capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
        return bak
    return None


def write_assoc(inst_dir):
    exe = os.path.join(inst_dir, APP_EXE)
    icon = os.path.join(inst_dir, '_internal', 'assets', 'markdown-file.ico')
    if not os.path.isfile(icon):
        icon = os.path.join(inst_dir, 'assets', 'markdown-file.ico')
    for ext in EXTENSIONS:
        reg_set(r'Software\Classes\%s' % ext, '', PROG_ID)
    reg_set(r'Software\Classes\%s' % PROG_ID, '', 'ReadMD Markdown Reader')
    reg_set(r'Software\Classes\%s\DefaultIcon' % PROG_ID, '', '"%s",0' % icon)
    reg_set(r'Software\Classes\%s\shell\open\command' % PROG_ID, '',
            '"%s" "%%1"' % exe, typ=winreg.REG_EXPAND_SZ)
    reg_set(r'Software\Classes\%s\shell\openwith' % PROG_ID, '', '')
    reg_set(r'Software\Classes\Applications\%s\shell\open\command' % APP_EXE, '',
            '"%s" "%%1"' % exe, typ=winreg.REG_EXPAND_SZ)
    try:
        subprocess.run(['ie4uinit.exe', '-show'], capture_output=True,
                       creationflags=subprocess.CREATE_NO_WINDOW)
    except Exception:
        pass


def remove_assoc():
    bak = os.path.join(app_data_dir(), 'backup', '.md.reg.bak')
    if os.path.isfile(bak):
        subprocess.run(['reg', 'import', bak], capture_output=True,
                       creationflags=subprocess.CREATE_NO_WINDOW)
    for ext in EXTENSIONS:
        if reg_get(r'Software\Classes\%s' % ext) == PROG_ID:
            reg_del_tree(r'Software\Classes\%s' % ext)
    reg_del_tree(r'Software\Classes\%s' % PROG_ID)
    reg_del_tree(r'Software\Classes\Applications\%s' % APP_EXE)


# ---------------------------------------------------------------- 快捷方式
def get_special_folder(name):
    try:
        r = subprocess.run(
            ['powershell.exe', '-NoProfile', '-NonInteractive', '-Command',
             '[Environment]::GetFolderPath("%s")' % name],
            capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
        v = r.stdout.decode('utf-8', 'replace').strip()
        return v or None
    except Exception:
        return None


def create_shortcut(target, lnk_path, icon):
    try:
        ps = ('$ws = New-Object -ComObject WScript.Shell;'
              '$s = $ws.CreateShortcut("%s");'
              '$s.TargetPath = "%s";'
              '$s.IconLocation = "%s,0";'
              '$s.Description = "ReadMD Markdown Reader";'
              '$s.Save()' % (lnk_path, target, icon))
        subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', ps],
                       capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
    except Exception:
        pass
    return os.path.isfile(lnk_path)


# ---------------------------------------------------------------- 卸载信息
def write_uninstall_entry(inst_dir):
    exe = os.path.join(inst_dir, UNINST_EXE)
    size = 0
    for root, _dirs, files in os.walk(inst_dir):
        for f in files:
            try:
                size += os.path.getsize(os.path.join(root, f))
            except OSError:
                pass
    reg_set(UNINSTALL_KEY, 'DisplayName', APP_NAME + ' Markdown Reader')
    reg_set(UNINSTALL_KEY, 'DisplayVersion', APP_VERSION)
    reg_set(UNINSTALL_KEY, 'Publisher', PUBLISHER)
    reg_set(UNINSTALL_KEY, 'DisplayIcon', '"%s",0' % os.path.join(inst_dir, APP_EXE))
    reg_set(UNINSTALL_KEY, 'InstallLocation', inst_dir)
    reg_set(UNINSTALL_KEY, 'UninstallString', '"%s" --uninstall' % exe)
    reg_set(UNINSTALL_KEY, 'EstimatedSize', int(size / 1024), typ=winreg.REG_DWORD)
    reg_set(UNINSTALL_KEY, 'NoModify', 1, typ=winreg.REG_DWORD)
    reg_set(UNINSTALL_KEY, 'NoRepair', 1, typ=winreg.REG_DWORD)
    reg_set(UNINSTALL_KEY, 'URLInfoAbout', RELEASE_URL)


def remove_uninstall_entry():
    reg_del_tree(UNINSTALL_KEY)


# ---------------------------------------------------------------- 安装 / 卸载
def _copy_file(src, dst, optional=False):
    if src is None or not os.path.isfile(src):
        if optional:
            return
        raise InstallError('verification_failed', '缺少程序文件：%s' % src, str(src or ''))
    for i in range(5):
        try:
            shutil.copy2(src, dst)
            return
        except PermissionError:
            time.sleep(0.5)
    raise InstallError('file_in_use', '文件被占用，无法写入：%s' % dst, dst,
                       ['close_app_retry', 'change_dir'])


def _copy_tree(src_dir, dst_dir):
    """整目录复制（onedir：ReadMD.exe + _internal），带占用重试。"""
    os.makedirs(dst_dir, exist_ok=True)
    for root, _dirs, files in os.walk(src_dir):
        rel = os.path.relpath(root, src_dir)
        target_root = dst_dir if rel == '.' else os.path.join(dst_dir, rel)
        os.makedirs(target_root, exist_ok=True)
        for f in files:
            src = os.path.join(root, f)
            dst = os.path.join(target_root, f)
            for i in range(5):
                try:
                    shutil.copy2(src, dst)
                    break
                except PermissionError:
                    time.sleep(0.5)
            else:
                raise InstallError('file_in_use', '文件被占用，无法写入：%s' % dst, dst,
                                   ['close_app_retry', 'change_dir'])


def _safe_remove_install_child(path, parent):
    """Delete only a uniquely named child we created in this install operation."""
    if not _is_child(path, parent) or os.path.dirname(os.path.abspath(path)) != os.path.abspath(parent):
        raise InstallError('invalid_dir', '拒绝清理安装目录以外的路径。', path)
    if os.path.isdir(path):
        shutil.rmtree(path)


def _stage_name(parent, label):
    path = os.path.join(parent, '.%s.%s-%s' % (APP_NAME.lower(), label, uuid.uuid4().hex))
    if not _is_child(path, parent) or os.path.dirname(path) != os.path.abspath(parent):
        raise InstallError('invalid_dir', '无法创建安全的临时安装目录。', path)
    return path


def _write_install_manifest(inst_dir, opts):
    with open(os.path.join(inst_dir, 'install.json'), 'w', encoding='utf-8') as f:
        json.dump({'app': APP_NAME, 'version': APP_VERSION,
                   'installed_at': time.strftime('%Y-%m-%d %H:%M:%S'),
                   'dir': inst_dir, 'assoc': bool(opts.get('assoc', True)),
                   'webview2': bool(opts.get('webview2', False))},
                  f, ensure_ascii=False, indent=2)


def _validate_staged_install(path):
    missing = [name for name in (APP_EXE, 'install.json') if not os.path.isfile(os.path.join(path, name))]
    if missing:
        raise InstallError('verification_failed', '安装文件校验失败：%s' % ', '.join(missing), path)


def _copy_install_payload(stage_dir, opts):
    app_dir = bundled_app_dir()
    if app_dir is not None:
        _copy_tree(app_dir, stage_dir)
    else:
        _copy_file(bundled_exe(APP_EXE), os.path.join(stage_dir, APP_EXE))
    _copy_file(bundled_exe(UNINST_EXE), os.path.join(stage_dir, UNINST_EXE), optional=True)
    if opts.get('webview2', False):
        rt = bundled_webview2_runtime_dir()
        if rt is not None:
            _copy_tree(rt, os.path.join(stage_dir, 'webview2_runtime'))
    _write_install_manifest(stage_dir, opts)
    _validate_staged_install(stage_dir)


def _commit_staged_install(stage_dir, inst_dir):
    """Swap staged files into place, restoring the prior version on any failure."""
    parent = os.path.dirname(inst_dir)
    backup = None
    moved_old = False
    try:
        if os.path.exists(inst_dir):
            backup = _stage_name(parent, 'backup')
            os.replace(inst_dir, backup)  # same parent/volume: atomic rename
            moved_old = True
        os.replace(stage_dir, inst_dir)
    except Exception as exc:
        # If the new directory was moved into place before a later exception,
        # remove only the known destination and restore the original atomically.
        try:
            if moved_old and backup and os.path.isdir(backup):
                if os.path.isdir(inst_dir):
                    _safe_remove_install_child(inst_dir, parent)
                os.replace(backup, inst_dir)
        except Exception as rollback_error:
            raise InstallError('rollback_failed', '替换安装失败，且旧版本恢复失败：%s' % rollback_error,
                               inst_dir, ['change_dir']) from exc
        code = 'file_in_use' if isinstance(exc, PermissionError) else 'replace_failed'
        raise InstallError(code, '无法替换当前安装，旧版本已保留：%s' % exc, inst_dir,
                           ['close_app_retry', 'elevate', 'change_dir']) from exc
    # A backup is no longer needed only after the new directory is in place.
    if backup and os.path.isdir(backup):
        try:
            _safe_remove_install_child(backup, parent)
        except OSError:
            # It is safe to leave a uniquely named backup for deferred cleanup;
            # never trade the working new install for cleanup convenience.
            pass


def do_install(opts, progress):
    opts = opts or {}
    check = preflight_install(opts.get('dir', ''), opts)
    if not check['ok']:
        raise InstallError(check['code'], check['message'], check.get('path', ''), check.get('actions'))
    inst_dir = check['path']
    parent = os.path.dirname(inst_dir)
    if opts.get('force') and app_running():
        stop_app()
        if app_running():
            raise InstallError('file_in_use', 'ReadMD 仍在运行，无法升级。', inst_dir,
                               ['close_app_retry', 'change_dir'])
    stage_dir = _stage_name(parent, 'staging')
    progress(0, 'prepare', '准备安全的临时安装目录')
    try:
        os.makedirs(stage_dir, exist_ok=False)
        progress(22, 'copy', '复制并校验程序文件')
        _copy_install_payload(stage_dir, opts)
        progress(34, 'runtime', '切换到新版本')
        _commit_staged_install(stage_dir, inst_dir)
        # From here stage_dir no longer exists. Desktop/registry work is performed
        # only after a verified executable is in its final location.
        progress(46, 'assoc', '注册文件关联')
        if opts.get('assoc', True):
            backup_assoc()
            write_assoc(inst_dir)
        progress(68, 'shortcut', '创建快捷方式')
        app_path = os.path.join(inst_dir, APP_EXE)
        if opts.get('desktop', True):
            d = get_special_folder('Desktop')
            if d:
                create_shortcut(app_path, os.path.join(d, APP_NAME + '.lnk'), app_path)
        if opts.get('startmenu', True):
            p = get_special_folder('Programs')
            if p:
                create_shortcut(app_path, os.path.join(p, APP_NAME + '.lnk'), app_path)
        progress(88, 'uninst', '写入卸载信息')
        write_uninstall_entry(inst_dir)
        progress(100, 'done', '安装完成')
    except InstallError:
        raise
    except PermissionError as exc:
        raise InstallError('permission_denied', '安装目录没有写入权限。', getattr(exc, 'filename', inst_dir),
                           ['elevate', 'change_dir']) from exc
    except OSError as exc:
        raise InstallError('install_failed', '安装失败：%s' % exc, getattr(exc, 'filename', inst_dir),
                           ['close_app_retry', 'change_dir']) from exc
    finally:
        if os.path.isdir(stage_dir):
            try:
                _safe_remove_install_child(stage_dir, parent)
            except OSError:
                pass


def _cleanup_dir(path):
    """同步重试删除；若仍被占用（如杀软瞬时锁定），再用 VBS 后台清理兜底。"""
    for i in range(20):
        if not os.path.isdir(path):
            return
        try:
            shutil.rmtree(path)
            return
        except OSError:
            time.sleep(0.5)
    if os.path.isdir(path):
        vbs = os.path.join(app_data_dir(), 'cleanup_readmd.vbs')
        os.makedirs(os.path.dirname(vbs), exist_ok=True)
        with open(vbs, 'w', encoding='utf-8') as f:
            f.write('On Error Resume Next\n')
            f.write('Set fso = CreateObject("Scripting.FileSystemObject")\n')
            f.write('p = "%s"\n' % path)
            f.write('For i = 1 To 60\n')
            f.write('  WScript.Sleep 500\n')
            f.write('  If Not fso.FolderExists(p) Then WScript.Quit\n')
            f.write('  fso.DeleteFolder p, True\n')
            f.write('Next\n')
        try:
            subprocess.Popen(['wscript.exe', vbs], creationflags=subprocess.CREATE_NO_WINDOW)
        except Exception:
            pass


def do_uninstall(progress):
    inst_dir = detect_install_dir()
    progress(6, 'stop', '关闭运行中的 ReadMD')
    if app_running():
        stop_app()
    progress(26, 'assoc', '移除文件关联')
    had_assoc = True
    if inst_dir:
        try:
            with open(os.path.join(inst_dir, 'install.json'), encoding='utf-8') as f:
                had_assoc = bool(json.load(f).get('assoc', True))
        except Exception:
            pass
    if had_assoc:
        remove_assoc()
    progress(48, 'shortcut', '删除快捷方式')
    for folder in ('Desktop', 'Programs'):
        d = get_special_folder(folder)
        if d:
            p = os.path.join(d, APP_NAME + '.lnk')
            if os.path.isfile(p):
                try:
                    os.remove(p)
                except OSError:
                    pass
    progress(66, 'uninst', '移除卸载信息')
    remove_uninstall_entry()
    inst_file = os.path.join(app_data_dir(), 'instance.json')
    if os.path.isfile(inst_file):
        try:
            os.remove(inst_file)
        except OSError:
            pass
    progress(86, 'delete', '清理安装目录')
    if inst_dir and os.path.isdir(inst_dir):
        _cleanup_dir(inst_dir)
    progress(100, 'done', '卸载完成')


# ---------------------------------------------------------------- HTTP
class SetupHandler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        u = urlparse(self.path)
        rel = u.path.lstrip('/') or 'installer/setup.html'
        root = os.path.normpath(asset_root())
        fp = os.path.normpath(os.path.join(root, rel))
        if not fp.startswith(root):
            self.send_error(403)
            return
        if not os.path.isfile(fp):
            self.send_error(404)
            return
        ctype = mimetypes.guess_type(fp)[0] or 'application/octet-stream'
        if ctype.startswith('text/') or ctype == 'application/javascript':
            ctype += '; charset=utf-8'
        with open(fp, 'rb') as f:
            data = f.read()
        self.send_response(200)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(data)))
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        self.wfile.write(data)


# ---------------------------------------------------------------- 桥接
class Api(object):
    """暴露给前端 window.pywebview.api 的方法。"""

    def __init__(self, state):
        self._window = None
        self._state = state
        self._lock = threading.Lock()

    def get_state(self):
        with self._lock:
            return dict(self._state)

    def get_progress(self):
        with self._lock:
            return dict(self._state.get('progress', {}))

    def choose_dir(self, current=''):
        try:
            import webview
            return self._window.create_file_dialog(
                webview.FOLDER_DIALOG, current or default_install_dir())
        except Exception:
            return None

    def preflight_install(self, opts):
        opts = opts or {}
        return preflight_install(opts.get('dir', ''), opts)

    def elevate(self, opts):
        return request_elevation(opts or {})

    def start_install(self, opts):
        opts = opts or {}
        opts.setdefault('force', False)
        check = preflight_install(opts.get('dir', ''), opts)
        if not check['ok']:
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 0, 'step': 'error',
                                           'text': '安装前检查未通过', 'done': False,
                                           'error': check['message'], 'code': check['code'],
                                           'path': check['path'], 'actions': check['actions']}
            return check
        threading.Thread(target=self._run_install, args=(opts,), daemon=True).start()
        return _result(True, 'started', check['path'], '安装已开始。')

    def start_uninstall(self):
        threading.Thread(target=self._run_uninstall, daemon=True).start()
        return True

    def launch(self):
        inst = detect_install_dir()
        if inst:
            try:
                subprocess.Popen([os.path.join(inst, APP_EXE)],
                                 creationflags=subprocess.CREATE_NO_WINDOW)
            except Exception:
                return False
            return True
        return False

    def open_dir(self, path=''):
        try:
            os.startfile(path or default_install_dir())
            return True
        except Exception:
            return False

    def minimize(self):
        try:
            self._window.minimize()
        except Exception:
            pass
        return True

    def close(self):
        try:
            self._window.destroy()
        except Exception:
            pass
        return True

    def _run_install(self, opts):
        def cb(p, step, text):
            with self._lock:
                self._state['progress'] = {'running': True, 'percent': p, 'step': step,
                                           'text': text, 'done': False, 'error': '', 'code': '',
                                           'path': '', 'actions': []}
        try:
            do_install(opts, cb)
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 100, 'step': 'done',
                                           'text': '安装完成', 'done': True, 'error': '', 'code': '',
                                           'path': '', 'actions': []}
                self._state['installed'] = is_installed()
        except InstallError as e:
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 0, 'step': 'error',
                                           'text': '安装失败', 'done': False, 'error': str(e),
                                           'code': e.code, 'path': e.path, 'actions': e.actions}
        except Exception as e:
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 0, 'step': 'error',
                                           'text': '安装失败', 'done': False, 'error': str(e),
                                           'code': 'install_failed', 'path': '',
                                           'actions': ['close_app_retry', 'change_dir']}

    def _run_uninstall(self):
        def cb(p, step, text):
            with self._lock:
                self._state['progress'] = {'running': True, 'percent': p, 'step': step,
                                           'text': text, 'done': False, 'error': '', 'code': '',
                                           'path': '', 'actions': []}
        try:
            do_uninstall(cb)
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 100, 'step': 'done',
                                           'text': '卸载完成', 'done': True, 'error': '', 'code': '',
                                           'path': '', 'actions': []}
                self._state['installed'] = is_installed()
        except Exception as e:
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 0, 'step': 'error',
                                           'text': '卸载失败', 'done': False, 'error': str(e),
                                           'code': 'uninstall_failed', 'path': '', 'actions': ['change_dir']}


# ---------------------------------------------------------------- 入口
def run_install_silent(opts):
    lines = []

    def cb(p, step, text):
        lines.append('%d %s %s' % (p, step, text))
    try:
        do_install(opts, cb)
        for l in lines:
            print(l)
        print('INSTALL OK %s' % opts['dir'])
        return 0
    except Exception as e:
        for l in lines:
            print(l)
        print('INSTALL FAILED %s' % e)
        return 1


def run_uninstall_silent():
    lines = []

    def cb(p, step, text):
        lines.append('%d %s %s' % (p, step, text))
    try:
        do_uninstall(cb)
        for l in lines:
            print(l)
        print('UNINSTALL OK')
        return 0
    except Exception as e:
        for l in lines:
            print(l)
        print('UNINSTALL FAILED %s' % e)
        return 1


def run_gui(uninstall_mode):
    import webview
    inst = is_installed()
    state = {
        'mode': 'uninstall' if (uninstall_mode or is_uninstaller()) else 'install',
        'installed': inst,
        'default_dir': (inst or {}).get('dir') or default_install_dir(),
        'version': APP_VERSION,
        'running': app_running(),
        'win7': is_win7(),
        'webview2Default': is_win7() and not system_webview2_installed(),
        'progress': {'running': False, 'percent': 0, 'step': '', 'text': '', 'done': False, 'error': '',
                     'code': '', 'path': '', 'actions': []},
    }
    if state['mode'] == 'uninstall' and inst is None:
        state['mode'] = 'install'
    api = Api(state)
    url = 'http://127.0.0.1:%d/installer/setup.html' % server_port
    try:
        window = webview.create_window(
            'ReadMD 安装程序', url,
            js_api=api, width=760, height=520, min_size=(560, 380),
            resizable=True, text_select=True, background_color='#0a0e18')
    except Exception:
        window = webview.create_window(
            'ReadMD 安装程序', url,
            js_api=api, width=760, height=520, min_size=(560, 380),
            text_select=True, background_color='#0a0e18')

    api._window = window
    # Win7：安装器 UI 同样使用内置固定版运行时（打过补丁的 pywebview）
    rt = bundled_webview2_runtime_dir()
    if rt is not None:
        os.environ['READMD_WEBVIEW2_RUNTIME'] = rt
        os.environ['READMD_WEBVIEW2_USERDATA'] = os.path.join(app_data_dir(), 'setup_userdata')
    webview.start()
    return 0


def main():
    ap = argparse.ArgumentParser(description='ReadMD 安装程序')
    ap.add_argument('--uninstall', action='store_true', help='图形卸载模式')
    ap.add_argument('--install-silent', nargs='?', const='', metavar='DIR', help='静默安装')
    ap.add_argument('--uninstall-silent', action='store_true', help='静默卸载')
    ap.add_argument('--force', action='store_true', help='静默安装时强制关闭运行中的 ReadMD')
    ap.add_argument('--elevated-payload', metavar='FILE', help=argparse.SUPPRESS)
    ap.add_argument('--elevation-token', metavar='TOKEN', help=argparse.SUPPRESS)
    ap.add_argument('--version', action='store_true', help='输出版本号')
    args = ap.parse_args()

    if args.version:
        print(APP_VERSION)
        return 0
    if args.elevated_payload is not None:
        try:
            opts = consume_elevation_payload(args.elevated_payload, args.elevation_token)
        except InstallError as exc:
            print('ELEVATION FAILED [%s] %s' % (exc.code, exc))
            return 2
        # The elevated copy is deliberately non-interactive: it uses only the
        # validated one-shot payload and reports a normal stable error code.
        return run_install_silent(opts)
    if args.install_silent is not None:
        d = args.install_silent or default_install_dir()
        return run_install_silent({'dir': d, 'assoc': True, 'desktop': False,
                                   'startmenu': False, 'force': args.force,
                                   'webview2': is_win7() and not system_webview2_installed()})
    if args.uninstall_silent:
        return run_uninstall_silent()
    return run_gui(args.uninstall)


if __name__ == '__main__':
    server_port = 0
    try:
        srv = ThreadingHTTPServer(('127.0.0.1', 0), SetupHandler)
        server_port = srv.server_port
        srv.daemon_threads = True
        threading.Thread(target=srv.serve_forever, daemon=True, name='setup-http').start()
    except Exception:
        server_port = 0
    sys.exit(main())
