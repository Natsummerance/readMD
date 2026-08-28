# -*- coding: utf-8 -*-
"""Keep generated public platform claims aligned with release/platform-matrix."""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PUBLIC = ROOT / "website" / "public"


def update(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    original = text
    # Download cards are generated as compact HTML blocks.  Remove the source
    # project card entirely; HarmonyOS is explicitly out of scope for 2.3.7.
    text = re.sub(r"<a[^>]*harmonyos-app[^>]*>.*?</a>", "", text, flags=re.I | re.S)
    text = re.sub(r"<tr[^>]*(?:HarmonyOS|鸿蒙)[^>]*>.*?</tr>", "", text, flags=re.I | re.S)
    text = text.replace(", HarmonyOS", "").replace(", UOS, HarmonyOS", ", UOS")
    text = text.replace(", HarmonyOS NEXT", "")
    text = text.replace("、鸿蒙、", "、")
    text = text.replace("、HarmonyOS", "")
    text = text.replace("HarmonyOS DevEco project, ", "")
    text = text.replace("HarmonyOS ArkTS project", "Experimental source preview (not supported in v2.3.7)")
    text = text.replace("鸿蒙 ArkTS 源码工程", "实验性源码预览（v2.3.7 不支持）")
    text = text.replace("华为鸿蒙 DevEco 源码工程与", "实验性源码预览与")
    text = text.replace("HarmonyOS NEXT / OpenHarmony through a separate ArkTS source project.", "HarmonyOS/OpenHarmony is out of scope for v2.3.7; the source preview is experimental.")
    text = text.replace("HarmonyOS NEXT / OpenHarmony は独立した ArkTS プロジェクト。", "HarmonyOS/OpenHarmony は v2.3.7 の対象外です。ソースは実験的プレビューです。")
    text = text.replace("鸿蒙 HarmonyOS NEXT / OpenHarmony 通过独立 ArkTS 工程支持。", "HarmonyOS/OpenHarmony は v2.3.7 の対象外です。")
    text = text.replace("HarmonyOS NEXT remains a DevEco source project.", "HarmonyOS/OpenHarmony is out of scope for v2.3.7; the source preview is experimental.")
    text = text.replace("HarmonyOS NEXT は DevEco ソースプロジェクトです。", "HarmonyOS/OpenHarmony は v2.3.7 の対象外です。ソースは実験的プレビューです。")
    text = text.replace("HarmonyOS NEXT 仍是 DevEco 源码项目。", "HarmonyOS/OpenHarmony 不属于 v2.3.7，源码仅作实验性预览。")
    text = text.replace("HarmonyOS NEXT 仍是 DevEco 原始碼專案。", "HarmonyOS/OpenHarmony 不屬於 v2.3.7，原始碼僅作實驗性預覽。")
    text = text.replace("HarmonyOS NEXT / OpenHarmony 透過獨立 ArkTS 專案支援。", "HarmonyOS/OpenHarmony 不屬於 v2.3.7，原始碼僅作實驗性預覽。")
    text = text.replace("HarmonyOS/OpenHarmony は v2.3.7 の対象外です。", "HarmonyOS/OpenHarmony 不属于 v2.3.7，源码仅作实验性预览。")
    if text != original:
        path.write_text(text, encoding="utf-8", newline="\n")


def main() -> int:
    for path in PUBLIC.rglob("*"):
        if path.is_file() and path.suffix.lower() in {".html", ".xml", ".txt"}:
            update(path)
    print("platform claims updated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
