# -*- coding: utf-8 -*-
"""ReadMD Windows 全版本 (Win7 / Win8 / Win10 / Win11 / WinServer / Windows on ARM) 原生适配模块。

提供：
1. Windows 版本与内部版本号 (Build) 精准识别；
2. 处理器架构识别 (x86_64 / x86 / ARM64 WoA 骁龙 X Elite / Surface Pro X)；
3. Edge WebView2 运行时注册表与固定版路径探针；
4. 独立 Browser App 模式（调用 msedge.exe / chrome.exe --app= 零依赖秒开）；
5. 注册表深色/浅色外观主题探针；
6. 资源管理器文件定位 (explorer /select) 与原生错误弹窗；
7. Windows 环境自检与诊断探针 (--check-windows)。
"""

import logging
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile

IS_WIN = sys.platform == 'win32'


def is_windows():
    return IS_WIN


def get_windows_version_info():
    """获取 Windows 版本号与友好展示名称。"""
    info = {'major': 0, 'minor': 0, 'build': 0, 'name': 'Unknown Windows', 'is_win7': False, 'is_win11': False}
    if not IS_WIN:
        return info
    try:
        ver_str = platform.version()
        match = re.match(r'(\d+)\.(\d+)\.(\d+)', ver_str)
        if match:
            info['major'] = int(match.group(1))
            info['minor'] = int(match.group(2))
            info['build'] = int(match.group(3))

        # 根据 NT 内核版本与 Build 判定
        if info['major'] == 10 and info['build'] >= 22000:
            info['name'] = 'Windows 11 (Build %d)' % info['build']
            info['is_win11'] = True
        elif info['major'] == 10:
            info['name'] = 'Windows 10 (Build %d)' % info['build']
        elif info['major'] == 6 and info['minor'] == 3:
            info['name'] = 'Windows 8.1'
        elif info['major'] == 6 and info['minor'] == 2:
            info['name'] = 'Windows 8'
        elif info['major'] == 6 and info['minor'] == 1:
            info['name'] = 'Windows 7 SP1'
            info['is_win7'] = True
        else:
            info['name'] = 'Windows %s' % ver_str
    except Exception as exc:
        logging.debug('get_windows_version_info failed: %s', exc)
    return info


def is_win7():
    return get_windows_version_info().get('is_win7', False)


def is_win11():
    return get_windows_version_info().get('is_win11', False)


def architecture():
    """精确检测 Windows 宿主机的真实原生架构 (含 ARM64 / WoA 检测)。"""
    if not IS_WIN:
        return (platform.machine() or '').lower()
    # 优先检测 ARM64 环境变量（如骁龙 X Elite / Surface Pro X）
    arch = os.environ.get('PROCESSOR_ARCHITEW6432', os.environ.get('PROCESSOR_ARCHITECTURE', '')).upper()
    if 'ARM64' in arch:
        return 'arm64'
    if '64' in arch or 'AMD64' in arch:
        return 'x86_64'
    if '86' in arch:
        return 'x86'
    machine = (platform.machine() or '').lower()
    if 'arm' in machine or 'aarch64' in machine:
        return 'arm64'
    return 'x86_64'


def is_arm64():
    return architecture() == 'arm64'


def probe_webview2_installed():
    """通过注册表与固定路径探针检测系统是否已安装 Microsoft Edge WebView2 运行时。"""
    if not IS_WIN:
        return {'installed': False, 'version': '', 'path': ''}
    try:
        import winreg
        guid = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
        keys = [
            (winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\%s' % guid),
            (winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\Microsoft\EdgeUpdate\Clients\%s' % guid),
            (winreg.HKEY_CURRENT_USER, r'Software\Microsoft\EdgeUpdate\Clients\%s' % guid),
        ]
        for root, subkey in keys:
            try:
                with winreg.OpenKey(root, subkey) as key:
                    ver, _ = winreg.QueryValueEx(key, 'pv')
                    if ver and ver != '0.0.0.0':
                        loc, _ = '', None
                        try:
                            loc, _ = winreg.QueryValueEx(key, 'location')
                        except Exception:
                            pass
                        return {'installed': True, 'version': str(ver), 'path': str(loc or 'System Evergreen')}
            except Exception:
                continue
    except Exception as exc:
        logging.debug('probe_webview2_installed failed: %s', exc)

    # 探针检测常见内置路径
    for cand in [
        os.path.join(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)'), r'Microsoft\EdgeWebView\Application'),
        os.path.join(os.environ.get('ProgramFiles', r'C:\Program Files'), r'Microsoft\EdgeWebView\Application'),
        os.path.join(os.environ.get('LocalAppData', ''), r'Microsoft\EdgeWebView\Application'),
    ]:
        if os.path.isdir(cand):
            for child in os.listdir(cand):
                if re.match(r'^\d+\.\d+\.\d+\.\d+$', child):
                    return {'installed': True, 'version': child, 'path': os.path.join(cand, child)}

    return {'installed': False, 'version': '', 'path': ''}


def find_app_browser():
    """在 Windows 上查找支持 --app= 独立应用窗口模式的 Edge / Chrome / Chromium 浏览器。"""
    if not IS_WIN:
        return None
    candidates = [
        # Microsoft Edge (Win10/11 默认自带)
        os.path.join(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)'), r'Microsoft\Edge\Application\msedge.exe'),
        os.path.join(os.environ.get('ProgramFiles', r'C:\Program Files'), r'Microsoft\Edge\Application\msedge.exe'),
        os.path.join(os.environ.get('LocalAppData', ''), r'Microsoft\Edge\Application\msedge.exe'),
        # Google Chrome
        os.path.join(os.environ.get('ProgramFiles', r'C:\Program Files'), r'Google\Chrome\Application\chrome.exe'),
        os.path.join(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)'), r'Google\Chrome\Application\chrome.exe'),
        os.path.join(os.environ.get('LocalAppData', ''), r'Google\Chrome\Application\chrome.exe'),
        # Brave / Chromium
        os.path.join(os.environ.get('ProgramFiles', r'C:\Program Files'), r'BraveSoftware\Brave-Browser\Application\brave.exe'),
    ]
    # Also check PATH
    for name in ('msedge.exe', 'chrome.exe', 'brave.exe'):
        path = shutil.which(name)
        if path:
            candidates.insert(0, path)

    for cand in candidates:
        if cand and os.path.isfile(cand) and os.access(cand, os.X_OK):
            return cand
    return None


def launch_browser_app(url, width=1160, height=820):
    """在 Windows 上以原生无边框独立应用窗口 (App Mode) 启动 ReadMD。"""
    browser = find_app_browser()
    if not browser:
        try:
            import webbrowser
            webbrowser.open(url)
            return None
        except Exception:
            return None

    user_data_dir = os.path.join(tempfile.gettempdir(), 'readmd_win_app_profile')
    os.makedirs(user_data_dir, exist_ok=True)

    cmd = [
        browser,
        '--app=%s' % url,
        '--user-data-dir=%s' % user_data_dir,
        '--window-size=%d,%d' % (width, height),
        '--no-first-run',
        '--no-default-browser-check',
        '--disable-sync',
        '--disable-background-networking',
        '--disable-features=Translate',
    ]

    try:
        logging.info('Launching Windows Browser App window: %s', ' '.join(cmd))
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0x08000000)
        )
        return proc
    except Exception as exc:
        logging.warning('Failed to launch Windows browser app: %s', exc)
        try:
            import webbrowser
            webbrowser.open(url)
        except Exception:
            pass
        return None


