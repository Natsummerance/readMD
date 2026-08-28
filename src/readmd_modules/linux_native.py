# -*- coding: utf-8 -*-
"""ReadMD Linux 与信创国产操作系统 (统信 UOS / 银河麒麟 / 深度 Deepin / openEuler) 原生适配模块。

提供：
1. 操作系统发行版与桌面环境 (DDE / UKUI / GNOME / KDE) 检测；
2. Wayland 与 X11 显示协议自适应配置；
3. 系统深色/浅色外观主题探针；
4. 原生文件管理器打开与系统通知；
5. WebKitGTK 与 QtWebEngine 启动环境变量注入；
6. WebKit 4.1/4.0/6.0 动态探测与 Qt/Browser-App 四重降级自愈矩阵；
7. 独立 Browser App 模式（零原生 GUI 依赖即可秒开）；
8. 信创真机环境自检与诊断探针 (--check-linux)。
"""

import logging
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile

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
    if 'loongarch' in machine:
        return 'loongarch64'
    if 'mips' in machine:
        return 'mips64el'
    if 'sw_64' in machine or 'sw64' in machine:
        return 'sw64'
    if machine in ('x86_64', 'amd64'):
        return 'x86_64'
    return machine or 'unknown'


def detect_cpu_vendor():
    """Identify domestic and standard CPUs where GPU drivers commonly need a safe fallback."""
    if not IS_LINUX:
        return ''
    samples = []
    try:
        if os.path.exists('/proc/cpuinfo'):
            with open('/proc/cpuinfo', 'r', encoding='utf-8', errors='ignore') as handle:
                samples.append(handle.read())
    except Exception as exc:
        logging.debug('cpuinfo probe failed: %s', exc)
    for path in ('/proc/device-tree/model', '/proc/device-tree/vendor', '/sys/devices/soc0/machine'):
        try:
            if os.path.exists(path):
                with open(path, 'rb') as handle:
                    samples.append(handle.read(512).decode('ascii', errors='ignore'))
        except Exception:
            pass
    text = ' '.join(samples).lower()
    if re.search(r'phytium|ft-?\d{3,4}|feiteng|tengyun|d2000|e2000|s2500', text):
        return 'phytium'
    if re.search(r'kunpeng|kirin|hi36\d{2}|hi62\d{2}|hi37\d{2}', text):
        return 'kunpeng'
    if re.search(r'loongson|godson|3a5000|3c5000|3a6000', text):
        return 'loongson'
    if re.search(r'zhaoxin|centaurhauls|kaihua|kaixian', text):
        return 'zhaoxin'
    if re.search(r'hygon|dhyana', text):
        return 'hygon'
    if 'intel' in text:
        return 'intel'
    if 'amd' in text:
        return 'amd'
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


def probe_webkit_version():
    """Probe available WebKit2 / WebKit GObject Introspection API version."""
    if not IS_LINUX:
        return None
    try:
        import gi
        for ver in ('4.1', '4.0', '6.0'):
            try:
                gi.require_version('WebKit2', ver)
                from gi.repository import WebKit2  # noqa: F401
                return ver
            except (ValueError, ImportError, AttributeError):
                pass
        try:
            gi.require_version('WebKit', '6.0')
            from gi.repository import WebKit  # noqa: F401
            return '6.0'
        except (ValueError, ImportError, AttributeError):
            pass
    except Exception as exc:
        logging.debug('probe_webkit_version failed: %s', exc)
    return None


def probe_qt_webengine():
    """Probe whether QtWebEngine (PyQt5/PySide2/PyQt6/PySide6) is available."""
    for mod in ('PyQt5.QtWebEngineWidgets', 'PySide2.QtWebEngineWidgets',
                'PyQt6.QtWebEngineWidgets', 'PySide6.QtWebEngineWidgets'):
        try:
            __import__(mod)
            return True
        except Exception:
            pass
    return False


def find_app_browser():
    """Find a suitable browser supporting standalone application window mode on Linux."""
    candidates = [
        # 银河麒麟与统信 UOS 专有浏览器
        'kylin-browser',
        'uos-browser',
        'browser',
        # Chromium 体系（支持 --app= 独立无边框应用窗口模式）
        'google-chrome-stable',
        'google-chrome',
        'chromium-browser',
        'chromium',
        'microsoft-edge-stable',
        'microsoft-edge',
        'brave-browser',
        'opera',
        'vivaldi',
        # GNOME Web (Epiphany) 支持 --application-mode
        'epiphany-browser',
        'epiphany',
        # Firefox 体系（支持 --new-window）
        'firefox',
    ]
    for name in candidates:
        path = shutil.which(name)
        if path and os.path.isfile(path) and os.access(path, os.X_OK):
            return path
    return None


def launch_browser_app(url, window_title='ReadMD', width=1280, height=800):
    """Launch the URL inside a native standalone browser application window."""
    browser = find_app_browser()
    if not browser:
        try:
            import webbrowser
            webbrowser.open(url)
            return None
        except Exception:
            return None

    name = os.path.basename(browser).lower()
    user_data_dir = os.path.join(tempfile.gettempdir(), 'readmd_browser_app_profile')
    os.makedirs(user_data_dir, exist_ok=True)

    cmd = []
    if any(c in name for c in ('chrome', 'chromium', 'kylin', 'uos', 'edge', 'brave', 'browser', 'opera', 'vivaldi')):
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
    elif 'epiphany' in name:
        cmd = [browser, '--application-mode=%s' % url]
    elif 'firefox' in name:
        cmd = [browser, '--new-window', url]
    else:
        cmd = [browser, url]

    try:
        logging.info('Launching native Linux app browser: %s', ' '.join(cmd))
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True
        )
        return proc
    except Exception as exc:
        logging.warning('Failed to launch browser app: %s', exc)
        try:
            import webbrowser
            webbrowser.open(url)
        except Exception:
            pass
        return None


