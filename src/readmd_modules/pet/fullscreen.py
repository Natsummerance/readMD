# -*- coding: utf-8 -*-
"""Foreground fullscreen probing so the desktop pet can hide itself."""

from __future__ import annotations

import os
import sys
from typing import Any, Optional, Tuple

_RECT_KEYS = ("left", "top", "right", "bottom")


def window_covers_monitor(window_rect: Any, monitor_rect: Any) -> bool:
    try:
        window = tuple(int(round(float(window_rect[key]))) for key in _RECT_KEYS)
        monitor = tuple(int(round(float(monitor_rect[key]))) for key in _RECT_KEYS)
    except (KeyError, TypeError, ValueError):
        return False
    return window == monitor


def foreground_fullscreen(platform: Optional[str] = None) -> bool:
    current = sys.platform if platform is None else platform
    if current != "win32":
        return False
    rects = _win32_foreground_rects()
    if rects is None:
        return False
    window, monitor = rects
    return window_covers_monitor(window, monitor)


def _win32_foreground_rects() -> Optional[Tuple[dict, dict]]:
    if os.name != "nt":
        return None
    import ctypes
    import ctypes.wintypes

    # ctypes.wintypes does not define MONITORINFO.
    class _MonitorInfo(ctypes.Structure):
        _fields_ = [
            ("cbSize", ctypes.wintypes.DWORD),
            ("rcMonitor", ctypes.wintypes.RECT),
            ("rcWork", ctypes.wintypes.RECT),
            ("dwFlags", ctypes.wintypes.DWORD),
        ]

    user32 = ctypes.windll.user32
    handle = user32.GetForegroundWindow()
    if not handle:
        return None
    window = ctypes.wintypes.RECT()
    if not user32.GetWindowRect(handle, ctypes.byref(window)):
        return None
    monitor = user32.MonitorFromWindow(handle, 2)  # MONITOR_DEFAULTTONEAREST
    if not monitor:
        return None
    info = _MonitorInfo()
    info.cbSize = ctypes.sizeof(info)
    if not user32.GetMonitorInfoW(monitor, ctypes.byref(info)):
        return None
    return (
        {"left": window.left, "top": window.top, "right": window.right, "bottom": window.bottom},
        {"left": info.rcMonitor.left, "top": info.rcMonitor.top,
         "right": info.rcMonitor.right, "bottom": info.rcMonitor.bottom},
    )
