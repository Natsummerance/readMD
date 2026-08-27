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
import re
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


def detect_distro_info():
    """Return normalized distribution fields used by the Kylin compatibility layer."""
    info = {'id': detect_distro(), 'version_id': '', 'pretty_name': ''}
    if not IS_LINUX:
        return info
    try:
        if os.path.exists('/etc/os-release'):
            values = {}
            with open('/etc/os-release', 'r', encoding='utf-8', errors='ignore') as handle:
                for line in handle:
                    key, sep, value = line.strip().partition('=')
                    if sep:
                        values[key.strip().upper()] = value.strip().strip('"').strip("'")
            if values.get('ID'):
                lowered = values['ID'].lower()
                if 'uos' in lowered or 'uniontech' in lowered:
                    info['id'] = 'uos'
                elif 'kylin' in lowered or 'neokylin' in lowered:
                    info['id'] = 'kylinos'
                elif 'deepin' in lowered:
                    info['id'] = 'deepin'
                else:
                    info['id'] = lowered
            info['version_id'] = values.get('VERSION_ID', '')
            info['pretty_name'] = values.get('PRETTY_NAME', '')
    except Exception as exc:
        logging.debug('detect_distro_info failed: %s', exc)
    return info


def architecture():
    machine = (platform.machine() or '').lower()
    if machine in ('arm64', 'aarch64'):
        return 'arm64'
    if machine.startswith('armv'):
        return 'arm'
    return machine or 'unknown'


def detect_cpu_vendor():
    """Identify domestic ARM CPUs where GPU drivers commonly need a safe fallback."""
    if not IS_LINUX or architecture() != 'arm64':
        return ''
    samples = []
    try:
        with open('/proc/cpuinfo', 'r', encoding='utf-8', errors='ignore') as handle:
            samples.append(handle.read())
    except Exception as exc:
        logging.debug('cpuinfo probe failed: %s', exc)
    for path in ('/proc/device-tree/model', '/proc/device-tree/vendor'):
        try:
            with open(path, 'rb') as handle:
                samples.append(handle.read(512).decode('ascii', errors='ignore'))
        except Exception:
            pass
    text = ' '.join(samples).lower()
    if re.search(r'phytium|ft-?\d{3,4}|feiteng|tengyun|d2000|e2000|s2500', text):
        return 'phytium'
    if re.search(r'kunpeng|kirin', text):
        return text.split()[0] if text.split() else 'unknown-domestic'
    return ''


def is_kylin_v10():
    info = detect_distro_info()
    return info['id'] == 'kylinos' and bool(re.search(r'v\s*10|(^|[^\d])10([^\d]|$)', info['version_id'], re.I))


def is_phytium():
    return detect_cpu_vendor() == 'phytium'


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

    def set_env_default(name, value):
        # CI shells sometimes export compatibility switches as empty values.
        if not os.environ.get(name):
            os.environ[name] = value

    # 优先兼容 Wayland 与 X11
    if is_wayland():
        set_env_default('GDK_BACKEND', 'wayland,x11')
    else:
        set_env_default('GDK_BACKEND', 'x11')

    legacy_gpu = (
        (is_kylin_v10() and architecture() == 'arm64' and is_phytium())
        or os.environ.get('READMD_SOFTWARE_WEBKIT', '').lower() in ('1', 'true', 'yes')
    )
    if legacy_gpu:
        # UKUI/X11 plus software WebGL is the stable path on Phytium D2000/E2000
        # boards whose vendor GL drivers fail inside WebKitGTK.
        set_env_default('GDK_BACKEND', 'x11')
        set_env_default('WEBKIT_DISABLE_COMPOSITING_MODE', '1')
        set_env_default('WEBKIT_DISABLE_DMABUF_RENDERER', '1')
        set_env_default('LIBGL_ALWAYS_SOFTWARE', '1')
        set_env_default('GALLIUM_DRIVER', 'llvmpipe')
        set_env_default('MESA_LOADER_DRIVER_OVERRIDE', 'swrast')
    else:
        # WebKitGTK hardware acceleration works on mainstream Linux GPUs.
        set_env_default('WEBKIT_DISABLE_COMPOSITING_MODE', '0')
    # 针对 HiDPI 屏幕分数缩放
    set_env_default('GDK_DPI_SCALE', '1')


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
