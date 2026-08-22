#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Persistent feedback ledger for published Xiaohongshu variants."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


METRIC_FIELDS = ("impressions", "likes", "collects", "comments", "shares", "follows")
REQUIRED_FIELDS = ("release", "title", "title_formula_id", "hook_type", "published_at")
IMMUTABLE_FIELDS = (
    "release",
    "title",
    "title_formula_id",
    "hook_type",
    "published_at",
    "variant_id",
    "copy_frame",
    "note_id",
    "publisher_target_id",
    "published_url",
)
METRIC_SOURCES = {"xiaohongshu-web", "manual"}
COMMENT_THEME_TERMS = {
    "presentation": ("放映", "上台", "演示", "ppt", "幻灯"),
    "academic": ("论文", "组会", "课程", "讲义", "学术", "答辩"),
    "code": ("代码", "编程", "脚本", "code"),
    "table": ("表格",),
    "formula": ("公式", "latex", "mathjax"),
    "diagram": ("图表", "流程图", "mermaid", "图形"),
    "conversion": ("转换", "导入", "网页", "word", "pdf"),
    "export-share": ("导出", "分享", "发布", "html"),
    "local-privacy": ("本地", "离线", "隐私", "不上传"),
    "stability-performance": ("卡顿", "崩溃", "性能", "速度", "稳定", "错误"),
}
COMMENT_INTENT_TERMS = {
    "request": ("希望", "能不能", "可以", "建议", "增加", "想要", "需要", "支持"),
    "question": ("吗", "怎么", "如何", "是否", "会不会", "?", "？"),
    "praise": ("好用", "不错", "厉害", "方便", "喜欢", "终于", "强大"),
    "concern": ("问题", "错误", "崩溃", "卡顿", "慢", "丢失", "担心", "兼容"),
}


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
    identity_conflicts = [
        key
        for key in IMMUTABLE_FIELDS
        if key != "release"
        and key in clean_patch
        and records[index].get(key) not in (None, "")
        and records[index].get(key) != clean_patch[key]
    ]
    if identity_conflicts:
        raise ValueError("immutable publication fields cannot change: " + ", ".join(identity_conflicts))
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


