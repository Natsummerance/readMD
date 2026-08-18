# -*- coding: utf-8 -*-
"""Small PyObjC bridge for behavior that should feel native inside ReadMD.app."""

import os


def _file_url(path):
    from Foundation import NSURL
    return NSURL.fileURLWithPath_(os.path.abspath(path))


def open_path(path):
    """Open a file or directory through NSWorkspace."""
    from AppKit import NSWorkspace
    return bool(NSWorkspace.sharedWorkspace().openURL_(_file_url(path)))


def reveal_path(path):
    """Reveal a file in Finder through the native workspace API."""
    from AppKit import NSWorkspace
    NSWorkspace.sharedWorkspace().activateFileViewerSelectingURLs_([_file_url(path)])
    return True


def show_error(title, message):
    """Display an application-modal NSAlert without invoking osascript."""
    from AppKit import NSAlert, NSAlertStyleCritical
    alert = NSAlert.alloc().init()
    alert.setAlertStyle_(NSAlertStyleCritical)
    alert.setMessageText_(str(title))
    alert.setInformativeText_(str(message))
    alert.addButtonWithTitle_('好')
    alert.runModal()
    return True
