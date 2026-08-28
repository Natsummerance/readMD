# -*- coding: utf-8 -*-
"""Fail the release if executable AI instructions are duplicated in clients.

This is intentionally a narrow policy check, not a natural-language linter:
user requests and labels may contain the word prompt, but a runtime system
instruction must be resolved from a Skill.  The vendored originals and Skill
documents are excluded because they are immutable source material.
"""
from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCAN_ROOTS = [ROOT / "readmd.py", ROOT / "src", ROOT / "packages", ROOT / "assets" / "js"]
SKIP_PARTS = {"upstream", "node_modules", "dist", "build", "__pycache__"}
TEXT_SUFFIXES = {".py", ".js", ".ts", ".tsx", ".ets", ".html"}

# These patterns describe an actual embedded system message, rather than a
# normal user request.  Skill files are deliberately outside SCAN_ROOTS.
FORBIDDEN = [
    re.compile(r"system_prompt\s*=\s*(['\"]).{20,}", re.I),
    re.compile(r"['\"]role['\"]\s*:\s*['\"]system['\"]\s*,\s*['\"]content['\"]\s*:\s*(['\"]).{20,}", re.I),
    re.compile(r"openAiPanelWithPrompt\s*\([^,]+,\s*`[^`]*(?:请将|请深度解析|You are|You must)", re.I | re.S),
]


def files():
    for root in SCAN_ROOTS:
        candidates = [root] if root.is_file() else root.rglob("*")
        for path in candidates:
            if path.is_file() and path.suffix.lower() in TEXT_SUFFIXES and not (SKIP_PARTS & set(path.parts)):
                yield path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="verify prompt source policy")
    parser.parse_args()
    violations = []
    for path in files():
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            continue
        for pattern in FORBIDDEN:
            match = pattern.search(text)
            if match:
                line = text.count("\n", 0, match.start()) + 1
                violations.append(f"{path.relative_to(ROOT)}:{line}: {pattern.pattern}")
    if violations:
        print("runtime AI prompt policy failed:")
        print("\n".join(violations))
        return 1
    print("runtime AI prompt policy passed (system instructions come from Skills)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
