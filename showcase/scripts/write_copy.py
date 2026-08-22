#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate a Xiaohongshu package from evidence-backed story data."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from content_memory import load_learning_records, partition_records, summarize
from copy_profiles import profile_for_story


BANNED_REPLACEMENTS = {
    "二维码": "扫码",
    "公众号": "公开主页",
    "微信": "聊天工具",
    "闲鱼": "二手平台",
}

TOPICS = ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"]


def _clean(value: str) -> str:
    for old, new in BANNED_REPLACEMENTS.items():
        value = value.replace(old, new)
    return value.replace("\n", " ").strip()


def _title_candidates(story: dict[str, Any]) -> list[dict[str, str]]:
    profile = profile_for_story(story)
    visual_count = sum(1 for claim in story["claims"] if claim.get("shot_ids"))
    number = max(3, min(visual_count, 8))
    return [
        {"formula_id": formula_id, "text": text.replace("{number}", str(number))}
        for formula_id, text in profile["titles"].items()
    ]


def _select_title(candidates: list[dict[str, str]], history: list[dict[str, Any]] | None) -> tuple[dict[str, str], dict[str, Any]]:
    if not history:
        chosen = candidates[0]
        return chosen, {
            "strategy": "formula order without publication history",
            "scores": {item["formula_id"]: 0 for item in candidates},
            "avoided_formulas": [],
            "sample_size": 0,
        }

    summary = summarize(history)
    stats = summary["formula_stats"]
    max_score = max((item["score"] for item in stats.values()), default=0.0)
    recent = set(summary["recent_formulas"])
    scored: list[tuple[float, dict[str, str]]] = []
    avoided: list[str] = []
    for index, candidate in enumerate(candidates):
        formula = candidate["formula_id"]
        stat = stats.get(formula)
        score = 10 - index
        reason_bits = []
        confidence_ok = bool(stat and stat.get("confidence") != "low")
        if stat and confidence_ok:
            score += (stat["score"] / max_score) * 20 if max_score else 0
            reason_bits.append("historical performance")
        else:
            score += 3
            reason_bits.append("low-confidence evidence held as exploration" if stat else "unexplored")
        if formula in recent:
            score -= 12
            avoided.append(formula)
            reason_bits.append("recent fatigue penalty")
        scored.append((score, candidate))
        candidate.setdefault("_reason", "+".join(reason_bits))
    chosen = max(scored, key=lambda item: item[0])[1]
    return chosen, {
        "strategy": "confidence-gated historical winner with recent-fatigue penalty",
        "scores": {candidate["formula_id"]: round(value, 3) for value, candidate in scored},
        "reasons": {candidate["formula_id"]: candidate.pop("_reason", "") for candidate in candidates},
        "avoided_formulas": sorted(set(avoided)),
        "sample_size": len(history),
    }


COMMENT_SCENARIOS = {
    "general": "课程讲义、组会报告、技术分享或论文汇报",
    "presentation": "课堂讲授、技术分享或会议演示",
    "academic": "课程讲义、组会报告或论文汇报",
    "code": "代码教程、技术笔记或示例文档",
    "table": "数据表格、对比报告或项目清单",
    "formula": "公式讲义、论文推导或学术笔记",
    "diagram": "流程图、架构图或图表笔记",
    "conversion": "网页剪藏、Word 资料或 PDF 内容",
    "export-share": "发布稿、分享页或 HTML 输出",
    "local-privacy": "本地草稿、私人笔记或未上传资料",
    "stability-performance": "长文、大文档或复杂项目",
}


def _comment_focus(history: list[dict[str, Any]]) -> str:
    themes: dict[str, dict[str, Any]] = {}
    focus_releases: dict[str, set[str]] = {}
    for record in history:
        insights = record.get("comment_insights")
        if not isinstance(insights, dict):
            continue
        release = str(record.get("release", ""))
        for item in insights.get("themes", []):
            if not isinstance(item, dict):
                continue
            theme = str(item.get("theme", "general"))
            stats = themes.setdefault(theme, {"mentions": 0, "weighted_score": 0})
            focus_releases.setdefault(theme, set()).add(release)
            stats["mentions"] += int(item.get("mentions", 0))
            stats["weighted_score"] += int(item.get("weighted_score", 0))
    ranked = sorted(
        themes.items(),
        key=lambda item: (-item[1]["weighted_score"], -item[1]["mentions"], item[0]),
    )
    return next(
        (
            theme
            for theme, stats in ranked
            if len(focus_releases[theme]) >= 2 and stats["weighted_score"] >= 3
        ),
        "general",
    )


