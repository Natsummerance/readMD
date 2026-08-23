#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Detect AI fingerprints and weak resonance in Xiaohongshu copy."""
from __future__ import annotations

import math
import re
from typing import Any


GENERIC_ADJECTIVES = (
    "强大", "高效", "安全", "极致", "丝滑", "智能", "先进",
    "颠覆性", "革命性", "一站式", "全方位",
)
AI_CLICHES = (
    "重磅升级", "效率起飞", "颠覆想象", "无缝体验", "重新定义",
    "赋能", "助力开发者腾飞", "next-gen", "revolutionary",
)
FIXED_CONNECTORS = (
    "然而", "但是", "事实上", "值得注意的是", "总而言之", "综上所述",
    "首先", "其次", "最后",
)
REVERSAL_PATTERN = re.compile(r"不是[^。！？\n]{1,32}，?(?:而是|是)")
DEPTH_TERMS = ("本质上", "归根结底")
BLESSING_TERMS = ("你值得", "祝你", "愿你")
CONCRETE_TERMS = (
    "Markdown", "MD", "PPT", "代码", "表格", "公式", "图表",
    "讲义", "组会", "论文", "PDF", "Word", "OCR", "Reveal",
)
RESONANCE_TERMS = (
    "写完", "复制", "重做", "折磨", "烦", "省掉", "砍掉",
    "不会", "保留", "自己的电脑", "本地", "上台", "放映",
)


def _sentences(body: str) -> list[str]:
    return [item.strip() for item in re.split(r"[。！？\n]+", body) if item.strip()]


def _emoji_count(body: str) -> int:
    return len(re.findall(r"[\U0001F300-\U0001FAFF\u2600-\u27BF]", body))


def _length_coefficient_of_variation(sentences: list[str]) -> float:
    lengths = [len(sentence) for sentence in sentences if len(sentence) >= 4]
    if len(lengths) < 3:
        return 1.0
    mean = sum(lengths) / len(lengths)
    if mean == 0:
        return 0.0
    variance = sum((length - mean) ** 2 for length in lengths) / (len(lengths) - 1)
    return math.sqrt(variance) / mean


def _finding(finding_id: str, severity: str, message: str) -> dict[str, str]:
    return {"id": finding_id, "severity": severity, "message": message}


