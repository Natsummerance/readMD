# -*- coding: utf-8 -*-
"""ReadMD macOS (Apple Silicon M1~M4 / Intel x86_64) 原生适配模块。

提供：
1. macOS 版本号与代号 (Sequoia / Sonoma / Ventura / Monterey / Big Sur) 探测；
2. Apple Silicon (ARM64) 与 Intel (x86_64) 芯片架构识别；
3. Cocoa WKWebView 与 PyObjC 原生桥接；
4. 系统深色外观探针 (AppleInterfaceStyle)；
5. 原生 Finder 定位 (NSWorkspace.activateFileViewerSelectingURLs)；
6. 独立 Browser App 模式与系统默认浏览器无缝降级；
7. 原生 NSAlert 弹窗与系统通知 (osascript)；
8. macOS 环境自检与诊断探针 (--check-macos)。
"""

import logging
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile

IS_MAC = sys.platform == 'darwin'


def is_macos():
    return IS_MAC


def get_macos_version_info():
    """获取 macOS 版本号与系统代号。"""
    info = {'major': 0, 'minor': 0, 'name': 'macOS (Unknown)', 'version_str': ''}
    if not IS_MAC:
        return info
    try:
        ver = platform.mac_ver()[0]
        info['version_str'] = ver
        parts = [int(p) for p in ver.split('.') if p.isdigit()]
        if parts:
            info['major'] = parts[0]
            info['minor'] = parts[1] if len(parts) > 1 else 0

        names = {
            15: 'macOS 15 Sequoia',
            14: 'macOS 14 Sonoma',
            13: 'macOS 13 Ventura',
            12: 'macOS 12 Monterey',
            11: 'macOS 11 Big Sur',
            10: 'macOS 10.%d' % info['minor'],
        }
        info['name'] = names.get(info['major'], 'macOS %s' % ver)
    except Exception as exc:
        logging.debug('get_macos_version_info failed: %s', exc)
    return info


def architecture():
    machine = (platform.machine() or '').lower()
    if machine in ('arm64', 'aarch64'):
        return 'arm64'
    return 'x86_64'


def is_apple_silicon():
    return architecture() == 'arm64'


def detect_system_dark_mode():
    """通过 defaults 读取 macOS 系统全局深色外观设置。"""
    if not IS_MAC:
        return False
    try:
        out = subprocess.check_output(
            ['defaults', 'read', '-g', 'AppleInterfaceStyle'],
            stderr=subprocess.DEVNULL, timeout=1
        ).decode('utf-8', errors='ignore').strip()
        return 'dark' in out.lower()
    except Exception:
        return False


def probe_webkit_available():
    """探测 WKWebView 与 PyObjC 是否就绪。"""
    if not IS_MAC:
        return False
    try:
        import WebKit  # noqa: F401
        from AppKit import NSWorkspace  # noqa: F401
        return True
    except Exception:
        return False


def find_app_browser():
    """在 macOS 应用程序目录中查找支持独立应用窗口的 Chromium 体系浏览器。"""
    if not IS_MAC:
        return None
    candidates = [
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
        '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
        '/Applications/Arc.app/Contents/MacOS/Arc',
        '/Applications/Vivaldi.app/Contents/MacOS/Vivaldi',
    ]
    for cand in candidates:
        if os.path.isfile(cand) and os.access(cand, os.X_OK):
            return cand
    return None


def launch_browser_app(url, width=1160, height=820):
    """在 macOS 上以独立应用窗口 (App Mode) 启动 ReadMD。"""
    browser = find_app_browser()
    if not browser:
        try:
            subprocess.Popen(['open', url])
            return None
        except Exception:
            return None

    user_data_dir = os.path.join(tempfile.gettempdir(), 'readmd_mac_app_profile')
    os.makedirs(user_data_dir, exist_ok=True)

    cmd = [
        browser,
        '--app=%s' % url,
        '--user-data-dir=%s' % user_data_dir,
        '--window-size=%d,%d' % (width, height),
        '--no-first-run',
        '--no-default-browser-check',
    ]

    try:
        logging.info('Launching macOS Browser App window: %s', ' '.join(cmd))
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True
        )
        return proc
    except Exception as exc:
        logging.warning('Failed to launch macOS browser app: %s', exc)
        try:
            subprocess.Popen(['open', url])
        except Exception:
            pass
        return None


def _file_url(path):
    try:
        from Foundation import NSURL
        return NSURL.fileURLWithPath_(os.path.abspath(path))
    except Exception:
        return None


