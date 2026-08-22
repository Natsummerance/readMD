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
        name = str(record.get(key, ""))
        if not name.strip():
            continue
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


def _recommended(stats: dict[str, dict[str, Any]]) -> str | None:
    return next((name for name, item in stats.items() if item["confidence"] != "low"), None)


def _comment_focus(records: list[dict[str, Any]]) -> dict[str, Any]:
    aggregated: dict[str, dict[str, Any]] = {}
    comment_releases: set[str] = set()
    for record in records:
        insights = record.get("comment_insights")
        if not isinstance(insights, dict):
            continue
        release = str(record.get("release", ""))
        comment_releases.add(release)
        for item in insights.get("themes", []):
            if not isinstance(item, dict):
                continue
            theme = str(item.get("theme", "general"))
            stats = aggregated.setdefault(theme, {
                "releases": set(),
                "mentions": 0,
                "weighted_score": 0,
                "confidence": "low",
            })
            stats["releases"].add(release)
            stats["mentions"] += int(item.get("mentions", 0))
            stats["weighted_score"] += int(item.get("weighted_score", 0))

    themes: dict[str, dict[str, Any]] = {}
    for theme, stats in aggregated.items():
        release_count = len(stats["releases"])
        confidence = "low"
        if release_count >= 3 and stats["weighted_score"] >= 8:
            confidence = "high"
        elif release_count >= 2 and stats["weighted_score"] >= 3:
            confidence = "medium"
        themes[theme] = {
            "release_count": release_count,
            "mentions": stats["mentions"],
            "weighted_score": stats["weighted_score"],
            "confidence": confidence,
        }

    ordered = sorted(
        themes.items(),
        key=lambda item: (
            item[1]["confidence"] == "low",
            -item[1]["weighted_score"],
            -item[1]["mentions"],
            item[0],
        ),
    )
    recommended = next((theme for theme, stats in ordered if stats["confidence"] != "low"), None)
    return {
        "schema_version": 1,
        "comment_release_count": len(comment_releases),
        "themes": themes,
        "recommended_theme": recommended,
        "confidence": next((stats["confidence"] for theme, stats in ordered if theme == recommended), "low"),
    }


def generate_report(records: list[dict[str, Any]], output_dir: Path) -> dict[str, Any]:
    learning, pending = partition_records(records)
    formula_stats = _stats(learning, "title_formula_id")
    hook_stats = _stats(learning, "hook_type")
    frame_stats = _stats(learning, "copy_frame")
    comment_focus = _comment_focus(learning)
    recommended_formula = _recommended(formula_stats)
    recommended_hook_type = _recommended(hook_stats)
    recommended_copy_frame = _recommended(frame_stats)
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
        "frame_stats": frame_stats,
        "recommended_formula": recommended_formula,
        "recommended_hook_type": recommended_hook_type,
        "recommended_copy_frame": recommended_copy_frame,
        "comment_focus": comment_focus,
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
    lines.extend(["", "## Copy frames", "", "| Frame | Publications | Impressions | Weighted engagement | Score | Confidence |", "| --- | ---: | ---: | ---: | ---: | --- |"])
    for frame, stats in frame_stats.items():
        lines.append(f"| {frame} | {stats['publications']} | {stats['impressions']} | {stats['weighted_engagement']} | {stats['score']} | {stats['confidence']} |")
    lines.extend([
        "",
        "## Comment focus",
        "",
        "| Theme | Releases | Mentions | Weighted score | Confidence |",
        "| --- | ---: | ---: | ---: | --- |",
    ])
    for theme, stats in sorted(comment_focus["themes"].items(), key=lambda item: (-item[1]["weighted_score"], item[0])):
        lines.append(f"| {theme} | {stats['release_count']} | {stats['mentions']} | {stats['weighted_score']} | {stats['confidence']} |")
    lines.extend([
        "",
        "## Next selection",
        "",
        f"- Preferred title formula: `{recommended_formula or 'insufficient evidence'}`",
        f"- Preferred hook type: `{recommended_hook_type or 'insufficient evidence'}`",
        f"- Preferred copy frame: `{recommended_copy_frame or 'insufficient evidence'}`",
        f"- Preferred comment focus: `{comment_focus.get('recommended_theme') or 'insufficient evidence'}`",
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
