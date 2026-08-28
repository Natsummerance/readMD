# -*- coding: utf-8 -*-
"""ReadMD 全系统·全平台·全架构原生运行自适应统一调度模块。

统一调度并暴露 Windows / macOS / Linux / 国产信创生态的原生探针与诊断接口。
"""

import logging
import platform
import sys

def get_current_platform_flavor():
    """返回当前宿主机的平台类型标识。"""
    if sys.platform == 'win32':
        return 'windows'
    if sys.platform == 'darwin':
        return 'macos'
    if sys.platform.startswith('linux'):
        return 'linux'
    return 'unknown'


def get_unified_diagnosis():
    """获取当前平台的统一诊断结构化数据。"""
    flavor = get_current_platform_flavor()
    if flavor == 'windows':
        try:
            from src.readmd_modules import windows_native
            data = windows_native.diagnose_system()
            data['platform_flavor'] = 'windows'
            return data
        except Exception as exc:
            logging.debug('windows_native diagnosis failed: %s', exc)
    elif flavor == 'macos':
        try:
            from src.readmd_modules import macos_native
            data = macos_native.diagnose_system()
            data['platform_flavor'] = 'macos'
            return data
        except Exception as exc:
            logging.debug('macos_native diagnosis failed: %s', exc)
    elif flavor == 'linux':
        try:
            from src.readmd_modules import linux_native
            data = linux_native.diagnose_system()
            data['platform_flavor'] = 'linux'
            return data
        except Exception as exc:
            logging.debug('linux_native diagnosis failed: %s', exc)

    return {
        'platform_flavor': flavor,
        'platform': sys.platform,
        'architecture': (platform.machine() or 'unknown').lower(),
        'status': 'unsupported',
    }


def format_unified_report():
    """格式化当前系统的原生开箱即用诊断报告。"""
    flavor = get_current_platform_flavor()
    if flavor == 'windows':
        try:
            from src.readmd_modules import windows_native
            return windows_native.format_diagnosis_report()
        except Exception:
            pass
    elif flavor == 'macos':
        try:
            from src.readmd_modules import macos_native
            return macos_native.format_diagnosis_report()
        except Exception:
            pass
    elif flavor == 'linux':
        try:
            from src.readmd_modules import linux_native
            return linux_native.format_diagnosis_report()
        except Exception:
            pass

    return (
        "=" * 64 + "\n" +
        " ReadMD 通用操作系统环境诊断报告\n" +
        "=" * 64 + "\n" +
        "[*] 操作系统平台: %s\n" % sys.platform +
        "[*] 处理器架构: %s\n" % (platform.machine() or 'unknown') +
        "=" * 64
    )


def launch_native_app_window(url, width=1160, height=820):
    """跨平台统一调用独立应用窗口 (App Mode) 启动 ReadMD。"""
    flavor = get_current_platform_flavor()
    if flavor == 'windows':
        try:
            from src.readmd_modules import windows_native
            return windows_native.launch_browser_app(url, width=width, height=height)
        except Exception:
            pass
    elif flavor == 'macos':
        try:
            from src.readmd_modules import macos_native
            return macos_native.launch_browser_app(url, width=width, height=height)
        except Exception:
            pass
    elif flavor == 'linux':
        try:
            from src.readmd_modules import linux_native
            return linux_native.launch_browser_app(url, width=width, height=height)
        except Exception:
            pass

    try:
        import webbrowser
        webbrowser.open(url)
        return None
    except Exception:
        return None
