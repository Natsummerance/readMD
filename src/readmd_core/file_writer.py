# -*- coding: utf-8 -*-
"""Atomic text persistence shared by the desktop bridge and local HTTP API."""

from __future__ import annotations

import math
import os
import shutil
import stat
import tempfile


def _same_mtime(left: float, right: float) -> bool:
    return math.isclose(float(left), float(right), rel_tol=0.0, abs_tol=1e-6)


def save_text_atomic(path, content, encoding="utf-8", expected_mtime=None):
    """Write text through a same-directory temporary file.

    Returns a result dictionary instead of raising ordinary I/O failures. When
    *expected_mtime* is supplied, stale editor state cannot overwrite a file
    that changed after it was opened.
    """
    path = os.path.abspath(path)
    backup = None
    temp_path = None
    try:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        old_stat = os.stat(path) if os.path.isfile(path) else None
        if expected_mtime is not None and old_stat and not _same_mtime(
            old_stat.st_mtime, expected_mtime
        ):
            return {
                "ok": False,
                "conflict": True,
                "error": "文件已在编辑后被其他程序修改",
                "current_mtime": old_stat.st_mtime,
            }

        if old_stat and not os.path.exists(path + ".bak"):
            shutil.copy2(path, path + ".bak")
            backup = path + ".bak"

        directory = os.path.dirname(path)
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding=encoding or "utf-8",
            newline="",
            dir=directory,
            prefix="." + os.path.basename(path) + ".",
            suffix=".tmp",
            delete=False,
        ) as handle:
            temp_path = handle.name
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())

        if old_stat:
            os.chmod(temp_path, stat.S_IMODE(old_stat.st_mode))
        os.replace(temp_path, path)
        temp_path = None
        return {
            "ok": True,
            "path": path,
            "backup": backup,
            "mtime": os.stat(path).st_mtime,
        }
    except Exception as exc:
        if temp_path and os.path.exists(temp_path):
            try:
                os.unlink(temp_path)
            except OSError:
                pass
        return {"ok": False, "error": str(exc)}
