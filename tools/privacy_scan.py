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
    re.compile(rb"[a-zA-Z]:[/\\](?:users|programming|project|workspace|home|desktop|downloads|natsumer|[a-zA-Z0-9_.-]+[/\\]\.(?:codex|gemini|antigravity))", re.IGNORECASE),
    re.compile(rb"[a-zA-Z]:[/\\][a-zA-Z0-9_.-]+[/\\](?:skills|plugins|creator)[/\\]", re.IGNORECASE),
    re.compile(rb"/(?:Users|home)/[a-zA-Z0-9_.-]+/(?:\.codex|\.gemini|Programming|Projects)", re.IGNORECASE),
]
# Real personal documents must never enter the repo; filename hit = failure,
# checked before every other early-return path in scan_file.
SENSITIVE_NAME_PATTERNS = [
    re.compile(r"北京交通大学软件学院毕业实习文档"),
]


def tracked_files():
    raw = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT)
    return [p.decode("utf-8") for p in raw.split(b"\0") if p]


def candidate_files():
    """Return tracked plus non-ignored working-tree files.

    Release privacy checks must cover newly generated Skills, provider
    snapshots and other untracked candidate files.  Ignored build directories
    are checked separately by the explicit artifact arguments in release CI.
    """
    try:
        raw = subprocess.check_output(
            ["git", "ls-files", "-co", "--exclude-standard", "-z"], cwd=ROOT
        )
        return [p.decode("utf-8") for p in raw.split(b"\0") if p]
    except (OSError, subprocess.CalledProcessError):
        return tracked_files()


def iter_external(paths):
    for value in paths:
        path = os.path.abspath(value)
        if os.path.isfile(path):
            yield path, os.path.basename(path)
        elif os.path.isdir(path):
            for base, dirs, files in os.walk(path):
                dirs[:] = [d for d in dirs if d not in ('node_modules', '.git', '__pycache__', '_internal')]
                for name in files:
                    full = os.path.join(base, name)
                    yield full, os.path.relpath(full, path)


def scan_file(path, label, failures, is_source_tree=True):
    norm_label = label.replace('\\', '/')
    for pattern in SENSITIVE_NAME_PATTERNS:
        if pattern.search(norm_label):
            failures.append("sensitive real-document name in %s" % label)
            return
    if "/assets/vendor/" in norm_label or norm_label.startswith("assets/vendor/"):
        return
    if "/assets/upstream/" in norm_label or norm_label.startswith("assets/upstream/"):
        return
    if "/tests/" in norm_label or norm_label.startswith("tests/"):
        return
    if (
        "verify-macos" in norm_label
        or norm_label.startswith("Contents/")
        or "/Contents/" in norm_label
        or norm_label.startswith("Frameworks/")
        or "/Frameworks/" in norm_label
    ):
        return
    if norm_label.endswith(('.png', '.ico', '.icns', '.lock', '.svg', '.woff', '.woff2', '.ttf', '.eot')):
        return
    binary_extensions = (
        '.exe', '.dll', '.pyd', '.pyc', '.dylib', '.so', '.zip', '.gz',
        '.bin', '.dat', '.obj', '.o', '.a', '.node', '.vsix', '.hap',
        '.deb', '.appimage', '.AppImage', '.tar', '.xz', '.bz2', '.7z', '.pak', '.dmg',
        '.dylib', '.strings', '.nib', '.storyboardc'
    )
    is_binary = norm_label.endswith(binary_extensions) or norm_label.endswith('/ReadMD') or '/MacOS/ReadMD' in norm_label or norm_label.endswith('ReadMD')
    if is_binary:
        return
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError:
        return
    lower = data.lower()
    # The public provider-catalog attribution is an intentional, audited
    # source; it is not a private credential or runtime integration.
    public_ccswitch_attribution = (
        norm_label in ("assets/providers/provider-catalog.json", "provider-catalog.json",
                       "src/readmd_modules/ai.py", "tools/build_provider_catalog.py",
                       "tools/package_local_rc.py", "source-snapshot-manifest.json",
                       "THIRD_PARTY_LICENSES.md", "candidate.json", "SHA256SUMS.txt") or
        norm_label.endswith("/assets/providers/provider-catalog.json") or
        norm_label.endswith("src/readmd_modules/ai.py") or
        norm_label.endswith("tools/build_provider_catalog.py") or
        norm_label.endswith("tools/package_local_rc.py") or
        norm_label.endswith("/source-snapshot-manifest.json") or
        norm_label.endswith("/THIRD_PARTY_LICENSES.md") or
        norm_label.endswith("/candidate.json") or
        norm_label.endswith("/SHA256SUMS.txt")
    )
    retired_tokens = [token for token in RETIRED
                      if not (token == ("cc" + "-switch") and public_ccswitch_attribution)]
    for token in retired_tokens:
        if token.encode("ascii") in lower:
            failures.append("retired provider marker in %s" % label)
    for pattern in KEY_PATTERNS:
        if pattern.search(data):
            failures.append("possible plaintext API key in %s" % label)
    if is_source_tree and not ("/dist/" in norm_label or norm_label.startswith("dist/")):
        for pattern in LOCAL_PATH_PATTERNS:
            if pattern.search(data):
                failures.append("hardcoded local absolute path in %s" % label)


def main():
    failures = []
    targets = sys.argv[1:]
    if targets:
        scanned = list(iter_external(targets))
        for path, label in scanned:
            scan_file(path, label, failures, is_source_tree=False)
        count = len(scanned)
    else:
        files = candidate_files()
        for rel in files:
            scan_file(os.path.join(ROOT, rel), rel, failures, is_source_tree=True)
        count = len(files)
    if failures:
        print("PRIVACY SCAN FAILED")
        print("\n".join(sorted(set(failures))))
        return 1
    print("privacy scan PASSED (%d files)" % count)
    return 0


if __name__ == "__main__":
    sys.exit(main())
