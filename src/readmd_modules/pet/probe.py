# -*- coding: utf-8 -*-
"""Explicit manual native-window probe. Never run by the normal application."""

from __future__ import annotations

import json

from .window_adapter import NativePetProbe


def main():
    import webview

    report = NativePetProbe.probe_capabilities(webview)
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    if not report["native_window"]:
        return 2
    NativePetProbe.create_probe_window(webview)
    webview.start()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
