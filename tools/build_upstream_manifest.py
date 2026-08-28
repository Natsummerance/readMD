# -*- coding: utf-8 -*-
"""Build and verify the immutable offline upstream source manifest.

The manifest is intentionally data-only.  Runtime code uses it as an allowlist
for the read-only provenance endpoint; no source file is imported or executed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from typing import Any, Dict, Iterable


ROOT = Path(__file__).resolve().parents[1]
UPSTREAM_ROOT = ROOT / "assets" / "upstream"
MANIFEST = UPSTREAM_ROOT / "manifest.json"


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _files() -> Iterable[Path]:
    if not UPSTREAM_ROOT.is_dir():
        return []
    return sorted(
        path
        for path in UPSTREAM_ROOT.rglob("*")
        if path.is_file()
        and path != MANIFEST
        and "__pycache__" not in path.parts
    )


def build() -> Dict[str, Any]:
    entries = []
    for path in _files():
        rel = path.relative_to(ROOT).as_posix()
        entries.append(
            {
                "path": rel,
                "bytes": path.stat().st_size,
                "sha256": _sha256(path),
            }
        )
    sources: Dict[str, Dict[str, Any]] = {}
    for entry in entries:
        parts = Path(entry["path"]).parts
        if len(parts) < 4:
            continue
        source_id = "/".join(parts[2:4])
        item = sources.setdefault(source_id, {"id": source_id, "files": 0, "bytes": 0})
        item["files"] += 1
        item["bytes"] += entry["bytes"]
    return {
        "schema_version": 1,
        "purpose": "ReadMD offline upstream source allowlist; files are immutable snapshots",
        "sources": sorted(sources.values(), key=lambda item: item["id"]),
        "files": entries,
    }


def verify(data: Dict[str, Any]) -> None:
    expected = {item["path"]: item for item in data.get("files", []) if isinstance(item, dict)}
    actual = {entry["path"]: entry for entry in build()["files"]}
    if set(expected) != set(actual):
        missing = sorted(set(expected) - set(actual))
        added = sorted(set(actual) - set(expected))
        raise SystemExit(f"upstream manifest file set mismatch; missing={missing[:5]} added={added[:5]}")
    for path, item in expected.items():
        if item.get("bytes") != actual[path]["bytes"] or item.get("sha256") != actual[path]["sha256"]:
            raise SystemExit(f"upstream manifest hash mismatch: {path}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="verify the checked-in manifest")
    args = parser.parse_args()
    if args.check:
        if not MANIFEST.is_file():
            raise SystemExit("upstream manifest is missing")
        verify(json.loads(MANIFEST.read_text(encoding="utf-8")))
        print(f"upstream manifest verified ({len(build()['files'])} files)")
        return 0
    UPSTREAM_ROOT.mkdir(parents=True, exist_ok=True)
    data = build()
    MANIFEST.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {MANIFEST} ({len(data['files'])} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
