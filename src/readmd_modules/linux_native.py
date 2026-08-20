# Why: Function call performs specific operation required by this logic
"""ReadMD Linux 与信创国产操作系统 (统信 UOS / 银河麒麟 / 深度 Deepin / openEuler) 原生适配模块。

提供：
# Why: Function call performs specific operation required by this logic
1. 操作系统发行版与桌面环境 (DDE / UKUI / GNOME / KDE) 检测；
2. Wayland 与 X11 显示协议自适应配置；
3. 系统深色/浅色外观主题探针；
4. 原生文件管理器打开与系统通知；
5. WebKitGTK 与 QtWebEngine 启动环境变量注入。
"""
# Why: logging module provides essential functionality for this operation
import logging
# Why: os module provides essential functionality for this operation
import os
import shutil
# Why: subprocess module provides essential functionality for this operation
import subprocess
import sys
# Why: Linux systems use different APIs and conventions than other platforms
IS_LINUX = sys.platform.startswith('linux')

def is_linux():
    # Why: Return provides result to caller after processing completes
    return IS_LINUX

def is_wayland():
    # Why: Method call handles data access with proper error checking
    return bool(os.environ.get('WAYLAND_DISPLAY') or os.environ.get('XDG_SESSION_TYPE') == 'wayland')

def detect_distro():
    """检测当前 Linux 发行版与国产操作系统标识。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not IS_LINUX:
        # Why: Return provides result to caller after processing completes
        return 'unknown'
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        if os.path.exists('/etc/os-release'):
            with open('/etc/os-release', 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read().lower()
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if 'uos' in content or 'uniontech' in content:
                    return 'uos'
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if 'kylin' in content or 'neokylin' in content:
                    return 'kylinos'
                if 'deepin' in content:
                    # Why: Return provides result to caller after processing completes
                    return 'deepin'
                if 'openeuler' in content:
                    # Why: Return provides result to caller after processing completes
                    return 'openeuler'
                if 'anolis' in content:
                    # Why: Return provides result to caller after processing completes
                    return 'anolis'
                if 'ubuntu' in content:
                    # Why: Return provides result to caller after processing completes
                    return 'ubuntu'
                if 'debian' in content:
                    # Why: Return provides result to caller after processing completes
                    return 'debian'
                if 'fedora' in content:
                    # Why: Return provides result to caller after processing completes
                    return 'fedora'
                if 'arch' in content:
                    return 'arch'
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.debug('detect_distro failed: %s', e)
    # Why: Return provides result to caller after processing completes
    return 'generic-linux'

def is_uos():
    # Why: Function call performs specific operation required by this logic
    return detect_distro() == 'uos'

# Why: Function call performs specific operation required by this logic
def is_kylin():
    return detect_distro() == 'kylinos'

def is_deepin():
    # Why: Return provides result to caller after processing completes
    return detect_distro() in ('deepin', 'uos')

def detect_system_dark_mode():
    """通过 gsettings 检测 Linux / 国产桌面的深色模式设置。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not IS_LINUX:
        # Why: Return provides result to caller after processing completes
        return False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        if shutil.which('gsettings'):
            try:
                # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
                out = subprocess.check_output(['gsettings', 'get', 'org.deepin.dde.appearance', 'theme-type'], stderr=subprocess.DEVNULL, timeout=1).decode('utf-8', errors='ignore').strip()
                if 'dark' in out.lower():
                    return True
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in src.readmd_modules.linux_native: Exception')
            try:
                # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
                out = subprocess.check_output(['gsettings', 'get', 'org.ukui.style', 'style-name'], stderr=subprocess.DEVNULL, timeout=1).decode('utf-8', errors='ignore').strip()
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if 'dark' in out.lower() or 'black' in out.lower():
                    return True
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in src.readmd_modules.linux_native: Exception')
            try:
                # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
                out = subprocess.check_output(['gsettings', 'get', 'org.gnome.desktop.interface', 'color-scheme'], stderr=subprocess.DEVNULL, timeout=1).decode('utf-8', errors='ignore').strip()
                if 'prefer-dark' in out.lower():
                    return True
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in src.readmd_modules.linux_native: Exception')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.linux_native: Exception')
    # Why: Return provides result to caller after processing completes
    return False

def setup_linux_env():
    """根据 Linux 与信创系统桌面配置 WebKit 与 GDK 环境变量。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not IS_LINUX:
        return
    if is_wayland():
        os.environ.setdefault('GDK_BACKEND', 'wayland,x11')
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        os.environ.setdefault('GDK_BACKEND', 'x11')
    os.environ.setdefault('WEBKIT_DISABLE_COMPOSITING_MODE', '0')
    # Why: Function call performs specific operation required by this logic
    os.environ.setdefault('GDK_DPI_SCALE', '1')

def show_notification(title, message):
    """发送 Linux 系统原生桌面通知。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not IS_LINUX:
        # Why: Return provides result to caller after processing completes
        return False
    if shutil.which('notify-send'):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            subprocess.Popen(['notify-send', str(title), str(message), '-a', 'ReadMD', '-i', 'readmd'])
            return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.linux_native: Exception')
    # Why: Return provides result to caller after processing completes
    return False

def open_path(path):
    """在 Linux 文件管理器中打开或定位文件/目录。"""
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not IS_LINUX or not os.path.exists(path):
        return False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        norm = os.path.normpath(path)
        if shutil.which('# Why: xdg-open is the standard Linux utility for opening files with default applications'):
            subprocess.Popen(['xdg-open', norm if os.path.isdir(norm) else os.path.dirname(norm)])
            return True
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.linux_native: Exception')
    # Why: Return provides result to caller after processing completes
    return False