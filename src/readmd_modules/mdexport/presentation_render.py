# -*- coding: utf-8 -*-
"""ReadMD 沉浸式 Reveal.js 演说模式与幻灯片编译导出器。

支持语法：
- `<!-- slide -->`：横向下一页幻灯片；
- `<!-- subslide -->`：垂直下钻幻灯片；
- `<!-- note -->`：演讲者备注（按 'S' 键唤起独立演讲者计时窗口）。
- Front-matter 配置：
  ---
  presentation:
    theme: league.css
    transition: slide
    slideNumber: true
  ---
"""

import json
import re
from typing import Any, Dict, List, Optional, Tuple

SLIDE_SPLIT_REGEX = re.compile(r'^[ \t]*<!--\s*slide\s*-->[ \t]*$', re.MULTILINE | re.IGNORECASE)
SUBSLIDE_SPLIT_REGEX = re.compile(r'^[ \t]*<!--\s*subslide\s*-->[ \t]*$', re.MULTILINE | re.IGNORECASE)
NOTE_SPLIT_REGEX = re.compile(r'^[ \t]*<!--\s*note\s*-->\s*([\s\S]*?)(?=(?:^[ \t]*<!--\s*(?:slide|subslide|note)\s*-->)|\Z)', re.MULTILINE | re.IGNORECASE)


def parse_frontmatter(content: str) -> Tuple[Dict[str, Any], str]:
    """提取文档头部的 YAML Front-matter 配置。"""
    meta: Dict[str, Any] = {}
    body = content
    if content.startswith('---'):
        parts = content.split('---', 2)
        if len(parts) >= 3:
            raw_yaml = parts[1]
            body = parts[2].lstrip('\n')
            # 简单解析 theme, transition, title
            for line in raw_yaml.splitlines():
                if ':' in line:
                    k, v = line.split(':', 1)
                    k = k.strip().lower()
                    v = v.strip().strip('"\'')
                    if k in ('theme', 'transition', 'title', 'author'):
                        meta[k] = v
    return meta, body


def split_slides_structure(content: str) -> List[List[Dict[str, str]]]:
    """将 Markdown 切片为二维矩阵 [Horizontal_Slide [Vertical_SubSlide {content, note}]]."""
    _, body = parse_frontmatter(content)
    
    # 智能自愈分片：若文档未显式包含 <!-- slide --> 标记，则智能按 H1/H2 标题分片
    if not SLIDE_SPLIT_REGEX.search(body):
        heading_splits = [c.strip() for c in re.split(r'(?=^#{1,2}\s)', body, flags=re.MULTILINE) if c.strip()]
        if len(heading_splits) > 1:
            raw_horizontal = heading_splits
        else:
            raw_horizontal = [body]
    else:
        raw_horizontal = SLIDE_SPLIT_REGEX.split(body)
    
    slides_matrix: List[List[Dict[str, str]]] = []
    
    for h_chunk in raw_horizontal:
        h_chunk = h_chunk.strip()
        if not h_chunk:
            continue
        
        raw_vertical = SUBSLIDE_SPLIT_REGEX.split(h_chunk)
        vertical_slides: List[Dict[str, str]] = []
        
        for v_chunk in raw_vertical:
            v_chunk = v_chunk.strip()
            if not v_chunk:
                continue
            
            # 提取演讲者备注
            notes = []
            def extract_note(match: re.Match) -> str:
                notes.append(match.group(1).strip())
                return ""
            
            clean_slide_content = NOTE_SPLIT_REGEX.sub(extract_note, v_chunk).strip()
            note_text = "\n\n".join(notes)
            
            vertical_slides.append({
                "content": clean_slide_content,
                "note": note_text
            })
            
        if vertical_slides:
            slides_matrix.append(vertical_slides)
            
    return slides_matrix


def render_presentation_html(content: str, title: str = "ReadMD Presentation",
                             theme: str = "black", transition: str = "slide") -> str:
    """将 Markdown 编译为完整的单文件 Reveal.js HTML 演说稿。"""
    meta, _ = parse_frontmatter(content)
    theme = meta.get('theme', theme)
    transition = meta.get('transition', transition)
    title = meta.get('title', title)
    
    slides_matrix = split_slides_structure(content)
    
    sections_html = []
    
    for h_idx, v_slides in enumerate(slides_matrix):
        if len(v_slides) == 1:
            slide = v_slides[0]
            note_tag = f'<aside class="notes">{slide["note"]}</aside>' if slide["note"] else ""
            escaped_md = slide["content"].replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            sections_html.append(f'<section data-markdown><textarea data-template>{escaped_md}\n{note_tag}</textarea></section>')
        else:
            sub_sections = []
            for v_idx, slide in enumerate(v_slides):
                note_tag = f'<aside class="notes">{slide["note"]}</aside>' if slide["note"] else ""
                escaped_md = slide["content"].replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
                sub_sections.append(f'<section data-markdown><textarea data-template>{escaped_md}\n{note_tag}</textarea></section>')
            sections_html.append(f'<section>\n' + "\n".join(sub_sections) + '\n</section>')

    slides_body = "\n".join(sections_html)

    return f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
  <title>{title}</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/reveal.css">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/theme/{theme}.css" id="theme">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/plugin/highlight/monokai.css">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.css">
  <style>
    .reveal pre code {{ max-height: 500px; font-size: 0.85em; }}
    .reveal table {{ font-size: 0.75em; margin: 0 auto; }}
    .reveal h1, .reveal h2, .reveal h3 {{ text-transform: none; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }}
  </style>
</head>
<body>
  <div class="reveal">
    <div class="slides">
{slides_body}
    </div>
  </div>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/reveal.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/plugin/markdown/markdown.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/plugin/highlight/highlight.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/plugin/notes/notes.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/plugin/math/math.js"></script>
  <script>
    Reveal.initialize({{
      controls: true,
      progress: true,
      center: true,
      hash: true,
      transition: '{transition}',
      slideNumber: 'c/t',
      plugins: [ RevealMarkdown, RevealHighlight, RevealNotes, RevealMath.KaTeX ]
    }});
  </script>
</body>
</html>"""

# 兼容别名
generate_presentation_html = render_presentation_html