def probe_gui_backends():
    """Probe available Linux GUI backends and return diagnostic capabilities."""
    webkit_ver = probe_webkit_version()
    qt_avail = probe_qt_webengine()
    app_browser = find_app_browser()
    xdg_avail = bool(shutil.which('xdg-open'))

    preferred = 'browser-app'
    if webkit_ver:
        preferred = 'gtk'
    elif qt_avail:
        preferred = 'qt'
    elif app_browser:
        preferred = 'browser-app'
    elif xdg_avail:
        preferred = 'xdg-open'
    else:
        preferred = 'unknown'

    return {
        'gtk_webkit': webkit_ver,
        'qt_webengine': qt_avail,
        'app_browser': app_browser,
        'xdg_open': xdg_avail,
        'preferred_backend': preferred,
    }


def diagnose_system():
    """Gather full diagnostic information about Linux distribution and graphics stack."""
    distro = detect_distro_info()
    arch = architecture()
    cpu_vendor = detect_cpu_vendor()
    backends = probe_gui_backends()
    dark_mode = detect_system_dark_mode()
    wayland = is_wayland()

    is_phytium_chip = (cpu_vendor == 'phytium')
    is_kylin_10 = (distro['id'] == 'kylinos' and bool(re.search(r'v\s*10|(^|[^\d])10([^\d]|$)', distro['version_id'], re.I)))

    return {
        'is_linux': IS_LINUX,
        'distro': distro,
        'architecture': arch,
        'cpu_vendor': cpu_vendor or 'generic',
        'is_phytium': is_phytium_chip,
        'is_kylin_v10': is_kylin_10,
        'is_uos': is_uos(),
        'is_deepin': is_deepin(),
        'wayland': wayland,
        'system_dark_mode': dark_mode,
        'backends': backends,
        # Browser-App/xdg-open keeps a page visible but cannot expose the
        # pywebview bridge used by file dialogs, export, OCR and AI.  It is a
        # diagnostic fallback, never evidence of native feature parity.
        'status': 'ready' if (backends['gtk_webkit'] or backends['qt_webengine'])
                   else ('degraded' if (backends['app_browser'] or backends['xdg_open']) else 'blocked'),
    }


def format_diagnosis_report():
    """Generate formatted human-readable diagnostic report for Linux / Kylin."""
    diag = diagnose_system()
    backends = diag['backends']
    distro = diag['distro']

    lines = [
        "=" * 64,
        " ReadMD 信创与 Linux 操作系统原生适配环境诊断报告",
        "=" * 64,
        "[*] 操作系统类型: %s (ID: %s, Version: %s)" % (
            distro['pretty_name'] or distro['id'],
            distro['id'],
            distro['version_id'] or 'N/A'
        ),
        "[*] 处理器架构: %s (CPU 厂商/特性: %s)" % (
            diag['architecture'],
            diag['cpu_vendor']
        ),
        "[*] 银河麒麟 V10: %s" % ('是 (已启用专有兼容层)' if diag['is_kylin_v10'] else '否'),
        "[*] 统信 UOS / 深度: %s" % ('是 (已启用 DDE 原生适配)' if diag['is_uos'] or diag['is_deepin'] else '否'),
        "[*] 飞腾 Phytium 处理器: %s" % ('是 (已启用 Mesa llvmpipe 渲染自愈防花屏)' if diag['is_phytium'] else '否'),
        "[*] 显示服务器: %s" % ('Wayland (双协议自适应)' if diag['wayland'] else 'X11'),
        "[*] 桌面深色模式: %s" % ('已开启' if diag['system_dark_mode'] else '浅色/默认'),
        "-" * 64,
        "[*] 图形引擎探测 (四重自愈双轨矩阵):",
        "  - WebKitGTK 原生引擎: %s" % (
            ('已就绪 (版本: %s)' % backends['gtk_webkit']) if backends['gtk_webkit'] else '未安装或缺少绑定 (将自动平滑降级)'
        ),
        "  - QtWebEngine 原生引擎: %s" % ('已就绪' if backends['qt_webengine'] else '未就绪'),
        "  - 独立 Browser App 模式: %s" % (
            ('已就绪 (%s)' % backends['app_browser']) if backends['app_browser'] else '未找到适配浏览器'
        ),
        "  - 系统默认浏览器 (xdg-open): %s" % ('可用' if backends['xdg_open'] else '未找到'),
        "  - 自动首选启动链路: %s" % backends['preferred_backend'],
        "-" * 64,
        "[*] 综合就绪状态: %s" % (
            '[OK] 原生全生态开箱即用' if diag['status'] == 'ready'
            else ('[WARNING] 仅有浏览器降级，完整功能需要 WebKitGTK/QtWebEngine'
                  if diag['status'] == 'degraded' else '[BLOCKED] 缺少原生图形引擎')
        ),
        "=" * 64,
    ]
    return "\n".join(lines)


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
