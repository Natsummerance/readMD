#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate a Xiaohongshu package from evidence-backed story data."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from content_memory import load_records, summarize


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
    visual_count = sum(1 for claim in story["claims"] if claim.get("shot_ids"))
    number = max(3, min(visual_count, 8))
    return [
        {"formula_id": "#36", "text": "不用重做PPT，Markdown直接放映"},
        {"formula_id": "#9", "text": "Markdown写完，居然能直接上台"},
        {"formula_id": "#22", "text": "给要上台讲文档的人做的MD工具"},
        {"formula_id": "#61", "text": "别再把Markdown只当笔记了"},
        {"formula_id": "#12", "text": f"看完这{number}张，你会重新看Markdown"},
        {"formula_id": "#26", "text": f"ReadMD更新：{number}个文档工作台升级"},
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
        if stat:
            score += (stat["score"] / max_score) * 20 if max_score else 0
            reason_bits.append("historical performance")
        else:
            score += 3
            reason_bits.append("unexplored")
        if formula in recent:
            score -= 12
            avoided.append(formula)
            reason_bits.append("recent fatigue penalty")
        scored.append((score, candidate))
        candidate.setdefault("_reason", "+".join(reason_bits))
    chosen = max(scored, key=lambda item: item[0])[1]
    return chosen, {
        "strategy": "historical winner with recent-fatigue penalty",
        "scores": {candidate["formula_id"]: round(value, 3) for value, candidate in scored},
        "reasons": {candidate["formula_id"]: candidate.pop("_reason", "") for candidate in candidates},
        "avoided_formulas": sorted(set(avoided)),
        "sample_size": len(history),
    }


def generate_copy(
    story: dict[str, Any],
    *,
    repository: str,
    previous_release: str,
    history: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    candidates = _title_candidates(story)
    selected_title, title_selection = _select_title([dict(item) for item in candidates], history)
    valid_candidates = [item for item in candidates if len(item["text"]) <= 20]
    chosen_title = selected_title if len(selected_title["text"]) <= 20 else next(iter(valid_candidates), candidates[0])
    chosen_title["text"] = _clean(chosen_title["text"])
    release = story["release"]
    prerelease = story["version_state"] != "release"
    state_text = "预览版" if prerelease else "正式版"
    visual_claims = [claim for claim in story["claims"] if claim.get("shot_ids")]
    invisible_claims = [claim for claim in story["claims"] if not claim.get("shot_ids")]

    opening = "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。"
    disclosure = f"先说清楚：这是 ReadMD {release} {state_text}，文件仍然保留在你自己的电脑里。"
    evidence = "下面的画面来自当前版本真实运行状态，不是概念图。"
    primary_paragraphs = {
        "presentation.reveal": "放映界面可以直接换主题、调字号、切开场和转场；AST 保护分片会尽量保住代码块、表格和公式，不让长文档在幻灯片里被腰斩。",
        "overview.editor": "编辑器和实时预览在同一屏里，先改内容再确认排版，不用在几个窗口之间来回追版本。",
        "editor.diagram-picker": "图表从面板里选，渲染结果留在文档里；适合论文、报告和需要长期维护的技术笔记。",
        "convert.home": "打开、转换、AI 和网页抓取都从一个本地入口开始，资料不会被拆到一串临时工具里。",
    }
    primary_id = story.get("primary_shot", "overview.reader")
    primary_text = primary_paragraphs.get(primary_id, _clean(next((item["user_value"] for item in visual_claims if primary_id.replace("-", ".") in item["shot_ids"]), story["angle"])))
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
            "如果你常写课程讲义、组会报告、技术分享或论文汇报，它会省掉“重新做一遍演示稿”这一步。",
            f"安装包在 GitHub Releases 页面。不想翻链接的话，可以直接 GitHub 搜 {repository}，进仓库后点 Releases 就能找到对应平台。",
            "你会先拿哪一份 Markdown 试放映？评论区说说场景，我会把高频路径排进下一轮打磨。",
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
    history = content_memory.load_records(args.history)
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