def generate_copy(
    story: dict[str, Any],
    *,
    repository: str,
    previous_release: str,
    history: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    history, _pending_history = partition_records(history or [])
    candidates = _title_candidates(story)
    selected_title, title_selection = _select_title([dict(item) for item in candidates], history)
    valid_candidates = [item for item in candidates if len(item["text"]) <= 20]
    chosen_title = selected_title if len(selected_title["text"]) <= 20 else next(iter(valid_candidates), candidates[0])
    chosen_title["text"] = _clean(chosen_title["text"])
    profile = profile_for_story(story)
    focus = _comment_focus(history)
    release = story["release"]
    prerelease = story["version_state"] != "release"
    state_text = "预览版" if prerelease else "正式版"
    visual_claims = [claim for claim in story["claims"] if claim.get("shot_ids")]
    invisible_claims = [claim for claim in story["claims"] if not claim.get("shot_ids")]

    opening = profile["opening"]
    disclosure = f"先说清楚：这是 ReadMD {release} {state_text}，文件仍然保留在你自己的电脑里。"
    evidence = "下面的画面来自当前版本真实运行状态，不是概念图。"
    primary_id = story.get("primary_shot", "overview.editor")
    primary_text = profile.get(
        "primary_paragraph",
        _clean(next((item["user_value"] for item in visual_claims if primary_id.replace("-", ".") in item["shot_ids"]), story["angle"])),
    )
    supporting = [claim for claim in visual_claims if primary_id not in claim["shot_ids"]]
    support_bits = {
        "overview.editor": "改稿时回到同屏预览",
        "overview.reader": "阅读端保持目录和公式排版",
        "convert.home": "资料仍从一个本地入口进来",
        "academic.latex-bib": "学术排版不另起一套工具",
        "editor.code-chunk": "代码示例可以就地验证",
    }
    support_text = "、".join(support_bits[item["shot_ids"][0]] for item in supporting[:2] if item["shot_ids"] and item["shot_ids"][0] in support_bits)

    paragraphs = [opening, disclosure, f"这一版的核心就一件事：{story['angle']}。", primary_text]
    if support_text:
        paragraphs.append(f"它没有脱离原来的工作流：{support_text}。")
    paragraphs.append(evidence)

    if invisible_claims:
        fixes = "；".join(_clean(claim["user_value"]) for claim in invisible_claims[:2])
        paragraphs.append(f"还有一些不适合单独拍图的底层修复也在这版里，比如{fixes}。它们不抢画面，但会让日常使用更稳。")

    paragraphs.extend(
        [
            f"如果你常处理{COMMENT_SCENARIOS[focus]}，它会省掉“{profile['saved_step']}”这一步。",
            f"安装包在 GitHub Releases 页面。不想翻链接的话，可以直接 GitHub 搜 {repository}，进仓库后点 Releases 就能找到对应平台。",
            profile["cta"],
        ]
    )
    body = "\n\n".join(paragraphs)

    padding = [
        "渲染阶段只处理显示结果，不会替你改写原始 Markdown 文件。",
        "所有演示都来自同一个本地工作流，不需要先把文档上传到别处。",
        "对长文档来说，稳定的目录和搜索比炫技功能更重要。",
        "转换结果会开成新标签页，方便先检查再保存。",
        "界面支持跟随系统语言，中文和英文术语都保持统一。",
        "目录和全文搜索跨页联动，长文档不会因为一次渲染丢掉入口。",
        "暗色主题只影响显示，源文件内容不变。",
        "公式和图表在阅读页直接渲染，减少截图拼接。",
        "本地优先意味着草稿、笔记和讲稿都留在自己的设备里。",
    ]
    while len(body) > 900:
        parts = body.split("\n\n")
        if len(parts) <= 4:
            break
        parts.pop(-3)
        body = "\n\n".join(parts)
    pad_index = 0
    while len(body) < 600 and pad_index < len(padding):
        body += "\n\n" + padding[pad_index]
        pad_index += 1

    return {
        "title": chosen_title["text"],
        "primary_shot": primary_id,
        "title_formula_id": chosen_title["formula_id"],
        "title_candidates": candidates,
        "title_selection": title_selection,
        "body": body,
        "topics": TOPICS,
        "version_state": story["version_state"],
        "claim_ids": [claim["id"] for claim in story["claims"]],
        "source_urls": [
            f"https://github.com/{repository}/releases/tag/{release}",
            f"https://github.com/{repository}/compare/{previous_release}...{release}",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--story", type=Path, required=True)
    parser.add_argument("--repository", default="Natsummerance/readMD")
    parser.add_argument("--history", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    story = json.loads(args.story.read_text(encoding="utf-8"))
    history = load_learning_records(args.history)
    result = generate_copy(story, repository=args.repository, previous_release=story["previous_release"], history=history)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    (args.output_dir / "metadata.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    (args.output_dir / "title.txt").write_text(result["title"], encoding="utf-8")
    (args.output_dir / "body.txt").write_text(result["body"], encoding="utf-8")
    (args.output_dir / "topics.txt").write_text("\n".join(result["topics"]), encoding="utf-8")
    print(args.output_dir / "metadata.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
