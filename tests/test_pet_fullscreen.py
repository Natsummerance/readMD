# -*- coding: utf-8 -*-
"""Fullscreen hiding must come from a pure, platform-gated probe."""

from src.readmd_modules.pet import fullscreen


def test_window_counts_as_fullscreen_only_when_it_covers_the_whole_monitor():
    monitor = {"left": 0, "top": 0, "right": 1920, "bottom": 1080}

    assert fullscreen.window_covers_monitor(dict(monitor), monitor) is True
    assert fullscreen.window_covers_monitor(
        {"left": 0, "top": 0, "right": 1920, "bottom": 1040}, monitor) is False
    assert fullscreen.window_covers_monitor(
        {"left": -1920, "top": 0, "right": 0, "bottom": 1080}, monitor) is False


def test_malformed_rects_never_report_fullscreen():
    monitor = {"left": 0, "top": 0, "right": 1920, "bottom": 1080}

    assert fullscreen.window_covers_monitor(None, monitor) is False
    assert fullscreen.window_covers_monitor({}, monitor) is False
    assert fullscreen.window_covers_monitor(
        {"left": "x", "top": 0, "right": 1920, "bottom": 1080}, monitor) is False


def test_non_windows_platforms_never_report_fullscreen():
    assert fullscreen.foreground_fullscreen(platform="darwin") is False
    assert fullscreen.foreground_fullscreen(platform="linux") is False


def test_windows_probe_reports_whether_the_foreground_rect_covers_the_monitor(monkeypatch):
    monitor = {"left": 0, "top": 0, "right": 1920, "bottom": 1080}

    monkeypatch.setattr(fullscreen, "_win32_foreground_rects", lambda: (dict(monitor), monitor))
    assert fullscreen.foreground_fullscreen(platform="win32") is True

    monkeypatch.setattr(fullscreen, "_win32_foreground_rects", lambda: None)
    assert fullscreen.foreground_fullscreen(platform="win32") is False
