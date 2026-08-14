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
import json
import mimetypes
import os
import shutil
import subprocess
import sys
import threading
import time
import winreg
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
APP_VERSION = '2.1.1'
PUBLISHER = 'Natsummerance'
PROG_ID = 'ReadMD.markdown'
EXTENSIONS = ['.md', '.markdown', '.mdown', '.mkd']
RELEASE_URL = 'https://github.com/Natsummerance/readMD/releases'

INSTALL_STEPS = [
    ('prepare', '准备安装目录'),
    ('copy', '复制程序文件'),
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


def is_uninstaller():
    if getattr(sys, 'frozen', False):
        name = os.path.basename(sys.executable).lower()
    else:
        name = os.path.basename(sys.argv[0]).lower()
    return name.startswith('readmduninstall')


def bundled_exe(name):
    if getattr(sys, '_MEIPASS', None):
        p = os.path.join(sys._MEIPASS, name)
        if os.path.isfile(p):
            return p
    for cand in (resource_path(name), os.path.join(asset_root(), 'dist', name)):
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
    for ext in EXTENSIONS:
        reg_set(r'Software\Classes\%s' % ext, '', PROG_ID)
    reg_set(r'Software\Classes\%s' % PROG_ID, '', 'ReadMD Markdown Reader')
    reg_set(r'Software\Classes\%s\DefaultIcon' % PROG_ID, '', '"%s",0' % exe)
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
        raise RuntimeError('缺少程序文件：%s' % src)
    for i in range(5):
        try:
            shutil.copy2(src, dst)
            return
        except PermissionError:
            time.sleep(0.5)
    raise RuntimeError('文件被占用，无法写入：%s' % dst)


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
                raise RuntimeError('文件被占用，无法写入：%s' % dst)


def do_install(opts, progress):
    inst_dir = os.path.abspath(opts['dir'])
    if not inst_dir or inst_dir == os.path.dirname(inst_dir):
        raise RuntimeError('无效的安装目录')
    progress(0, 'prepare', '准备安装目录')
    os.makedirs(inst_dir, exist_ok=True)
    if not os.access(inst_dir, os.W_OK):
        raise RuntimeError('目录不可写：%s' % inst_dir)
    if opts.get('force') and app_running():
        stop_app()
    progress(22, 'copy', '复制程序文件')
    # 升级兼容：先清掉旧 onedir 的 _internal 残留（避免陈旧 DLL / 资源）
    old_internal = os.path.join(inst_dir, '_internal')
    if os.path.isdir(old_internal):
        try:
            shutil.rmtree(old_internal)
        except OSError:
            pass
    app_dir = bundled_app_dir()
    if app_dir is not None:
        _copy_tree(app_dir, inst_dir)
    else:
        _copy_file(bundled_exe(APP_EXE), os.path.join(inst_dir, APP_EXE))
    _copy_file(bundled_exe(UNINST_EXE), os.path.join(inst_dir, UNINST_EXE), optional=True)
    with open(os.path.join(inst_dir, 'install.json'), 'w', encoding='utf-8') as f:
        json.dump({'app': APP_NAME, 'version': APP_VERSION,
                   'installed_at': time.strftime('%Y-%m-%d %H:%M:%S'),
                   'dir': inst_dir, 'assoc': bool(opts.get('assoc', True))},
                  f, ensure_ascii=False, indent=2)
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

    def start_install(self, opts):
        opts = opts or {}
        opts.setdefault('force', False)
        threading.Thread(target=self._run_install, args=(opts,), daemon=True).start()
        return True

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
                                           'text': text, 'done': False, 'error': ''}
        try:
            do_install(opts, cb)
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 100, 'step': 'done',
                                           'text': '安装完成', 'done': True, 'error': ''}
                self._state['installed'] = is_installed()
        except Exception as e:
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 0, 'step': 'error',
                                           'text': '安装失败', 'done': False, 'error': str(e)}

    def _run_uninstall(self):
        def cb(p, step, text):
            with self._lock:
                self._state['progress'] = {'running': True, 'percent': p, 'step': step,
                                           'text': text, 'done': False, 'error': ''}
        try:
            do_uninstall(cb)
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 100, 'step': 'done',
                                           'text': '卸载完成', 'done': True, 'error': ''}
                self._state['installed'] = is_installed()
        except Exception as e:
            with self._lock:
                self._state['progress'] = {'running': False, 'percent': 0, 'step': 'error',
                                           'text': '卸载失败', 'done': False, 'error': str(e)}


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
        'default_dir': default_install_dir(),
        'version': APP_VERSION,
        'running': app_running(),
        'progress': {'running': False, 'percent': 0, 'step': '', 'text': '', 'done': False, 'error': ''},
    }
    if state['mode'] == 'uninstall' and inst is None:
        state['mode'] = 'install'
    api = Api(state)
    url = 'http://127.0.0.1:%d/installer/setup.html' % server_port
    try:
        window = webview.create_window(
            'ReadMD 安装程序', url,
            js_api=api, width=980, height=700, min_size=(880, 640),
            frameless=True, easy_drag=True, shadow=True, resizable=True,
            text_select=True, background_color='#0a0e18')
    except Exception:
        window = webview.create_window(
            'ReadMD 安装程序', url,
            js_api=api, width=980, height=700, min_size=(880, 640),
            text_select=True, background_color='#0a0e18')
    api._window = window
    webview.start()
    return 0


def main():
    ap = argparse.ArgumentParser(description='ReadMD 安装程序')
    ap.add_argument('--uninstall', action='store_true', help='图形卸载模式')
    ap.add_argument('--install-silent', nargs='?', const='', metavar='DIR', help='静默安装')
    ap.add_argument('--uninstall-silent', action='store_true', help='静默卸载')
    ap.add_argument('--force', action='store_true', help='静默安装时强制关闭运行中的 ReadMD')
    ap.add_argument('--version', action='store_true', help='输出版本号')
    args = ap.parse_args()

    if args.version:
        print(APP_VERSION)
        return 0
    if args.install_silent is not None:
        d = args.install_silent or default_install_dir()
        return run_install_silent({'dir': d, 'assoc': True, 'desktop': False,
                                   'startmenu': False, 'force': args.force})
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
