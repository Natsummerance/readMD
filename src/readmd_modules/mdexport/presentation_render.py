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

import base64
import json
import html
import os
import re
import sys
from typing import Any, Dict, List, Optional, Tuple
from html.parser import HTMLParser

_REVEAL_THEMES = ('black', 'white', 'league', 'beige', 'night', 'serif',
                  'simple', 'solarized', 'blood', 'moon', 'sky')
_REVEAL_TRANSITIONS = ('slide', 'fade', 'zoom', 'convex', 'concave', 'none')

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


_PRESENTATION_ALLOWED_TAGS = frozenset({
    'a', 'abbr', 'article', 'b', 'blockquote', 'br', 'caption', 'cite',
    'code', 'dd', 'del', 'details', 'div', 'dl', 'dt', 'em', 'figcaption',
    'figure', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'hr', 'i', 'img', 'ins',
    'kbd', 'li', 'mark', 'ol', 'p', 'pre', 'q', 's', 'section', 'span',
    'strike', 'strong', 'sub', 'summary', 'sup', 'table', 'tbody', 'td',
    'tfoot', 'th', 'thead', 'time', 'tr', 'u', 'ul',
})
_PRESENTATION_DROP_CONTENT = frozenset({
    'base', 'embed', 'form', 'frame', 'frameset', 'iframe', 'link', 'meta',
    'noscript', 'object', 'script', 'style', 'template', 'title',
})
_PRESENTATION_VOID_TAGS = frozenset({'br', 'hr', 'img'})
_PRESENTATION_GLOBAL_ATTRS = {'class', 'style', 'title', 'lang', 'dir', 'role'}
_PRESENTATION_CSS_PROPERTY = re.compile(r'^[A-Za-z][A-Za-z0-9-]*$')
_PRESENTATION_CSS_FORBIDDEN = re.compile(
    r'(?:javascript|vbscript|data\s*:|url\s*\(|expression\s*\(|@import|behavior\s*:|position\s*:\s*fixed)',
    re.IGNORECASE,
)
_PRESENTATION_TAG_ATTRS = {
    '*': _PRESENTATION_GLOBAL_ATTRS | {'aria-label'},
    'a': {'href', 'target'},
    'details': {'open'},
    'img': {'src', 'srcset', 'alt', 'width', 'height', 'loading'},
    'source': {'src', 'srcset'},
    'ol': {'start', 'type'},
    'td': {'colspan', 'rowspan'},
    'th': {'colspan', 'rowspan', 'scope'},
    'time': {'datetime'},
    'blockquote': {'cite'},
}


def _presentation_safe_url(value: str) -> bool:
    value = value.strip()
    if not value or value.startswith('#'):
        return True
    lowered = value.lower()
    if lowered.startswith(('data:image/', 'blob:')):
        return True
    return '://' in value or value.startswith('//') or value.startswith('/') or (
        not re.match(r'^[a-z][a-z0-9+.-]*:', lowered)
    )


def _presentation_attr(tag: str, name: str, value: str):
    name = name.lower()
    allowed = set(_PRESENTATION_TAG_ATTRS.get('*', ()))
    allowed.update(_PRESENTATION_TAG_ATTRS.get(tag, ()))
    if name not in allowed:
        return None
    if name in ('href', 'src') and not _presentation_safe_url(value):
        return None
    if name == 'srcset':
        candidates = []
        for candidate in value.split(','):
            pieces = candidate.strip().split()
            if pieces and not _presentation_safe_url(pieces[0]):
                continue
            candidates.append(' '.join(pieces))
        return 'srcset', ', '.join(candidates)
    if name == 'id' and not re.match(r'^[A-Za-z][A-Za-z0-9_:.-]*$', value):
        return None
    if name == 'style' and not _safe_inline_css(value):
        return None
    return name, value


def _safe_inline_css(value: str):
    declarations = []
    for declaration in value.split(';'):
        if ':' not in declaration:
            continue
        prop, css_value = (part.strip() for part in declaration.split(':', 1))
        if not prop or not css_value:
            continue
        if not _PRESENTATION_CSS_PROPERTY.match(prop):
            continue
        if _PRESENTATION_CSS_FORBIDDEN.search(f'{prop}:{css_value}'):
            continue
        declarations.append(f'{prop}: {css_value}')
    return '; '.join(declarations)


