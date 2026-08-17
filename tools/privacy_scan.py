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


def tracked_files():
    raw = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT)
    return [p.decode("utf-8") for p in raw.split(b"\0") if p]


def main():
    failures = []
    for rel in tracked_files():
        path = os.path.join(ROOT, rel)
        try:
            data = open(path, "rb").read()
        except OSError:
            continue
        lower = data.lower()
        for token in RETIRED:
            if token.encode("ascii") in lower:
                failures.append("retired provider marker in %s" % rel)
        if rel.startswith("assets/vendor/") or rel.endswith(('.png', '.ico', '.icns')):
            continue
        for pattern in KEY_PATTERNS:
            if pattern.search(data):
                failures.append("possible plaintext API key in %s" % rel)
    if failures:
        print("PRIVACY SCAN FAILED")
        print("\n".join(sorted(set(failures))))
        return 1
    print("privacy scan PASSED (%d tracked files)" % len(tracked_files()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
