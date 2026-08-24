#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Evidence-aligned copy contracts for each selectable release mechanism."""
from __future__ import annotations

import re
from typing import Any


PRESENTATION_FRAMES = {
    "outcome-led": [
        (
            "core",
            "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。",
            "你会先拿哪一份 Markdown 试放映？评论区说说场景，我会把高频路径排进下一轮打磨。",
        ),
        (
            "workflow",
            "Markdown 写完只是前半段；这次不用再把它搬进 PPT，放映和修改在同一条工作流里。",
            "你会先用课程讲义、组会报告还是技术分享来试？评论区说说场景。",
        ),
        (
            "decision",
            "定稿后还要把 Markdown 复制成 PPT，这一步最磨人；现在不用复制，MD 可以直接上台。",
            "哪一份 Markdown 最适合先试？代码、表格还是公式？评论区告诉我。",
        ),
        (
            "source",
            "写完的 Markdown 不用复制到别的工具；ReadMD 把阅读、修改和放映接成一条路。",
            "你会拿哪类内容先跑一遍完整流程？讲义、论文还是技术笔记？",
        ),
    ],
    "identity-led": [
        (
            "core",
            "如果你要把笔记变成课程讲义、组会报告或论文汇报，就知道重做 PPT 有多烦。ReadMD 把这一步砍掉：Markdown 直接放映。",
            "你下一份要上台的 Markdown 是讲义、组会报告还是论文？评论区说说场景。",
        ),
        (
            "workflow",
            "如果你常写论文或组会报告，就不用再把 Markdown 复制进 PPT；同一份文件可以直接讲。",
            "你的下一场分享是课程、组会还是论文答辩？评论区对号入座。",
        ),
        (
            "decision",
            "要上台讲自己文档的人，最怕格式在复制时走样；MD 这次能直接保留工作流。",
            "你会讲哪一份材料？课程讲义、组会报告还是论文教程？",
        ),
        (
            "source",
            "给要把笔记变成正式汇报的人：不用重建 PPT，Markdown 就是演示入口。",
            "你会先讲哪一类文件？论文、组会记录还是课程讲义？",
        ),
    ],
    "mechanism-curiosity": [
        (
            "core",
            "很多人把 Markdown 写完就停在笔记里；其实同一份文件可以直接上台放映。ReadMD 让写作和演示留在同一条路径。",
            "你想先试哪类内容：代码、表格还是公式？评论区告诉我，我会优先打磨这条路径。",
        ),
        (
            "workflow",
            "写完的 Markdown 为什么能直接放映？因为写作、预览和演示被接成同一条路径。",
            "你想先验证哪一段链路：代码、表格还是公式？评论区选一个。",
        ),
        (
            "decision",
            "它不用把 Markdown 另存为幻灯片；写完后的阅读、修改和放映共用同一个源文件。",
            "你最想保住哪种排版？代码块、表格还是公式？评论区补充场景。",
        ),
        (
            "source",
            "从写作到上台只有一条路径；Markdown 不用导出成 PPT，显示状态由同一份源文件驱动。",
            "你会先用哪个机制试一遍：公式渲染、表格分片还是代码运行？",
        ),
    ],
}


def _generated_frames(profile: dict[str, Any]) -> dict[str, list[tuple[str, str, str]]]:
    artifact = profile["artifact"]
    action = profile["short_action"]
    task_hook = profile["task_hook"]
    return {
        "outcome-led": [
            ("core", f"{profile['opening']} Markdown 保持为主文件。", profile["cta"]),
            (
                "workflow",
                f"处理{task_hook}时，不用把它搬去别的工具：继续{action}，{profile['workflow']}。",
                f"你会先用{profile['options']}中的哪一类来试？评论区说说场景。",
            ),
            (
                "decision",
                f"处理{task_hook}时，{profile['decision_pain']}，这一步最磨人；现在不用绕路，Markdown 可以直接{action}。",
                f"哪份材料最适合先用来{action}？{profile['options']}都可以，评论区告诉我。",
            ),
            (
                "source",
                f"{artifact}不用脱离源文件；ReadMD 把处理{task_hook}、阅读、修改和{action}接成一条 Markdown 路。",
                f"你会先用哪类内容跑一遍完整流程？{profile['scenarios']}都可以。",
            ),
        ],
        "identity-led": [
            (
                "core",
                f"如果你要{profile['audience_task']}，就知道{profile['pain']}。ReadMD 把这一步砍掉：Markdown 可以直接{action}。",
                f"你下一份要处理{artifact}的材料是什么？{profile['scenarios']}都可以说说。",
            ),
            (
                "workflow",
                f"如果你常处理{task_hook}，就不用把它搬出 Markdown；继续{action}时，同一份文件能继续维护。",
                f"你下一场要处理{artifact}的场景是课程、组会还是个人项目？评论区对号入座。",
            ),
            (
                "decision",
                f"要{profile['audience_task']}的人，最怕处理{task_hook}时{profile['decision_pain']}；ReadMD 这次让 Markdown 能直接{action}。",
                f"你会先处理哪份材料？课程、报告还是{profile['options']}？",
            ),
            (
                "source",
                f"给要处理{profile['task_hook']}的人：不用换工具，Markdown 就是工作入口。",
                f"你会先放进哪一类文件？{profile['scenarios']}都可以。",
            ),
        ],
        "mechanism-curiosity": [
            (
                "core",
                f"很多人把{task_hook}停在旧流程里；其实它能留在 Markdown 里，直接{action}。ReadMD 让内容和结果走同一条路径。",
                f"你想先试哪类{artifact}：{profile['options']}？评论区告诉我，我会优先打磨这条路径。",
            ),
            (
                "workflow",
                f"{task_hook}为什么能稳定更新？因为它不用离开当前文档；{profile['workflow']}。",
                f"你想先验证哪段链路：{profile['options']}？评论区选一个。",
            ),
            (
                "decision",
                f"它不用一次性截图；{task_hook}的 Markdown 状态由源文件驱动。{profile['proof']}。",
                "你最想保住哪种状态：源文件、渲染结果还是可更新结构？评论区补充场景。",
            ),
            (
                "source",
                f"不用导出中间稿：从内容到{artifact}只有一条路径；{action}之后，Markdown 显示状态仍由源文件驱动。",
                f"你会先用哪份{artifact}试这条路径？评论区说说场景。",
            ),
        ],
    }


