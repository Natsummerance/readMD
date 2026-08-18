# -*- coding: utf-8 -*-
"""Windows-only native helpers kept out of the macOS application bundle."""


def show_error(title, message):
    import ctypes
    ctypes.windll.user32.MessageBoxW(0, str(message), str(title), 0x10)
    return True
