#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Persistent feedback ledger for published Xiaohongshu variants."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


METRIC_FIELDS = ("impressions", "likes", "collects", "comments", "shares", "follows")
REQUIRED_FIELDS = ("release", "title", "title_formula_id", "hook_type", "published_at")


def _validate_record(record: dict[str, Any]) -> None:
    missing = [key for key in REQUIRED_FIELDS if not str(record.get(key, "")).strip()]
    if missing:
        raise ValueError("missing publication fields: " + ", ".join(missing))
    for key in METRIC_FIELDS:
        value = record.get(key)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise ValueError(f"{key} must be a non-negative integer")


def load_records(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    records: list[dict[str, Any]] = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise ValueError(f"invalid ledger line {number}: {exc}") from exc
    return records


def append_record(path: Path, record: dict[str, Any]) -> dict[str, Any]:
    clean = dict(record)
    _validate_record(clean)
    records = load_records(path)
    if any(item.get("release") == clean["release"] for item in records):
        raise ValueError(f"release already exists in ledger: {clean['release']}")
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(clean, ensure_ascii=False, sort_keys=True) + "\n")
    return clean


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    formulas: dict[str, dict[str, Any]] = {}
    for record in records:
        formula = str(record.get("title_formula_id", ""))
        stats = formulas.setdefault(formula, {
            "publications": 0,
            "impressions": 0,
            "weighted_engagement": 0,
            "score": 0.0,
        })
        impressions = int(record.get("impressions", 0))
        engagement = (
            int(record.get("likes", 0))
            + int(record.get("collects", 0)) * 2
            + int(record.get("comments", 0)) * 3
            + int(record.get("shares", 0)) * 4
        )
        stats["publications"] += 1
        stats["impressions"] += impressions
        stats["weighted_engagement"] += engagement
    for stats in formulas.values():
        stats["score"] = round(stats["weighted_engagement"] / max(stats["impressions"], 1), 6)
    ranked = sorted(formulas.items(), key=lambda item: (item[1]["score"], item[1]["weighted_engagement"]), reverse=True)
    recommended = ranked[0][0] if ranked else None
    return {
        "schema_version": 1,
        "record_count": len(records),
        "formula_stats": formulas,
        "recommended_formula": recommended,
        "recent_formulas": [item.get("title_formula_id") for item in records[-2:]],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    record_parser = commands.add_parser("record")
    record_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    record_parser.add_argument("--record", type=Path, required=True, help="JSON file containing one publication result")
    summary_parser = commands.add_parser("summary")
    summary_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    args = parser.parse_args()
    if args.command == "record":
        record = json.loads(args.record.read_text(encoding="utf-8"))
        print(json.dumps(append_record(args.ledger, record), ensure_ascii=False, indent=2))
    else:
        print(json.dumps(summarize(load_records(args.ledger)), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