SUPPORT_PHRASES: dict[str, str] = {
    "overview.reader": "阅读端保持目录和公式排版",
    "overview.editor": "改稿时回到同屏预览",
    "presentation.reveal": "放映端直接承接讲稿",
    "editor.diagram-picker": "科研图表留在文档里",
    "academic.latex-bib": "学术排版不另起一套工具",
    "editor.code-chunk": "代码示例可以就地验证",
    "convert.home": "资料仍从一个本地入口进来",
    "sharing.export": "手机端可直接查看当前文档",
}


COMMENT_SCENARIOS: dict[str, str] = {
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


COMMENT_SHOT_FOCUS: dict[str, str] = {
    "presentation": "presentation.reveal",
    "academic": "academic.latex-bib",
    "code": "editor.code-chunk",
    "table": "overview.editor",
    "formula": "academic.latex-bib",
    "diagram": "editor.diagram-picker",
    "conversion": "convert.home",
    "export-share": "sharing.export",
    "local-privacy": "overview.reader",
    "stability-performance": "overview.editor",
}


RESONANCE_INTENT_FRAME_WEIGHTS: dict[str, dict[str, int]] = {
    "request": {"workflow": 8},
    "question": {"decision": 7, "workflow": 2},
    "concern": {"source": 8},
    "praise": {"core": 4},
}


RESONANCE_INTENT_TITLE_WEIGHTS: dict[str, dict[str, int]] = {
    "request": {"#36": 8, "#61": 4},
    "question": {"#9": 8, "#12": 4},
    "concern": {"#61": 6, "#36": 3},
    "praise": {"#22": 6},
}


RESONANCE_CONCERN_RESPONSE = (
    "常见顾虑先说清：源文件仍留在本地，放映、导出和分享只处理显示结果，不会替你改写原稿。"
)


RESONANCE_TOPIC_TERMS: dict[str, tuple[str, ...]] = {
    "presentation": ("PPT", "演讲", "课程讲义", "组会报告"),
    "academic": ("论文写作", "课程讲义", "研究生", "学术排版"),
    "code": ("编程", "技术教程", "代码运行", "Python", "JavaScript"),
    "formula": ("LaTeX", "论文写作", "研究生", "学术排版"),
    "diagram": ("流程图", "科研绘图", "Mermaid", "架构图"),
    "conversion": ("PDF", "Word", "网页剪藏", "OCR", "资料整理"),
    "export-share": ("文档分享", "局域网分享", "手机查看"),
}


def resonance_frame_adjustment(
    directive: dict[str, Any] | None,
    *,
    copy_frame: str,
) -> tuple[float, list[str]]:
    """Return the evidence-gated narrative preference for one comment intent."""
    if not isinstance(directive, dict) or directive.get("applied") is not True:
        return 0.0, []
    evidence = directive.get("evidence")
    if not isinstance(evidence, dict) or evidence.get("confidence") not in {"medium", "high"}:
        return 0.0, []
    intents = evidence.get("top_intents")
    if not isinstance(intents, list):
        return 0.0, []

    reasons: list[str] = []
    adjustment = 0.0
    for raw_intent in intents:
        intent = str(raw_intent).strip()
        weight = int(RESONANCE_INTENT_FRAME_WEIGHTS.get(intent, {}).get(copy_frame, 0))
        if weight:
            adjustment += weight
            reasons.append(f"comment {intent} intent prefers the {copy_frame} narrative")
    return min(adjustment, 12.0), reasons


def resonance_topic_adjustment(
    directive: dict[str, Any] | None,
    *,
    topics: list[str],
) -> tuple[float, str | None]:
    """Return a bounded preference for search terms matching the comment focus."""
    if not isinstance(directive, dict) or directive.get("applied") is not True:
        return 0.0, None
    evidence = directive.get("evidence")
    if not isinstance(evidence, dict) or evidence.get("confidence") not in {"medium", "high"}:
        return 0.0, None
    if directive.get("support_available") is not True:
        return 0.0, None

    focus = str(evidence.get("focus", "")).strip()
    preferred_terms = RESONANCE_TOPIC_TERMS.get(focus, ())
    normalized_topics = {str(item).strip() for item in topics}
    if not preferred_terms or not any(term in normalized_topics for term in preferred_terms):
        return 0.0, None
    return 11.0, f"comment {focus} focus matches topic search terms"


def resonance_title_adjustment(
    directive: dict[str, Any] | None,
    *,
    title_formula_id: str,
) -> tuple[float, list[str]]:
    """Return the evidence-gated title-formula preference for comment intents."""
    if not isinstance(directive, dict) or directive.get("applied") is not True:
        return 0.0, []
    evidence = directive.get("evidence")
    if not isinstance(evidence, dict) or evidence.get("confidence") not in {"medium", "high"}:
        return 0.0, []
    if directive.get("support_available") is not True:
        return 0.0, []

    intents = evidence.get("top_intents")
    if not isinstance(intents, list):
        return 0.0, []

    reasons: list[str] = []
    adjustment = 0.0
    for raw_intent in intents:
        intent = str(raw_intent).strip()
        weight = int(RESONANCE_INTENT_TITLE_WEIGHTS.get(intent, {}).get(title_formula_id, 0))
        if weight:
            adjustment += weight
            reasons.append(f"comment {intent} intent prefers the {title_formula_id} title")
    return min(adjustment, 8.0), reasons


MECHANISM_TOPIC_SETS: dict[str, list[dict[str, Any]]] = {
    "overview.editor": [
        {"label": "writing-core", "topics": ["Markdown", "效率工具", "程序员", "写作", "笔记软件"]},
        {"label": "research-writing", "topics": ["Markdown", "课程讲义", "论文写作", "研究生", "写作"]},
    ],
    "presentation.reveal": [
        {"label": "talk-core", "topics": ["Markdown", "PPT", "演讲", "程序员", "效率工具"]},
        {"label": "academic-talk", "topics": ["Markdown", "PPT", "课程讲义", "组会报告", "演讲"]},
    ],
    "editor.diagram-picker": [
        {"label": "diagram-core", "topics": ["Markdown", "流程图", "科研绘图", "论文", "研究生"]},
        {"label": "engineering-diagram", "topics": ["Markdown", "Mermaid", "架构图", "技术方案", "科研绘图"]},
    ],
    "academic.latex-bib": [
        {"label": "latex-core", "topics": ["LaTeX", "论文写作", "研究生", "学术排版", "Markdown"]},
        {"label": "reference-flow", "topics": ["LaTeX", "参考文献", "Zotero", "学术排版", "论文写作"]},
    ],
    "editor.code-chunk": [
        {"label": "code-core", "topics": ["编程", "Markdown", "程序员", "技术教程", "代码运行"]},
        {"label": "language-examples", "topics": ["Python", "JavaScript", "代码运行", "开发者教程", "Markdown"]},
    ],
    "convert.home": [
        {"label": "convert-core", "topics": ["PDF", "资料整理", "Markdown", "效率工具", "Word"]},
        {"label": "capture-ocr", "topics": ["网页剪藏", "OCR", "资料整理", "Markdown", "效率工具"]},
    ],
    "sharing.export": [
        {"label": "share-core", "topics": ["文档分享", "Markdown", "效率工具", "开源项目", "程序员"]},
        {"label": "lan-share", "topics": ["局域网分享", "手机查看", "文档分享", "Markdown", "效率工具"]},
    ],
}


MECHANISM_TOPICS: dict[str, list[str]] = {
    primary_shot: topic_sets[0]["topics"]
    for primary_shot, topic_sets in MECHANISM_TOPIC_SETS.items()
}


MECHANISM_TOPIC_MARKERS: dict[str, set[str]] = {
    "overview.editor": {"写作", "笔记软件"},
    "presentation.reveal": {"PPT", "演讲"},
    "editor.diagram-picker": {"流程图", "科研绘图", "论文", "研究生"},
    "academic.latex-bib": {"LaTeX", "论文写作", "研究生", "学术排版"},
    "editor.code-chunk": {"编程", "技术教程", "代码运行"},
    "convert.home": {"PDF", "资料整理", "Word"},
    "sharing.export": {"文档分享", "开源项目"},
}


# Each experiment keeps the psychological trigger of its source Xiaohongshu
# formula. A formula id alone is attribution, not evidence that the title still
# uses that mechanism.
TITLE_FORMULA_CONTRACTS: dict[str, dict[str, Any]] = {
    "#36": {
        "family": "outcome-promise",
        "removal_any": ("不用", "没有", "无需"),
        "result_any": ("直接", "能", "可以", "进", "收", "改", "看", "放映", "分享", "排版"),
    },
    "#9": {
        "family": "curiosity-gap",
        "surprise_any": ("居然", "为何", "怎么", "为什么", "意想不到"),
    },
    "#22": {
        "family": "identity-fit",
        "prefix": "给",
        "suffix": "的人",
    },
    "#61": {
        "family": "action-interruption",
        "action_any": ("别再", "别把", "别让", "停止"),
    },
    "#12": {
        "family": "perspective-shift",
        "carousel": re.compile(r"看完这\d+张"),
        "shift_any": ("重新", "不再相同", "换个"),
    },
    "#26": {
        "family": "number-anchor",
        "carousel": re.compile(r"\d+张图"),
        "anchor": "看懂",
    },
}

EXPERIMENT_TITLE_FORMULAS = tuple(TITLE_FORMULA_CONTRACTS)


def title_formula_errors(title: str, formula_id: str) -> list[str]:
    """Return structural mismatches between a title and its declared formula."""
    value = str(title).strip()
    contract = TITLE_FORMULA_CONTRACTS.get(str(formula_id).strip())
    if contract is None:
        return [f"unknown title formula: {formula_id}"]

    errors: list[str] = []
    if any(key in contract for key in ("removal_any", "result_any")):
        if not any(term in value for term in contract.get("removal_any", ())):
            errors.append(f"title formula {formula_id} is missing a removal condition")
        if not any(term in value for term in contract.get("result_any", ())):
            errors.append(f"title formula {formula_id} is missing an outcome")
    if "surprise_any" in contract and not any(term in value for term in contract["surprise_any"]):
        errors.append(f"title formula {formula_id} is missing a curiosity signal")
    if "prefix" in contract and not value.startswith(contract["prefix"]):
        errors.append(f"title formula {formula_id} is missing identity targeting")
    if "suffix" in contract and contract["suffix"] not in value:
        errors.append(f"title formula {formula_id} is missing the target reader")
    if "action_any" in contract and not any(term in value for term in contract["action_any"]):
        errors.append(f"title formula {formula_id} is missing an action interruption")
    if "carousel" in contract and not contract["carousel"].search(value):
        errors.append(f"title formula {formula_id} is missing its carousel anchor")
    if "shift_any" in contract and not any(term in value for term in contract["shift_any"]):
        errors.append(f"title formula {formula_id} is missing a perspective shift")
    if "anchor" in contract and contract["anchor"] not in value:
        errors.append(f"title formula {formula_id} is missing the comprehension anchor")
    return errors


def title_candidate_errors(candidates: list[dict[str, Any]]) -> list[str]:
    """Validate the experiment pool before publication history can rank it."""
    errors: list[str] = []
    formulas = [str(item.get("formula_id", "")).strip() for item in candidates]
    expected = set(EXPERIMENT_TITLE_FORMULAS)
    if len(formulas) != len(expected) or set(formulas) != expected:
        errors.append("title candidate formulas do not match the approved experiment set")

    families = {
        TITLE_FORMULA_CONTRACTS[formula]["family"]
        for formula in formulas
        if formula in TITLE_FORMULA_CONTRACTS
    }
    if len(families) < 3:
        errors.append("title candidates must cover at least 3 trigger families")
    for item in candidates:
        errors.extend(title_formula_errors(str(item.get("text", "")), str(item.get("formula_id", ""))))
    return errors


PROFILES: dict[str, dict[str, Any]] = {
    "overview.editor": {
        "artifact": "Markdown 稿件",
        "support_priorities": [
            "overview.reader",
            "editor.diagram-picker",
            "academic.latex-bib",
            "editor.code-chunk",
            "convert.home",
            "sharing.export",
            "presentation.reveal",
        ],
        "narrative_angle": "ReadMD 让写作和预览留在同一条工作流",
        "task_hook": "同屏改稿窗口",
        "short_action": "在同屏编辑器里改稿",
        "pain": "改一段 Markdown 就要切窗口核对格式，思路反复被打断",
        "decision_pain": "源稿改动后预览跟不上",
        "workflow": "源稿、光标位置和实时预览留在同一条工作流里",
        "options": "课程讲义、技术笔记、项目说明",
        "proof": "编辑器内容和预览状态由同一份源文件驱动",
        "audience_task": "反复打磨一份 Markdown 稿件",
        "scenarios": "课程讲义、技术笔记、项目说明",
        "titles": {
            "#36": "不用切窗口，Markdown同屏改",
            "#9": "Markdown改完，居然立刻出排版",
            "#22": "给反复改稿的人做的MD工具",
            "#61": "别再把写作和预览拆开",
            "#12": "看完这{number}张，你会重新看MD编辑",
            "#26": "{number}张图，看懂同屏改稿",
        },
        "opening": "改一段就要切窗口核对格式。这次把这一步砍掉：Markdown 源稿和实时预览在同一屏。",
        "primary_paragraph": "编辑器和实时预览在同一屏里，先改内容再确认排版，不用在几个窗口之间来回追版本。",
        "saved_step": "在几个窗口之间追版本",
        "cover": {
            "formula_id": "#36",
            "title": "同屏改稿",
            "caption": "改完立刻看到排版，不用切窗口。",
        },
        "cover_variants": {
            "#36": {"formula_id": "#36", "title": "同屏改稿", "caption": "改完立刻看到排版，不用切窗口。"},
            "#9": {"formula_id": "#9", "title": "改完就能看", "caption": "源稿和预览同步出现，不用猜排版。"},
            "#22": {"formula_id": "#22", "title": "反复改稿的人", "caption": "为讲义和笔记保留同屏工作流。"},
            "#61": {"formula_id": "#61", "title": "别再切窗口", "caption": "Markdown 源稿和预览留在同一屏。"},
            "#12": {"formula_id": "#12", "title": "换个改稿法", "caption": "同一份 Markdown 可以边写边看。"},
            "#26": {"formula_id": "#26", "title": "截图看改稿", "caption": "真实画面拆解同屏改稿流程。"},
        },
        "summary": {
            "title": "改稿不切窗",
            "caption": "源稿和预览留在同一屏。",
            "proof_points": ["同屏预览", "本地源文件", "少一次切换"],
        },
        "cta": "你会先用哪份 Markdown 同屏修改？课程讲义、技术笔记还是项目说明？",
        "hook_contract": {
            "task": ["改", "窗口", "格式"],
            "removal": ["不用", "砍掉", "直接"],
            "mechanism": ["Markdown", "同屏"],
        },
    },
    "presentation.reveal": {
        "artifact": "Markdown 演示稿",
        "support_priorities": [
            "editor.code-chunk",
            "editor.diagram-picker",
            "overview.editor",
            "academic.latex-bib",
            "convert.home",
            "sharing.export",
            "overview.reader",
        ],
        "narrative_angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
        "task_hook": "上台放映的演示稿",
        "short_action": "直接放映",
        "pain": "文档已经写完，讲的时候还要复制进 PPT",
        "decision_pain": "定稿后还要把 Markdown 复制成 PPT",
        "workflow": "写作、预览和放映被接成同一条工作流",
        "options": "代码、表格、公式",
        "proof": "阅读、修改和放映共用同一个源文件",
        "audience_task": "把笔记变成课程讲义、组会报告或论文汇报",
        "scenarios": "课程讲义、组会报告、论文汇报",
        "titles": {
            "#36": "不用重做PPT，Markdown直接放映",
            "#9": "Markdown写完，居然能直接上台",
            "#22": "给要上台讲文档的人做的MD工具",
            "#61": "别再把Markdown只当笔记了",
            "#12": "看完这{number}张，你会重新看Markdown",
            "#26": "{number}张图，看懂MD直接放映",
        },
        "opening": "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。",
        "primary_paragraph": "放映界面可以直接换主题、调字号、切开场和转场；AST 保护分片会尽量保住代码块、表格和公式，不让长文档在幻灯片里被腰斩。",
        "saved_step": "重新做一遍演示稿",
        "cover": {
            "formula_id": "#36",
            "title": "写完就能讲",
            "caption": "Markdown 直接放映，不用重做 PPT。",
        },
        "cover_variants": {
            "#36": {"formula_id": "#36", "title": "写完就能讲", "caption": "Markdown 直接放映，不用重做 PPT。"},
            "#9": {"formula_id": "#9", "title": "为何直接讲", "caption": "同一份 Markdown 从写作走到上台。"},
            "#22": {"formula_id": "#22", "title": "上台讲文档的人", "caption": "课程、组会和论文汇报都适用。"},
            "#61": {"formula_id": "#61", "title": "别重做PPT", "caption": "讲稿定稿后直接进入放映。"},
            "#12": {"formula_id": "#12", "title": "换个讲法", "caption": "写作、修改和放映共用一份文件。"},
            "#26": {"formula_id": "#26", "title": "截图看放映", "caption": "真实画面拆解放映完整路径。"},
        },
        "summary": {
            "title": "一条放映路",
            "caption": "写作、修改和上台共用一份文件。",
            "proof_points": ["同一份 MD", "真实排版", "直接放映"],
        },
        "cta": "你会先拿哪一份 Markdown 试放映？评论区说说场景，我会把高频路径排进下一轮打磨。",
        "hook_contract": {
            "task": ["写完", "复制", "PPT", "讲"],
            "removal": ["砍掉", "不用", "直接"],
            "mechanism": ["Markdown", "MD", "放映"],
        },
        "frames": PRESENTATION_FRAMES,
    },
    "editor.diagram-picker": {
        "artifact": "科研图表",
        "support_priorities": [
            "academic.latex-bib",
            "editor.code-chunk",
            "overview.editor",
            "overview.reader",
            "presentation.reveal",
            "convert.home",
            "sharing.export",
        ],
        "narrative_angle": "ReadMD 把科研图表放进同一条 Markdown 工作流",
        "task_hook": "科研图表语法",
        "short_action": "从面板选图",
        "pain": "画科研图表还要回忆语法，改一次就很折磨",
        "decision_pain": "图表语法一改就废",
        "workflow": "图表结构、源码和文档内容在同一条 Markdown 工作流里",
        "options": "流程图、架构图、时序图",
        "proof": "面板结构、源码和渲染结果共用同一份文档",
        "audience_task": "把论文或技术笔记里的关系画成科研图表",
        "scenarios": "论文、实验记录、技术报告",
        "titles": {
            "#36": "不用记语法，科研图表直接选",
            "#9": "科研图表，居然可以面板里选",
            "#22": "给论文画图的人做的MD工具",
            "#61": "别再手写一版就废的图表语法",
            "#12": "看完这{number}张，你会重新看科研图",
            "#26": "{number}张图，看懂科研图表选图",
        },
        "opening": "画科研图表还要回忆语法，改一次就很折磨。这次不用硬记：Markdown 面板选择结构，结果留在文档里。",
        "primary_paragraph": "图表从面板里选，渲染结果留在文档里；适合论文、报告和需要长期维护的技术笔记。",
        "saved_step": "手写一遍就报废的图表语法",
        "cover": {
            "formula_id": "#36",
            "title": "图表直接选",
            "caption": "科研图从面板进入 Markdown，不背语法。",
        },
        "cover_variants": {
            "#36": {"formula_id": "#36", "title": "图表直接选", "caption": "科研图从面板进入 Markdown，不背语法。"},
            "#9": {"formula_id": "#9", "title": "图表怎么选", "caption": "科研图从面板进入 Markdown 文档。"},
            "#22": {"formula_id": "#22", "title": "论文画图的人", "caption": "流程、架构和时序图可以长期维护。"},
            "#61": {"formula_id": "#61", "title": "别背图语法", "caption": "从面板选择结构，渲染留在文档里。"},
            "#12": {"formula_id": "#12", "title": "换种画图法", "caption": "科研图表和文稿留在同一条路径。"},
            "#26": {"formula_id": "#26", "title": "截图看选图", "caption": "真实画面拆解科研图表入口。"},
        },
        "summary": {
            "title": "图随文稿走",
            "caption": "科研图表留在可维护的 Markdown 里。",
            "proof_points": ["面板选图", "源码可改", "渲染留档"],
        },
        "cta": "你会先给论文里的哪类内容画图？流程、架构还是时序？评论区说说场景。",
        "hook_contract": {
            "task": ["图表", "语法", "画"],
            "removal": ["不用", "面板", "选择", "直接"],
            "mechanism": ["科研图表", "文档", "Markdown"],
        },
    },
    "academic.latex-bib": {
        "artifact": "学术排版",
        "support_priorities": [
            "editor.diagram-picker",
            "overview.editor",
            "presentation.reveal",
            "overview.reader",
            "editor.code-chunk",
            "convert.home",
            "sharing.export",
        ],
        "narrative_angle": "ReadMD 让公式、定理和引用共用一条学术排版路径",
        "task_hook": "公式和文献格式",
        "short_action": "在文档里排公式",
        "pain": "公式和文献格式总在交稿前折磨人",
        "decision_pain": "公式换个工具就走样",
        "workflow": "公式、定理盒子和引用沿用同一套 Markdown 排版规则",
        "options": "推导、定理、参考文献",
        "proof": "公式、定理盒子和引用由同一份源文件驱动",
        "audience_task": "把论文和学术笔记排到能交的程度",
        "scenarios": "课程讲义、论文推导、学术笔记",
        "titles": {
            "#36": "不用换模板，公式保持论文排版",
            "#9": "LaTeX公式，居然不用来回调",
            "#22": "给写论文的人做的学术MD工具",
            "#61": "别再为公式格式重复返工",
            "#12": "看完这{number}张，你会重新看学术排版",
            "#26": "{number}张图，看懂论文级排版",
        },
        "opening": "公式和文献格式总在交稿前折磨人。这次不用另起工具：LaTeX 和引用留在 Markdown 里。",
        "primary_paragraph": "公式、定理盒子和参考文献沿用同一套排版；写作时不用在笔记、LaTeX 和最终稿之间反复搬运。",
        "saved_step": "在另一个工具里重调公式格式",
        "cover": {
            "formula_id": "#36",
            "title": "公式不走样",
            "caption": "LaTeX 和引用留在同一条工作流。",
        },
        "cover_variants": {
            "#36": {"formula_id": "#36", "title": "公式不走样", "caption": "LaTeX 和引用留在同一条工作流。"},
            "#9": {"formula_id": "#9", "title": "公式为何稳", "caption": "LaTeX 和引用由同一条工作流驱动。"},
            "#22": {"formula_id": "#22", "title": "写论文的人", "caption": "推导、定理和参考文献不离开笔记。"},
            "#61": {"formula_id": "#61", "title": "别重复调格式", "caption": "公式排版在 Markdown 里一次到位。"},
            "#12": {"formula_id": "#12", "title": "换种排版法", "caption": "学术笔记保持论文级排版。"},
            "#26": {"formula_id": "#26", "title": "截图看排版", "caption": "真实画面拆解学术排版路径。"},
        },
        "summary": {
            "title": "论文级排版",
            "caption": "公式、定理和引用在同一处维护。",
            "proof_points": ["LaTeX", "定理盒子", "文献引用"],
        },
        "cta": "你下一份要处理的是课程讲义还是论文？评论区说说公式场景。",
        "hook_contract": {
            "task": ["公式", "文献", "交稿"],
            "removal": ["不用", "留在", "直接"],
            "mechanism": ["LaTeX", "Markdown", "排版"],
        },
    },
    "editor.code-chunk": {
        "artifact": "文档代码块",
        "support_priorities": [
            "overview.editor",
            "editor.diagram-picker",
            "presentation.reveal",
            "overview.reader",
            "convert.home",
            "sharing.export",
            "academic.latex-bib",
        ],
        "narrative_angle": "ReadMD 让文档里的代码就地运行并保留输出",
        "task_hook": "可运行代码块",
        "short_action": "就地运行代码",
        "pain": "教程写到代码就要切出去验证，上下文很容易断",
        "decision_pain": "示例代码和文档说明脱节",
        "workflow": "代码、运行状态和输出留在同一条文档路径里",
        "options": "配置脚本、算法示例、数据处理",
        "proof": "代码块、运行按钮和输出区共用同一份文档",
        "audience_task": "写出能跟着复现的技术教程",
        "scenarios": "代码教程、技术笔记、示例文档",
        "titles": {
            "#36": "不用切终端，文档代码直接跑",
            "#9": "Markdown代码块，居然能运行",
            "#22": "给写技术教程的人做的MD工具",
            "#61": "别再让示例代码停在文档里",
            "#12": "看完这{number}张，你会重新看代码块",
            "#26": "{number}张图，看懂代码就地跑",
        },
        "opening": "教程写到代码，还要切出去验证一遍。这次不用切换：代码块直接在 Markdown 里运行。",
        "primary_paragraph": "代码块保留运行按钮、状态和输出；读者看到的不是死代码，而是能跟着复现的步骤。",
        "saved_step": "另开终端重复验证示例",
        "cover": {
            "formula_id": "#36",
            "title": "代码就地跑",
            "caption": "教程里的示例能运行，读者能复现。",
        },
        "cover_variants": {
            "#36": {"formula_id": "#36", "title": "代码就地跑", "caption": "教程里的示例能运行，读者能复现。"},
            "#9": {"formula_id": "#9", "title": "代码为何能跑", "caption": "运行按钮、状态和输出都在文档里。"},
            "#22": {"formula_id": "#22", "title": "写技术教程的人", "caption": "读者可以跟着示例直接复现。"},
            "#61": {"formula_id": "#61", "title": "别只贴代码", "caption": "让示例就地运行并保留输出。"},
            "#12": {"formula_id": "#12", "title": "换个教程法", "caption": "说明、代码和结果不会脱节。"},
            "#26": {"formula_id": "#26", "title": "截图看运行", "caption": "真实画面拆解就地运行过程。"},
        },
        "summary": {
            "title": "能跑的教程",
            "caption": "代码、结果和说明留在同一段。",
            "proof_points": ["就地运行", "保留输出", "读者复现"],
        },
        "cta": "你会先验证哪段代码？配置脚本、算法示例还是数据处理？评论区挑一个。",
        "hook_contract": {
            "task": ["教程", "代码", "验证"],
            "removal": ["不用", "直接", "就地"],
            "mechanism": ["Markdown", "代码块", "运行"],
        },
    },
    "convert.home": {
        "artifact": "零散资料",
        "support_priorities": [
            "overview.editor",
            "overview.reader",
            "academic.latex-bib",
            "editor.code-chunk",
            "editor.diagram-picker",
            "sharing.export",
            "presentation.reveal",
        ],
        "narrative_angle": "ReadMD 把网页、PDF 和 Word 收进同一条本地工作流",
        "task_hook": "网页、PDF 和 Word 资料",
        "short_action": "收进本地工作台",
        "pain": "网页、Word 和 PDF 分散在不同窗口，资料格式很难归拢",
        "decision_pain": "资料转换后找不到来源",
        "workflow": "打开、转换、AI 和抓取入口接在同一条本地工作流里",
        "options": "网页、PDF、Word",
        "proof": "转换入口、新标签页和源文件都在本地工作台里",
        "audience_task": "把零散资料整理成可检索的 Markdown",
        "scenarios": "网页剪藏、PDF 资料、Word 文档",
        "titles": {
            "#36": "不用来回搬，资料收进同一个入口",
            "#9": "零散资料，居然能进一个工作台",
            "#22": "给整理资料的人做的MD工具",
            "#61": "别再让资料散落在临时工具里",
            "#12": "看完这{number}张，你会重新看资料整理",
            "#26": "{number}张图，看懂资料一处收",
        },
        "opening": "网页、Word 和 PDF 分散在不同窗口，资料格式很难归拢。这次不用搬运：它们进入同一条 Markdown 工作流。",
        "primary_paragraph": "打开、转换、AI 和网页抓取都从一个本地入口开始，资料不会被拆到一串临时工具里。",
        "saved_step": "在不同工具之间搬运资料",
        "cover": {
            "formula_id": "#36",
            "title": "资料进工作台",
            "caption": "网页、PDF 和 Word 收进同一个入口。",
        },
        "cover_variants": {
            "#36": {"formula_id": "#36", "title": "资料进工作台", "caption": "网页、PDF 和 Word 收进同一个入口。"},
            "#9": {"formula_id": "#9", "title": "资料去哪了", "caption": "网页、PDF 和 Word 进入同一个入口。"},
            "#22": {"formula_id": "#22", "title": "整理资料的人", "caption": "剪藏和文档转成可检索的 Markdown。"},
            "#61": {"formula_id": "#61", "title": "别散存资料", "caption": "转换和阅读收进本地工作台。"},
            "#12": {"formula_id": "#12", "title": "换个整理法", "caption": "零散资料接进同一条工作流。"},
            "#26": {"formula_id": "#26", "title": "截图看归拢", "caption": "真实画面拆解资料收拢入口。"},
        },
        "summary": {
            "title": "资料一处收",
            "caption": "网页和文档进入同一条本地流程。",
            "proof_points": ["网页/PDF", "Word 文档", "本地工作台"],
        },
        "cta": "你会先把网页、PDF 还是 Word 收进来？评论区说说整理场景。",
        "hook_contract": {
            "task": ["网页", "Word", "PDF", "资料"],
            "removal": ["不用", "进入", "收进"],
            "mechanism": ["Markdown", "本地", "工作流"],
        },
    },
    "sharing.export": {
        "artifact": "本地文档",
        "support_priorities": [
            "overview.editor",
            "overview.reader",
            "presentation.reveal",
            "convert.home",
            "academic.latex-bib",
            "editor.code-chunk",
            "editor.diagram-picker",
        ],
        "narrative_angle": "ReadMD 让本地文档直接生成可控制的共享入口",
        "task_hook": "分享或导出的本地文档",
        "short_action": "生成共享入口",
        "pain": "想把 Markdown 发给别人，还要先导出副本，版本很快对不上",
        "decision_pain": "分享出去的文件已经过期",
        "workflow": "共享入口、访问令牌和当前文档留在同一条本地工作流里",
        "options": "讲义、报告、长笔记",
        "proof": "扫码入口、随机令牌和启停控制都指向当前文档",
        "audience_task": "把正在维护的 Markdown 发给别人看",
        "scenarios": "讲义、报告、长笔记",
        "titles": {
            "#36": "不用重复导出，文档直接分享",
            "#9": "本地文档，居然能直接到手机",
            "#22": "给要分享文档的人做的MD工具",
            "#61": "别再为了分享多存一份副本",
            "#12": "看完这{number}张，你会重新看文档分享",
            "#26": "{number}张图，看懂文档直接分享",
        },
        "opening": "想把 Markdown 发给别人，还要先导出一份副本，版本很快对不上。这次不用多存：当前文档直接生成共享入口。",
        "primary_paragraph": "局域网共享面板提供扫码入口、随机令牌和启停控制；手机打开的是当前文档，不需要对方装软件。",
        "saved_step": "为了分享再导出一份副本",
        "cover": {
            "formula_id": "#36",
            "title": "文档到手机",
            "caption": "本地文档生成共享入口，不用再导出副本。",
        },
        "cover_variants": {
            "#36": {"formula_id": "#36", "title": "文档到手机", "caption": "本地文档生成共享入口，不用再导出副本。"},
            "#9": {"formula_id": "#9", "title": "怎么发到手机", "caption": "当前文档生成共享入口，不用导出副本。"},
            "#22": {"formula_id": "#22", "title": "要分享文档的人", "caption": "讲义和报告可以直接到手机。"},
            "#61": {"formula_id": "#61", "title": "别多存副本", "caption": "分享入口始终指向当前文档。"},
            "#12": {"formula_id": "#12", "title": "换个分享法", "caption": "令牌和启停控制保持可控。"},
            "#26": {"formula_id": "#26", "title": "截图看分享", "caption": "真实画面拆解可控分享流程。"},
        },
        "summary": {
            "title": "分享可控",
            "caption": "当前文档可以发给手机，也能停止。",
            "proof_points": ["扫码入口", "随机令牌", "随时停用"],
        },
        "cta": "你会先分享哪份 Markdown？讲义、报告还是长笔记？",
        "hook_contract": {
            "task": ["分享", "导出", "版本"],
            "removal": ["不用", "直接", "当前"],
            "mechanism": ["Markdown", "共享入口"],
        },
    },
}


def profile_for_story(story: dict[str, Any]) -> dict[str, Any]:
    return PROFILES.get(str(story.get("primary_shot", "overview.editor")), PROFILES["overview.editor"])


def cover_for_title(profile: dict[str, Any], title_formula_id: str) -> dict[str, Any]:
    """Return the feed hook that matches the selected title's psychological trigger."""
    variants = profile.get("cover_variants", {})
    variant = variants.get(str(title_formula_id))
    if not isinstance(variant, dict):
        raise ValueError(f"missing cover variant for {title_formula_id}")
    return dict(variant)


def frames_for_story(story: dict[str, Any]) -> dict[str, list[tuple[str, str, str]]]:
    profile = profile_for_story(story)
    return profile.get("frames") or _generated_frames(profile)
