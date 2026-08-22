#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Persistent feedback ledger for published Xiaohongshu variants."""
from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
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


def is_pending_record(record: dict[str, Any]) -> bool:
    return record.get("metrics_status") == "pending"


def partition_records(records: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    learning = [record for record in records if not is_pending_record(record)]
    pending = [record for record in records if is_pending_record(record)]
    return learning, pending


def load_learning_records(path: Path) -> list[dict[str, Any]]:
    return partition_records(load_records(path))[0]


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


def upsert_record(path: Path, record: dict[str, Any]) -> dict[str, Any]:
    records = load_records(path)
    existing = next((item for item in records if item.get("release") == record.get("release")), None)
    if not existing:
        return append_record(path, record)
    clean_patch = {key: value for key, value in record.items() if value is not None}
    merged = {**existing, **clean_patch, "release": record["release"]}
    _validate_record(merged)
    return update_record(path, record["release"], clean_patch)


def update_record(path: Path, release: str, patch: dict[str, Any]) -> dict[str, Any]:
    records = load_records(path)
    index = next((index for index, item in enumerate(records) if item.get("release") == release), None)
    if index is None:
        raise ValueError(f"release not found in ledger: {release}")
    clean_patch = {key: value for key, value in patch.items() if value is not None}
    merged = {**records[index], **clean_patch, "release": release, "updated_at": datetime.now(timezone.utc).isoformat()}
    _validate_record(merged)
    records[index] = merged
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.parent.mkdir(parents=True, exist_ok=True)
    with temporary.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")
    temporary.replace(path)
    return merged


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
        "recent_hook_types": [item.get("hook_type") for item in records[-2:]],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    record_parser = commands.add_parser("record")
    record_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    record_parser.add_argument("--record", type=Path, required=True, help="JSON file containing one publication result")
    update_parser = commands.add_parser("update")
    update_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    update_parser.add_argument("--release", required=True)
    update_parser.add_argument("--record", type=Path, required=True, help="JSON file containing metric fields to merge")
    summary_parser = commands.add_parser("summary")
    summary_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    args = parser.parse_args()
    if args.command == "record":
        record = json.loads(args.record.read_text(encoding="utf-8"))
        print(json.dumps(append_record(args.ledger, record), ensure_ascii=False, indent=2))
    elif args.command == "update":
        patch = json.loads(args.record.read_text(encoding="utf-8"))
        print(json.dumps(update_record(args.ledger, args.release, patch), ensure_ascii=False, indent=2))
    else:
        print(json.dumps(summarize(load_records(args.ledger)), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
