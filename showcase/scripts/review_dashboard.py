#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Build a self-contained preflight review dashboard for a content package."""
from __future__ import annotations

import argparse
import html
import json
from pathlib import Path
from typing import Any


FORBIDDEN = ("<script", "<img", "<table", "class=", "id=", "http://", "https://")


def _read_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def _escaped(value: Any) -> str:
    return html.escape(str(value), quote=False)


def _status_chip(ok: bool) -> str:
    color = "#157347" if ok else "#c1121f"
    label = "PASS" if ok else "NEEDS FIX"
    return f'<span style="display:inline-block;padding:8px 14px;border-radius:999px;background:{color};color:#ffffff;font-size:22px;font-weight:700">{label}</span>'


def _section(title: str, inner: str) -> str:
    return (
        '<section style="background:#fbfcfd;border:1px solid #d8dee6;border-radius:18px;'
        'padding:28px;margin-bottom:24px">'
        f'<h2 style="margin:0 0 18px;font-size:30px;line-height:1.2;color:#182029">{_escaped(title)}</h2>'
        f"{inner}</section>"
    )


def _metric(label: str, value: Any) -> str:
    return (
        '<div style="flex:1;min-width:180px;background:#f2f4f6;border-radius:14px;padding:20px">'
        f'<p style="margin:0;font-size:21px;color:#5b6875">{_escaped(label)}</p>'
        f'<p style="margin:8px 0 0;font-size:32px;font-weight:800;color:#182029">{_escaped(value)}</p></div>'
    )


def _top_variants(ranked: list[dict[str, Any]], chosen_variant_id: Any, limit: int = 5) -> list[dict[str, Any]]:
    ordered = sorted(
        ranked,
        key=lambda item: (bool(item.get("ok")), float(item.get("adjusted_score", 0))),
        reverse=True,
    )
    chosen = next((item for item in ordered if item.get("variant_id") == chosen_variant_id), None)
    selected = [chosen] if chosen else []
    selected.extend(item for item in ordered if item.get("variant_id") != chosen_variant_id)
    return selected[:limit]


def _comment_resonance_html(comment_focus: dict[str, Any]) -> str:
    themes = [
        {"theme": str(theme), **stats}
        for theme, stats in comment_focus.get("themes", {}).items()
        if isinstance(stats, dict)
    ]
    confident = [
        item for item in themes
        if item.get("confidence") in ("medium", "high")
    ]
    confident.sort(key=lambda item: (
        item.get("confidence") != "high",
        -int(item.get("weighted_score", 0)),
        -int(item.get("mentions", 0)),
        item["theme"],
    ))

    if not confident:
        return '<p style="margin:0;font-size:23px;color:#5b6875">No confident comment evidence yet</p>'

    items = "".join(
        f'<div style="flex:1;min-width:240px;background:#f2f4f6;border-radius:14px;padding:20px">'
        f'<p style="margin:0;font-size:26px;font-weight:800;color:#182029">{_escaped(item["theme"])}</p>'
        f'<p style="margin:8px 0 0;font-size:22px;color:#5b6875">'
        f'{_escaped(int(item.get("mentions", 0)))} mentions · '
        f'weighted {_escaped(int(item.get("weighted_score", 0)))} · '
        f'{_escaped(item.get("confidence"))} confidence</p>'
        f'<p style="margin:6px 0 0;font-size:20px;color:#5b6875">'
        f'{_escaped(int(item.get("release_count", 0)))} releases · '
        f'{_escaped(", ".join(item.get("top_intents") or []) or "general")} · anonymized</p></div>'
        for item in confident[:3]
    )
    return (
        '<div style="display:flex;gap:14px;flex-wrap:wrap">'
        f'{items}</div>'
        '<p style="margin:14px 0 0;font-size:21px;color:#5b6875">'
        'Only medium/high-confidence anonymized comment themes are shown.</p>'
    )


