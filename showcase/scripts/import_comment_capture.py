#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Import a local comment capture as anonymized resonance evidence."""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from content_memory import import_comment_snapshot, load_records


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _likes(value: Any) -> int:
    if isinstance(value, bool) or value is None:
        return 0
    if isinstance(value, int):
        return value
    if isinstance(value, str) and value.isdigit():
        return int(value)
    raise ValueError("comment likes must be a non-negative integer or digit string")


def _append_comment(item: Any, output: list[dict[str, Any]]) -> None:
    """Normalize common public-page and legacy snapshot fields recursively."""
    if not isinstance(item, dict):
        return
    text = str(item.get("content", item.get("text", ""))).strip()
    if text:
        output.append({"text": text, "likes": _likes(item.get("like_count", item.get("likes", 0)))})
    replies = item.get("sub_comments", item.get("subComments", []))
    if isinstance(replies, list):
        for reply in replies:
            _append_comment(reply, output)


def comments_from_capture(record: dict[str, Any]) -> list[dict[str, Any]]:
    comments = record.get("comments") if isinstance(record, dict) else None
    if comments is None and isinstance(record, list):
        comments = record
    if not isinstance(comments, list):
        raise ValueError("comment capture must contain a comments list")

    flattened: list[dict[str, Any]] = []
    for item in comments:
        _append_comment(item, flattened)
    return flattened


def _identity_conflict(left: Any, right: Any) -> bool:
    return bool(left and right and str(left) != str(right))


def import_capture(
    ledger: Path,
    capture: Path,
    *,
    release: str,
    captured_at: str,
    title: str | None = None,
    note_id: str | None = None,
) -> dict[str, Any]:
    record = json.loads(capture.read_text(encoding="utf-8"))
    if not isinstance(record, (dict, list)):
        raise ValueError("comment capture must be a JSON object or array")

    ledger_record = next((item for item in load_records(ledger) if item.get("release") == release), None)
    if ledger_record is None:
        raise ValueError(f"release not found in feedback ledger: {release}")

    capture_title = record.get("title") if isinstance(record, dict) else None
    capture_note_id = record.get("note_id", record.get("noteId")) if isinstance(record, dict) else None
    selected_title = title or capture_title or ledger_record.get("title")
    selected_note_id = note_id or capture_note_id or ledger_record.get("note_id")
    if _identity_conflict(title or capture_title, ledger_record.get("title")):
        raise ValueError("capture title conflicts with immutable ledger title")
    if _identity_conflict(note_id or capture_note_id, ledger_record.get("note_id")):
        raise ValueError("capture note ID conflicts with immutable ledger note ID")

    comments = comments_from_capture(record)
    updated = import_comment_snapshot(
        ledger,
        release,
        {"comments": comments},
        source="xiaohongshu-web",
        captured_at=captured_at,
    )
    insights = updated.get("comment_insights", {})
    return {
        "ok": True,
        "release": release,
        "imported_count": len(comments),
        "unique_count": insights.get("unique_count"),
        "top_theme": insights.get("top_theme"),
        "themes": insights.get("themes", []),
        "comments_captured_at": updated.get("comments_captured_at"),
        "capture_sha256": _sha256(capture),
        "capture_bytes": int(capture.stat().st_size),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    parser.add_argument("--release", required=True)
    parser.add_argument("--capture", type=Path, required=True)
    parser.add_argument("--captured-at", required=True, help="ISO-8601 time when comments were captured")
    parser.add_argument("--title", help="exact published title override")
    parser.add_argument("--note-id", help="native Xiaohongshu note ID override")
    args = parser.parse_args()
    try:
        result = import_capture(
            args.ledger,
            args.capture,
            release=args.release,
            captured_at=args.captured_at,
            title=args.title,
            note_id=args.note_id,
        )
    except Exception as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
