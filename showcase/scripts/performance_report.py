#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Turn the publication ledger into a local performance review."""
from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from content_memory import load_records, partition_records


def _engagement(record: dict[str, Any]) -> int:
    return (
        int(record.get("likes", 0))
        + int(record.get("collects", 0)) * 2
        + int(record.get("comments", 0)) * 3
        + int(record.get("shares", 0)) * 4
    )


def _stats(records: list[dict[str, Any]], key: str) -> dict[str, dict[str, Any]]:
    output: dict[str, dict[str, Any]] = {}
    for record in records:
        name = str(record.get(key, "unknown"))
        stats = output.setdefault(name, {
            "publications": 0,
            "impressions": 0,
            "weighted_engagement": 0,
            "score": 0.0,
            "confidence": "low",
        })
        impressions = int(record.get("impressions", 0))
        engagement = _engagement(record)
        stats["publications"] += 1
        stats["impressions"] += impressions
        stats["weighted_engagement"] += engagement
    for stats in output.values():
        stats["score"] = round(stats["weighted_engagement"] / max(stats["impressions"], 1), 6)
        if stats["impressions"] >= 1000 and stats["publications"] >= 2:
            stats["confidence"] = "medium"
        if stats["impressions"] >= 3000 and stats["publications"] >= 3:
            stats["confidence"] = "high"
    return dict(sorted(output.items(), key=lambda item: item[1]["score"], reverse=True))


def generate_report(records: list[dict[str, Any]], output_dir: Path) -> dict[str, Any]:
    learning, pending = partition_records(records)
    formula_stats = _stats(learning, "title_formula_id")
    hook_stats = _stats(learning, "hook_type")
    recommended_formula = next(iter(formula_stats), None)
    recommended_hook_type = next(iter(hook_stats), None)
    total_impressions = sum(int(record.get("impressions", 0)) for record in learning)
    total_engagement = sum(_engagement(record) for record in learning)
    data = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "learning_count": len(learning),
        "pending_count": len(pending),
        "total_impressions": total_impressions,
        "total_weighted_engagement": total_engagement,
        "formula_stats": formula_stats,
        "hook_stats": hook_stats,
        "recommended_formula": recommended_formula,
        "recommended_hook_type": recommended_hook_type,
        "pending_releases": [
            {"release": item.get("release"), "title": item.get("title"), "title_formula_id": item.get("title_formula_id")}
            for item in pending
        ],
    }
    lines = [
        "# ReadMD Publication Performance Review",
        "",
        f"Generated: {data['generated_at']}",
        "",
        f"Learning inputs: {len(learning)} releases, {total_impressions} impressions.",
        f"Pending metrics: {len(pending)} releases are excluded from learning.",
        "",
        "## Title formulas",
        "",
        "| Formula | Publications | Impressions | Weighted engagement | Score | Confidence |",
        "| --- | ---: | ---: | ---: | ---: | --- |",
    ]
    for formula, stats in formula_stats.items():
        lines.append(f"| {formula} | {stats['publications']} | {stats['impressions']} | {stats['weighted_engagement']} | {stats['score']} | {stats['confidence']} |")
    lines.extend(["", "## Hook types", "", "| Hook | Publications | Impressions | Weighted engagement | Score | Confidence |", "| --- | ---: | ---: | ---: | ---: | --- |"])
    for hook, stats in hook_stats.items():
        lines.append(f"| {hook} | {stats['publications']} | {stats['impressions']} | {stats['weighted_engagement']} | {stats['score']} | {stats['confidence']} |")
    lines.extend([
        "",
        "## Next selection",
        "",
        f"- Preferred title formula: `{recommended_formula or 'insufficient data'}`",
        f"- Preferred hook type: `{recommended_hook_type or 'insufficient data'}`",
        "- Pending metrics remain excluded from learning until they are marked complete.",
        "",
    ])
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "performance-report.md").write_text("\n".join(lines), encoding="utf-8")
    (output_dir / "performance-report.json").write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    return {**data, "ok": True}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).parents[1] / "reports")
    args = parser.parse_args()
    report = generate_report(load_records(args.ledger), args.output_dir)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
