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


def build_story(
    *,
    release: str,
    previous_release: str,
    notes: str,
    diff: str = "",
    shot_library_path: Path,
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

    relevant_ids = [shot_id for _, _, shot_id in sorted(scored, reverse=True)[:3]]
    mandatory = ["overview.reader", "overview.editor", "convert.home"]
    selected = list(dict.fromkeys(relevant_ids + mandatory))
    selected.sort(key=lambda item: (item != "overview.reader", item != "overview.editor", item == "convert.home"))

    claims: list[dict[str, Any]] = []
    for shot_id in selected:
        shot = shots[shot_id]
        sources = list(dict.fromkeys(shot.get("evidence", []) + (["release/release_notes.md"] if shot_id in relevant_ids else [])))
        claims.append(
            {
                "id": shot_id.replace(".", "-"),
                "user_value": shot["description"],
                "shot_ids": [shot_id],
                "sources": sources,
                "kind": "visual",
            }
        )

    mapped_terms = {term.lower() for shot_id in selected for term in shots[shot_id].get("keywords", [])}
    invisible_fixes: list[str] = []
    for line in notes.splitlines():
        match = re.match(r"^\s*(?:[-*]|\d+[.)])\s+(.+)$", line)
        clean = match.group(1).strip() if match else ""
        if not clean or len(clean) < 8:
            continue
        if not any(term in clean.lower() for term in mapped_terms):
            invisible_fixes.append(clean)
    for fix in invisible_fixes[:2]:
        claims.append(
            {
                "id": _slug(fix)[:48],
                "user_value": fix,
                "shot_ids": [],
                "sources": ["release/release_notes.md"],
                "kind": "invisible",
            }
        )

    categories = {shots[shot_id]["category"] for shot_id in selected}
    if "Presentation" in categories:
        angle = "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映"
    elif "Science" in categories:
        angle = "ReadMD 把学术写作需要的公式和科学图表放进同一个本地工作台"
    else:
        angle = "ReadMD 正在从 Markdown 阅读器变成完整本地文档工作台"

    card_plan: list[dict[str, Any]] = [
        {
            "index": 1,
            "file": _card_name(1, None, "cover"),
            "role": "cover",
            "shot_id": None,
            "title": angle,
            "caption": "",
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
                "caption": shot["description"],
                "ui_min_ratio": 0.70 if shot["role"] == "pure_ui_hero" else 0.55,
            }
        )
    card_plan.append(
        {
            "index": len(card_plan) + 1,
            "file": _card_name(len(card_plan) + 1, None, "summary"),
            "role": "summary",
            "shot_id": selected[0],
            "title": "本地 Markdown 工作台",
            "caption": "阅读、编辑、转换、学术排版与共享在同一处完成。",
            "ui_min_ratio": 0.30,
        }
    )

    return {
        "schema_version": 1,
        "release": release,
        "previous_release": previous_release,
        "version_state": "prerelease" if is_prerelease(release) else "release",
        "angle": angle,
        "selected_shots": selected,
        "shots": [shots[shot_id] for shot_id in selected],
        "claims": claims,
        "invisible_fixes": invisible_fixes[:2],
        "card_plan": card_plan,
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }


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
