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


HOOK_FRAMES = {
    "outcome-led": [
        (
            "core",
            "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。",
            "你会先拿哪一份 Markdown 试放映？评论区说说场景，我会把高频路径排进下一轮打磨。",
        ),
        (
            "workflow",
            "Markdown 写完只是前半段；这次不用再把它搬进 PPT，放映和修改在同一条工作流里。",
            "你会先用课程讲义、组会报告还是技术分享来试？评论区说说场景。",
        ),
        (
            "decision",
            "定稿后还要把 Markdown 复制成 PPT，这一步最磨人；现在不用复制，MD 可以直接上台。",
            "哪一份 Markdown 最适合先试？代码、表格还是公式？评论区告诉我。",
        ),
        (
            "source",
            "写完的 Markdown 不用复制到别的工具；ReadMD 把阅读、修改和放映接成一条路。",
            "你会拿哪类内容先跑一遍完整流程？讲义、论文还是技术笔记？",
        ),
    ],
    "identity-led": [
        (
            "core",
            "如果你要把笔记变成课程讲义、组会报告或论文汇报，就知道重做 PPT 有多烦。ReadMD 把这一步砍掉：Markdown 直接放映。",
            "你下一份要上台的 Markdown 是讲义、组会报告还是论文？评论区说说场景。",
        ),
        (
            "workflow",
            "如果你常写论文或组会报告，就不用再把 Markdown 复制进 PPT；同一份文件可以直接讲。",
            "你的下一场分享是课程、组会还是论文答辩？评论区对号入座。",
        ),
        (
            "decision",
            "要上台讲自己文档的人，最怕格式在复制时走样；MD 这次能直接保留工作流。",
            "你会讲哪一份材料？课程讲义、组会报告还是论文教程？",
        ),
        (
            "source",
            "给要把笔记变成正式汇报的人：不用重建 PPT，Markdown 就是演示入口。",
            "你会先讲哪一类文件？论文、组会记录还是课程讲义？",
        ),
    ],
    "mechanism-curiosity": [
        (
            "core",
            "很多人把 Markdown 写完就停在笔记里；其实同一份文件可以直接上台放映。ReadMD 让写作和演示留在同一条路径。",
            "你想先试哪类内容：代码、表格还是公式？评论区告诉我，我会优先打磨这条路径。",
        ),
        (
            "workflow",
            "写完的 Markdown 为什么能直接放映？因为写作、预览和演示被接成同一条路径。",
            "你想先验证哪一段链路：代码、表格还是公式？评论区选一个。",
        ),
        (
            "decision",
            "它不用把 Markdown 另存为幻灯片；写完后的阅读、修改和放映共用同一个源文件。",
            "你最想保住哪种排版？代码块、表格还是公式？评论区补充场景。",
        ),
        (
            "source",
            "从写作到上台只有一条路径；Markdown 不用导出成 PPT，显示状态由同一份源文件驱动。",
            "你会先用哪个机制试一遍：公式渲染、表格分片还是代码运行？",
        ),
    ],
}

TITLE_FORMULAS = ("#36", "#9", "#22", "#61", "#12")


def _normalize_for_originality(value: str) -> str:
    return re.sub(r"\s+", "", value).casefold()


def text_trigrams(body: str) -> set[str]:
    normalized = _normalize_for_originality(body)
    return {
        hashlib.sha256(normalized[index:index + 3].encode("utf-8")).hexdigest()[:12]
        for index in range(max(0, len(normalized) - 2))
    }


def jaccard_similarity(left: set[str], right: set[str]) -> float:
    if not left and not right:
        return 1.0
    if not left or not right:
        return 0.0
    return len(left & right) / len(left | right)


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
    known_closings = {frame[2] for frames in HOOK_FRAMES.values() for frame in frames}
    paragraphs[1:] = [item for item in paragraphs[1:] if item not in known_closings]
    paragraphs.append(closing)
    return "\n\n".join(paragraphs)


