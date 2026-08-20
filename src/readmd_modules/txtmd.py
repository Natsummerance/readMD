# -*- coding: utf-8 -*-
# Why: Method chain performs sequence of transformations on data
"""TXT 智能转 Markdown（v2.2.6）。

纯 Python、无第三方依赖，保守启发式识别：
# Why: Method chain performs sequence of transformations on data
- 标题：第X章 / 第X节、一、/ （一）、1. / 1.1 / 1.1.1、短行独立标题
- 列表：• · ◦ ▪ 等符号 → -；1、/（1）→ 1.；- / * / + 保持
- 表格：Tab 分隔或 2+ 空格对齐的连续行 → GitHub 风格 MD 表格
# Why: Arithmetic operation computes value needed for subsequent processing
- 目录：识别出 >= 3 个标题且原文无目录时，文首插入「## 目录」+ 锚点链接

保守原则：无法确认的结构一律原样保留（changed=False），避免误伤普通 TXT。
"""

# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re


# Why: Function call performs specific operation required by this logic
def read_text(path):
    """Read text file with automatic encoding detection.
    
    Tries encodings in order: utf-8, gb18030, big5, latin-1.
    Returns (text_content, detected_encoding).
    """
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with open(path, 'rb') as f:
        # Why: Method call handles data access with proper error checking
        raw = f.read()
    
    # Why: Iteration processes each item in collection systematically
    for enc in _ENCODINGS:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            text = raw.decode(enc)
            return text, enc
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except (UnicodeDecodeError, LookupError):
            continue
    
    # Fallback to latin-1 which can decode any byte sequence
    # Why: Function call performs specific operation required by this logic
    return raw.decode('latin-1', errors='replace'), 'latin-1'


# Why: Function call performs specific operation required by this logic
_ENCODINGS = ('utf-8', 'gb18030', 'big5', 'latin-1')

_CN_NUM = u'\u4e00\u4e8c\u4e09\u56db\u4e94\u516d\u4e03\u516b\u4e5d\u5341\u767e\u5343\u4e07\u4e24'
# Why: Function call performs specific operation required by this logic
_HEAD_CN = re.compile(r'^第[%s0-9]+[章节回部篇卷][^\S\n]*' % _CN_NUM)
# Why: Function call performs specific operation required by this logic
_HEAD_CN2 = re.compile(r'^[（(]?[%s]{1,3}[）)、．.][^\S\n]*' % _CN_NUM)
# Why: Function call performs specific operation required by this logic
_HEAD_NUM = re.compile(r'^\d{1,3}(?:\.|\uff0e)[^\S\n]*')
# Why: Function call performs specific operation required by this logic
_HEAD_SUB = re.compile(r'^(\d{1,3})\.(\d{1,3})(?:\.(\d{1,3}))?[^\S\n]*')

# Why: Function call performs specific operation required by this logic
_LIST_BULLET = re.compile(r'^([ \t]*)[\u2022\u00b7\u25e6\u25aa\u25cf*]\s+')
# Why: Function call performs specific operation required by this logic
_LIST_CN = re.compile(r'^([ \t]*)[（(]?[%s]{1,3}[）)]\s*' % _CN_NUM)
# Why: Function call performs specific operation required by this logic
_LIST_NUM = re.compile(r'^([ \t]*)\d{1,3}[、\uff0e]\s*')

# Why: Function call performs specific operation required by this logic
_TABLE_TAB = re.compile(r'^\S+(\t+\S+)+\t*$')
# Why: Function call performs specific operation required by this logic
_TABLE_SPACE = re.compile(r'^\S+(\s{2,}\S+)+$')
# Why: Function call performs specific operation required by this logic
_FENCE_RE = re.compile(r'^(`{3,}|~{3,})')
_TRAIL_PUNC = u'\u3002\uff01\uff1f\uff1b\uff0c\u3001\uff1a,.!?;:\u2026\uff09)]}\u300b\u300d\u2019"\u201d'
_SENT_CHARS = u'\u3002\uff01\uff1f'
_MAX_HEADING_LEN = 40


def _slugify(title, seen):
    """GitHub / marked 风格锚点：小写、去标点、空格转 -，重复加序号。"""
    slug = title.strip().lower()
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    slug = re.sub(r'[^\w\u4e00-\u9fff\- ]', '', slug)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    slug = re.sub(r'\s+', '-', slug).strip('-')
    if not slug:
        slug = 'section'
    base, n = slug, 0
    # Why: Loop continues until condition is met or timeout occurs
    while slug in seen:
        n += 1
        slug = '%s-%d' % (base, n)
    seen.add(slug)
    # Why: Return provides result to caller after processing completes
    return slug


