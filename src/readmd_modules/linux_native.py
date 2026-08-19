# -*- coding: utf-8 -*-
"""ReadMD Linux 与信创国产操作系统 (统信 UOS / 银河麒麟 / 深度 Deepin / openEuler) 原生适配模块。

提供：
1. 操作系统发行版与桌面环境 (DDE / UKUI / GNOME / KDE) 检测；
2. Wayland 与 X11 显示协议自适应配置；
3. 系统深色/浅色外观主题探针；
4. 原生文件管理器打开与系统通知；
5. WebKitGTK 与 QtWebEngine 启动环境变量注入。
"""

import logging
import os
import platform
import shutil
import subprocess
import sys

IS_LINUX = sys.platform.startswith('linux')


def is_linux():
    return IS_LINUX


def is_wayland():
    return bool(os.environ.get('WAYLAND_DISPLAY') or os.environ.get('XDG_SESSION_TYPE') == 'wayland')


def detect_distro():
    """检测当前 Linux 发行版与国产操作系统标识。"""
    if not IS_LINUX:
        return 'unknown'
    try:
        if os.path.exists('/etc/os-release'):
            with open('/etc/os-release', 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read().lower()
                if 'uos' in content or 'uniontech' in content:
                    return 'uos'
                if 'kylin' in content or 'neokylin' in content:
                    return 'kylinos'
                if 'deepin' in content:
                    return 'deepin'
                if 'openeuler' in content:
                    return 'openeuler'
                if 'anolis' in content:
                    return 'anolis'
                if 'ubuntu' in content:
                    return 'ubuntu'
                if 'debian' in content:
                    return 'debian'
                if 'fedora' in content:
                    return 'fedora'
                if 'arch' in content:
                    return 'arch'
    except Exception as e:
        logging.debug('detect_distro failed: %s', e)
    return 'generic-linux'


def is_uos():
    return detect_distro() == 'uos'


def is_kylin():
    return detect_distro() == 'kylinos'


def is_deepin():
    return detect_distro() in ('deepin', 'uos')


def detect_system_dark_mode():
    """通过 gsettings 检测 Linux / 国产桌面的深色模式设置。"""
    if not IS_LINUX:
        return False
    try:
        # 1. 统信 UOS / Deepin DDE
        if shutil.which('gsettings'):
            try:
                out = subprocess.check_output(
                    ['gsettings', 'get', 'org.deepin.dde.appearance', 'theme-type'],
                    stderr=subprocess.DEVNULL, timeout=1
                ).decode('utf-8', errors='ignore').strip()
                if 'dark' in out.lower():
                    return True
            except Exception:
                pass

            # 2. 银河麒麟 UKUI
            try:
                out = subprocess.check_output(
                    ['gsettings', 'get', 'org.ukui.style', 'style-name'],
                    stderr=subprocess.DEVNULL, timeout=1
                ).decode('utf-8', errors='ignore').strip()
                if 'dark' in out.lower() or 'black' in out.lower():
                    return True
            except Exception:
                pass

            # 3. GNOME / 标准 FreeDesktop
            try:
                out = subprocess.check_output(
                    ['gsettings', 'get', 'org.gnome.desktop.interface', 'color-scheme'],
                    stderr=subprocess.DEVNULL, timeout=1
                ).decode('utf-8', errors='ignore').strip()
                if 'prefer-dark' in out.lower():
                    return True
            except Exception:
                pass
    except Exception:
        pass
    return False


def setup_linux_env():
    """根据 Linux 与信创系统桌面配置 WebKit 与 GDK 环境变量。"""
    if not IS_LINUX:
        return

    # 优先兼容 Wayland 与 X11
    if is_wayland():
        os.environ.setdefault('GDK_BACKEND', 'wayland,x11')
    else:
        os.environ.setdefault('GDK_BACKEND', 'x11')

    # WebKitGTK 硬件加速在特定虚拟机/信创板卡上兼容性配置
    os.environ.setdefault('WEBKIT_DISABLE_COMPOSITING_MODE', '0')
    # 针对 HiDPI 屏幕分数缩放
    os.environ.setdefault('GDK_DPI_SCALE', '1')


def show_notification(title, message):
    """发送 Linux 系统原生桌面通知。"""
    if not IS_LINUX:
        return False
    if shutil.which('notify-send'):
        try:
            subprocess.Popen(['notify-send', str(title), str(message), '-a', 'ReadMD', '-i', 'readmd'])
            return True
        except Exception:
            pass
    return False


def open_path(path):
    """在 Linux 文件管理器中打开或定位文件/目录。"""
    if not IS_LINUX or not os.path.exists(path):
        return False
    try:
        norm = os.path.normpath(path)
        if shutil.which('xdg-open'):
            subprocess.Popen(['xdg-open', norm if os.path.isdir(norm) else os.path.dirname(norm)])
            return True
    except Exception:
        pass
    return False