def _contract_field(label: str, value: Any, emphasis: bool = False) -> str:
    value_style = (
        "margin:6px 0 0;font-size:25px;line-height:1.35;font-weight:800;color:#182029"
        if emphasis
        else "margin:6px 0 0;font-size:22px;line-height:1.45;color:#182029"
    )
    return (
        '<div style="min-width:260px;flex:1;background:#ffffff;border:1px solid #d8dee6;'
        'border-radius:14px;padding:20px">'
        f'<p style="margin:0;font-size:19px;letter-spacing:.04em;color:#5b6875">{_escaped(label)}</p>'
        f'<p style="{value_style}">{_escaped(value)}</p></div>'
    )


def _proof_chip(value: Any) -> str:
    return (
        '<span style="display:inline-block;margin:0 8px 8px 0;padding:9px 14px;'
        'background:#f2f4f6;border-top:3px solid #d6482c;border-radius:10px;'
        f'font-size:21px;font-weight:700;color:#182029">{_escaped(value)}</span>'
    )


def _mechanism_contract_html(story: dict[str, Any]) -> str:
    if not isinstance(story, dict) or not story:
        return (
            '<p style="margin:0;font-size:23px;color:#c1121f">'
            'Mechanism contract is unavailable because story.json is missing.</p>'
        )

    primary = str(story.get("primary_shot", "")).strip()
    angle = str(story.get("angle", "")).strip()
    cover = story.get("cover_hook", {}) if isinstance(story.get("cover_hook"), dict) else {}
    summary = story.get("summary_hook", {}) if isinstance(story.get("summary_hook"), dict) else {}
    proof_points = summary.get("proof_points", [])
    proof_points = proof_points if isinstance(proof_points, list) else []
    feature_cards = [
        item for item in story.get("card_plan", [])
        if isinstance(item, dict) and item.get("role") in {"pure_ui_hero", "annotated_ui"}
    ]

    overview = "".join([
        _contract_field("Primary mechanism", primary or "Missing", emphasis=True),
        _contract_field("Cover formula", cover.get("formula_id") or "Missing", emphasis=True),
        _contract_field("Summary headline", summary.get("title") or "Missing", emphasis=True),
    ])
    proof_html = "".join(_proof_chip(point) for point in proof_points) or (
        '<span style="font-size:22px;color:#c1121f">Missing proof points</span>'
    )
    feature_items = "".join(
        '<li style="margin:0 0 12px;list-style:none;background:#f2f4f6;border-radius:12px;'
        'padding:16px 18px;font-size:22px;line-height:1.45;color:#182029">'
        f'<strong style="display:block;font-size:19px;color:#5b6875">{_escaped(item.get("shot_id"))}</strong>'
        f'{_escaped(item.get("caption"))}</li>'
        for item in feature_cards
    ) or '<li style="margin:0;list-style:none;font-size:22px;color:#c1121f">Missing feature captions</li>'
    cover_value = cover.get("caption") or "Missing"
    if cover.get("title"):
        cover_value = f"{cover.get('title')}：{cover_value}"

    return (
        f'<div style="display:flex;gap:14px;flex-wrap:wrap;margin-bottom:18px">{overview}</div>'
        f'{_contract_field("Core narrative", angle or "Missing")}'
        '<div style="margin-top:18px">'
        f'{_contract_field("Cover hook", cover_value)}'
        '</div>'
        f'<p style="margin:22px 0 10px;font-size:22px;font-weight:800;color:#182029">Summary proof points</p>'
        f'<div>{proof_html}</div>'
        f'<p style="margin:24px 0 10px;font-size:22px;font-weight:800;color:#182029">Card reader values</p>'
        f'<ul style="margin:0;padding:0">{feature_items}</ul>'
    )


