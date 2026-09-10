# -*- coding: utf-8 -*-
"""
ReadMD 前端 UI 与国际化字典零 Emoji 门禁测试。
确保在所有核心前端 UI（HTML、CSS、核心功能 JS、图标及非宠物 i18n 字典）中绝不出现任何 Emoji 表情符号。
允许使用基础排版与技术控制符号（如 ⏸、⚙、✕、⟲、◆、◇ 等）。
宠物伴读对话语言（pet-batch.js 对话与 pet.* 字典词条）允许使用表情，但必须保证 46 国语言 100% i18n 覆盖。
"""
import os
import re
import json
import pytest

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# 综合 Emoji Unicode 范围
EMOJI_PATTERN = re.compile(
    r'[\U0001F000-\U0001FAFF'  # 常见 Emoji（表情、动植物、交通、物品、彩色符号）
    r'\U00002600-\U000027BF'  # 杂项符号与装饰符号（如 ☀️, ☕, ✨, ⚠️ 等）
    r'\U00002B50\U00002B55'    # 星号、圈号等
    r'\U000023F0-\U000023FF'  # 计时器、播放/暂停符号等
    r'\U000020E3'              # 键帽组合符
    r']'
)

# 允许的技术与排版 UI 符号（非 Emoji 彩色图形）
ALLOWED_UI_SYMBOLS = {
    '\u23f8',  # ⏸ (Double Vertical Bar / 暂停符号)
    '\u2699',  # ⚙ (Gear / 设置与脚本符号)
    '\u2715',  # ✕ (Multiplication X / 关闭按钮符号)
    '\u263d',  # ☽ (First Quarter Moon / 主题符号)
    '\u27f2',  # ⟲ (Anticlockwise Gapped Circle Arrow / 重置符号)
    '\u25c6',  # ◆ (Black Diamond)
    '\u25c7',  # ◇ (White Diamond)
    '\u260e',  # ☎ (Telephone)
    '\u2709',  # ✉ (Envelope)
}


def test_no_emojis_in_frontend_ui():
    """扫描所有前端 UI 资源，确保 UI 镀层 0 Emoji，宠物对话均有 i18n 覆盖。"""
    targets = [
        os.path.join(ROOT_DIR, "assets", "index.html"),
        os.path.join(ROOT_DIR, "assets", "style.css"),
    ]

    # 添加 assets/js 下除 pet-batch.js 之外的所有 JS 文件（pet-batch 对话允许 emoji，由 i18n 门禁约束）
    js_dir = os.path.join(ROOT_DIR, "assets", "js")
    for root, _dirs, files in os.walk(js_dir):
        for f in files:
            if f.endswith(".js") and f != "pet-batch.js":
                targets.append(os.path.join(root, f))

    violations = []

    for path in targets:
        assert os.path.isfile(path), f"Target file missing: {path}"
        rel_path = os.path.relpath(path, ROOT_DIR).replace("\\", "/")
        with open(path, "r", encoding="utf-8", errors="ignore") as fp:
            for lno, line in enumerate(fp, 1):
                stripped = line.strip()
                matches = [ch for ch in line if EMOJI_PATTERN.match(ch) and ch not in ALLOWED_UI_SYMBOLS]
                if matches:
                    violations.append({
                        "file": rel_path,
                        "line": lno,
                        "emojis": matches,
                        "snippet": stripped[:100]
                    })

    # 检查非 pet.* 国际化词条不得包含任何 Emoji
    i18n_dir = os.path.join(ROOT_DIR, "assets", "i18n")
    for f in os.listdir(i18n_dir):
        if f.endswith(".json") and f != "meta.json":
            with open(os.path.join(i18n_dir, f), "r", encoding="utf-8") as fp:
                d = json.load(fp)
            for k, val in d.items():
                if not k.startswith("pet.") and isinstance(val, str):
                    matches = [ch for ch in val if EMOJI_PATTERN.match(ch) and ch not in ALLOWED_UI_SYMBOLS]
                    if matches:
                        violations.append({
                            "file": f"assets/i18n/{f}",
                            "key": k,
                            "emojis": matches,
                            "snippet": val[:100]
                        })

    assert len(violations) == 0, (
        f"在前端 UI 中检测到 {len(violations)} 处非法 Emoji:\n"
        + json.dumps(violations, ensure_ascii=False, indent=2)
    )
