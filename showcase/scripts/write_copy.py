#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate a Xiaohongshu package from evidence-backed story data."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from content_memory import engagement_score, load_learning_records, partition_records, summarize
from copy_profiles import (
    COMMENT_SCENARIOS,
    COMMENT_SHOT_FOCUS,
    TITLE_FORMULA_CONTRACTS,
    MECHANISM_TOPIC_SETS,
    RESONANCE_CONCERN_RESPONSE,
    SUPPORT_PHRASES,
    resonance_topic_adjustment,
    profile_for_story,
    title_candidate_errors,
)


BANNED_REPLACEMENTS = {
    "二维码": "扫码",
    "公众号": "公开主页",
    "微信": "聊天工具",
    "闲鱼": "二手平台",
}


def topic_set_id(topics: list[str]) -> str:
    """Return a stable identity for the exact search-term combination."""
    clean_topics = [str(item).strip() for item in topics if str(item).strip()]
    payload = "\n".join(clean_topics)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()[:12]


def _clean(value: str) -> str:
    for old, new in BANNED_REPLACEMENTS.items():
        value = value.replace(old, new)
    return value.replace("\n", " ").strip()


def _planned_card_count(story: dict[str, Any]) -> int:
    """Use the publishable carousel size, not the number of textual claims."""
    card_plan = story.get("card_plan")
    if isinstance(card_plan, list) and card_plan:
        return max(4, min(len(card_plan), 9))
    selected = story.get("selected_shots")
    fallback = len(selected) + 2 if isinstance(selected, list) else 4
    return max(4, min(fallback, 9))


def _title_candidates(story: dict[str, Any]) -> list[dict[str, Any]]:
    profile = profile_for_story(story)
    number = _planned_card_count(story)
    candidates = [
        {
            "formula_id": formula_id,
            "text": text.replace("{number}", str(number)),
            **{
                field: TITLE_FORMULA_CONTRACTS[formula_id][field]
                for field in ("source_template", "adaptation")
            },
        }
        for formula_id, text in profile["titles"].items()
    ]
    errors = title_candidate_errors(candidates)
    if errors:
        raise ValueError("; ".join(errors))
    return candidates


def _select_title(candidates: list[dict[str, Any]], history: list[dict[str, Any]] | None) -> tuple[dict[str, Any], dict[str, Any]]:
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
    scored: list[tuple[float, dict[str, Any]]] = []
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


def _resonance_focus(history: list[dict[str, Any]]) -> dict[str, Any]:
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
            stats = themes.setdefault(theme, {"mentions": 0, "weighted_score": 0, "intents": {}})
            focus_releases.setdefault(theme, set()).add(release)
            stats["mentions"] += int(item.get("mentions", 0))
            stats["weighted_score"] += int(item.get("weighted_score", 0))
            for intent in item.get("intents", []):
                stats["intents"][str(intent)] = stats["intents"].get(str(intent), 0) + 1
    ranked = sorted(
        themes.items(),
        key=lambda item: (
            -item[1]["weighted_score"],
            -item[1]["mentions"],
            -sum(item[1]["intents"].values()),
            item[0],
        ),
    )
    chosen = next(
        (
            (theme, stats)
            for theme, stats in ranked
            if len(focus_releases[theme]) >= 2 and stats["weighted_score"] >= 3
        ),
        None,
    )
    if not chosen:
        return {
            "schema_version": 1,
            "focus": "general",
            "release_count": 0,
            "mentions": 0,
            "weighted_score": 0,
            "confidence": "low",
            "top_intents": [],
        }

    theme, stats = chosen
    release_count = len(focus_releases[theme])
    confidence = "medium" if release_count >= 2 else "low"
    if release_count >= 3 and stats["weighted_score"] >= 8:
        confidence = "high"
    return {
        "schema_version": 1,
        "focus": theme,
        "release_count": release_count,
        "mentions": int(stats["mentions"]),
        "weighted_score": int(stats["weighted_score"]),
        "confidence": confidence,
        "top_intents": sorted(
            stats["intents"],
            key=lambda intent: (-stats["intents"][intent], intent),
        ),
    }


