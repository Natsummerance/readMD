#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Score Xiaohongshu copy against the local hot-post mechanism rubric."""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

from style_audit import audit_style
from copy_profiles import (
    IMPLEMENTATION_JARGON,
    MECHANISM_TOPIC_MARKERS,
    EXPERIMENT_TITLE_FORMULAS,
    profile_for_story,
    title_formula_errors,
    title_semantic_errors,
)


BANNED = ("公众号", "微信", "闲鱼", "咸鱼", "转卖", "出票", "转让", "售票", "二维码", "淘口令", "淘宝")
CLICHES = ("重磅升级", "效率起飞", "颠覆想象", "革命性", "无缝体验", "next-gen", "revolutionary")


def _sentences(text: str) -> list[str]:
    return [item.strip() for item in re.split(r"[。！？\n]+", text) if item.strip()]


def _repeated_sentence(text: str) -> list[str]:
    counts: dict[str, int] = {}
    for sentence in _sentences(text):
        if len(sentence) >= 8:
            counts[sentence] = counts.get(sentence, 0) + 1
    return [sentence for sentence, count in counts.items() if count > 1]


def _score_title(metadata: dict[str, Any], story: dict[str, Any]) -> tuple[int, list[str]]:
    title = str(metadata.get("title", ""))
    formula_id = str(metadata.get("title_formula_id", ""))
    score = 0.0
    failures = title_formula_errors(title, formula_id)
    if len(title) <= 20:
        score += 4
    if re.fullmatch(r"#\d+", formula_id) and formula_id in EXPERIMENT_TITLE_FORMULAS:
        score += 4
    # Formula families carry a psychological trigger even when the surface wording
    # relies on identity (#22), curiosity (#9), or number anchoring (#12).
    formula_signal = formula_id in {"#9", "#12", "#22", "#26", "#36", "#61"}
    tensions = sum(term in title.lower() for term in ("不用", "别再", "居然", "直接", "看完", "重新", "上台", "ppt", "md", "markdown"))
    score += 7 if tensions >= 2 else (4 if tensions == 1 or formula_signal else 0)
    failures.extend(title_semantic_errors(title, str(story.get("primary_shot", ""))))
    return round(score), failures


def _score_hook(body: str, story: dict[str, Any]) -> tuple[int, list[str]]:
    first = body.split("\n\n", 1)[0]
    contract = profile_for_story(story)["hook_contract"]
    score = 0.0
    if any(term in first for term in contract["task"]):
        score += 5
    if any(term in first for term in contract["removal"]):
        score += 5
    if any(term.lower() in first.lower() for term in contract["mechanism"]):
        score += 5
    return round(score), []


def _score_focus(story: dict[str, Any], body: str) -> tuple[int, list[str]]:
    selected = story.get("selected_shots", [])
    primary = story.get("primary_shot")
    features = [item for item in selected if item not in {"overview.reader", "overview.editor", "convert.home"}]
    score = 0.0
    if len(selected) > 1 and selected[1] == primary:
        score += 8
    mechanism_visible = (
        story.get("primary_shot") == "presentation.reveal"
        and "放映" in body
        and "Markdown" in body
    )
    if mechanism_visible or str(story.get("angle", ""))[:12] in body:
        score += 6
    score += 6 if len(features) <= 3 else 3
    return round(score), []


def _score_evidence(story: dict[str, Any], body: str) -> tuple[int, list[str]]:
    claims = story.get("claims", [])
    score = 0.0
    if claims and all(item.get("sources") for item in claims):
        score += 7
    score += 5 if body.count("真实运行状态") == 1 else (2 if "真实运行" in body else 0)
    score += 3 if any("release/" in source or "README" in source for claim in claims for source in claim.get("sources", [])) else 0
    return round(score), []


def _score_voice(story: dict[str, Any], metadata: dict[str, Any], body: str) -> tuple[int, list[str]]:
    repeated = _repeated_sentence(body)
    score = 0.0
    if not repeated:
        score += 6
    if not any(term in body.lower() for term in CLICHES):
        score += 3
    state_count = body.count("预览版") + body.count("更新线")
    if story.get("version_state") == "prerelease":
        score += 3 if state_count == 1 else 0
    else:
        score += 3 if "正式版" in body else 0
    score += 3 if body.count("!") + body.count("！") <= 1 else 0
    return round(score), []