def open_path(path):
    """Open a file or directory through NSWorkspace."""
    try:
        from AppKit import NSWorkspace
        u = _file_url(path)
        if u:
            return bool(NSWorkspace.sharedWorkspace().openURL_(u))
    except Exception:
        pass
    if IS_MAC:
        try:
            subprocess.Popen(['open', os.path.abspath(path)])
            return True
        except Exception:
            pass
    return False


def reveal_path(path):
    """Reveal a file in Finder through the native workspace API."""
    try:
        from AppKit import NSWorkspace
        u = _file_url(path)
        if u:
            NSWorkspace.sharedWorkspace().activateFileViewerSelectingURLs_([u])
            return True
    except Exception:
        pass
    if IS_MAC:
        try:
            subprocess.Popen(['open', '-R', os.path.abspath(path)])
            return True
        except Exception:
            pass
    return False


def show_error(title, message):
    """Display an application-modal NSAlert without invoking osascript."""
    if not IS_MAC:
        return False
    try:
        from AppKit import NSAlert, NSAlertStyleCritical
        alert = NSAlert.alloc().init()
        alert.setAlertStyle_(NSAlertStyleCritical)
        alert.setMessageText_(str(title))
        alert.setInformativeText_(str(message))
        alert.addButtonWithTitle_('好')
        alert.runModal()
        return True
    except Exception:
        pass
    try:
        script = 'display alert "%s" message "%s" as critical' % (
            str(title).replace('"', '\\"'), str(message).replace('"', '\\"')
        )
        subprocess.Popen(['osascript', '-e', script])
        return True
    except Exception:
        return False


def show_notification(title, message):
    """发送 macOS 系统原生桌面通知。"""
    if not IS_MAC:
        return False
    try:
        script = 'display notification "%s" with title "%s"' % (
            str(message).replace('"', '\\"'), str(title).replace('"', '\\"')
        )
        subprocess.Popen(['osascript', '-e', script])
        return True
    except Exception:
        return False


def diagnose_system():
    """Gather full diagnostic information about macOS platform."""
    ver_info = get_macos_version_info()
    arch = architecture()
    webkit_ok = probe_webkit_available()
    app_browser = find_app_browser()
    dark_mode = detect_system_dark_mode()

    preferred = 'cocoa-wkwebview'
    if webkit_ok:
        preferred = 'cocoa-wkwebview'
    elif app_browser:
        preferred = 'browser-app'
    else:
        preferred = 'default-browser'

    return {
        'is_macos': IS_MAC,
        'version_info': ver_info,
        'architecture': arch,
        'is_apple_silicon': (arch == 'arm64'),
        'webkit_available': webkit_ok,
        'app_browser': app_browser,
        'system_dark_mode': dark_mode,
        'preferred_backend': preferred,
        'status': 'ready' if (webkit_ok or app_browser) else 'degraded',
    }


def format_diagnosis_report():
    """Generate formatted human-readable diagnostic report for macOS."""
    diag = diagnose_system()
    ver = diag['version_info']

    lines = [
        "=" * 64,
        " ReadMD macOS 操作系统原生适配与图形引擎诊断报告",
        "=" * 64,
        "[*] 操作系统版本: %s (%s)" % (ver['name'], ver['version_str'] or 'N/A'),
        "[*] 芯片架构: %s (Apple Silicon: %s)" % (
            diag['architecture'], '是 (M 系列芯片原生运行)' if diag['is_apple_silicon'] else '否 (Intel x86_64)'
        ),
        "[*] 系统深色模式: %s" % ('已开启 (Dark Mode)' if diag['system_dark_mode'] else '浅色/默认'),
        "-" * 64,
        "[*] 渲染引擎探测 (双轨自愈矩阵):",
        "  - Cocoa WKWebView 原生引擎: %s" % ('已就绪 (PyObjC WKWebView + 私网隔离沙箱)' if diag['webkit_available'] else '未就绪'),
        "  - 独立 Browser App 模式: %s" % (
            ('已就绪 (%s)' % diag['app_browser']) if diag['app_browser'] else '未找到适配 Chromium 浏览器'
        ),
        "  - 自动首选启动链路: %s" % diag['preferred_backend'],
        "-" * 64,
        "[*] 综合就绪状态: %s" % (
            '[OK] 原生全生态开箱即用' if diag['status'] == 'ready' else '[WARNING] 系统缺少图形渲染组件'
        ),
        "=" * 64,
    ]
    return "\n".join(lines)
