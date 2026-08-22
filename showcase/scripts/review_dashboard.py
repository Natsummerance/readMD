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


def build_dashboard(inputs: dict[str, Any]) -> str:
    release = inputs.get("release", "")
    title = inputs.get("title", "")
    strategy = inputs.get("strategy", "unknown")
    body = str(inputs.get("body", "")).strip()
    qa = inputs.get("qa", {})
    copy_review = inputs.get("copy_review", {})
    variants = inputs.get("variants", {})
    wechat_qa = inputs.get("wechat_qa", {})
    performance = inputs.get("performance", {})

    gates = [
        ("Package QA", bool(qa.get("ok")), qa.get("errors", [])),
        ("Semantic alignment", bool(copy_review.get("ok")), copy_review.get("hard_failures", [])),
        ("Variant selection", bool(variants.get("ok")), []),
        ("WeChat adapter", bool(wechat_qa.get("ok")), wechat_qa.get("errors", [])),
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
    variant_html = "".join(
        f'<div style="border-left:5px solid {"#d6482c" if item.get("strategy") == variants.get("chosen_strategy") else "#d8dee6"};'
        f'padding:16px 20px;margin-bottom:14px;background:#f2f4f6;border-radius:12px">'
        f'<p style="margin:0;font-size:26px;font-weight:800;color:#182029">{_escaped(item.get("title"))}</p>'
        f'<p style="margin:8px 0 0;font-size:22px;color:#5b6875">{_escaped(item.get("strategy"))} · '
        f'{_escaped(item.get("semantic_score"))} / {_escaped(item.get("adjusted_score"))}</p></div>'
        for item in ranked
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
        _metric("Learning releases", performance.get("learning_count", 0)),
        _metric("Pending metrics", performance.get("pending_count", 0)),
        _metric("Recommended formula", performance.get("recommended_formula", "insufficient data")),
    ])

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
  {_section("Semantic dimensions", f'<div style="display:flex;gap:14px;flex-wrap:wrap">{score_items}</div>')}
  {_section("Style resonance", style_inner)}
  {_section("Variant ranking", variant_html)}
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
    return {
        "release": metadata.get("release", ""),
        "title": metadata.get("title", ""),
        "strategy": metadata.get("strategy", metadata.get("hook_type", "unknown")),
        "body": (package_dir / "body.txt").read_text(encoding="utf-8").strip() if (package_dir / "body.txt").exists() else "",
        "qa": _read_json(package_dir / "qa.json"),
        "copy_review": _read_json(package_dir / "copy-review.json"),
        "variants": _read_json(package_dir / "variants.json"),
        "wechat_qa": _read_json(package_dir / "wechat" / "wechat-qa.json"),
        "performance": _read_json(package_dir / "performance-report.json"),
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
