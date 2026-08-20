# -*- coding: utf-8 -*-
"""ReadMD 正文 [TOC] 自动内嵌层级目录树生成引擎。

支持语法：
- `[TOC]`
- `[toc]`
- `<!-- @import "[TOC]" -->`
- 带参数定制：`[TOC] {depth_from=2 depth_to=4 ordered_list=false ignore=["参考资料"]}`
"""

import re
from typing import Dict, List, Optional, Tuple

TOC_MARKER_PATTERN = re.compile(
    r'^[ \t]*(?:\[(?:TOC|toc)\]|<!--\s*@import\s*["\']\[TOC\]["\']\s*-->)(?:\s*\{([^}]*)\})?[ \t]*$',
    re.MULTILINE
)

HEADING_PATTERN = re.compile(r'^(#{1,6})[ \t]+(.+?)[ \t]*#*[ \t]*$')


def slugify_heading(heading_text: str) -> str:
    """生成与 GitHub / 浏览器兼容的标题锚点链接。"""
    # 移除行内代码、图片、链接标记
    text = re.sub(r'`([^`]+)`', r'\1', heading_text)
    text = re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', text)
    text = re.sub(r'!\[([^\]]*)\]\([^)]+\)', r'\1', text)
    text = re.sub(r'<[^>]+>', '', text)
    
    # 转小写并替换空格与标点
    slug = text.strip().lower()
    # 替换空格为短横线
    slug = re.sub(r'[ \t]+', '-', slug)
    # 移除特殊标点符号（保留字母、数字、中文、下划线、短横线）
    slug = re.sub(r'[^\w\u4e00-\u9fa5\-]', '', slug)
    return slug or "section"


def extract_headings(markdown_content: str) -> List[Tuple[int, str, str]]:
    """提取 Markdown 文档中的全部有效标题 (level, title, slug)，自动规避代码块内的注释。"""
    headings: List[Tuple[int, str, str]] = []
    in_code_block = False
    fence_char = ""

    for line in markdown_content.splitlines():
        stripped = line.strip()
        # 检查代码块定界符
        if stripped.startswith('```') or stripped.startswith('~~~'):
            curr_fence = stripped[:3]
            if not in_code_block:
                in_code_block = True
                fence_char = curr_fence
            elif curr_fence == fence_char:
                in_code_block = False
            continue

        if in_code_block:
            continue

        match = HEADING_PATTERN.match(line)
        if match:
            level = len(match.group(1))
            title = match.group(2).strip()
            slug = slugify_heading(title)
            headings.append((level, title, slug))

    return headings


def generate_toc_markdown(headings: List[Tuple[int, str, str]],
                          depth_from: int = 1,
                          depth_to: int = 6,
                          ordered_list: bool = False,
                          ignore_titles: Optional[List[str]] = None) -> str:
    """根据标题列表生成层级 Markdown 目录树。"""
    if not headings:
        return ""

    ignore_set = set(t.lower() for t in (ignore_titles or []))
    filtered = [
        (lvl, title, slug) for lvl, title, slug in headings
        if depth_from <= lvl <= depth_to and title.lower() not in ignore_set
    ]

    if not filtered:
        return ""

    min_level = min(lvl for lvl, _, _ in filtered)
    toc_lines = []

    # 用于多级序号统计
    counters = [0] * 7

    for lvl, title, slug in filtered:
        indent = "  " * (lvl - min_level)
        clean_title = re.sub(r'[*_~`]', '', title)
        
        if ordered_list:
            counters[lvl] += 1
            # 重置子级别
            for i in range(lvl + 1, 7):
                counters[i] = 0
            prefix = f"{counters[lvl]}. "
        else:
            prefix = "- "

        toc_lines.append(f"{indent}{prefix}[{clean_title}](#{slug})")

    return "\n".join(toc_lines)


def process_toc_markers(content: str) -> str:
    """扫描并就地替换文档中的全部 [TOC] 标记为实际生成的目录树。"""
    headings = extract_headings(content)

    def replace_toc(match: re.Match) -> str:
        attr_str = match.group(1) or ""
        depth_from = 1
        depth_to = 6
        ordered = False

        # 简单提取属性
        from_m = re.search(r'depth_from\s*=\s*(\d+)', attr_str)
        if from_m:
            depth_from = int(from_m.group(1))
        to_m = re.search(r'depth_to\s*=\s*(\d+)', attr_str)
        if to_m:
            depth_to = int(to_m.group(1))
        if 'ordered_list=true' in attr_str.lower() or 'ordered=true' in attr_str.lower():
            ordered = True

        toc_md = generate_toc_markdown(headings, depth_from=depth_from,
                                       depth_to=depth_to, ordered_list=ordered)
        return f"\n{toc_md}\n" if toc_md else ""

    return TOC_MARKER_PATTERN.sub(replace_toc, content)
