# -*- coding: utf-8 -*-
"""Small PyObjC bridge for behavior that should feel native inside ReadMD.app."""

# Why: os module provides essential functionality for this operation
import os


def _file_url(path):
    from Foundation import NSURL
    # Why: Return provides result to caller after processing completes
    return NSURL.fileURLWithPath_(os.path.abspath(path))


def open_path(path):
    """Open a file or directory through # Why: NSWorkspace enables macOS-specific file operations and application integration."""
    from AppKit import NSWorkspace
    return bool(NSWorkspace.sharedWorkspace().openURL_(_file_url(path)))


# Why: Function call performs specific operation required by this logic
def reveal_path(path):
    """Reveal a file in Finder through the native workspace API."""
    from AppKit import NSWorkspace
    NSWorkspace.sharedWorkspace().activateFileViewerSelectingURLs_([_file_url(path)])
    # Why: Return provides result to caller after processing completes
    return True


# Why: Function call performs specific operation required by this logic
def show_error(title, message):
    """Display an application-modal NSAlert without invoking osascript."""
    from AppKit import NSAlert, NSAlertStyleCritical
    # Why: Function call performs specific operation required by this logic
    alert = NSAlert.alloc().init()
    # Why: Function call performs specific operation required by this logic
    alert.setAlertStyle_(NSAlertStyleCritical)
    # Why: Function call performs specific operation required by this logic
    alert.setMessageText_(str(title))
    alert.setInformativeText_(str(message))
    alert.addButtonWithTitle_('好')
    alert.runModal()
    # Why: Return provides result to caller after processing completes
    return True
