#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Build and rank complete Xiaohongshu copy variants from one fact core."""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from audit_copy import audit_copy
from content_memory import load_learning_records, partition_records, summarize


OPENINGS = {
    "outcome-led": (
        "#36",
        "outcome-led",
        "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。",
        "你会先拿哪一份 Markdown 试放映？评论区说说场景，我会把高频路径排进下一轮打磨。",
    ),
    "identity-led": (
        "#22",
        "identity-led",
        "如果你要把笔记变成课程讲义、组会报告或论文汇报，就知道重做 PPT 有多烦。ReadMD 把这一步砍掉：Markdown 直接放映。",
        "你下一份要上台的 Markdown 是讲义、组会报告还是论文？评论区说说场景。",
    ),
    "mechanism-curiosity": (
        "#9",
        "mechanism-curiosity",
        "很多人把 Markdown 写完就停在笔记里；其实同一份文件可以直接上台放映。ReadMD 让写作和演示留在同一条路径。",
        "你想先试哪类内容：代码、表格还是公式？评论区告诉我，我会优先打磨这条路径。",
    ),
}

TITLES = {
    "outcome-led": "不用重做PPT，Markdown直接放映",
    "identity-led": "给要上台讲文档的人做的MD直接放映工具",
    "mechanism-curiosity": "Markdown写完，居然能直接上台",
}


def _normalize_for_originality(value: str) -> str:
    return re.sub(r"\s+", "", value).casefold()


def text_fingerprints(body: str) -> dict[str, str]:
    paragraphs = [item.strip() for item in body.split("\n\n") if item.strip()]
    return {
        "body_sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
        "opening": _normalize_for_originality(paragraphs[0] if paragraphs else ""),
        "closing": _normalize_for_originality(paragraphs[-1] if paragraphs else ""),
    }


def projected_composition(story: dict[str, Any]) -> dict[str, Any]:
    cards: list[dict[str, Any]] = [{
        "file": "xhs-01-cover.jpg",
        "role": "cover",
        "ui_min_ratio": 0,
        "ui_area_ratio": 0.35,
    }]
    for shot_id in story.get("selected_shots", []):
        role = "pure_ui_hero" if shot_id == "overview.reader" else "annotated_ui"
        minimum = 0.7 if role == "pure_ui_hero" else 0.55
        cards.append({
            "file": f"xhs-{len(cards) + 1:02d}-{shot_id.replace('.', '-')}.jpg",
            "role": role,
            "ui_min_ratio": minimum,
            "ui_area_ratio": minimum + 0.03,
        })
    cards.append({
        "file": f"xhs-{len(cards) + 1:02d}-summary.jpg",
        "role": "summary",
        "ui_min_ratio": 0.3,
        "ui_area_ratio": 0.34,
    })
    return {
        "overflow_errors": [],
        "design_audit": {"contrast_errors": [], "small_text": [], "images_failed": []},
        "cards": cards,
    }


def _replace_paragraphs(body: str, opening: str, closing: str) -> str:
    paragraphs = [item.strip() for item in body.split("\n\n") if item.strip()]
    paragraphs[0] = opening
    # A shorter source document may have padded trailing facts after the base CTA.
    # Remove every known strategy CTA before adding this variant's closing question.
    known_closings = {closing for _, _, _, closing in OPENINGS.values()}
    paragraphs[1:] = [item for item in paragraphs[1:] if item not in known_closings]
    paragraphs.append(closing)
    return "\n\n".join(paragraphs)


def build_variants(*, story: dict[str, Any], base_metadata: dict[str, Any]) -> list[dict[str, Any]]:
    variants: list[dict[str, Any]] = []
    candidates = {item["formula_id"]: item["text"] for item in base_metadata.get("title_candidates", [])}
    for strategy, (formula_id, hook_type, opening, closing) in OPENINGS.items():
        variant = copy.deepcopy(base_metadata)
        variant["strategy"] = strategy
        variant["hook_type"] = hook_type
        variant["title_formula_id"] = formula_id
        variant["title"] = TITLES.get(strategy, candidates.get(formula_id, variant["title"]))
        if len(variant["title"]) > 20:
            raise ValueError(f"variant title exceeds 20 characters: {variant['title']}")
        variant["body"] = _replace_paragraphs(variant["body"], opening, closing)
        report = audit_copy(
            story=story,
            metadata=variant,
            composition=projected_composition(story),
        )
        variant["_report"] = report
        variants.append(variant)
    return variants


