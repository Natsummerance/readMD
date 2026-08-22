#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Select the highest semantic version strictly below the released tag."""
from __future__ import annotations

import argparse
import re
import sys


SEMVER_RE = re.compile(
    r"^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)


def _prerelease_key(value: str | None) -> tuple[tuple[int, int, str], ...]:
    if not value:
        # A normal version has higher precedence than any prerelease.
        return ((2, 0, ""),)
    identifiers: list[tuple[int, int, str]] = []
    for part in value.split("."):
        if part.isdigit():
            identifiers.append((0, int(part), ""))
        else:
            identifiers.append((1, 0, part))
    return tuple(identifiers)


def version_key(tag: str) -> tuple[int, int, int, tuple[tuple[int, int, str], ...]] | None:
    match = SEMVER_RE.fullmatch(tag.strip())
    if not match:
        return None
    major, minor, patch, prerelease = match.groups()
    return (
        int(major),
        int(minor),
        int(patch),
        _prerelease_key(prerelease),
    )


def select_previous(current: str, tags: list[str]) -> str:
    current_key = version_key(current)
    if current_key is None:
        raise ValueError(f"current release is not semantic version: {current}")

    candidates: list[tuple[tuple[int, int, int, tuple[tuple[int, int, str], ...]], str]] = []
    seen = set()
    for tag in tags:
        tag = tag.strip()
        if not tag or tag in seen:
            continue
        seen.add(tag)
        key = version_key(tag)
        if key is not None and key < current_key:
            candidates.append((key, tag))
    if not candidates:
        raise ValueError(f"no previous semantic version tag found below {current}")
    return max(candidates, key=lambda item: (item[0], item[1]))[1]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--current", required=True)
    parser.add_argument("tags", nargs="*")
    args = parser.parse_args()
    try:
        print(select_previous(args.current, args.tags))
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
