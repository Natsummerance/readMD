#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate a Xiaohongshu package from evidence-backed story data."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


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
    state = "预览版" if story["version_state"] != "release" else "更新"
    return [
        {"formula_id": "#26", "text": f"ReadMD{state}：{number}个文档工作台升级"},
        {"formula_id": "#12", "text": f"看完这{number}张，你会重新看Markdown"},
        {"formula_id": "#22", "text": "为被Markdown格式折磨的人做的工具"},
    ]


def generate_copy(story: dict[str, Any], *, repository: str, previous_release: str) -> dict[str, Any]:
    candidates = _title_candidates(story)
    chosen_title = next((item for item in candidates if len(item["text"]) <= 20), candidates[0])
    chosen_title["text"] = _clean(chosen_title["text"])
    release = story["release"]
    prerelease = story["version_state"] != "release"
    state_text = "预览版" if prerelease else "正式版"
    visual_claims = [claim for claim in story["claims"] if claim.get("shot_ids")]
    invisible_claims = [claim for claim in story["claims"] if not claim.get("shot_ids")]

    paragraphs = [
        "每次有人问我软件长什么样，我都觉得一张文字海报说明不了什么。这次直接让程序自己运行、进入真实界面，再把实际画面放进笔记里。",
        f"先说清楚：这是 ReadMD {release} {state_text}。它是一个免费开源的本地 Markdown 阅读器、编辑器和转换工具，文件保留在你自己的电脑里。",
        f"这一版的重点很直接：{story['angle']}。",
    ]
    for index, claim in enumerate(visual_claims, 1):
        paragraphs.append(f"{index}️⃣ {_clean(claim['user_value'])}。对应画面不是概念图，而是当前版本真实运行后的状态。")

    if invisible_claims:
        fixes = "；".join(_clean(claim["user_value"]) for claim in invisible_claims[:2])
        paragraphs.append(f"还有一些不适合单独拍图的底层修复也在这版里，比如{fixes}。它们不抢画面，但会让日常使用更稳。")

    paragraphs.extend(
        [
            "如果你只是偶尔看笔记，它会是一个双击就能用的阅读器；如果你要写论文、整理代码片段、转换 Word 或 PDF，或者想把文档投到手机上继续看，它的价值会更明显。",
            f"安装包在 GitHub Releases 页面。不想翻链接的话，可以直接 GitHub 搜 {repository}，进仓库后点 Releases 就能找到对应平台。",
            "你最想让下一个版本优先打磨哪一块？是阅读排版、编辑体验、学术写作、格式转换，还是移动端共享？评论区告诉我，我会把这批反馈排进后面的开发计划。",
        ]
    )
    body = "\n\n".join(paragraphs)

    padding = [
        "渲染阶段只处理显示结果，不会替你改写原始 Markdown 文件。",
        "所有演示都来自同一个本地工作流，不需要先把文档上传到别处。",
        "对长文档来说，稳定的目录和搜索比炫技功能更重要。",
        "转换结果会开成新标签页，方便先检查再保存。",
        "界面支持跟随系统语言，中文和英文术语都保持统一。",
    ]
    trimmings = [
        "它不抢画面，但会让日常使用更稳。",
        "或者想把文档投到手机上继续看，它的价值会更明显。",
        "我会把这批反馈排进后面的开发计划。",
        "文件保留在你自己的电脑里。",
    ]
    while len(body) > 900 and trimmings:
        candidate = trimmings.pop(0)
        body = body.replace(candidate + "。", "", 1).replace(candidate, "", 1)
        body = "\n\n".join(part.strip() for part in body.split("\n\n") if part.strip())
    pad_index = 0
    while len(body) < 600 and pad_index < len(padding):
        body += "\n\n" + padding[pad_index]
        pad_index += 1

    return {
        "title": chosen_title["text"],
        "title_formula_id": chosen_title["formula_id"],
        "title_candidates": candidates,
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
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    story = json.loads(args.story.read_text(encoding="utf-8"))
    result = generate_copy(story, repository=args.repository, previous_release=story["previous_release"])
    args.output_dir.mkdir(parents=True, exist_ok=True)
    (args.output_dir / "metadata.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    (args.output_dir / "title.txt").write_text(result["title"], encoding="utf-8")
    (args.output_dir / "body.txt").write_text(result["body"], encoding="utf-8")
    (args.output_dir / "topics.txt").write_text("\n".join(result["topics"]), encoding="utf-8")
    print(args.output_dir / "metadata.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
