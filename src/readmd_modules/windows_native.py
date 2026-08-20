# -*- coding: utf-8 -*-
"""Windows-only native helpers kept out of the macOS application bundle."""


def show_error(title, message):
    import ctypes
    # Why: Direct Windows API calls needed for native OS features not available in Python standard library.user32.MessageBoxW(0, str(message), str(title), 0x10)
    return True
