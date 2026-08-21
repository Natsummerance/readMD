# -*- coding: utf-8 -*-
"""ReadMD 沉浸式 Reveal.js 演说模式与幻灯片编译导出器。

支持语法：
- `<!-- slide -->` 或 `---`：横向下一页幻灯片；
- `<!-- subslide -->` 或 `--`：垂直下钻幻灯片；
- `<!-- note ... -->` 或 `note:`：演讲者备注（按 'S' 键唤起独立演讲者计时与备注窗口）；
- 智能自愈：对于长篇 Markdown 文档，自动按 H1/H2/H3 层级和篇幅长度智能分片，杜绝页面截断。
- Front-matter 配置：
  ---
  presentation:
    theme: league
    transition: slide
    slideNumber: true
  ---
"""

import json
import re
from typing import Any, Dict, List, Optional, Tuple

SLIDE_SPLIT_REGEX = re.compile(
    r'(?:^[ \t]*<!--\s*slide\s*-->[ \t]*$|^[ \t]*---[ \t]*$)',
    re.MULTILINE | re.IGNORECASE
)
SUBSLIDE_SPLIT_REGEX = re.compile(
    r'(?:^[ \t]*<!--\s*subslide\s*-->[ \t]*$|^[ \t]*--[ \t]*$)',
    re.MULTILINE | re.IGNORECASE
)
NOTE_SPLIT_REGEX = re.compile(
    r'^[ \t]*<!--\s*note\s*-->\s*([\s\S]*?)(?=(?:^[ \t]*<!--\s*(?:slide|subslide|note)\s*-->)|\Z)',
    re.MULTILINE | re.IGNORECASE
)


def parse_frontmatter(content: str) -> Tuple[Dict[str, Any], str]:
    """提取文档头部的 YAML Front-matter 配置。"""
    meta: Dict[str, Any] = {}
    body = content
    if content.startswith('---'):
        parts = content.split('---', 2)
        if len(parts) >= 3:
            raw_yaml = parts[1]
            body = parts[2].lstrip('\n')
            for line in raw_yaml.splitlines():
                if ':' in line:
                    k, v = line.split(':', 1)
                    k = k.strip().lower()
                    v = v.strip().strip('"\'')
                    if k in ('theme', 'transition', 'title', 'author', 'slidenumber'):
                        meta[k] = v
    return meta, body


def _auto_split_long_chunk(chunk: str, max_chars: int = 900) -> List[str]:
    """若单张幻灯片内容过长，自动按 H3 标题或段落智能分片，避免单页超屏。"""
    chunk = chunk.strip()
    if len(chunk) <= max_chars:
        return [chunk] if chunk else []
    
    # 尝试按 H3 标题分片
    h3_splits = [c.strip() for c in re.split(r'(?=^#{3}\s)', chunk, flags=re.MULTILINE) if c.strip()]
    if len(h3_splits) > 1:
        res = []
        for s in h3_splits:
            res.extend(_auto_split_long_chunk(s, max_chars))
        return res
    
    # 尝试按双换行分段聚合
    paragraphs = [p.strip() for p in re.split(r'\n{2,}', chunk) if p.strip()]
    if len(paragraphs) <= 1:
        return [chunk]
    
    slides = []
    current_acc = []
    current_len = 0
    for p in paragraphs:
        if current_len + len(p) > max_chars and current_acc:
            slides.append('\n\n'.join(current_acc))
            current_acc = [p]
            current_len = len(p)
        else:
            current_acc.append(p)
            current_len += len(p)
    if current_acc:
        slides.append('\n\n'.join(current_acc))
    return slides