def _topic_experiment_html(topic_experiment: dict[str, Any], performance: dict[str, Any]) -> str:
    if not isinstance(topic_experiment, dict) or not topic_experiment:
        return (
            '<p style="margin:0;font-size:23px;color:#c1121f">'
            'Topic experiment is unavailable because metadata fields are missing.</p>'
        )

    topics = topic_experiment.get("topics", [])
    topics = topics if isinstance(topics, list) else []
    topic_chips = "".join(_proof_chip(topic) for topic in topics) or (
        '<span style="font-size:22px;color:#c1121f">Missing search terms</span>'
    )
    selection = topic_experiment.get("topic_set_selection", {})
    selection = selection if isinstance(selection, dict) else {}
    topic_set_stats = performance.get("topic_set_stats", {})
    selected_id = str(topic_experiment.get("topic_set_id", ""))
    selected_stat = topic_set_stats.get(selected_id, {}) if isinstance(topic_set_stats, dict) else {}
    recommended_set_id = performance.get("recommended_topic_set")
    recommended_set = topic_set_stats.get(recommended_set_id, {}) if isinstance(topic_set_stats, dict) else {}
    recommended_confidence = str(recommended_set.get("confidence", "low"))
    if recommended_confidence in {"medium", "high"}:
        recommendation = (
            f'{recommended_set_id} · {recommended_set.get("label", "unknown")} · '
            f'search {_escaped(performance.get("recommended_topic") or "insufficient evidence")} · '
            f'{recommended_confidence} confidence'
        )
        recommendation_color = "#157347"
    else:
        recommendation = "No confident topic-set evidence yet"
        recommendation_color = "#5b6875"

    overview = "".join([
        _contract_field("Selected label", topic_experiment.get("topic_set_label") or "Missing", emphasis=True),
        _contract_field("Topic-set ID", topic_experiment.get("topic_set_id") or "Missing", emphasis=True),
        _contract_field(
            "Recommended topic set",
            f"{recommended_set_id} · {recommended_set.get('label', 'unknown')}"
            if recommended_set_id else "Insufficient evidence",
        ),
        _contract_field(
            "Recommended search term",
            performance.get("recommended_topic") or "Insufficient evidence",
        ),
        _contract_field("History samples", selection.get("sample_size", 0)),
    ])
    avoided = selection.get("avoided_topic_sets", [])
    avoided = avoided if isinstance(avoided, list) else []
    avoided_text = ", ".join(str(item) for item in avoided) or "None"

    return (
        f'<div style="display:flex;gap:14px;flex-wrap:wrap;margin-bottom:18px">{overview}</div>'
        f'<p style="margin:0 0 10px;font-size:22px;font-weight:800;color:#182029">Published search terms</p>'
        f'<div>{topic_chips}</div>'
        f'<div style="margin-top:18px">'
        f'{_contract_field("Selection rule", selection.get("strategy") or "Missing")}'
        f'{_contract_field("Recent-fatigue avoids", avoided_text)}'
        '</div>'
        '<div style="margin-top:18px;background:#f2f4f6;border-left:4px solid #d6482c;'
        'border-radius:12px;padding:20px">'
        f'<p style="margin:0;font-size:19px;letter-spacing:.04em;color:#5b6875">Next-release evidence</p>'
        f'<p style="margin:8px 0 0;font-size:24px;line-height:1.4;font-weight:800;'
        f'color:{recommendation_color}">{recommendation}</p></div>'
    )