def _parse_timestamp(value: str, label: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(str(value).replace("Z", "+00:00"))
    except (TypeError, ValueError) as exc:
        raise ValueError(f"{label} must be an ISO-8601 timestamp") from exc
    if parsed.tzinfo is None:
        raise ValueError(f"{label} must include a timezone")
    return parsed


def import_metric_snapshot(
    path: Path,
    release: str,
    snapshot: dict[str, Any],
    *,
    source: str,
    captured_at: str,
) -> dict[str, Any]:
    """Merge platform counts without changing the published experiment identity."""
    if source not in METRIC_SOURCES:
        raise ValueError(f"metric source must be one of: {', '.join(sorted(METRIC_SOURCES))}")
    captured = _parse_timestamp(captured_at, "captured_at")
    records = load_records(path)
    existing = next((record for record in records if record.get("release") == release), None)
    if existing is None:
        raise ValueError(f"release not found in ledger: {release}")

    conflicts = [
        key
        for key in IMMUTABLE_FIELDS
        if key != "release"
        and key in snapshot
        and existing.get(key) not in (None, "")
        and existing.get(key) != snapshot[key]
    ]
    if conflicts:
        raise ValueError("snapshot conflicts with immutable publication fields: " + ", ".join(conflicts))

    metrics: dict[str, int] = {}
    for key in METRIC_FIELDS:
        if key not in snapshot:
            continue
        value = snapshot[key]
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise ValueError(f"{key} must be a non-negative integer")
        metrics[key] = value

    previous_captured = existing.get("metrics_captured_at")
    if previous_captured and captured <= _parse_timestamp(previous_captured, "metrics_captured_at"):
        raise ValueError("metric snapshot is not newer than the ledger snapshot")

    complete = all(key in metrics for key in METRIC_FIELDS)
    patch: dict[str, Any] = {
        **metrics,
        "metrics_status": "complete" if complete else "pending",
        "metrics_source": source,
        "metrics_captured_at": captured.isoformat(),
    }
    if "audit_status" in snapshot and str(snapshot["audit_status"]).strip():
        patch["audit_status"] = str(snapshot["audit_status"]).strip()
    return update_record(path, release, patch)


def _normalize_comment(value: str) -> str:
    return re.sub(r"\s+", "", str(value)).casefold()


def _comment_hash(value: str) -> str:
    return hashlib.sha256(_normalize_comment(value).encode("utf-8")).hexdigest()[:16]


def _comment_themes(text: str) -> list[str]:
    lowered = _normalize_comment(text)
    return [name for name, terms in COMMENT_THEME_TERMS.items() if any(term.lower() in lowered for term in terms)] or ["general"]


def _comment_intents(text: str) -> list[str]:
    lowered = _normalize_comment(text)
    return [name for name, terms in COMMENT_INTENT_TERMS.items() if any(term.lower() in lowered for term in terms)] or ["observation"]


def import_comment_snapshot(
    path: Path,
    release: str,
    snapshot: dict[str, Any],
    *,
    source: str,
    captured_at: str,
) -> dict[str, Any]:
    """Convert public comments into anonymized resonance themes without storing raw text."""
    if source not in METRIC_SOURCES:
        raise ValueError(f"comment source must be one of: {', '.join(sorted(METRIC_SOURCES))}")
    captured = _parse_timestamp(captured_at, "captured_at")
    records = load_records(path)
    existing = next((record for record in records if record.get("release") == release), None)
    if existing is None:
        raise ValueError(f"release not found in ledger: {release}")

    conflicts = [
        key
        for key in IMMUTABLE_FIELDS
        if key != "release"
        and key in snapshot
        and existing.get(key) not in (None, "")
        and existing.get(key) != snapshot[key]
    ]
    if conflicts:
        raise ValueError("snapshot conflicts with immutable publication fields: " + ", ".join(conflicts))

    comments = snapshot.get("comments")
    if not isinstance(comments, list):
        raise ValueError("snapshot.comments must be a list")

    unique_comments: dict[str, tuple[str, int]] = {}
    for item in comments:
        if not isinstance(item, dict):
            raise ValueError("each comment must be an object")
        text = str(item.get("text", "")).strip()
        if not text or len(text) > 500:
            raise ValueError("comment text must contain 1-500 characters")
        likes = item.get("likes", 0)
        if not isinstance(likes, int) or isinstance(likes, bool) or likes < 0:
            raise ValueError("comment likes must be a non-negative integer")
        digest = _comment_hash(text)
        previous_likes = unique_comments.get(digest, (text, -1))[1]
        if likes > previous_likes:
            unique_comments[digest] = (text, likes)

    theme_stats: dict[str, dict[str, Any]] = {}
    for text, likes in unique_comments.values():
        weight = likes + 1
        intents = _comment_intents(text)
        for theme in _comment_themes(text):
            stats = theme_stats.setdefault(theme, {"mentions": 0, "weighted_score": 0, "intents": {}})
            stats["mentions"] += 1
            stats["weighted_score"] += weight
            for intent in intents:
                stats["intents"][intent] = stats["intents"].get(intent, 0) + 1

    themes = [
        {
            "theme": theme,
            "mentions": stats["mentions"],
            "weighted_score": stats["weighted_score"],
            "intents": sorted(stats["intents"]),
        }
        for theme, stats in sorted(
            theme_stats.items(),
            key=lambda item: (-item[1]["weighted_score"], -item[1]["mentions"], item[0]),
        )
    ]
    insights = {
        "schema_version": 1,
        "imported_count": len(comments),
        "unique_count": len(unique_comments),
        "themes": themes,
        "top_theme": themes[0]["theme"] if themes else None,
        "evidence_hashes": sorted(unique_comments),
    }

    previous_captured = existing.get("comments_captured_at")
    if previous_captured and captured <= _parse_timestamp(previous_captured, "comments_captured_at"):
        raise ValueError("comment snapshot is not newer than the ledger snapshot")

    patch = {
        "comment_insights": insights,
        "comments_source": source,
        "comments_captured_at": captured.isoformat(),
    }
    return update_record(path, release, patch)


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
    metrics_parser = commands.add_parser("metrics")
    metrics_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    metrics_parser.add_argument("--release", required=True)
    metrics_parser.add_argument("--record", type=Path, required=True, help="JSON file containing platform metric counts")
    metrics_parser.add_argument("--source", choices=sorted(METRIC_SOURCES), required=True)
    metrics_parser.add_argument("--captured-at", required=True)
    comments_parser = commands.add_parser("comments")
    comments_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    comments_parser.add_argument("--release", required=True)
    comments_parser.add_argument("--record", type=Path, required=True, help="JSON file containing a public comment snapshot")
    comments_parser.add_argument("--source", choices=sorted(METRIC_SOURCES), required=True)
    comments_parser.add_argument("--captured-at", required=True)
    summary_parser = commands.add_parser("summary")
    summary_parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    args = parser.parse_args()
    if args.command == "record":
        record = json.loads(args.record.read_text(encoding="utf-8"))
        print(json.dumps(append_record(args.ledger, record), ensure_ascii=False, indent=2))
    elif args.command == "update":
        patch = json.loads(args.record.read_text(encoding="utf-8"))
        print(json.dumps(update_record(args.ledger, args.release, patch), ensure_ascii=False, indent=2))
    elif args.command == "metrics":
        snapshot = json.loads(args.record.read_text(encoding="utf-8"))
        updated = import_metric_snapshot(
            args.ledger,
            args.release,
            snapshot,
            source=args.source,
            captured_at=args.captured_at,
        )
        print(json.dumps(updated, ensure_ascii=False, indent=2))
    elif args.command == "comments":
        snapshot = json.loads(args.record.read_text(encoding="utf-8"))
        updated = import_comment_snapshot(
            args.ledger,
            args.release,
            snapshot,
            source=args.source,
            captured_at=args.captured_at,
        )
        print(json.dumps(updated, ensure_ascii=False, indent=2))
    else:
        print(json.dumps(summarize(load_records(args.ledger)), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
