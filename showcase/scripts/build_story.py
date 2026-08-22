#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Turn a release into an evidence-backed visual story."""
from __future__ import annotations

import argparse
import json
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from copy_profiles import cover_for_title, profile_for_story

def is_prerelease(release: str) -> bool:
    return bool(re.search(r"(?:beta|alpha|rc|preview|pre)", release, re.I))


def load_shot_library(path: Path) -> dict[str, dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        raise ValueError("shot library schema_version must be 1")
    return data["shots"]


def _keyword_hits(text: str, keywords: list[str]) -> int:
    lower = text.lower()
    return sum(1 for keyword in keywords if keyword.lower() in lower)


def _slug(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or "feature"


def _card_name(index: int, shot: dict[str, Any] | None, role: str) -> str:
    if index == 1:
        stem = "cover"
    elif role == "summary":
        stem = "summary"
    else:
        source = shot["id"] if shot else role
        stem = _slug(source.replace(".", "-"))
    return f"xhs-{index:02d}-{stem}.jpg"


USER_VALUES = {
    "overview.reader": "打开文档就能看到完整排版、目录和公式渲染",
    "overview.editor": "讲稿和源文件在同一处修改，预览不会跑偏",
    "presentation.reveal": "写完的 Markdown 能直接上台放映，代码、表格和公式不会被切片",
    "editor.diagram-picker": "科研图表从语法记忆变成面板选择",
    "academic.latex-bib": "学术公式和定理盒子保持论文级排版",
    "editor.code-chunk": "文档里的代码可以直接运行并保留输出",
    "convert.home": "资料进入本地工作台后，转换和阅读不散落在多个工具里",
    "sharing.export": "同一份文档可以继续发到手机上查看",
}


def _extract_core_fixes(notes: str) -> list[str]:
    """Read user-visible fix titles, never the release download inventory."""
    section_match = re.search(
        r"^##[^\n]*(?:核心修复|重要更新|更新内容)[^\n]*\n(.*?)(?=^##\s|\Z)",
        notes,
        flags=re.M | re.S,
    )
    section = section_match.group(1) if section_match else notes
    excluded = (
        ".exe", ".zip", ".dmg", ".appimage", ".deb", ".hap", ".vsix",
        "sha256", "测试", "覆盖率", "i18n", "语言字典", "release assets",
    )
    fixes: list[str] = []
    headings = [line for line in section.splitlines() if re.match(r"^###\s+", line)]
    for line in section.splitlines():
        pattern = r"^###\s+(.+)$" if headings else r"^\s*(?:[-*]|\d+[.)])\s+(.+)$"
        match = re.match(pattern, line)
        clean = re.sub(r"^\d+\.\s*", "", match.group(1)).strip() if match else ""
        if len(clean) < 8 or any(term in clean.lower() for term in excluded):
            continue
        fixes.append(clean)
    return fixes


def build_story(
    *,
    release: str,
    previous_release: str,
    notes: str,
    diff: str = "",
    shot_library_path: Path,
    notes_source: str = "release/release_notes.md",
) -> dict[str, Any]:
    shots = load_shot_library(Path(shot_library_path))
    evidence_text = "\n".join((notes, diff))
    scored: list[tuple[float, float, str]] = []
    for shot_id, shot in shots.items():
        hits = _keyword_hits(evidence_text, shot.get("keywords", []))
        relevance = min(1.0, hits * 0.34)
        score = relevance * 0.55 + float(shot.get("visuality", 0)) * 0.45
        if relevance > 0:
            scored.append((score, relevance, shot_id))

    ranked_relevant = [shot_id for _, _, shot_id in sorted(scored, reverse=True)]
    fixed_order = ["overview.reader", "overview.editor", "convert.home"]
    relevant_features = [shot_id for shot_id in ranked_relevant if shot_id not in fixed_order]
    relevant_features = relevant_features[:3]
    primary_shot = relevant_features[0] if relevant_features else "overview.editor"
    mechanism_profile = profile_for_story({"primary_shot": primary_shot})
    cover_hook = dict(mechanism_profile["cover"])
    summary_hook = {
        **mechanism_profile["summary"],
        "proof_points": list(mechanism_profile["summary"]["proof_points"]),
    }
    selected = list(dict.fromkeys(["overview.reader"] + relevant_features + fixed_order[1:]))

    claims: list[dict[str, Any]] = []
    for shot_id in selected:
        shot = shots[shot_id]
        sources = list(dict.fromkeys(shot.get("evidence", []) + ([notes_source] if shot_id in relevant_features else [])))
        claims.append(
            {
                "id": shot_id.replace(".", "-"),
                "user_value": USER_VALUES.get(shot_id, shot["description"]),
                "shot_ids": [shot_id],
                "sources": sources,
                "kind": "visual",
            }
        )

    invisible_fixes = _extract_core_fixes(notes)
    for index, fix in enumerate(invisible_fixes[:2], 1):
        claims.append(
            {
                "id": f"invisible-{index}",
                "user_value": fix,
                "shot_ids": [],
                "sources": [notes_source],
                "kind": "invisible",
            }
        )

    angle = mechanism_profile["narrative_angle"]

    card_plan: list[dict[str, Any]] = [
        {
            "index": 1,
            "file": _card_name(1, None, "cover"),
            "role": "cover",
            "shot_id": None,
            "title": cover_hook["title"],
            "caption": cover_hook["caption"],
            "ui_min_ratio": 0.0,
        }
    ]
    for shot_id in selected:
        index = len(card_plan) + 1
        shot = shots[shot_id]
        card_plan.append(
            {
                "index": index,
                "file": _card_name(index, shot, shot["role"]),
                "role": shot["role"],
                "shot_id": shot_id,
                "title": shot["name"],
                "caption": USER_VALUES.get(shot_id, shot["description"]),
                "ui_min_ratio": 0.70 if shot["role"] == "pure_ui_hero" else 0.55,
            }
        )
    card_plan.append(
        {
            "index": len(card_plan) + 1,
            "file": _card_name(len(card_plan) + 1, None, "summary"),
            "role": "summary",
            "shot_id": selected[0],
            "title": summary_hook["title"],
            "caption": summary_hook["caption"],
            "proof_points": summary_hook["proof_points"],
            "ui_min_ratio": 0.30,
        }
    )

    return {
        "schema_version": 1,
        "release": release,
        "previous_release": previous_release,
        "version_state": "prerelease" if is_prerelease(release) else "release",
        "angle": angle,
        "primary_shot": primary_shot,
        "cover_hook": cover_hook,
        "summary_hook": summary_hook,
        "narrative_angle": angle,
        "selected_shots": selected,
        "shots": [shots[shot_id] for shot_id in selected],
        "claims": claims,
        "invisible_fixes": invisible_fixes[:2],
        "card_plan": card_plan,
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }


def apply_selected_cover(story: dict[str, Any], metadata: dict[str, Any]) -> dict[str, Any]:
    """Sync the first-image trigger with the winning title formula."""
    profile = profile_for_story(story)
    formula_id = str(metadata.get("title_formula_id", ""))
    cover_hook = cover_for_title(profile, formula_id)
    story["cover_hook"] = cover_hook
    story["cover_variant_formula_id"] = formula_id
    if story.get("card_plan"):
        story["card_plan"][0].update({
            "title": cover_hook["title"],
            "caption": cover_hook["caption"],
        })
    return story


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", required=True)
    parser.add_argument("--previous-release", required=True)
    parser.add_argument("--notes", type=Path, required=True)
    parser.add_argument("--diff", type=Path)
    parser.add_argument("--shot-library", type=Path, default=Path(__file__).parents[1] / "shot_library.json")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    story = build_story(
        release=args.release,
        previous_release=args.previous_release,
        notes=args.notes.read_text(encoding="utf-8"),
        diff=args.diff.read_text(encoding="utf-8") if args.diff and args.diff.exists() else "",
        shot_library_path=args.shot_library,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(story, ensure_ascii=False, indent=2), encoding="utf-8")
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