def _score_visual(composition: dict[str, Any]) -> tuple[int, list[str]]:
    cards = composition.get("cards", [])
    audit = composition.get("design_audit", {})
    score = 0.0
    if not any(audit.get(key) for key in ("contrast_errors", "small_text", "images_failed")):
        score += 5
    if not composition.get("overflow_errors"):
        score += 3
    if cards and all(float(card.get("ui_area_ratio", 0)) + 0.01 >= float(card.get("ui_min_ratio", 0)) for card in cards):
        score += 5
    score += 2 if len(cards) > 1 and cards[1].get("role") == "pure_ui_hero" else 0
    return round(score), []


def _score_compliance(metadata: dict[str, Any]) -> tuple[int, list[str]]:
    topics = metadata.get("topics", [])
    body = str(metadata.get("body", ""))
    score = 0.0
    if isinstance(topics, list) and len(topics) == 5 and all(not str(topic).startswith("#") for topic in topics):
        score += 2
    if not re.search(r"https?://|www\.", body, re.I) and not any(term in body for term in BANNED):
        score += 2
    score += 1 if "\n\n" in body else 0
    return round(score), []


def audit_copy(*, story: dict[str, Any], metadata: dict[str, Any], composition: dict[str, Any]) -> dict[str, Any]:
    style_report = audit_style(str(metadata.get("body", "")), audience="程序员")
    title_report = _score_title(metadata, story)
    scores = {
        "title": title_report[0],
        "hook": _score_hook(str(metadata.get("body", "")), story)[0],
        "focus": _score_focus(story, str(metadata.get("body", "")))[0],
        "evidence": _score_evidence(story, str(metadata.get("body", "")))[0],
        "voice": _score_voice(story, metadata, str(metadata.get("body", "")))[0],
        "visual": _score_visual(composition)[0],
        "compliance": _score_compliance(metadata)[0],
    }
    hard_failures: list[str] = []
    hard_failures.extend(title_report[1])
    body = str(metadata.get("body", ""))
    title = str(metadata.get("title", ""))
    cards = composition.get("cards", [])
    if len(title) > 20 or not metadata.get("title_formula_id"):
        hard_failures.append("title contract failed")
    topic_markers = MECHANISM_TOPIC_MARKERS.get(str(story.get("primary_shot", "")), set())
    topics = [str(item) for item in metadata.get("topics", [])]
    if topic_markers and not any(marker in topics for marker in topic_markers):
        hard_failures.append("topics missing mechanism marker")
    declared_counts = [int(match.group(1)) for match in re.finditer(r"(\d+)\s*张", title)]
    if any(count != len(cards) for count in declared_counts):
        hard_failures.append("title carousel count does not match composed cards")
    repeated = _repeated_sentence(body)
    if repeated:
        hard_failures.append("repeated sentences: " + "; ".join(repeated))
    if re.search(r"\.(exe|zip|deb|hap|vsix|appimage)\b", body, re.I):
        hard_failures.append("release asset filename leaked into copy")
    jargon = [term for term in IMPLEMENTATION_JARGON if term.lower() in body.lower()]
    if jargon:
        hard_failures.append("implementation jargon leaked into copy: " + ", ".join(jargon))
    if story.get("version_state") == "prerelease":
        if "预览版" not in body and "更新线" not in body:
            hard_failures.append("prerelease disclosure missing")
        if "正式发布" in body or "正式版" in body:
            hard_failures.append("prerelease uses formal-release wording")
    if any(not claim.get("sources") for claim in story.get("claims", [])):
        hard_failures.append("claim missing evidence")
    decision_rule = str(story.get("decision_rule", ""))
    if decision_rule and decision_rule not in body:
        hard_failures.append("save-worthy decision rule missing from copy")
    for key in ("contrast_errors", "small_text", "images_failed"):
        if composition.get("design_audit", {}).get(key):
            hard_failures.append(f"design audit {key} failed")
    if not style_report["ok"]:
        hard_failures.extend(f"style gate: {item}" for item in style_report["hard_failures"])

    total = sum(scores.values())
    minimum_ok = all(value / weight >= 0.75 for value, weight in zip(scores.values(), (15, 15, 20, 15, 15, 15, 5)))
    ok = not hard_failures and total >= 88 and minimum_ok
    return {
        "schema_version": 1,
        "ok": ok,
        "total_score": total,
        "scores": scores,
        "minimum_dimension_pass": minimum_ok,
        "hard_failures": sorted(hard_failures),
        "style": style_report,
    }


def audit_package(package_dir: Path) -> dict[str, Any]:
    load = lambda name: json.loads((package_dir / name).read_text(encoding="utf-8"))
    report = audit_copy(story=load("story.json"), metadata=load("metadata.json"), composition=load("composition.json"))
    (package_dir / "copy-review.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    report = audit_package(args.package)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