def build_variants(*, story: dict[str, Any], base_metadata: dict[str, Any]) -> list[dict[str, Any]]:
    variants: list[dict[str, Any]] = []
    candidate_map = {item["formula_id"]: item for item in base_metadata.get("title_candidates", [])}
    title_options = [candidate_map[formula_id] for formula_id in TITLE_FORMULAS]
    if len(title_options) != len(TITLE_FORMULAS):
        missing = sorted(set(TITLE_FORMULAS) - set(candidate_map))
        raise ValueError(f"title experiment formulas missing: {missing}")
    for hook_type, frames in HOOK_FRAMES.items():
        for frame_id, opening, closing in frames:
            for title_option in title_options:
                variant = copy.deepcopy(base_metadata)
                base_variant_id = f"{hook_type}__{title_option['formula_id'].lstrip('#')}"
                variant["variant_id"] = base_variant_id if frame_id == "core" else f"{base_variant_id}__{frame_id}"
                variant["copy_frame"] = frame_id
                variant["strategy"] = hook_type
                variant["hook_type"] = hook_type
                variant["title_formula_id"] = title_option["formula_id"]
                variant["title"] = title_option["text"]
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

    def dimension_stats(key: str) -> dict[str, dict[str, Any]]:
        output: dict[str, dict[str, Any]] = {}
        for record in records:
            name = str(record.get(key, ""))
            stats = output.setdefault(name, {
                "publications": 0,
                "impressions": 0,
                "weighted_engagement": 0,
                "score": 0.0,
                "confidence_ok": False,
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
        for stats in output.values():
            stats["score"] = round(stats["weighted_engagement"] / max(stats["impressions"], 1), 6)
            stats["confidence_ok"] = stats["publications"] >= 2 and stats["impressions"] >= 1000
        return output

    hook_stats = dimension_stats("hook_type")
    formula_stats = dimension_stats("title_formula_id")
    hook_usage = {
        str(record.get("hook_type", "")): sum(
            1 for item in records if str(item.get("hook_type", "")) == str(record.get("hook_type", ""))
        )
        for record in records
    }
    max_hook_score = max((item["score"] for item in hook_stats.values()), default=0.0)
    max_formula_score = max((item["score"] for item in formula_stats.values()), default=0.0)

    prior_fingerprints = [
        {key: record.get(key) for key in ("release", "body_sha256", "opening", "closing", "body_trigrams")}
        for record in records
        if record.get("body_sha256") or record.get("opening") or record.get("closing") or record.get("body_trigrams")
    ]
    used_openings = {str(record["opening"]) for record in records if record.get("opening")}
    used_closings = {str(record["closing"]) for record in records if record.get("closing")}

    def remaining_frame_count(hook_type: str) -> int:
        return sum(
            1
            for _, opening, closing in HOOK_FRAMES[hook_type]
            if _normalize_for_originality(opening) not in used_openings
            and _normalize_for_originality(closing) not in used_closings
        )

    frame_inventory = {hook_type: remaining_frame_count(hook_type) for hook_type in HOOK_FRAMES}
    max_frame_inventory = max(frame_inventory.values(), default=0)

    ranked: list[dict[str, Any]] = []
    max_body_similarity = 0.0
    similarity_source = ""
    for variant in variants:
        adjustment = 0.0
        reasons = []
        report = dict(variant.pop("_report"))
        fingerprints = text_fingerprints(variant["body"])
        variant_trigrams = text_trigrams(variant["body"])
        originality_failures = []
        for prior in prior_fingerprints:
            release_name = prior.get("release") or "previous release"
            if fingerprints["body_sha256"] and prior.get("body_sha256") == fingerprints["body_sha256"]:
                originality_failures.append(f"body hash matches {release_name}")
            if fingerprints["opening"] and prior.get("opening") == fingerprints["opening"]:
                originality_failures.append(f"opening matches {release_name}")
            if fingerprints["closing"] and prior.get("closing") == fingerprints["closing"]:
                originality_failures.append(f"closing matches {release_name}")
            prior_trigrams = set(prior.get("body_trigrams") or [])
            similarity = jaccard_similarity(variant_trigrams, prior_trigrams)
            if similarity > max_body_similarity:
                max_body_similarity = similarity
                similarity_source = release_name
            if similarity >= 0.85:
                originality_failures.append(
                    f"near-duplicate body ({similarity:.2f}) matches {release_name}"
                )
            elif similarity >= 0.70:
                adjustment -= 12
                reasons.append(f"near-duplicate penalty against {release_name} ({similarity:.2f})")
        if originality_failures:
            report["hard_failures"] = sorted(set(report.get("hard_failures", []) + originality_failures))
            report["ok"] = False
        hook_type = variant["hook_type"]
        available_frames = frame_inventory[hook_type]
        if hook_type in recent_hooks:
            adjustment -= 6
            reasons.append("recent hook fatigue penalty")
        if variant["title_formula_id"] in recent_formulas:
            adjustment -= 4
            reasons.append("recent formula fatigue penalty")
        stat = hook_stats.get(hook_type)
        if stat and stat["confidence_ok"] and max_hook_score:
            bonus = stat["score"] / max_hook_score * 12
            adjustment += bonus
            reasons.append("historical hook performance bonus")
        formula_stat = formula_stats.get(variant["title_formula_id"])
        if formula_stat and formula_stat["confidence_ok"] and max_formula_score:
            bonus = formula_stat["score"] / max_formula_score * 8
            adjustment += bonus
            reasons.append("historical title performance bonus")
        hook_deficit = max(hook_usage.values(), default=0) - hook_usage.get(hook_type, 0)
        coverage_bonus = hook_deficit * 8
        if coverage_bonus:
            adjustment += coverage_bonus
            reasons.append("underexplored dimension coverage bonus")
        inventory_deficit = max_frame_inventory - available_frames
        if inventory_deficit:
            adjustment -= inventory_deficit * 35
            reasons.append(f"scarce copy-frame inventory penalty ({available_frames} remaining)")
        ranked.append({
            "variant_id": variant["variant_id"],
            "strategy": variant["strategy"],
            "title": variant["title"],
            "title_formula_id": variant["title_formula_id"],
            "hook_type": hook_type,
            "copy_frame": variant["copy_frame"],
            "remaining_copy_frames": available_frames,
            "semantic_score": report["total_score"],
            "history_adjustment": round(adjustment, 3),
            "adjusted_score": round(report["total_score"] + adjustment, 3),
            "ok": report["ok"],
            "hard_failures": report["hard_failures"],
            "originality_failures": originality_failures,
            "max_body_similarity": round(max_body_similarity, 3),
            "max_similarity_source": similarity_source,
            "reasons": reasons,
        })

    eligible = [item for item in ranked if item["ok"]]
    if not eligible:
        raise ValueError("no copy variant passes semantic and originality QA; refresh the copy frame pool")
    winner_summary = max(eligible, key=lambda item: item["adjusted_score"])
    winner = next(variant for variant in variants if variant["variant_id"] == winner_summary["variant_id"])
    return winner, {
        "schema_version": 1,
        "chosen_strategy": winner_summary["strategy"],
        "chosen_variant_id": winner_summary["variant_id"],
        "ok": True,
        "originality_gate": "pass",
        "selection_rule": (
            "semantic score plus confidence-gated historical hook/title performance, underexplored "
            "dimension coverage, renewable frame inventory, and minus recent fatigue; insufficient "
            "evidence creates no performance bonus"
        ),
        "copy_frame_inventory": frame_inventory,
        "hook_stats": hook_stats,
        "formula_stats": formula_stats,
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