def choose_variant(
    variants: list[dict[str, Any]],
    history: list[dict[str, Any]] | None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    records = history or []
    records, _pending_records = partition_records(records)
    summary = summarize(records)
    recent_hooks = set(summary.get("recent_hook_types", []))
    recent_formulas = set(summary.get("recent_formulas", []))
    hook_stats: dict[str, dict[str, Any]] = {}
    for record in records:
        hook_type = str(record.get("hook_type", ""))
        stats = hook_stats.setdefault(hook_type, {"impressions": 0, "weighted_engagement": 0})
        impressions = int(record.get("impressions", 0))
        engagement = (
            int(record.get("likes", 0))
            + int(record.get("collects", 0)) * 2
            + int(record.get("comments", 0)) * 3
            + int(record.get("shares", 0)) * 4
        )
        stats["impressions"] += impressions
        stats["weighted_engagement"] += engagement
    max_score = max((item["weighted_engagement"] / max(item["impressions"], 1) for item in hook_stats.values()), default=0.0)

    prior_fingerprints = [
        {key: record.get(key) for key in ("release", "body_sha256", "opening", "closing")}
        for record in records
        if record.get("body_sha256") or record.get("opening") or record.get("closing")
    ]

    ranked: list[dict[str, Any]] = []
    for variant in variants:
        report = dict(variant.pop("_report"))
        fingerprints = text_fingerprints(variant["body"])
        originality_failures = []
        for prior in prior_fingerprints:
            release_name = prior.get("release") or "previous release"
            if fingerprints["body_sha256"] and prior.get("body_sha256") == fingerprints["body_sha256"]:
                originality_failures.append(f"body hash matches {release_name}")
            if fingerprints["opening"] and prior.get("opening") == fingerprints["opening"]:
                originality_failures.append(f"opening matches {release_name}")
            if fingerprints["closing"] and prior.get("closing") == fingerprints["closing"]:
                originality_failures.append(f"closing matches {release_name}")
        if originality_failures:
            report["hard_failures"] = sorted(set(report.get("hard_failures", []) + originality_failures))
            report["ok"] = False
        adjustment = 0.0
        reasons = []
        hook_type = variant["hook_type"]
        if hook_type in recent_hooks:
            adjustment -= 6
            reasons.append("recent hook fatigue penalty")
        if variant["title_formula_id"] in recent_formulas:
            adjustment -= 4
            reasons.append("recent formula fatigue penalty")
        stat = hook_stats.get(hook_type)
        if stat and max_score:
            bonus = (stat["weighted_engagement"] / max(stat["impressions"], 1)) / max_score * 15
            adjustment += bonus
            reasons.append("historical hook performance bonus")
        ranked.append({
            "strategy": variant["strategy"],
            "title": variant["title"],
            "title_formula_id": variant["title_formula_id"],
            "hook_type": hook_type,
            "semantic_score": report["total_score"],
            "history_adjustment": round(adjustment, 3),
            "adjusted_score": round(report["total_score"] + adjustment, 3),
            "ok": report["ok"],
            "hard_failures": report["hard_failures"],
            "originality_failures": originality_failures,
            "reasons": reasons,
        })

    eligible = [item for item in ranked if item["ok"]]
    if not eligible:
        raise ValueError("no copy variant passes semantic QA")
    winner_summary = max(eligible, key=lambda item: item["adjusted_score"])
    winner = next(variant for variant in variants if variant["strategy"] == winner_summary["strategy"])
    return winner, {
        "schema_version": 1,
        "chosen_strategy": winner_summary["strategy"],
        "ok": True,
        "originality_gate": "pass",
        "selection_rule": "semantic score plus historical hook performance minus recent fatigue",
        "ranked": [item for item in ranked if "_report" not in item],
    }


def select_variant(
    *,
    story: dict[str, Any],
    base_metadata: dict[str, Any],
    history: list[dict[str, Any]] | None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    variants = build_variants(story=story, base_metadata=base_metadata)
    chosen, selection = choose_variant(variants, history)
    for variant in variants:
        variant.pop("_report", None)
    selection["variants"] = variants
    return chosen, selection


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--story", type=Path, required=True)
    parser.add_argument("--history", type=Path)
    args = parser.parse_args()
    metadata = json.loads((args.package / "metadata.json").read_text(encoding="utf-8"))
    story = json.loads(args.story.read_text(encoding="utf-8"))
    history = load_learning_records(args.history) if args.history else []
    chosen, selection = select_variant(story=story, base_metadata=metadata, history=history)
    (args.package / "metadata.json").write_text(json.dumps(chosen, ensure_ascii=False, indent=2), encoding="utf-8")
    (args.package / "variants.json").write_text(json.dumps(selection, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"chosen_strategy": chosen["strategy"], "selection": selection}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