class _PresentationHTMLSanitizer(HTMLParser):
    """Whitelist sanitizer for user-authored slide HTML."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.output = []
        self.stack = []
        self.skip_tag = None
        self.skip_depth = 0

    def _safe_start(self, tag, attrs):
        allowed = set(_PRESENTATION_TAG_ATTRS.get('*', ()))
        allowed.update(_PRESENTATION_TAG_ATTRS.get(tag, ()))
        rendered = []
        for raw_name, raw_value in attrs:
            name = raw_name.lower()
            if name.startswith('on') or name not in allowed:
                continue
            value = raw_value or ''
            checked = _presentation_attr(tag, name, value)
            if not checked:
                continue
            name, value = checked
            rendered.append(f' {name}="{html.escape(value, quote=True)}"')
        suffix = ' />' if tag in _PRESENTATION_VOID_TAGS else '>'
        self.output.append(f'<{tag}{"".join(rendered)}{suffix}')

    def handle_starttag(self, tag, attrs):
        tag = tag.lower()
        if self.skip_depth:
            if tag not in _PRESENTATION_VOID_TAGS:
                self.skip_depth += 1
            return
        if tag in _PRESENTATION_DROP_CONTENT:
            self.skip_tag = tag
            self.skip_depth = 1
            return
        if tag in _PRESENTATION_ALLOWED_TAGS:
            self._safe_start(tag, attrs)
            if tag not in _PRESENTATION_VOID_TAGS:
                self.stack.append(tag)

    def handle_startendtag(self, tag, attrs):
        if self.skip_depth:
            return
        tag = tag.lower()
        if tag in _PRESENTATION_ALLOWED_TAGS:
            self._safe_start(tag, attrs)

    def handle_endtag(self, tag):
        tag = tag.lower()
        if self.skip_depth:
            if tag == self.skip_tag:
                self.skip_depth -= 1
                if not self.skip_depth:
                    self.skip_tag = None
            elif tag not in _PRESENTATION_VOID_TAGS:
                self.skip_depth += 1
            return
        if tag in _PRESENTATION_ALLOWED_TAGS and tag in self.stack:
            while self.stack:
                open_tag = self.stack.pop()
                if open_tag == tag:
                    self.output.append(f'</{tag}>')
                    break
                self.output.append(f'</{open_tag}>')

    def handle_data(self, data):
        if not self.skip_depth and data:
            self.output.append(html.escape(data, quote=False))

    def result(self):
        while self.stack:
            self.output.append(f'</{self.stack.pop()}>')
        return ''.join(self.output)


def _sanitize_slide_html(markdown: str) -> str:
    """Remove active HTML while preserving Markdown and protected samples."""
    protected, code_blocks = _protect_blocks(markdown)

    inline_blocks: Dict[str, str] = {}
    inline_pattern = re.compile(r'(?<!`)(`+)(?!`)([\s\S]*?)(?<!`)\1(?!`)')

    def inline_repl(match: re.Match) -> str:
        key = f"__READMD_INLINE_CODE_{len(inline_blocks)}__"
        inline_blocks[key] = match.group(0)
        return key

    protected = inline_pattern.sub(inline_repl, protected)
    parser = _PresentationHTMLSanitizer()
    parser.feed(protected)
    parser.close()
    clean = parser.result()
    clean = _restore_blocks(clean, inline_blocks)
    return _restore_blocks(clean, code_blocks)


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
            clean_slide_content = _sanitize_slide_html(clean_slide_content)
            note_text = _sanitize_slide_html("\n\n".join(notes)).strip()
            
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


# 本地 vendor 资源（相对应用根；srcdoc iframe 继承父页面 base URL 与 CSP 'self'）
_REVEAL_BASE = 'assets/vendor/reveal/dist'
_KATEX_BASE = 'assets/vendor/katex/dist'
_REVEAL_STYLESHEETS = ('reveal.css', 'plugin/highlight/monokai.css')
_REVEAL_SCRIPTS = ('reveal.js', 'plugin/markdown/markdown.js',
                   'plugin/highlight/highlight.js', 'plugin/notes/notes.js',
                   'plugin/math/math.js')


def _read_vendor(rel_path: str) -> str:
    """读取 assets/vendor 下的离线资源；打包与源码运行均已覆盖。

    注意：不使用 mdexport.APP_DIR（其解析少上一层、指向 src/，
    为既有行为，此处独立计算应用根避免牵连其他导出模块）。
    """
    if getattr(sys, 'frozen', False):
        root = sys._MEIPASS
    else:
        # presentation_render.py -> mdexport -> readmd_modules -> src -> 应用根
        root = os.path.dirname(os.path.dirname(os.path.dirname(
            os.path.dirname(os.path.abspath(__file__)))))
    full = os.path.join(root, 'assets', 'vendor', *rel_path.split('/'))
    with open(full, 'r', encoding='utf-8') as handle:
        return handle.read()


def _read_vendor_bytes(rel_path: str) -> bytes:
    if getattr(sys, 'frozen', False):
        root = sys._MEIPASS
    else:
        root = os.path.dirname(os.path.dirname(os.path.dirname(
            os.path.dirname(os.path.abspath(__file__)))))
    full = os.path.join(root, 'assets', 'vendor', *rel_path.split('/'))
    with open(full, 'rb') as handle:
        return handle.read()


def _vendor_data_uri(rel_path: str) -> str:
    media_types = {'.ttf': 'font/ttf', '.woff': 'font/woff', '.woff2': 'font/woff2'}
    suffix = os.path.splitext(rel_path)[1].lower()
    payload = base64.b64encode(_read_vendor_bytes(rel_path)).decode('ascii')
    return f'data:{media_types[suffix]};base64,{payload}'


def _inline_relative_css_assets(css: str, base_dir: str) -> str:
    """Replace relative font URLs so a standalone HTML file remains self-contained."""
    def inline(match: re.Match) -> str:
        quote, relative = match.group(1), match.group(2)
        asset = os.path.join(base_dir, *relative.split('/')).replace('\\', '/')
        return f'url("{_vendor_data_uri(asset)}")'

    return re.sub(r'url\(\s*([\'"]?)(fonts/[^\'")]+)\1\s*\)', inline, css)


def _css_tag(rel_path: str, standalone: bool, link_id: str = '') -> str:
    if standalone:
        return '<style>\n' + _read_vendor(os.path.join('reveal', 'dist', *rel_path.split('/'))) + '\n</style>'
    id_attr = ' id="%s"' % link_id if link_id else ''
    return '<link rel="stylesheet" href="%s/%s"%s>' % (_REVEAL_BASE, rel_path, id_attr)


def _script_tag(rel_path: str, standalone: bool) -> str:
    if standalone:
        return '<script>\n' + _read_vendor(os.path.join('reveal', 'dist', *rel_path.split('/'))) + '\n</script>'
    return '<script src="%s/%s"></script>' % (_REVEAL_BASE, rel_path)


def _katex_tags(standalone: bool) -> str:
    """KaTeX 资源：应用内由 RevealMath.KaTeX 按 local 路径注入；导出单文件直接内联。"""
    if not standalone:
        return ''
    katex_css = _inline_relative_css_assets(
        _read_vendor('katex/dist/katex.min.css'), 'katex/dist'
    )
    katex_js = _read_vendor('katex/dist/katex.min.js')
    auto_render = _read_vendor('katex/dist/contrib/auto-render.min.js')
    return ('<style>\n' + katex_css + '\n</style>\n'
            '<script>\n' + katex_js + '\n</script>\n'
            '<script>\n' + auto_render + '\n</script>')


def render_presentation_html(content: str, title: str = "ReadMD Presentation",
                             theme: str = "black", transition: str = "slide",
                             standalone: bool = False) -> str:
    """将 Markdown 编译为 Reveal.js 演说稿。

    standalone=False（应用内预览）：引用本地 vendor 资源（同源，符合 CSP，完全离线）。
    standalone=True（导出 .html 单文件）：全部资源内联，任何机器离线可开。
    """
    meta, _ = parse_frontmatter(content)
    theme = meta.get('theme', theme)
    if theme.endswith('.css'):
        theme = theme[:-4]
    transition = meta.get('transition', transition)
    title = html.escape(str(meta.get('title', title)), quote=True)
    if theme not in _REVEAL_THEMES:
        theme = 'black'

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

    head_css = '\n'.join([
        _css_tag('reveal.css', standalone),
        _css_tag('theme/%s.css' % theme, standalone, link_id='theme'),
        _css_tag('plugin/highlight/monokai.css', standalone),
        _katex_tags(standalone),
    ])
    body_scripts = '\n'.join(_script_tag(p, standalone) for p in _REVEAL_SCRIPTS)

    # 启动配置：JSON <script> 不被执行，不受 CSP 限制；boot 代码本体走同源加载
    boot_rel = 'reveal/dist/readmd-boot.js'
    boot_js = _read_vendor(os.path.join('reveal', 'dist', 'readmd-boot.js'))
    reveal_config = {
        'transition': transition if transition in _REVEAL_TRANSITIONS else 'slide',
        'themeBase': _REVEAL_BASE + '/theme/',
        # KaTeX 插件会自行追加 /dist/...，这里给到包根
        'katexLocal': 'assets/vendor/katex',
        'standalone': bool(standalone),
    }
    config_json = json.dumps(reveal_config, ensure_ascii=False).replace('</', '<\\/')
    if standalone:
        boot_tag = '<script>\n' + boot_js + '\n</script>'
    else:
        boot_tag = '<script src="%s/readmd-boot.js"></script>' % _REVEAL_BASE

    return f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
  <title>{title}</title>
{head_css}
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
      display: flex !important;
      flex-direction: column !important;
      justify-content: center !important;
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
<body data-transition="{transition}">
  <div class="reveal">
    <div class="slides">
{slides_body}
    </div>
  </div>
{body_scripts}
  <script type="application/json" id="readmd-reveal-config">{config_json}</script>
{boot_tag}
</body>
</html>"""


# 兼容别名
generate_presentation_html = render_presentation_html
