# -*- coding: utf-8 -*-
"""Fail CI when retired private AI seeds or likely plaintext API keys reappear."""
import os
import re
import subprocess
import sys


ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RETIRED = ["cc" + "-switch", "xem" + "8k5", "hot" + "api", "penguins" + "aichat"]
KEY_PATTERNS = [
    re.compile(rb"\bsk-[A-Za-z0-9_-]{20,}\b"),
    re.compile(rb"\bAIza[0-9A-Za-z_-]{30,}\b"),
]
LOCAL_PATH_PATTERNS = [
    re.compile(rb"[a-zA-Z]:[/\\](?:users|programming|project|workspace|home|desktop|downloads)", re.IGNORECASE),
]


def tracked_files():
    raw = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT)
    return [p.decode("utf-8") for p in raw.split(b"\0") if p]


def iter_external(paths):
    for value in paths:
        path = os.path.abspath(value)
        if os.path.isfile(path):
            yield path, os.path.basename(path)
        elif os.path.isdir(path):
            for base, _dirs, files in os.walk(path):
                for name in files:
                    full = os.path.join(base, name)
                    yield full, os.path.relpath(full, path)


def scan_file(path, label, failures):
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError:
        return
    lower = data.lower()
    for token in RETIRED:
        if token.encode("ascii") in lower:
            failures.append("retired provider marker in %s" % label)
    norm_label = label.replace('\\', '/')
    if norm_label.startswith("assets/vendor/") or norm_label.startswith("tests/") or norm_label.endswith(('.png', '.ico', '.icns', '.lock', '.svg', '.woff', '.woff2', '.ttf', '.eot')):
        return
    binary_extensions = (
        '.exe', '.dll', '.pyd', '.pyc', '.dylib', '.so', '.zip', '.gz',
        '.bin', '.dat', '.obj', '.o', '.a', '.node', '.vsix', '.hap',
        '.deb', '.appimage', '.AppImage', '.tar', '.xz', '.bz2', '.7z', '.pak', '.dmg'
    )
    is_binary = norm_label.endswith(binary_extensions) or norm_label.endswith('/ReadMD') or '/MacOS/ReadMD' in norm_label or norm_label.endswith('ReadMD')
    if is_binary:
        return
    for pattern in KEY_PATTERNS:
        if pattern.search(data):
            failures.append("possible plaintext API key in %s" % label)
    for pattern in LOCAL_PATH_PATTERNS:
        if pattern.search(data):
            failures.append("hardcoded local absolute path in %s" % label)




def main():
    failures = []
    targets = sys.argv[1:]
    if targets:
        scanned = list(iter_external(targets))
        for path, label in scanned:
            scan_file(path, label, failures)
        count = len(scanned)
    else:
        files = tracked_files()
        for rel in files:
            scan_file(os.path.join(ROOT, rel), rel, failures)
        count = len(files)
    if failures:
        print("PRIVACY SCAN FAILED")
        print("\n".join(sorted(set(failures))))
        return 1
    print("privacy scan PASSED (%d files)" % count)
    return 0


if __name__ == "__main__":
    sys.exit(main())
