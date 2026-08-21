#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Export the approved XHS fact core as paste-safe WeChat Official Account HTML."""
from __future__ import annotations

import argparse
import html
import json
import re
from pathlib import Path
from typing import Any


BODY_STYLE = (
    "max-width:740px;margin:0 auto;padding:28px 24px;"
    "background-color:#ffffff;font-family:-apple-system,BlinkMacSystemFont,"
    "'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif;"
)
PARAGRAPH_STYLE = (
    "margin:0 0 18px;font-size:16px;line-height:1.82;color:#262626;"
    "font-family:-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif"
)
HEADING_STYLE = (
    "margin:0 0 22px;font-size:27px;line-height:1.32;font-weight:700;"
    "color:#111111;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif"
)
STRONG_STYLE = "font-weight:700;color:#111111"
CODE_STYLE = (
    "font-family:Consolas,'SFMono-Regular',Menlo,monospace;font-size:15px;"
    "background-color:#f3f4f6;color:#182029;padding:2px 5px;border-radius:3px"
)
TOPIC_STYLE = (
    "margin:26px 0 0;font-size:14px;line-height:1.7;color:#5b6875;"
    "font-family:-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif"
)
FOOTER_STYLE = (
    "margin-top:10px;padding-top:16px;border-top:1px solid #d8dee5;"
    "font-size:13px;line-height:1.7;color:#5b6875;"
    "font-family:-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif"
)


def _inline(value: str) -> str:
    escaped = html.escape(value, quote=False)
    escaped = re.sub(r"\*\*(.+?)\*\*", f'<strong style="{STRONG_STYLE}">\\1</strong>', escaped)
    escaped = re.sub(r"`([^`]+)`", f'<code style="{CODE_STYLE}">\\1</code>', escaped)
    return escaped


def render_html(metadata: dict[str, Any], story: dict[str, Any]) -> str:
    title = str(metadata.get("title", "")).strip()
    body = str(metadata.get("body", "")).strip()
    topics = metadata.get("topics", [])
    blocks = [block.strip() for block in re.split(r"\n\s*\n", body) if block.strip()]
    rendered_blocks = []
    for block in blocks:
        block = re.sub(r"。$", "", block)
        rendered_blocks.append(f'  <p style="{PARAGRAPH_STYLE}">{_inline(block)}</p>')

    topic_text = " ".join(f"#{str(topic).strip()}" for topic in topics if str(topic).strip())
    angle = str(story.get("angle", "")).strip()
    subtitle = f'  <p style="{PARAGRAPH_STYLE};font-weight:700;color:#111111">{_inline(angle)}</p>\n' if angle else ""
    topic_block = f'  <p style="{TOPIC_STYLE}">{html.escape(topic_text, quote=False)}</p>\n' if topic_text else ""
    footer = '  <p style="' + FOOTER_STYLE + '">GitHub 搜索 Natsummerance/readMD</p>\n'

    return (
        '<!doctype html>\n<html lang="zh-CN">\n<head>\n'
        '<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        f"<title>{html.escape(title, quote=True)}</title>\n</head>\n"
        f'<body style="{BODY_STYLE}">\n'
        f'  <h1 style="{HEADING_STYLE}">{_inline(title)}</h1>\n'
        f"{subtitle}"
        + "\n".join(rendered_blocks)
        + ("\n" if rendered_blocks else "")
        + topic_block
        + footer
        + "</body>\n</html>\n"
    )


def validate_wechat_html(path: Path) -> list[str]:
    try:
        content = path.read_text(encoding="utf-8")
    except Exception as exc:
        return [f"wechat html unreadable: {exc}"]

    errors: list[str] = []
    lowered = content.lower()
    for forbidden in ("<style", "<script", "<link", "<img", "<table", "class=", "id=", ":hover", ":before", ":after", "@import", "http://", "https://"):
        if forbidden in lowered:
            errors.append(f"forbidden wechat construct: {forbidden}")

    for match in re.finditer(r"<(h[1-6]|p|blockquote|ul|ol|li|hr)\b([^>]*)>", content, re.I):
        tag, attrs = match.group(1).lower(), match.group(2)
        if "style=" not in attrs.lower():
            errors.append(f"{tag} missing inline style")
        if tag == "p":
            style = re.search(r'style="([^"]+)"', attrs, re.I)
            style_value = style.group(1).lower() if style else ""
            if not all(key in style_value for key in ("font-size", "line-height", "color")):
                errors.append("paragraph inline style incomplete")

    text = re.sub(r"<[^>]+>", "", content).strip()
    if len(text) < 100:
        errors.append("wechat article is too short")
    return errors


def export_package(package_dir: Path, *, story: dict[str, Any] | None = None) -> dict[str, Any]:
    metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
    story = story or json.loads((package_dir / "story.json").read_text(encoding="utf-8"))
    output_dir = package_dir / "wechat"
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / "readmd-wechat.html"
    output_path.write_text(render_html(metadata, story), encoding="utf-8")
    errors = validate_wechat_html(output_path)
    report = {"schema_version": 1, "ok": not errors, "errors": errors, "output": str(output_path.resolve())}
    (output_dir / "wechat-qa.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    report = export_package(args.package)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