def build_dashboard(inputs: dict[str, Any]) -> str:
    release = inputs.get("release", "")
    title = inputs.get("title", "")
    strategy = inputs.get("strategy", "unknown")
    body = str(inputs.get("body", "")).strip()
    qa = inputs.get("qa", {})
    copy_review = inputs.get("copy_review", {})
    variants = inputs.get("variants", {})
    wechat_qa = inputs.get("wechat_qa", {})
    pattern_audit = inputs.get("pattern_audit", {})
    performance = inputs.get("performance", {})
    story = inputs.get("story", {})
    topic_experiment = inputs.get("topic_experiment", {})

    gates = [
        ("Package QA", bool(qa.get("ok")), qa.get("errors", [])),
        ("Semantic alignment", bool(copy_review.get("ok")), copy_review.get("hard_failures", [])),
        ("Variant selection", bool(variants.get("ok")), []),
        ("WeChat adapter", bool(wechat_qa.get("ok")), wechat_qa.get("errors", [])),
        ("Hot-post patterns", bool(pattern_audit.get("ok")), pattern_audit.get("errors", [])),
    ]
    all_pass = all(passed for _, passed, _ in gates)
    overall = "PASS" if all_pass else "NEEDS FIX"
    overall_color = "#157347" if all_pass else "#c1121f"
    score = int(copy_review.get("total_score", 0))

    gate_html = "".join(
        f'<div style="margin-bottom:16px"><strong style="font-size:25px;color:#182029">{_escaped(name)}</strong> '
        f'{_status_chip(passed)}<p style="margin:8px 0 0;font-size:23px;color:#5b6875">{_escaped("; ".join(map(str, errors)) or "No blockers")}</p></div>'
        for name, passed, errors in gates
    )

    ranked = variants.get("ranked", [])
    chosen_variant_id = variants.get("chosen_variant_id")
    chosen_strategy = variants.get("chosen_strategy")
    displayed_variants = _top_variants(ranked, chosen_variant_id)
    candidate_count = int(variants.get("candidate_count", len(ranked)))
    frame_inventory = [
        int(value)
        for value in variants.get("copy_frame_inventory", {}).values()
        if str(value).strip()
    ]
    minimum_frame_inventory = min(frame_inventory) if frame_inventory else 0
    experiment_summary = "".join([
        _metric("Experiment candidates", candidate_count),
        _metric("Copy-frame inventory", f"{minimum_frame_inventory} / hook"),
    ])
    variant_html = "".join(
        f'<div style="border:{"2px solid #d6482c" if item.get("variant_id") == chosen_variant_id or (not chosen_variant_id and item.get("strategy") == chosen_strategy) else "1px solid #d8dee6"};'
        f'padding:18px 20px;margin-bottom:14px;background:{"#ffffff" if item.get("variant_id") == chosen_variant_id else "#f2f4f6"};border-radius:12px">'
        f'<p style="margin:0;font-size:26px;font-weight:800;color:#182029">{_escaped(item.get("title"))}</p>'
        f'<p style="margin:8px 0 0;font-size:22px;color:#5b6875">{_escaped(item.get("strategy"))} · {_escaped(item.get("copy_frame", ""))} · {_escaped(item.get("variant_id", ""))}</p>'
        f'<p style="margin:6px 0 0;font-size:21px;color:#5b6875">{_escaped(item.get("remaining_copy_frames", ""))} renewable frames remaining</p>'
        f'<p style="margin:6px 0 0;font-size:23px;font-weight:800;color:#182029">{_escaped(item.get("semantic_score"))} / {_escaped(item.get("adjusted_score"))}</p></div>'
        for item in displayed_variants
    )

    score_items = "".join(
        _metric(name.replace("_", " ").title(), value)
        for name, value in copy_review.get("scores", {}).items()
    )
    style = copy_review.get("style", {})
    if style:
        score_items += _metric("Style resonance", f'{style.get("score", 0)} / 100')
        style_findings = style.get("findings", [])
        finding_html = "".join(
            f'<li style="margin-bottom:8px;font-size:23px;line-height:1.4;color:#5b6875">'
            f'{_escaped(item.get("severity"))}: {_escaped(item.get("message"))}</li>'
            for item in style_findings[-6:]
        )
        style_inner = (
            f'<p style="margin:0 0 14px;font-size:24px;color:#182029">Style resonance {_escaped(style.get("score", 0))} / 100</p>'
            + (f'<ul style="margin:0;padding-left:24px">{finding_html}</ul>' if finding_html else '<p style="margin:0;font-size:23px;color:#157347">No style findings</p>')
        )
    else:
        style_inner = '<p style="margin:0;font-size:23px;color:#5b6875">Style audit not available</p>'
    performance_metrics = "".join([
        _metric("Pattern checks", f'{pattern_audit.get("passed_count", 0)} / {pattern_audit.get("total_count", 0)}'),
        _metric("Learning releases", performance.get("learning_count", 0)),
        _metric("Pending metrics", performance.get("pending_count", 0)),
        _metric("Recommended formula", performance.get("recommended_formula", "insufficient data")),
        _metric("Recommended frame", performance.get("recommended_copy_frame") or "insufficient evidence"),
    ])
    comment_resonance_html = _comment_resonance_html(performance.get("comment_focus", {}))

    return f'''<!doctype html>
<html lang="zh-CN">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{_escaped(title)} · Preflight Review</title></head>
<body style="margin:0;background:#eef1f4;font-family:'Microsoft YaHei','Noto Sans CJK SC',sans-serif;color:#182029">
<main style="max-width:1080px;margin:0 auto;padding:48px 40px">
  <header style="background:#182029;color:#ffffff;border-radius:20px;padding:36px;margin-bottom:28px">
    <p style="margin:0;font-size:22px;letter-spacing:.08em;color:#ffb3a7">READMD PREFLIGHT</p>
    <h1 style="margin:14px 0 10px;font-size:46px;line-height:1.15">{_escaped(title)}</h1>
    <p style="margin:0;font-size:25px;color:#dbe2ea">{_escaped(release)} · {_escaped(strategy)}</p>
    <div style="margin-top:24px;display:flex;align-items:center;gap:18px">
      <span style="display:inline-block;padding:10px 20px;border-radius:999px;background:{overall_color};color:#ffffff;font-size:27px;font-weight:800">{overall}</span>
      <span style="font-size:26px;color:#ffffff">Semantic score {_escaped(score)} / 100</span>
    </div>
  </header>
  {_section("Release gates", gate_html)}
  {_section("Mechanism contract", _mechanism_contract_html(story))}
  {_section("Topic experiment", _topic_experiment_html(topic_experiment, performance))}
  {_section("Semantic dimensions", f'<div style="display:flex;gap:14px;flex-wrap:wrap">{score_items}</div>')}
  {_section("Style resonance", style_inner)}
  {_section("Comment resonance", comment_resonance_html)}
  {_section(
      "Top experiments",
      f'<div style="display:flex;gap:14px;flex-wrap:wrap;margin-bottom:18px">{experiment_summary}</div>'
      f'<p style="margin:0 0 16px;font-size:23px;color:#5b6875">Showing {len(displayed_variants)} of {candidate_count} candidates · selected pinned</p>'
      f'{variant_html}'
  )}
  {_section("Feedback loop", f'<div style="display:flex;gap:14px;flex-wrap:wrap">{performance_metrics}</div><p style="margin-top:16px;font-size:24px;color:#5b6875">Pending metrics: {_escaped(performance.get("pending_count", 0))}</p>')}
  {_section("Final copy", f'<p style="white-space:pre-wrap;margin:0;font-size:26px;line-height:1.55;color:#182029">{_escaped(body)}</p>')}
</main>
</body>
</html>'''