def split_slides_structure(content: str) -> List[List[Dict[str, str]]]:
    """将 Markdown 切片为二维矩阵 [Horizontal_Slide [Vertical_SubSlide {content, note}]]."""
    _, body = parse_frontmatter(content)
    
    # 判断是否存在显式 slide 分割符
    has_explicit_slide = bool(re.search(r'^[ \t]*<!--\s*slide\s*-->', body, re.MULTILINE | re.IGNORECASE))
    
    if has_explicit_slide:
        raw_horizontal = re.split(r'^[ \t]*<!--\s*slide\s*-->[ \t]*$', body, flags=re.MULTILINE | re.IGNORECASE)
    elif re.search(r'^[ \t]*---[ \t]*$', body, re.MULTILINE):
        # 显式 --- 分割符
        raw_horizontal = re.split(r'^[ \t]*---[ \t]*$', body, flags=re.MULTILINE)
    else:
        # 智能自愈分片：按 H1/H2/H3 分片
        heading_splits = [c.strip() for c in re.split(r'(?=^#{1,3}\s)', body, flags=re.MULTILINE) if c.strip()]
        if len(heading_splits) > 1:
            raw_horizontal = heading_splits
        else:
            raw_horizontal = [body]
    
    slides_matrix: List[List[Dict[str, str]]] = []
    
    for h_chunk in raw_horizontal:
        h_chunk = h_chunk.strip()
        if not h_chunk:
            continue
        
        # 检查是否包含子幻灯片
        if re.search(r'^[ \t]*<!--\s*subslide\s*-->', h_chunk, re.MULTILINE | re.IGNORECASE):
            raw_vertical = re.split(r'^[ \t]*<!--\s*subslide\s*-->[ \t]*$', h_chunk, flags=re.MULTILINE | re.IGNORECASE)
        elif not has_explicit_slide and re.search(r'^[ \t]*--[ \t]*$', h_chunk, re.MULTILINE):
            raw_vertical = re.split(r'^[ \t]*--[ \t]*$', h_chunk, flags=re.MULTILINE)
        else:
            # 针对单页超长文本进行智能自适应分片
            raw_vertical = _auto_split_long_chunk(h_chunk)
            if not raw_vertical:
                raw_vertical = [h_chunk]
        
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


def _escape_template_md(md: str) -> str:
    """仅转义可能破坏 <textarea> 的闭合标签，严格保留所有 Markdown / HTML / Math 语法。"""
    return md.replace('</textarea>', '&lt;/textarea&gt;')


def render_presentation_html(content: str, title: str = "ReadMD Presentation",
                             theme: str = "black", transition: str = "slide") -> str:
    """将 Markdown 编译为完整的单文件 Reveal.js HTML 演说稿。"""
    meta, _ = parse_frontmatter(content)
    theme = meta.get('theme', theme)
    if theme.endswith('.css'):
        theme = theme[:-4]
    transition = meta.get('transition', transition)
    title = meta.get('title', title)
    
    slides_matrix = split_slides_structure(content)
    
    sections_html = []
    
    for h_idx, v_slides in enumerate(slides_matrix):
        if len(v_slides) == 1:
            slide = v_slides[0]
            note_tag = f'\n<aside class="notes">{slide["note"]}</aside>' if slide["note"] else ""
            escaped_md = _escape_template_md(slide["content"])
            sections_html.append(f'<section data-markdown><textarea data-template>\n{escaped_md}{note_tag}\n</textarea></section>')
        else:
            sub_sections = []
            for v_idx, slide in enumerate(v_slides):
                note_tag = f'\n<aside class="notes">{slide["note"]}</aside>' if slide["note"] else ""
                escaped_md = _escape_template_md(slide["content"])
                sub_sections.append(f'<section data-markdown><textarea data-template>\n{escaped_md}{note_tag}\n</textarea></section>')
            sections_html.append('<section>\n' + "\n".join(sub_sections) + '\n</section>')

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
    .reveal pre {{ box-shadow: 0 5px 15px rgba(0,0,0,0.15); width: 100%; }}
    .reveal pre code {{ max-height: 520px; font-size: 0.85em; font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace; }}
    .reveal table {{ font-size: 0.75em; margin: 12px auto; border-collapse: collapse; }}
    .reveal table th, .reveal table td {{ padding: 6px 12px; border: 1px solid rgba(128,128,128,0.3); }}
    .reveal h1, .reveal h2, .reveal h3, .reveal h4 {{ text-transform: none; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }}
    .reveal blockquote {{ border-left: 4px solid #4a9eff; padding: 6px 16px; background: rgba(128,128,128,0.08); font-style: normal; }}
    .reveal .slides section {{ height: 100%; }}
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
      minScale: 0.2,
      maxScale: 2.0,
      plugins: [ RevealMarkdown, RevealHighlight, RevealNotes, RevealMath.KaTeX ]
    }});
  </script>
</body>
</html>"""

# 兼容别名
generate_presentation_html = render_presentation_html
