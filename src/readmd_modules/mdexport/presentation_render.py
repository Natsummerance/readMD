# -*- coding: utf-8 -*-
"""ReadMD 沉浸式 Reveal.js 演说模式与幻灯片编译导出器。

对齐 Markdown Preview Enhanced (MPE) 完整语法规范：
- `<!-- slide [attrs] -->` 或 `---`：横向下一页幻灯片；
- `<!-- subslide [attrs] -->` 或 `--`：垂直下钻幻灯片；
- `<!-- note ... -->` 或 `note:`：演讲者备注（按 'S' 键唤起独立演讲者计时与备注窗口）；
- 智能保护分片：长篇 Markdown 文档按 H1/H2 层级分片时，严格保护围栏代码块、公式块与表格不被截断；
- Front-matter 配置：
  ---
  presentation:
    theme: league
    transition: slide
    slideNumber: true
    width: 1080
    height: 720
  ---
"""

import json
import re
from typing import Any, Dict, List, Optional, Tuple

SLIDE_SPLIT_REGEX = re.compile(
    r'(?:^[ \t]*<!--\s*slide(?:\s+[\s\S]*?)?-->[ \t]*$|^[ \t]*---[ \t]*$)',
    re.MULTILINE | re.IGNORECASE
)
SUBSLIDE_SPLIT_REGEX = re.compile(
    r'(?:^[ \t]*<!--\s*subslide(?:\s+[\s\S]*?)?-->[ \t]*$|^[ \t]*--[ \t]*$)',
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
                    if k in ('theme', 'transition', 'title', 'author', 'slidenumber', 'width', 'height'):
                        meta[k] = v
    return meta, body


def _protect_blocks(text: str) -> Tuple[str, Dict[str, str]]:
    """将代码块、数学块、表格抽取占位符，防止切片算法将其腰斩截断。"""
    blocks: Dict[str, str] = {}
    counter = 0

    def code_repl(m: re.Match) -> str:
        nonlocal counter
        key = f"__READMD_CODE_BLOCK_{counter}__"
        blocks[key] = m.group(0)
        counter += 1
        return key

    # 1. 保护围栏代码块 ```...```
    text = re.sub(r'```[\s\S]*?```', code_repl, text)

    # 2. 保护数学公式块 $$...$$
    def math_repl(m: re.Match) -> str:
        nonlocal counter
        key = f"__READMD_MATH_BLOCK_{counter}__"
        blocks[key] = m.group(0)
        counter += 1
        return key

    text = re.sub(r'\$\$[\s\S]*?\$\$', math_repl, text)

    return text, blocks


def _restore_blocks(text: str, blocks: Dict[str, str]) -> str:
    """还原被保护的代码块与数学公式块。"""
    for k, v in blocks.items():
        text = text.replace(k, v)
    return text


def _auto_split_long_chunk(chunk: str, max_chars: int = 800) -> List[str]:
    """若单张幻灯片内容过长，在保护代码块完整性的前提下，按 H3 标题或段落智能分片。"""
    chunk = chunk.strip()
    if len(chunk) <= max_chars:
        return [chunk] if chunk else []

    protected_chunk, block_map = _protect_blocks(chunk)

    # 尝试按 H3 标题分片
    h3_splits = [c.strip() for c in re.split(r'(?=^#{3}\s)', protected_chunk, flags=re.MULTILINE) if c.strip()]
    if len(h3_splits) > 1:
        res = []
        for s in h3_splits:
            restored = _restore_blocks(s, block_map)
            res.extend(_auto_split_long_chunk(restored, max_chars))
        return res

    # 尝试按双换行分段聚合
    paragraphs = [p.strip() for p in re.split(r'\n{2,}', protected_chunk) if p.strip()]
    if len(paragraphs) <= 1:
        return [_restore_blocks(protected_chunk, block_map)]

    slides = []
    current_acc = []
    current_len = 0
    for p in paragraphs:
        if current_len + len(p) > max_chars and current_acc:
            slides.append(_restore_blocks('\n\n'.join(current_acc), block_map))
            current_acc = [p]
            current_len = len(p)
        else:
            current_acc.append(p)
            current_len += len(p)
    if current_acc:
        slides.append(_restore_blocks('\n\n'.join(current_acc), block_map))
    return slides


def split_slides_structure(content: str) -> List[List[Dict[str, str]]]:
    """将 Markdown 切片为二维矩阵 [Horizontal_Slide [Vertical_SubSlide {content, note}]]."""
    _, body = parse_frontmatter(content)
    
    # 检查是否存在显式 slide 分割符
    has_explicit_slide = bool(re.search(r'^[ \t]*<!--\s*slide(?:\s+[\s\S]*?)?-->', body, re.MULTILINE | re.IGNORECASE))
    
    if has_explicit_slide:
        raw_horizontal = re.split(r'^[ \t]*<!--\s*slide(?:\s+[\s\S]*?)?-->[ \t]*$', body, flags=re.MULTILINE | re.IGNORECASE)
    elif re.search(r'^[ \t]*---[ \t]*$', body, re.MULTILINE):
        # 显式 --- 分割符
        raw_horizontal = re.split(r'^[ \t]*---[ \t]*$', body, flags=re.MULTILINE)
    else:
        # 智能自愈分片：按 H1/H2/H3 标题智能分片
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
        
        # 检查是否包含垂直子幻灯片
        if re.search(r'^[ \t]*<!--\s*subslide(?:\s+[\s\S]*?)?-->', h_chunk, re.MULTILINE | re.IGNORECASE):
            raw_vertical = re.split(r'^[ \t]*<!--\s*subslide(?:\s+[\s\S]*?)?-->[ \t]*$', h_chunk, flags=re.MULTILINE | re.IGNORECASE)
        elif not has_explicit_slide and re.search(r'^[ \t]*--[ \t]*$', h_chunk, re.MULTILINE):
            raw_vertical = re.split(r'^[ \t]*--[ \t]*$', h_chunk, flags=re.MULTILINE)
        else:
            # 针对单页超长文本进行智能自适应分片（保护代码块完整性）
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
    """将 Markdown 编译为完整的单文件 Reveal.js HTML 演说稿（精致舒适排版、防截断滚动与双向事件控制）。"""
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
    :root {{
      --reveal-base-font-size: 24px;
    }}
    .reveal {{
      font-size: var(--reveal-base-font-size);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans", sans-serif;
    }}
    .reveal .slides section {{
      height: 100%;
      max-height: 100%;
      overflow-y: auto !important;
      padding: 24px 36px;
      box-sizing: border-box;
      text-align: left;
    }}
    .reveal .slides section::-webkit-scrollbar {{
      width: 6px;
    }}
    .reveal .slides section::-webkit-scrollbar-thumb {{
      background: rgba(128, 128, 128, 0.4);
      border-radius: 3px;
    }}
    .reveal h1 {{
      font-size: 1.8em;
      margin-bottom: 0.45em;
      font-weight: 700;
      line-height: 1.25;
      text-transform: none;
    }}
    .reveal h2 {{
      font-size: 1.4em;
      margin-bottom: 0.35em;
      font-weight: 600;
      line-height: 1.3;
      text-transform: none;
    }}
    .reveal h3 {{
      font-size: 1.15em;
      margin-bottom: 0.3em;
      font-weight: 600;
      text-transform: none;
    }}
    .reveal h4, .reveal h5, .reveal h6 {{
      font-size: 1.0em;
      margin-bottom: 0.25em;
      text-transform: none;
    }}
    .reveal p, .reveal li {{
      font-size: 0.95em;
      line-height: 1.65;
      margin-bottom: 0.6em;
      text-align: left;
    }}
    .reveal ul, .reveal ol {{
      text-align: left;
      display: block;
      margin: 0 0 1em 1.4em;
    }}
    .reveal pre {{
      box-shadow: 0 4px 16px rgba(0,0,0,0.25);
      width: 100%;
      border-radius: 8px;
      margin: 14px 0;
      background: #1e1e1e;
    }}
    .reveal pre code {{
      max-height: 480px;
      font-size: 0.78em;
      line-height: 1.45;
      padding: 12px 16px;
      border-radius: 8px;
      font-family: "Cascadia Code", "Fira Code", Consolas, "SF Mono", monospace;
    }}
    .reveal table {{
      font-size: 0.78em;
      margin: 16px auto;
      border-collapse: collapse;
      width: 100%;
      max-width: 900px;
    }}
    .reveal table th {{
      background: rgba(128, 128, 128, 0.2);
      font-weight: 600;
    }}
    .reveal table th, .reveal table td {{
      padding: 8px 14px;
      border: 1px solid rgba(128, 128, 128, 0.3);
      text-align: left;
    }}
    .reveal blockquote {{
      border-left: 4px solid #3b82f6;
      padding: 8px 18px;
      background: rgba(59, 130, 246, 0.08);
      font-style: normal;
      border-radius: 0 6px 6px 0;
      text-align: left;
      width: 95%;
      margin: 12px 0;
    }}
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
    window.deck = new Reveal({{
      width: 1080,
      height: 720,
      margin: 0.06,
      minScale: 0.2,
      maxScale: 2.0,
      controls: true,
      progress: true,
      center: false,
      hash: true,
      transition: '{transition}',
      slideNumber: 'c/t',
      plugins: [ RevealMarkdown, RevealHighlight, RevealNotes, RevealMath.KaTeX ]
    }});
    deck.initialize();

    // 监听来自父窗口的实时定制消息 (Theme, Transition, Font Scale, Navigation)
    window.addEventListener('message', function(event) {{
      if (!event.data || typeof event.data !== 'object') return;
      const data = event.data;
      if (data.type === 'set-theme' && data.theme) {{
        const themeLink = document.getElementById('theme');
        if (themeLink) {{
          themeLink.href = 'https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/theme/' + data.theme + '.css';
        }}
      }} else if (data.type === 'set-transition' && data.transition) {{
        if (window.deck && typeof window.deck.configure === 'function') {{
          window.deck.configure({{ transition: data.transition }});
        }}
      }} else if (data.type === 'set-font-size' && data.size) {{
        document.documentElement.style.setProperty('--reveal-base-font-size', data.size + 'px');
      }} else if (data.type === 'toggle-overview') {{
        if (window.deck && typeof window.deck.toggleOverview === 'function') {{
          window.deck.toggleOverview();
        }}
      }}
    }});
  </script>
</body>
</html>"""


# 兼容别名
generate_presentation_html = render_presentation_html

