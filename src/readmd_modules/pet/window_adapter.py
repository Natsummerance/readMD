# -*- coding: utf-8 -*-
"""Native-window capability probe for a future renderer adapter.

The probe is deliberately separate from the ReadMD main window: it may be
run manually to collect evidence, but never pretends that browser fallback or
an unverified renderer is native Live2D support.
"""

from __future__ import annotations

import inspect
import sys


class NativePetProbe:
    """Creates a tiny transparent native-window probe only on explicit request."""

    @staticmethod
    def probe_capabilities(webview_module, platform_name=None):
        platform_name = platform_name or sys.platform
        create_window = getattr(webview_module, "create_window", None)
        start = getattr(webview_module, "start", None)
        if not callable(create_window) or not callable(start):
            return _report(platform_name, False, False, False, "unavailable")
        try:
            signature = inspect.signature(create_window)
            parameters = signature.parameters
            accepts_keywords = any(param.kind == inspect.Parameter.VAR_KEYWORD for param in parameters.values())
            supports = lambda name: accepts_keywords or name in parameters
            transparent = supports("transparent")
            on_top = supports("on_top")
            frameless = supports("frameless")
        except (TypeError, ValueError):
            return _report(platform_name, False, False, False, "unknown")
        native_window = all((transparent, on_top, frameless))
        return _report(platform_name, native_window, transparent, on_top, "manual-verification-required")

    @staticmethod
    def create_probe_window(webview_module):
        report = NativePetProbe.probe_capabilities(webview_module)
        if not report["native_window"]:
            raise RuntimeError("native_pet_window_capability_unavailable")
        return webview_module.create_window(
            "ReadMD", html=NativePetProbe.probe_html(), width=220, height=240,
            min_size=(160, 160), frameless=True, on_top=True, transparent=True,
            text_select=False, zoomable=False, draggable=True, shadow=False,
            background_color="#000000",
        )

    @staticmethod
    def probe_html():
        # This is not a character or a replacement for a Live2D model. It only
        # makes transparency, positioning and drag handling visible to a tester.
        return """<!doctype html><meta charset=\"utf-8\"><style>
html,body{margin:0;width:100%;height:100%;background:transparent;overflow:hidden}
#probe{width:100%;height:100%;border-radius:50%;background:radial-gradient(circle at 35% 28%,#c8d8ff,#5c84f7 58%,#263764);box-sizing:border-box;border:1px solid rgba(255,255,255,.76);box-shadow:0 8px 26px rgba(0,0,0,.35)}
</style><div id=\"probe\" aria-label=\"ReadMD native pet capability probe\"></div>"""


def _report(platform_name, native_window, transparent, on_top, click_through):
    return {
        "platform": platform_name,
        "native_window": bool(native_window),
        "transparent_window": bool(transparent),
        "always_on_top": bool(on_top),
        "click_through": click_through,
        "drag_drop": "manual-verification-required",
        "multi_monitor": "manual-verification-required",
        "release_ready": False,
    }
