# -*- coding: utf-8 -*-
"""ReadMD AST 源码行号映射器 (Source Map Line Injector)。

为 Markdown 块级元素注入 `data-source-line="N"` 行号属性：
- 标题 (`<h1>` - `<h6>`)
- 段落 (`<p>`)
- 代码块 (`<pre>`)
- 引用块 (`<blockquote>`)
- 表格 (`<table>`)
- 列表 (`<ul>`, `<ol>`)

配合前端基于最近双锚点的线性插值算法 (Linear Interpolation between Anchors)，
彻底消除折叠公式与长表格引起的滚动漂移。
"""

import re
from typing import List, Tuple


def annotate_markdown_source_lines(markdown_content: str) -> str:
    """在 Markdown 文本中为每个块级元素头部注入 source-line 注释标记。"""
    lines = markdown_content.splitlines()
    annotated = []
    in_code = False
    fence_char = ""

    for line_idx, line in enumerate(lines, start=1):
        stripped = line.strip()

        # 处理代码块定界符
        if stripped.startswith('```') or stripped.startswith('~~~'):
            curr_fence = stripped[:3]
            if not in_code:
                in_code = True
                fence_char = curr_fence
                annotated.append(f'<!-- data-source-line="{line_idx}" -->\n{line}')
            elif curr_fence == fence_char:
                in_code = False
                annotated.append(line)
            continue

        if in_code:
            annotated.append(line)
            continue

        # 针对普通块级元素注入行号标记
        if stripped.startswith('#') or stripped.startswith('|') or stripped.startswith('>') or stripped.startswith('- ') or stripped.startswith('* '):
            annotated.append(f'<!-- data-source-line="{line_idx}" -->\n{line}')
        elif stripped and not stripped.startswith('<!--'):
            annotated.append(f'<!-- data-source-line="{line_idx}" -->\n{line}')
        else:
            annotated.append(line)

    return "\n".join(annotated)


def inject_source_line_attributes_to_html(html_content: str) -> str:
    """将 HTML 中的 `<!-- data-source-line="N" -->` 注释提升为其后紧邻标签的属性 `data-source-line="N"`。"""
    pattern = re.compile(
        r'<!--\s*data-source-line="(\d+)"\s*-->\s*<([a-zA-Z0-9]+)([^>]*)>',
        re.MULTILINE
    )

    def replace_tag(match: re.Match) -> str:
        line_no = match.group(1)
        tag_name = match.group(2)
        rest_attrs = match.group(3)
        return f'<{tag_name} data-source-line="{line_no}"{rest_attrs}>'

    return pattern.sub(replace_tag, html_content)
