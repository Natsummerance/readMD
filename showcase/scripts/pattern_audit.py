#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Run machine checks for the reviewed Xiaohongshu hot-post patterns."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from style_audit import AI_CLICHES, GENERIC_ADJECTIVES


TASK_TERMS = ("写完", "复制", "PPT", "格式", "折磨", "讲")
REMOVAL_TERMS = ("砍掉", "不用", "别再", "直接", "省掉")
SCENARIO_TERMS = ("课程", "讲义", "组会", "技术分享", "论文", "汇报")
CONCRETE_ANSWER_TERMS = ("Markdown", "MD", "PPT", "代码", "表格", "公式", "讲义", "组会", "论文")


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _paragraphs(body: str) -> list[str]:
    return [item.strip() for item in body.split("\n\n") if item.strip()]


def _box_ratio(card: dict[str, Any]) -> float:
    canvas_area = 1080 * 1440
    box = card.get("screenshot_box") or {}
    try:
        return float(box.get("width", 0)) * float(box.get("height", 0)) / canvas_area
    except (TypeError, ValueError):
        return 0.0


def _area_ok(card: dict[str, Any]) -> bool:
    try:
        return float(card.get("ui_area_ratio", 0)) + 0.01 >= float(card.get("ui_min_ratio", 0))
    except (TypeError, ValueError):
        return False


def _result(pattern_id: str, ok: bool, evidence: list[str], failures: list[str]) -> dict[str, Any]:
    return {
        "id": pattern_id,
        "ok": ok,
        "evidence": evidence if ok else [],
        "failures": [] if ok else failures,
    }