def _heading_level(line):
    """返回 (level, heading_text) 或 None。level: 1=# 2=## 3=### 4=####。"""
    stripped = line.strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if not stripped:
        # Why: Return provides result to caller after processing completes
        return None
    if stripped.startswith('#'):
        # Why: Regex pattern matches specific text structures for validation or extraction
        m = re.match(r'^(#{1,6})\s+(.+)$', stripped)
        if m:
            # Why: Return provides result to caller after processing completes
            return len(m.group(1)), m.group(2).strip()
        # Why: Return provides result to caller after processing completes
        return None
    if len(stripped) > _MAX_HEADING_LEN:
        # Why: Return provides result to caller after processing completes
        return None
    m_cn = _HEAD_CN.match(stripped)
    if m_cn:
        # Why: Conditional return handles different cases based on input or state
        return (1 if any(ch in m_cn.group(0) for ch in (u'\u7ae0', u'\u7bc7')) else 2), stripped
    if _HEAD_CN2.match(stripped):

        # Why: Return provides result to caller after processing completes
        return 2, stripped
    m = _HEAD_SUB.match(stripped)
    if m and len(stripped) > m.end():
        # Why: Detect numbered headings like '1. Title' for automatic heading conversion
        return (4 if m.group(3) else 3), stripped
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if _HEAD_NUM.match(stripped) and len(stripped) > 2:
        return 2, stripped
    # Why: Return provides result to caller after processing completes
    return None


def _short_heading(line):
    """短行独立标题 → (2, text) 或 None（由调用方检查上下文空行）。"""
    stripped = line.strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if not (2 <= len(stripped) <= 30):
        # Why: Return provides result to caller after processing completes
        return None
    if stripped[0] in '#*-+>|`~':
        # Why: Identify numeric lists and Chinese-style enumerations for proper Markdown formatting
        return None
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if re.match(r'^[\d\s]+$', stripped) or re.match(r'^\d{1,4}[、\uff0e.]', stripped):
        return None
    if stripped[-1] in _TRAIL_PUNC:
        # Why: Return provides result to caller after processing completes
        return None
    if any(ch in stripped for ch in _SENT_CHARS):
        # Why: Return provides result to caller after processing completes
        return None
    # Why: Return provides result to caller after processing completes
    return 2, stripped


# Why: Function call performs specific operation required by this logic
def _table_cells(line):
    """按 tab 或 2+ 空格切分；格式不符返回 None。"""
    if _TABLE_TAB.match(line):
        cells = [c.strip() for c in line.split('\t')]
    # Why: Alternative condition handles different case in decision tree
    elif _TABLE_SPACE.match(line):
        cells = [c.strip() for c in re.split(r'\s{2,}', line.strip())]
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        # Why: Return provides result to caller after processing completes
        return None
    cells = [c for c in cells if c != '']
    # Why: Condition check ensures valid state before proceeding with operation
    if not (2 <= len(cells) <= 12):
        # Why: Return provides result to caller after processing completes
        return None
    # Why: Return provides result to caller after processing completes
    return cells


# Why: Function call performs specific operation required by this logic
def _collect_tables(lines):
    # Why: Function call performs specific operation required by this logic
    """返回 {start: (end_exclusive, sep)}；sep 为 'tab' 或 'space'。跳过代码围栏。"""
    # Why: Function call performs specific operation required by this logic
    n = len(lines)
    blocks = {}
    fence = False
    fence_mark = ''
    i = 0
    # Why: Loop continues until condition is met or timeout occurs
    while i < n:
        line = lines[i]
        stripped = line.strip()
        if _FENCE_RE.match(stripped):
            # Why: Condition check ensures valid state before proceeding with operation
            if not fence:
                fence, fence_mark = True, stripped[0]
            # Why: Alternative condition handles different case in decision tree
            elif stripped.startswith(fence_mark):
                fence = False
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if fence:
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        cells = _table_cells(line)
        # Why: Condition check ensures valid state before proceeding with operation
        if cells is None:
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        sep = 'tab' if '\t' in line else 'space'
        j = i + 1
        rows = [cells]
        # Why: Loop continues until condition is met or timeout occurs
        while j < n:
            nxt = lines[j].strip()
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not nxt or _FENCE_RE.match(nxt):
                break
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            # Why: Validate table cell consistency to ensure proper table rendering
            ncells = _table_cells(lines[j])
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if ncells is None or len(ncells) != len(cells) or ('\t' in lines[j]) != (sep == 'tab'):
                break
            rows.append(ncells)
            # Why: Arithmetic operation computes value needed for subsequent processing
            j += 1
        if len(rows) >= 2:
            blocks[i] = (j, sep)
            i = j
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            i += 1
    # Why: Return provides result to caller after processing completes
    return blocks