def detect_system_dark_mode():
    """通过注册表读取 Windows 系统应用深色模式设置。"""
    if not IS_WIN:
        return False
    try:
        import winreg
        key_path = r'Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key_path) as key:
            val, _ = winreg.QueryValueEx(key, 'AppsUseLightTheme')
            return int(val) == 0
    except Exception:
        pass
    return False


def show_error(title, message):
    """显示 Windows 原生应用程序模态错误弹窗。"""
    if not IS_WIN:
        return False
    try:
        import ctypes
        ctypes.windll.user32.MessageBoxW(0, str(message), str(title), 0x10)
        return True
    except Exception:
        return False


def reveal_path(path):
    """在 Windows 资源管理器中高亮并选中文件。"""
    if not IS_WIN or not os.path.exists(path):
        return False
    try:
        norm = os.path.normpath(path)
        subprocess.Popen(['explorer.exe', '/select,', norm])
        return True
    except Exception:
        return False


def open_path(path):
    """在 Windows 默认关联程序中打开文件或目录。"""
    if not IS_WIN or not os.path.exists(path):
        return False
    try:
        os.startfile(os.path.normpath(path))
        return True
    except Exception:
        return False


def diagnose_system():
    """Gather full diagnostic information about Windows platform and Edge WebView2 stack."""
    ver_info = get_windows_version_info()
    arch = architecture()
    wv2 = probe_webview2_installed()
    app_browser = find_app_browser()
    dark_mode = detect_system_dark_mode()

    preferred = 'webview2'
    if wv2['installed']:
        preferred = 'webview2'
    elif app_browser:
        preferred = 'browser-app'
    else:
        preferred = 'default-browser'

    return {
        'is_windows': IS_WIN,
        'version_info': ver_info,
        'architecture': arch,
        'is_arm64': (arch == 'arm64'),
        'webview2': wv2,
        'app_browser': app_browser,
        'system_dark_mode': dark_mode,
        'preferred_backend': preferred,
        'status': 'ready' if (wv2['installed'] or app_browser) else 'degraded',
    }


def format_diagnosis_report():
    """Generate formatted human-readable diagnostic report for Windows."""
    diag = diagnose_system()
    ver = diag['version_info']
    wv2 = diag['webview2']

    lines = [
        "=" * 64,
        " ReadMD Windows 操作系统原生适配与图形引擎诊断报告",
        "=" * 64,
        "[*] 操作系统版本: %s (Major: %d, Minor: %d, Build: %d)" % (
            ver['name'], ver['major'], ver['minor'], ver['build']
        ),
        "[*] 处理器指令集架构: %s (Windows on ARM: %s)" % (
            diag['architecture'], '是 (ARM64 原生优化)' if diag['is_arm64'] else '否 (x86_64)'
        ),
        "[*] 系统深色模式: %s" % ('已开启 (Dark Theme)' if diag['system_dark_mode'] else '浅色/默认'),
        "-" * 64,
        "[*] 渲染引擎探测 (三重自愈双轨矩阵):",
        "  - Microsoft Edge WebView2 运行时: %s" % (
            ('已就绪 (版本: %s, 路径: %s)' % (wv2['version'], wv2['path'])) if wv2['installed'] else '未安装 (将自动平滑降级至 Browser App 模式)'
        ),
        "  - 独立 Browser App 模式 (msedge/chrome): %s" % (
            ('已就绪 (%s)' % diag['app_browser']) if diag['app_browser'] else '未找到兼容浏览器'
        ),
        "  - 自动首选启动链路: %s" % diag['preferred_backend'],
        "-" * 64,
        "[*] 综合就绪状态: %s" % (
            '[OK] 原生全生态开箱即用' if diag['status'] == 'ready' else '[WARNING] 建议安装 Edge WebView2 运行时以获得最佳体验'
        ),
        "=" * 64,
    ]
    return "\n".join(lines)