def _comment_history(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Comment evidence is independent of metric completeness."""
    return [
        record
        for record in records
        if isinstance(record.get("comment_insights"), dict)
    ]


def _resonance_directive(
    resonance: dict[str, Any],
    *,
    story: dict[str, Any],
    profile: dict[str, Any],
    available_shot_ids: set[str],
) -> dict[str, Any]:
    focus_shot = COMMENT_SHOT_FOCUS.get(resonance["focus"])
    support_available = bool(focus_shot and focus_shot in available_shot_ids)
    applied = (
        resonance["confidence"] != "low"
        and resonance["focus"] != "general"
        and support_available
    )
    scenario = COMMENT_SCENARIOS[resonance["focus"]]
    strengthen = (
        f"优先展示“{SUPPORT_PHRASES[focus_shot]}”，读者场景固定为{scenario}。"
        if applied and focus_shot in available_shot_ids and focus_shot in SUPPORT_PHRASES
        else f"保持当前核心机制，读者场景维持{scenario}。"
    )
    return {
        "schema_version": 1,
        "applied": applied,
        "support_available": support_available,
        "evidence": {
            key: value
            for key, value in resonance.items()
            if key != "schema_version"
        },
        "decisions": {
            "keep": str(story.get("angle") or profile.get("narrative_angle", "")),
            "strengthen": strengthen,
            "compress": "辅助能力最多保留两条，并且必须服务同一核心机制。",
            "delete": "不加入与评论焦点无关的新卖点。",
        },
    }


def available_shot_ids(story: dict[str, Any]) -> set[str]:
    primary_id = str(story.get("primary_shot", "overview.editor"))
    supporting_ids = [
        shot_id
        for claim in story.get("claims", [])
        for shot_id in (claim.get("shot_ids") or [])
        if shot_id != primary_id
    ]
    return {primary_id, *supporting_ids}


def build_resonance_directive(
    story: dict[str, Any],
    history: list[dict[str, Any]] | None,
) -> dict[str, Any]:
    """Derive the directive from publication evidence without package claims."""
    all_history = history or []
    learning_history, pending_history = partition_records(all_history)
    resonance = _resonance_focus(
        _comment_history([*learning_history, *pending_history])
    )
    return _resonance_directive(
        resonance,
        story=story,
        profile=profile_for_story(story),
        available_shot_ids=available_shot_ids(story),
    )


def _select_topic_set(
    primary_id: str,
    history: list[dict[str, Any]],
    *,
    resonance_directive: dict[str, Any] | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Choose a mechanism-approved search set with confidence-gated evidence."""
    raw_sets = MECHANISM_TOPIC_SETS.get(primary_id)
    if not raw_sets:
        raw_sets = [{"label": "core", "topics": ["Markdown", "效率工具", "程序员", "开源项目", "GitHub"]}]
    candidates = [
        {**item, "topic_set_id": topic_set_id(item["topics"])}
        for item in raw_sets
    ]
    if not history:
        chosen = candidates[0]
        return chosen, {
            "strategy": "default topic set without publication history",
            "chosen_topic_set_id": chosen["topic_set_id"],
            "chosen_label": chosen["label"],
            "scores": {item["topic_set_id"]: 0 for item in candidates},
            "reasons": {},
            "avoided_topic_sets": [],
            "sample_size": 0,
        }

    stats: dict[str, dict[str, Any]] = {}
    usage: dict[str, int] = {}
    for record in history:
        set_id = str(record.get("topic_set_id", ""))
        if not set_id:
            continue
        usage[set_id] = usage.get(set_id, 0) + 1
        stat = stats.setdefault(set_id, {
            "publications": 0,
            "impressions": 0,
            "weighted_engagement": 0,
            "score": 0.0,
            "confidence_ok": False,
        })
        impressions = int(record.get("impressions", 0))
        engagement = engagement_score(record)
        stat["publications"] += 1
        stat["impressions"] += impressions
        stat["weighted_engagement"] += engagement
    for stat in stats.values():
        stat["score"] = round(stat["weighted_engagement"] / max(stat["impressions"], 1), 6)
        stat["confidence_ok"] = stat["publications"] >= 2 and stat["impressions"] >= 1000

    recent_ids = {str(item.get("topic_set_id", "")) for item in history[-2:]}
    max_score = max((item["score"] for item in stats.values()), default=0.0)
    max_usage = max(usage.values(), default=0)
    scored: list[tuple[float, dict[str, Any]]] = []
    for index, candidate in enumerate(candidates):
        set_id = candidate["topic_set_id"]
        stat = stats.get(set_id)
        score = float(10 - index)
        reasons: list[str] = []
        if stat and stat["confidence_ok"]:
            score += (stat["score"] / max_score) * 20 if max_score else 0
            reasons.append("historical performance")
        else:
            score += 3
            reasons.append(
                "low-confidence evidence held as exploration"
                if stat
                else "unexplored"
            )
        if set_id in recent_ids:
            score -= 12
            reasons.append("recent topic-set fatigue penalty")
        coverage_bonus = (max_usage - usage.get(set_id, 0)) * 8
        if coverage_bonus:
            score += coverage_bonus
            reasons.append("underused topic set")
        focus_bonus, focus_reason = resonance_topic_adjustment(
            resonance_directive,
            topics=candidate["topics"],
        )
        if focus_bonus:
            score += focus_bonus
            reasons.append(focus_reason)
        scored.append((round(score, 3), candidate))
        candidate["_reason"] = "+".join(reasons)

    chosen = max(scored, key=lambda item: item[0])[1]
    return chosen, {
        "strategy": "confidence-gated historical performance with fatigue and coverage balancing",
        "chosen_topic_set_id": chosen["topic_set_id"],
        "chosen_label": chosen["label"],
        "scores": {
            item["topic_set_id"]: round(value, 3)
            for value, item in scored
        },
        "reasons": {
            item["topic_set_id"]: item.pop("_reason", "")
            for item in candidates
        },
        "avoided_topic_sets": sorted(recent_ids),
        "sample_size": len(history),
        "resonance_focus": str(
            (resonance_directive or {}).get("evidence", {}).get("focus", "general")
        ),
        "resonance_topic_bonuses": {
            item["topic_set_id"]: resonance_topic_adjustment(
                resonance_directive,
                topics=item["topics"],
            )[0]
            for item in candidates
        },
    }


def generate_copy(
    story: dict[str, Any],
    *,
    repository: str,
    previous_release: str,
    history: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    all_history = history or []
    history, pending_history = partition_records(all_history)
    candidates = _title_candidates(story)
    selected_title, title_selection = _select_title([dict(item) for item in candidates], history)
    profile = profile_for_story(story)
    primary_id = story.get("primary_shot", "overview.editor")
    visual_claims = [claim for claim in story["claims"] if claim.get("shot_ids")]
    supporting_ids = [
        item["shot_ids"][0]
        for item in visual_claims
        if item.get("shot_ids") and primary_id not in item["shot_ids"]
    ]
    unique_supporting_ids = list(dict.fromkeys(supporting_ids))
    available_shot_ids = {primary_id, *unique_supporting_ids}
    resonance_directive = build_resonance_directive(story, all_history)
    resonance = resonance_directive["evidence"]
    selected_topic_set, topic_set_selection = _select_topic_set(
        primary_id=primary_id,
        history=history,
        resonance_directive=resonance_directive,
    )
    valid_candidates = [item for item in candidates if len(item["text"]) <= 20]
    chosen_title = selected_title if len(selected_title["text"]) <= 20 else next(iter(valid_candidates), candidates[0])
    chosen_title["text"] = _clean(chosen_title["text"])
    reader_focus = resonance["focus"] if resonance_directive["applied"] else "general"
    release = story["release"]
    prerelease = story["version_state"] != "release"
    state_text = "预览版" if prerelease else "正式版"
    invisible_claims = [claim for claim in story["claims"] if not claim.get("shot_ids")]

    opening = profile["opening"]
    disclosure = f"先说清楚：这是 ReadMD {release} {state_text}，文件仍然保留在你自己的电脑里。"
    evidence = "下面的画面来自当前版本真实运行状态，不是概念图。"
    primary_text = profile.get(
        "primary_paragraph",
        _clean(next((item["user_value"] for item in visual_claims if primary_id.replace("-", ".") in item["shot_ids"]), story["angle"])),
    )
    focused_support_id = COMMENT_SHOT_FOCUS.get(reader_focus)
    prioritized_supporting_ids = (
        [focused_support_id]
        if resonance_directive["applied"] and focused_support_id in unique_supporting_ids
        else []
    )
    ordered_supporting_ids = prioritized_supporting_ids + [
        shot_id
        for shot_id in unique_supporting_ids
        if shot_id not in prioritized_supporting_ids
    ]
    support_priorities = profile.get("support_priorities", {})
    if prioritized_supporting_ids:
        supporting_ids = [
            *prioritized_supporting_ids,
            *sorted(
                ordered_supporting_ids[1:],
                key=lambda shot_id: (
                    support_priorities.index(shot_id)
                    if shot_id in support_priorities
                    else len(support_priorities)
                ),
            ),
        ]
    else:
        supporting_ids = sorted(
            ordered_supporting_ids,
            key=lambda shot_id: (
                support_priorities.index(shot_id)
                if shot_id in support_priorities
                else len(support_priorities)
            ),
        )
    support_text = "、".join(
        SUPPORT_PHRASES[shot_id]
        for shot_id in supporting_ids[:2]
        if shot_id in SUPPORT_PHRASES
    )

    paragraphs = [opening, disclosure, f"这一版的核心就一件事：{story['angle']}。", primary_text]
    if support_text:
        paragraphs.append(f"它没有脱离原来的工作流：{support_text}。")
    paragraphs.append(evidence)

    concern_intents = set(resonance_directive.get("evidence", {}).get("top_intents", []))
    if resonance_directive.get("applied") is True and "concern" in concern_intents:
        paragraphs.append(RESONANCE_CONCERN_RESPONSE)

    if invisible_claims:
        fixes = "；".join(_clean(claim["user_value"]).rstrip("。．.!！?？") for claim in invisible_claims[:2])
        paragraphs.append(f"还有一些不适合单独拍图的底层修复也在这版里，比如{fixes}。它们不抢画面，但会让日常使用更稳。")

    paragraphs.append(f"收藏这条{profile['decision_rule']}")

    paragraphs.extend(
        [
            f"如果你常处理{COMMENT_SCENARIOS[reader_focus]}，它会省掉“{profile['saved_step']}”这一步。",
            f"安装包在 GitHub Releases 页面。不想翻链接的话，可以直接 GitHub 搜 {repository}，进仓库后点 Releases 就能找到对应平台。",
        ]
    )
    cta = profile["cta"]

    def assembled(parts: list[str]) -> str:
        return "\n\n".join([*parts, cta])

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
    while len(assembled(paragraphs)) > 900:
        if len(paragraphs) <= 4:
            break
        focused_phrase = SUPPORT_PHRASES.get(COMMENT_SHOT_FOCUS.get(reader_focus, ""))
        scenario = COMMENT_SCENARIOS[reader_focus]
        protected = (
            focused_phrase,
            scenario,
            "下面的画面来自当前版本真实运行状态",
            RESONANCE_CONCERN_RESPONSE,
        )
        removable = [
            index
            for index in range(3, len(paragraphs) - 1)
            if not any(term in paragraphs[index] for term in protected)
        ]
        if not removable:
            break
        paragraphs.pop(max(removable))
    pad_index = 0
    while len(assembled(paragraphs)) < 600 and pad_index < len(padding):
        # Keep practical context before the scenario, download note, and CTA so
        # the comment prompt remains the post's final reader action.
        paragraphs.insert(max(3, len(paragraphs) - 2), padding[pad_index])
        pad_index += 1
    body = assembled(paragraphs)

    topics = selected_topic_set["topics"]
    return {
        "title": chosen_title["text"],
        "primary_shot": primary_id,
        "title_formula_id": chosen_title["formula_id"],
        "title_candidates": candidates,
        "title_selection": title_selection,
        "body": body,
        "topics": topics,
        "topic_set_id": topic_set_id(topics),
        "topic_set_label": selected_topic_set["label"],
        "topic_set_selection": topic_set_selection,
        "resonance_directive": resonance_directive,
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