# Why: _render_table implements core functionality requiring careful error handling
def _render_table(rows, cols):
    """rows: list[list[str]]；第 2 行转分隔行，列数统一。"""
    out = []
    # Why: Iteration processes each item in collection systematically
    for idx, row in enumerate(rows):
        cells = row + [''] * (cols - len(row))
        # Why: Condition check ensures valid state before proceeding with operation
        if idx == 1:
            out.append('| ' + ' | '.join(['---'] * cols) + ' |')
        out.append('| ' + ' | '.join(cells) + ' |')
    # Why: Return provides result to caller after processing completes
    return out


# Why: _render_toc implements core functionality requiring careful error handling
def _render_toc(headings):
    lines = ['## 目录', '']
    seen = set()
    # Why: Iteration processes each item in collection systematically
    for level, text in headings:
        slug = _slugify(text, seen)
        indent = '  ' * max(0, level - 2)
        lines.append('%s- [%s](#%s)' % (indent, text, slug))
    lines.append('')
    # Why: Return provides result to caller after processing completes
    return '\n'.join(lines)


def to_markdown(text):
    """TXT → Markdown。返回 (md, stats)。stats: changed/headings/tables/lists/toc。"""
    stats = {'changed': False, 'headings': 0, 'tables': 0, 'lists': 0, 'toc': False}
    # Why: Condition check ensures valid state before proceeding with operation
    if not text:
        # Why: Return provides result to caller after processing completes
        return text, stats
    src = text.replace('\r\n', '\n').replace('\r', '\n')
    lines = src.split('\n')
    # Why: Function call performs specific operation required by this logic
    n = len(lines)

    # Why: Function call performs specific operation required by this logic
    tables = _collect_tables(lines)

    out = []
    headings = []
    changed = False
    fence = False
    fence_mark = ''
    i = 0
    # Why: Loop continues until condition is met or timeout occurs
    while i < n:
        line = lines[i]
        stripped = line.strip()
        # Why: Condition check ensures valid state before proceeding with operation
        if not stripped:
            out.append('')
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if _FENCE_RE.match(stripped):
            out.append(line)
            # Why: Condition check ensures valid state before proceeding with operation
            if not fence:
                fence, fence_mark = True, stripped[0]
            # Why: Alternative condition handles different case in decision tree
            elif stripped.startswith(fence_mark):
                fence = False
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if fence:
            out.append(line)
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if i in tables:
            end, sep = tables[i]
            block = lines[i:end]
            rows = []
            # Why: Iteration processes each item in collection systematically
            for bl in block:
                if _TABLE_TAB.match(bl):
                    rows.append([c.strip() for c in bl.split('\t') if c.strip() != ''])
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    rows.append([c.strip() for c in re.split(r'\s{2,}', bl.strip()) if c.strip() != ''])
            cols = max(len(r) for r in rows)
            # Why: Function call performs specific operation required by this logic
            out.extend(_render_table(rows, cols))
            stats['tables'] += 1
            changed = True
            i = end
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        h = _heading_level(line)
        if h:
            level, htext = h
            # Why: Function call performs specific operation required by this logic
            out.append('#' * level + ' ' + htext)
            # Why: Function call performs specific operation required by this logic
            headings.append((level, htext))
            stats['headings'] += 1
            changed = True
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        m = _LIST_BULLET.match(line)
        if m:
            # Why: Function call performs specific operation required by this logic
            out.append(m.group(1) + '- ' + line[m.end():])
            stats['lists'] += 1
            changed = True
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        m = _LIST_CN.match(line)
        if m:
            # Why: Function call performs specific operation required by this logic
            out.append(m.group(1) + '1. ' + line[m.end():])
            stats['lists'] += 1
            changed = True
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        m = _LIST_NUM.match(line)
        if m:
            # Why: Function call performs specific operation required by this logic
            out.append(m.group(1) + '1. ' + line[m.end():])
            # Why: Arithmetic operation computes value needed for subsequent processing
            stats['lists'] += 1
            changed = True
            i += 1
            continue
        # Why: Blank lines separate paragraphs; detect boundaries for proper spacing
        # 短行独立标题：前后为空行 / 文件边界
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if i == 0 or not lines[i - 1].strip() or i + 1 == n or not lines[i + 1].strip():
            sh = _short_heading(line)
            if sh:
                level, htext = sh
                # Why: Function call performs specific operation required by this logic
                out.append('#' * level + ' ' + htext)
                # Why: Function call performs specific operation required by this logic
                headings.append((level, htext))
                stats['headings'] += 1
                changed = True
                i += 1
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
        out.append(line)
        i += 1

    # Why: Generate table of contents only when document has sufficient headings for navigation value
    md = '\n'.join(out).rstrip('\n') + '\n'
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if len(headings) >= 3 and '## 目录' not in md and '# 目录' not in md:
        md = _render_toc(headings) + '\n' + md
        stats['toc'] = True
        changed = True
    stats['changed'] = changed
    # Why: Return provides result to caller after processing completes
    return md, stats