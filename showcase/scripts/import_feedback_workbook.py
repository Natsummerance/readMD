#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Import official Xiaohongshu creator workbooks into the feedback ledger."""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import pandas as pd

from content_memory import import_metric_snapshot, load_records


METRIC_COLUMNS = {
    "曝光": "impressions",
    "点赞": "likes",
    "收藏": "collects",
    "评论": "comments",
    "分享": "shares",
    "涨粉": "follows",
}
REQUIRED_COLUMNS = {
    "首次发布时间",
    "笔记标题",
    "体裁",
    *METRIC_COLUMNS,
}
NOTE_ID_COLUMNS = (
    "笔记ID",
    "笔记 ID",
    "小红书笔记ID",
    "小红书笔记 ID",
    "note_id",
)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_creator_workbook(path: Path) -> pd.DataFrame:
    """Read the stable third-header layout emitted by the creator export."""
    if not path.is_file():
        raise FileNotFoundError(f"creator workbook does not exist: {path}")
    frame = pd.read_excel(path, header=1, engine="openpyxl")
    missing = sorted(REQUIRED_COLUMNS - set(frame.columns))
    if missing:
        raise ValueError("creator workbook is missing columns: " + ", ".join(missing))

    frame["笔记标题"] = frame["笔记标题"].map(lambda value: str(value).strip())
    for column in METRIC_COLUMNS:
        numeric = pd.to_numeric(frame[column], errors="coerce")
        invalid = int(numeric.isna().sum())
        if invalid:
            raise ValueError(f"column {column} contains {invalid} non-numeric values")
        frame[column] = numeric.astype(int)
    return frame


def snapshot_from_workbook(
    path: Path,
    *,
    title: str | None = None,
    note_id: str | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Return the immutable metric payload plus a compact provenance summary."""
    frame = read_creator_workbook(path)
    note_id_column = next((column for column in NOTE_ID_COLUMNS if column in frame.columns), None)

    if note_id:
        if note_id_column is None:
            raise ValueError("workbook has no native note-ID column; match by exact title instead")
        matches = frame[frame[note_id_column].map(lambda value: str(value).strip()) == note_id]
        match_key = f"note-id {note_id}"
    else:
        if not title:
            raise ValueError("an exact title or note ID is required")
        matches = frame[frame["笔记标题"] == title.strip()]
        match_key = f"title {title}"

    if matches.empty:
        raise ValueError(f"no creator workbook row matches {match_key}")
    if len(matches) > 1:
        raise ValueError(f"{len(matches)} creator workbook rows match {match_key}; use its note ID")

    row = matches.iloc[0]
    metrics = {
        metric: int(row[column])
        for column, metric in METRIC_COLUMNS.items()
    }
    provenance = {
        "match_key": match_key,
        "matched_rows": int(len(matches)),
        "workbook_sha256": _sha256(path),
        "workbook_bytes": int(path.stat().st_size),
    }
    return metrics, provenance


def import_workbook(
    ledger: Path,
    workbook: Path,
    *,
    release: str,
    captured_at: str,
    title: str | None = None,
    note_id: str | None = None,
) -> dict[str, Any]:
    records = load_records(ledger)
    record = next((item for item in records if item.get("release") == release), None)
    if record is None:
        raise ValueError(f"release not found in feedback ledger: {release}")

    selected_title = title or record.get("title")
    selected_note_id = note_id or record.get("note_id")
    metrics, provenance = snapshot_from_workbook(
        workbook,
        title=str(selected_title) if selected_title else None,
        note_id=str(selected_note_id) if selected_note_id else None,
    )
    updated = import_metric_snapshot(
        ledger,
        release,
        metrics,
        source="xiaohongshu-web",
        captured_at=captured_at,
    )
    return {
        "ok": True,
        "release": release,
        "metrics_status": updated.get("metrics_status"),
        "metrics": metrics,
        "metrics_captured_at": updated.get("metrics_captured_at"),
        **provenance,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    parser.add_argument("--release", required=True)
    parser.add_argument("--workbook", type=Path, required=True)
    parser.add_argument("--captured-at", required=True, help="ISO-8601 time when the workbook was exported")
    parser.add_argument("--title", help="exact published title; defaults to the ledger title")
    parser.add_argument("--note-id", help="native Xiaohongshu note ID; preferred over title")
    args = parser.parse_args()
    try:
        result = import_workbook(
            args.ledger,
            args.workbook,
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