def audit_style(body: str, audience: str = "程序员") -> dict[str, Any]:
    text = str(body).strip()
    sentences = _sentences(text)
    findings: list[dict[str, str]] = []
    lower = text.lower()

    length_cv = _length_coefficient_of_variation(sentences)
    if sentences and length_cv < 0.18:
        severity = "fail" if length_cv < 0.12 else "warn"
        findings.append(_finding(
            "uniform-rhythm",
            severity,
            f"Sentence lengths are too uniform (CV={length_cv:.2f}).",
        ))

    generic_hits = [term for term in GENERIC_ADJECTIVES if term in text]
    if len(generic_hits) >= 3:
        findings.append(_finding(
            "generic-adjective",
            "fail",
            "Multiple unsupported quality adjectives replace concrete outcomes.",
        ))
    elif generic_hits:
        findings.append(_finding(
            "generic-adjective",
            "warn",
            "Generic quality adjectives found: " + ", ".join(generic_hits),
        ))

    cliche_hits = [term for term in AI_CLICHES if term.lower() in lower]
    if cliche_hits:
        findings.append(_finding("ai-cliche", "fail", "AI launch clichés: " + ", ".join(cliche_hits)))

    connector_count = sum(text.count(term) for term in FIXED_CONNECTORS)
    reversal_matches = REVERSAL_PATTERN.findall(text)
    depth_hits = [term for term in DEPTH_TERMS if term in text]
    closing = text.split("\n\n")[-1]
    blessing_close = any(term in closing for term in BLESSING_TERMS)
    if connector_count >= 3:
        findings.append(_finding(
            "fixed-connectors",
            "fail",
            f"Fixed connectives are stacked {connector_count} times.",
        ))
    elif connector_count == 2:
        findings.append(_finding(
            "fixed-connectors",
            "warn",
            f"Fixed connectives appear {connector_count} times.",
        ))
    if len(reversal_matches) >= 3:
        findings.append(_finding(
            "reversal-density",
            "fail",
            f"“不是 X 而是 Y” appears {len(reversal_matches)} times; keep the sharpest reversal only.",
        ))
    elif len(reversal_matches) == 2:
        findings.append(_finding(
            "reversal-density",
            "warn",
            f"“不是 X 而是 Y” appears {len(reversal_matches)} times.",
        ))
    if len(depth_hits) >= 2:
        findings.append(_finding(
            "depth-overfit",
            "fail",
            "Abstraction markers overfit a practical product update: " + ", ".join(depth_hits),
        ))
    elif depth_hits:
        findings.append(_finding(
            "depth-overfit",
            "warn",
            "Abstraction marker found: " + ", ".join(depth_hits),
        ))
    if blessing_close:
        findings.append(_finding(
            "blessing-close",
            "fail",
            "The post closes with generic encouragement instead of a concrete action.",
        ))

    # Punctuation is stripped by sentence extraction, so look for a concrete artifact
    # and an engagement prompt anywhere rather than relying on a preserved question mark.
    specific_cta = any(term.lower() in text.lower() for term in CONCRETE_TERMS)
    engagement_cta = any(term in text for term in ("你会", "评论区", "哪一份", "哪一类", "说说"))
    if not specific_cta or not engagement_cta:
        findings.append(_finding(
            "generic-cta",
            "fail",
            "Closing prompt is not tied to a concrete artifact or scenario.",
        ))

    concrete_hits = [term for term in CONCRETE_TERMS if term.lower() in lower]
    resonance_hits = [term for term in RESONANCE_TERMS if term in text]
    if len(concrete_hits) < 3:
        findings.append(_finding("low-specificity", "warn", "Few concrete product artifacts are named."))
    if not resonance_hits:
        findings.append(_finding("weak-resonance", "fail", "No reader pain, stake, or local-control language found."))
    if audience and audience not in text and not any(term in text for term in ("程序员", "课程", "论文", "组会", "技术")):
        findings.append(_finding("audience-missing", "warn", "The intended audience is not explicit."))

    emoji_count = _emoji_count(text)
    if emoji_count > 4:
        findings.append(_finding("emoji-overload", "fail", f"{emoji_count} emojis reduce information density."))

    category_scores = {
        "rhythm": max(0, 25 - (15 if length_cv < 0.12 else 8 if length_cv < 0.18 else 0)),
        "voice": max(
            0,
            25
            - (20 if len(generic_hits) >= 3 else 8 if generic_hits else 0)
            - (8 if connector_count >= 3 else 4 if connector_count == 2 else 0)
            - (10 if len(reversal_matches) >= 3 else 5 if len(reversal_matches) == 2 else 0)
            - (8 if len(depth_hits) >= 2 else 3 if depth_hits else 0)
            - (6 if blessing_close else 0),
        ),
        "specificity": max(0, 25 - (10 if len(concrete_hits) < 3 else 0)),
        "resonance": max(0, 25 - (15 if not resonance_hits else 0) - (5 if not specific_cta or not engagement_cta else 0)),
    }
    score = round(sum(category_scores.values()))
    hard_failures = [
        finding["message"]
        for finding in findings
        if finding["severity"] == "fail"
    ]
    return {
        "schema_version": 1,
        "score": score,
        "ok": score >= 75 and not hard_failures,
        "metrics": {
            "sentence_count": len(sentences),
            "length_coefficient_of_variation": round(length_cv, 3),
            "generic_adjective_count": len(generic_hits),
            "cliche_count": len(cliche_hits),
            "fixed_connector_count": connector_count,
            "reversal_count": len(reversal_matches),
            "depth_term_count": len(depth_hits),
            "blessing_close": blessing_close,
            "emoji_count": emoji_count,
            "concrete_term_count": len(concrete_hits),
            "resonance_term_count": len(resonance_hits),
        },
        "category_scores": category_scores,
        "findings": findings,
        "hard_failures": hard_failures,
    }