def audit_patterns(
    *,
    story: dict[str, Any],
    metadata: dict[str, Any],
    composition: dict[str, Any],
    library_path: Path,
) -> dict[str, Any]:
    library = _load(library_path)
    title = str(metadata.get("title", ""))
    body = str(metadata.get("body", ""))
    paragraphs = _paragraphs(body)
    first = paragraphs[0] if paragraphs else ""
    last = paragraphs[-1] if paragraphs else ""
    cards = composition.get("cards", [])
    plan = story.get("card_plan", [])
    lower_body = body.lower()

    def card_for(index: int) -> dict[str, Any]:
        name = plan[index].get("file") if index < len(plan) else ""
        return next((item for item in cards if item.get("file") == name), {})

    cover_ok = len(title) <= 20 and bool(cards) and _box_ratio(cards[0]) >= 0.15
    pain_ok = any(term in first for term in TASK_TERMS) and any(term in first for term in REMOVAL_TERMS)
    product_plan = plan[1] if len(plan) > 1 else {}
    product_card = card_for(1)
    product_ok = (
        product_plan.get("role") == "pure_ui_hero"
        and product_plan.get("shot_id") == "overview.reader"
        and _area_ok(product_card)
        and bool(product_card.get("screenshot_box"))
    )
    selected = story.get("selected_shots", [])
    focus_ok = len(selected) > 1 and selected[1] == story.get("primary_shot")
    generic_hits = [term for term in GENERIC_ADJECTIVES if term in body]
    cliche_hits = [term for term in AI_CLICHES if term.lower() in lower_body]
    outcome_ok = len(generic_hits) < 3 and not cliche_hits
    scenario_hits = [term for term in SCENARIO_TERMS if term in body]
    audience_ok = len(scenario_hits) >= 3
    feature_cards = [
        (plan_item, card_for(index))
        for index, plan_item in enumerate(plan)
        if plan_item.get("role") in {"pure_ui_hero", "annotated_ui"}
    ]
    collectible_ok = bool(feature_cards) and all(
        bool(card.get("screenshot_box")) and _area_ok(card)
        for _, card in feature_cards
    )
    question_ok = ("？" in last or "?" in last) and any(
        term.lower() in last.lower() for term in CONCRETE_ANSWER_TERMS
    )
    design = composition.get("design_audit", {})
    clean_design = not composition.get("overflow_errors") and all(
        not design.get(key) for key in ("contrast_errors", "small_text", "images_failed")
    )
    series_ok = clean_design and 4 <= len(cards) <= 9
    anti_ppt_ok = clean_design and all(_area_ok(card) for card in cards)

    checks = {
        "one-hook-cover": _result(
            "one-hook-cover",
            cover_ok,
            [f"title length {len(title)}", f"cover UI region {_box_ratio(cards[0]) if cards else 0:.2%}"],
            ["cover must have a concise title and at least a 15% authentic UI region"],
        ),
        "pain-to-removal": _result(
            "pain-to-removal",
            pain_ok,
            ["first paragraph names the task and removal action"],
            ["opening must connect a concrete task to an active removal action"],
        ),
        "product-first-proof": _result(
            "product-first-proof",
            product_ok,
            ["card two is the complete overview.reader hero"],
            ["card two must be the pure overview.reader hero"],
        ),
        "single-primary-feature": _result(
            "single-primary-feature",
            focus_ok,
            [f"primary shot {story.get('primary_shot')} leads card three"],
            ["selected shots must place the declared primary shot immediately after the hero"],
        ),
        "outcome-not-adjective": _result(
            "outcome-not-adjective",
            outcome_ok,
            ["no unsupported quality-adjective cluster or launch cliché"],
            [f"generic adjectives: {generic_hits}; clichés: {cliche_hits}"],
        ),
        "reader-task-fit": _result(
            "reader-task-fit",
            audience_ok,
            [f"task scenarios: {scenario_hits}"],
            ["name at least three recognizable reader tasks"],
        ),
        "collectible-clarity": _result(
            "collectible-clarity",
            collectible_ok,
            ["every feature card has one authentic UI region"],
            ["each feature card needs its UI area contract and screenshot region"],
        ),
        "specific-question": _result(
            "specific-question",
            question_ok,
            ["closing prompt asks about a concrete artifact"],
            ["closing prompt must ask a scoped question about a concrete artifact"],
        ),
        "consistent-series-lock": _result(
            "consistent-series-lock",
            series_ok,
            ["DOM overflow and contrast audits are clean"],
            ["the package needs four to nine cards and clean shared design audits"],
        ),
        "anti-ppt-layout": _result(
            "anti-ppt-layout",
            anti_ppt_ok,
            ["cards keep real UI regions without bullet-slide overflow"],
            ["all cards must meet their UI-area contracts without overflow or failed assets"],
        ),
    }

    patterns = list(library.get("patterns", []))
    missing_checks = sorted({item.get("id") for item in patterns} - set(checks))
    unknown_ids = sorted(set(checks) - {item.get("id") for item in patterns})
    results = [_result(item["id"], checks[item["id"]]["ok"], checks[item["id"]]["evidence"], checks[item["id"]]["failures"]) for item in patterns]
    errors = [failure for result in results for failure in result["failures"]]
    if missing_checks:
        errors.append(f"patterns missing runtime checks: {missing_checks}")
    if unknown_ids:
        errors.append(f"runtime checks absent from library: {unknown_ids}")
    return {
        "schema_version": 1,
        "ok": not errors,
        "passed_count": sum(result["ok"] for result in results),
        "total_count": len(results),
        "patterns": results,
        "errors": errors,
    }


def audit_package(package_dir: Path, *, library_path: Path | None = None) -> dict[str, Any]:
    library_path = library_path or Path(__file__).resolve().parents[1] / "content" / "pattern-library.json"
    try:
        report = audit_patterns(
            story=_load(package_dir / "story.json"),
            metadata=_load(package_dir / "metadata.json"),
            composition=_load(package_dir / "composition.json"),
            library_path=library_path,
        )
    except Exception as exc:
        report = {
            "schema_version": 1,
            "ok": False,
            "passed_count": 0,
            "total_count": 0,
            "patterns": [],
            "errors": [f"hot-post pattern audit crashed: {exc}"],
        }
    output = package_dir / "pattern-audit.json"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
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