def validate_dashboard(html: str) -> list[str]:
    lowered = html.lower()
    return [item for item in FORBIDDEN if item in lowered]


def collect_inputs(package_dir: Path) -> dict[str, Any]:
    metadata = _read_json(package_dir / "metadata.json")
    story = _read_json(package_dir / "story.json")
    return {
        "story": story,
        "release": metadata.get("release", ""),
        "title": metadata.get("title", ""),
        "strategy": metadata.get("strategy", metadata.get("hook_type", "unknown")),
        "body": (package_dir / "body.txt").read_text(encoding="utf-8").strip() if (package_dir / "body.txt").exists() else "",
        "qa": _read_json(package_dir / "qa.json"),
        "copy_review": _read_json(package_dir / "copy-review.json"),
        "variants": _read_json(package_dir / "variants.json"),
        "wechat_qa": _read_json(package_dir / "wechat" / "wechat-qa.json"),
        "pattern_audit": _read_json(package_dir / "pattern-audit.json"),
        "performance": _read_json(package_dir / "performance-report.json"),
        "topic_experiment": {
            "primary_shot": metadata.get("primary_shot", ""),
            "topics": metadata.get("topics", []),
            "topic_set_id": metadata.get("topic_set_id", ""),
            "topic_set_label": metadata.get("topic_set_label", ""),
            "topic_set_selection": metadata.get("topic_set_selection", {}),
        },
    }


def generate_package(package_dir: Path) -> dict[str, Any]:
    inputs = collect_inputs(package_dir)
    html = build_dashboard(inputs)
    errors = validate_dashboard(html)
    output = package_dir / "review-dashboard.html"
    output.write_text(html, encoding="utf-8")
    report = {
        "schema_version": 1,
        "ok": not errors,
        "errors": errors,
        "overall_status": "PASS" if inputs.get("qa", {}).get("ok") is True else "NEEDS FIX",
        "output": str(output.resolve()),
    }
    (package_dir / "dashboard-qa.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    print(json.dumps(generate_package(args.package), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
