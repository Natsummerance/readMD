# -*- coding: utf-8 -*-
"""Native-window capability probe for a future renderer adapter.

The probe is deliberately separate from the ReadMD main window: it may be
run manually to collect evidence, but never pretends that browser fallback or
an unverified renderer is native Live2D support.

The pointer-offset drag contract is adapted from NousResearch/hermes-agent
(MIT), revision fb27614addac115d55299bc6538ae112fd01f688. See
``docs/third-party-notices/hermes-agent-pet.md``.
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
        bridge = PetProbeDragBridge()
        window = webview_module.create_window(
            "ReadMD", html=NativePetProbe.probe_html(), width=220, height=240,
            min_size=(160, 160), frameless=True, on_top=True, transparent=True,
            text_select=False, zoomable=False, draggable=False, shadow=False,
            js_api=bridge,
            background_color="#000000",
        )
        bridge.bind(window)
        # Keep the bridge available for native manual diagnostics. The public
        # pywebview API has no window identifier that can be passed back from JS.
        setattr(window, "_pet_probe_bridge", bridge)
        return window

    @staticmethod
    def probe_html():
        # This is not a character or a replacement for a Live2D model. It only
        # makes transparency, positioning and drag handling visible to a tester.
        return """<!doctype html><meta charset=\"utf-8\"><style>
html,body{margin:0;width:100%;height:100%;background:transparent;overflow:hidden}
#probe{width:100%;height:100%;border-radius:50%;background:radial-gradient(circle at 35% 28%,#c8d8ff,#5c84f7 58%,#263764);box-sizing:border-box;border:1px solid rgba(255,255,255,.76);box-shadow:0 8px 26px rgba(0,0,0,.35);touch-action:none;user-select:none;cursor:grab}
#probe.dragging{cursor:grabbing}
</style><div id=\"probe\" aria-label=\"ReadMD native pet capability probe\"></div><script>
(()=>{const p=document.getElementById('probe');let active=false;const api=()=>window.pywebview&&window.pywebview.api;
const call=(name,e)=>{const bridge=api();if(bridge&&bridge[name]){Promise.resolve(bridge[name](e.screenX,e.screenY)).catch(()=>{});}};
p.addEventListener('pointerdown',e=>{active=true;p.classList.add('dragging');p.setPointerCapture?.(e.pointerId);call('begin_drag',e);});
p.addEventListener('pointermove',e=>{if(active)call('move_drag',e);});
const stop=e=>{if(!active)return;active=false;p.classList.remove('dragging');call('end_drag',e);p.releasePointerCapture?.(e.pointerId);};
p.addEventListener('pointerup',stop);p.addEventListener('pointercancel',stop);})();
</script>"""


class PetProbeDragBridge:
    """Hermes-style pointer-offset drag for the pywebview native probe.

    Hermes's Electron overlay disables automatic window dragging and moves the
    native window from explicit screen coordinates. This bridge uses the same
    user-visible contract through pywebview's ``Window.move`` API, preventing
    two window drag systems from fighting each other.
    """

    def __init__(self, window=None):
        self._window = window
        self._offset = None

    def bind(self, window):
        self._window = window

    def begin_drag(self, screen_x, screen_y):
        window = self._require_window()
        self._offset = (int(round(float(screen_x))) - int(window.x),
                        int(round(float(screen_y))) - int(window.y))
        return {"ok": True}

    def move_drag(self, screen_x, screen_y):
        if self._offset is None:
            return {"ok": False, "code": "drag_not_started"}
        window = self._require_window()
        x = int(round(float(screen_x))) - self._offset[0]
        y = int(round(float(screen_y))) - self._offset[1]
        window.move(x, y)
        return {"ok": True, "x": x, "y": y}

    def end_drag(self, *_unused):
        self._offset = None
        return {"ok": True}

    def _require_window(self):
        if self._window is None:
            raise RuntimeError("pet_probe_window_unavailable")
        return self._window


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
