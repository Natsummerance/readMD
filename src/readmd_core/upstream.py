# -*- coding: utf-8 -*-
"""Read-only access to vendored upstream source snapshots.

The manifest is the only authority for source IDs and file IDs.  This module
never accepts an arbitrary filesystem path and never imports or executes the
vendored files, which keeps the provenance viewer safe for desktop, MCP and
VS Code clients alike.
"""
from __future__ import annotations

import hashlib
import json
import mimetypes
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
UPSTREAM_ROOT = ROOT / "assets" / "upstream"
MANIFEST = UPSTREAM_ROOT / "manifest.json"
MAX_SOURCE_BYTES = 2 * 1024 * 1024


class UpstreamSourceError(ValueError):
    pass


def _manifest() -> dict[str, Any]:
    try:
        data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise UpstreamSourceError("offline upstream manifest unavailable") from exc
    if not isinstance(data, dict) or not isinstance(data.get("files"), list):
        raise UpstreamSourceError("invalid offline upstream manifest")
    return data


def _file_id(path: str) -> str:
    return hashlib.sha256(path.encode("utf-8")).hexdigest()[:24]


def _file_entries(source_id: str | None = None) -> list[dict[str, Any]]:
    entries = []
    prefix = "assets/upstream/" + source_id.rstrip("/") + "/" if source_id else "assets/upstream/"
    for item in _manifest().get("files", []):
        if not isinstance(item, dict):
            continue
        path = str(item.get("path") or "")
        if not path.startswith(prefix) or path == prefix:
            continue
        rel = path[len(prefix):]
        if not rel or "/" not in rel and source_id is None:
            continue
        entries.append({
            "id": _file_id(path),
            "path": path,
            "relative_path": rel,
            "bytes": int(item.get("bytes") or 0),
            "sha256": str(item.get("sha256") or ""),
        })
    return entries


def list_sources() -> list[dict[str, Any]]:
    data = _manifest()
    result = []
    for source in data.get("sources", []):
        if not isinstance(source, dict) or not source.get("id"):
            continue
        source_id = str(source["id"])
        files = _file_entries(source_id)
        result.append({
            "id": source_id,
            "files": len(files),
            "bytes": int(source.get("bytes") or sum(x["bytes"] for x in files)),
            "manifest": "assets/upstream/manifest.json",
            "license": _license_for(source_id),
            "file_ids": [item["id"] for item in files],
        })
    return result


def _license_for(source_id: str) -> str:
    # License files are included in each fixed snapshot.  The API reports the
    # filename rather than guessing a license from a URL or network metadata.
    files = _file_entries(source_id)
    names = {Path(item["relative_path"]).name.upper() for item in files}
    if "LICENSE" in names:
        return "LICENSE"
    if "LICENSE.TXT" in names:
        return "LICENSE.txt"
    if "NOTICE" in names:
        return "NOTICE"
    return ""


def get_source(source_id: str) -> dict[str, Any]:
    source_id = str(source_id or "").strip().strip("/")
    source = next((item for item in list_sources() if item["id"] == source_id), None)
    if source is None:
        raise UpstreamSourceError("unknown upstream source")
    source["source_files"] = _file_entries(source_id)
    return source


def get_file(source_id: str, file_id: str) -> dict[str, Any]:
    source = get_source(source_id)
    item = next((entry for entry in source["source_files"] if entry["id"] == str(file_id)), None)
    if item is None:
        raise UpstreamSourceError("unknown upstream file")
    path = ROOT / Path(item["path"])
    try:
        path.resolve().relative_to(UPSTREAM_ROOT.resolve())
    except ValueError as exc:
        raise UpstreamSourceError("upstream file escapes snapshot root") from exc
    if item["bytes"] > MAX_SOURCE_BYTES:
        raise UpstreamSourceError("upstream file is too large for the viewer")
    try:
        content = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise UpstreamSourceError("upstream file is not readable text") from exc
    return {
        **item,
        "source_id": source["id"],
        "mime": mimetypes.guess_type(path.name)[0] or "text/plain",
        "content": content,
    }


__all__ = ["UpstreamSourceError", "list_sources", "get_source", "get_file"]
