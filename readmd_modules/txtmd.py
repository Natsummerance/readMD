# -*- coding: utf-8 -*-
"""TXT 智能转 Markdown（v2.2.5）。

纯 Python、无第三方依赖，保守启发式识别：
- 标题：第X章 / 第X节、一、/ （一）、1. / 1.1 / 1.1.1、短行独立标题
- 列表：• · ◦ ▪ 等符号 → -；1、/（1）→ 1.；- / * / + 保持
- 表格：Tab 分隔或 2+ 空格对齐的连续行 → GitHub 风格 MD 表格
- 目录：识别出 >= 3 个标题且原文无目录时，文首插入「## 目录」+ 锚点链接

保守原则：无法确认的结构一律原样保留（changed=False），避免误伤普通 TXT。
"""

import re

_ENCODINGS = ('utf-8', 'gb18030', 'big5', 'latin-1')

_CN_NUM = u'\u4e00\u4e8c\u4e09\u56db\u4e94\u516d\u4e03\u516b\u4e5d\u5341\u767e\u5343\u4e07\u4e24'
_HEAD_CN = re.compile(r'^第[%s0-9]+[章节回部篇卷][^\S\n]*' % _CN_NUM)
_HEAD_CN2 = re.compile(r'^[（(]?[%s]{1,3}[）)、．.][^\S\n]*' % _CN_NUM)
_HEAD_NUM = re.compile(r'^\d{1,3}(?:\.|\uff0e)[^\S\n]*')
_HEAD_SUB = re.compile(r'^(\d{1,3})\.(\d{1,3})(?:\.(\d{1,3}))?[^\S\n]*')

_LIST_BULLET = re.compile(r'^([ \t]*)[\u2022\u00b7\u25e6\u25aa\u25cf*]\s+')
_LIST_CN = re.compile(r'^([ \t]*)[（(]?[%s]{1,3}[）)]\s*' % _CN_NUM)
_LIST_NUM = re.compile(r'^([ \t]*)\d{1,3}[、\uff0e]\s*')

_TABLE_TAB = re.compile(r'^\S+(\t+\S+)+\t*$')
_TABLE_SPACE = re.compile(r'^\S+(\s{2,}\S+)+$')
_FENCE_RE = re.compile(r'^(`{3,}|~{3,})')
_TRAIL_PUNC = u'\u3002\uff01\uff1f\uff1b\uff0c\u3001\uff1a,.!?;:\u2026\uff09)]}\u300b\u300d\u2019"\u201d'
_SENT_CHARS = u'\u3002\uff01\uff1f'
_MAX_HEADING_LEN = 40


def read_text(path):
    """按编码优先级读取文本（UTF-8 / GB18030 / Big5 / Latin-1）。"""
    with open(path, 'rb') as f:
        data = f.read()
    if data.startswith(b'\xef\xbb\xbf'):
        return data.decode('utf-8-sig'), 'utf-8-sig'
    for enc in _ENCODINGS:
        try:
            return data.decode(enc), enc
        except (UnicodeDecodeError, LookupError):
            continue
    return data.decode('utf-8', errors='replace'), 'utf-8'


def _slugify(title, seen):
    """GitHub / marked 风格锚点：小写、去标点、空格转 -，重复加序号。"""
    slug = title.strip().lower()
    slug = re.sub(r'[^\w\u4e00-\u9fff\- ]', '', slug)
    slug = re.sub(r'\s+', '-', slug).strip('-')
    if not slug:
        slug = 'section'
    base, n = slug, 0
    while slug in seen:
        n += 1
        slug = '%s-%d' % (base, n)
    seen.add(slug)
    return slug


def _heading_level(line):
    """返回 (level, heading_text) 或 None。level: 1=# 2=## 3=### 4=####。"""
    stripped = line.strip()
    if not stripped:
        return None
    if stripped.startswith('#'):
        m = re.match(r'^(#{1,6})\s+(.+)$', stripped)
        if m:
            return len(m.group(1)), m.group(2).strip()
        return None
    if len(stripped) > _MAX_HEADING_LEN:
        return None
    if _HEAD_CN.match(stripped):
        return (1 if stripped[1:2] == u'\u7ae0' else 2), stripped
    if _HEAD_CN2.match(stripped):
        return 2, stripped
    m = _HEAD_SUB.match(stripped)
    if m and len(stripped) > m.end():
        return (4 if m.group(3) else 3), stripped
    if _HEAD_NUM.match(stripped) and len(stripped) > 2:
        return 2, stripped
    return None


def _short_heading(line):
    """短行独立标题 → (2, text) 或 None（由调用方检查上下文空行）。"""
    stripped = line.strip()
    if not (2 <= len(stripped) <= 30):
        return None
    if stripped[0] in '#*-+>|`~':
        return None
    if re.match(r'^[\d\s]+$', stripped) or re.match(r'^\d{1,4}[、\uff0e.]', stripped):
        return None
    if stripped[-1] in _TRAIL_PUNC:
        return None
    if any(ch in stripped for ch in _SENT_CHARS):
        return None
    return 2, stripped


def _table_cells(line):
    """按 tab 或 2+ 空格切分；格式不符返回 None。"""
    if _TABLE_TAB.match(line):
        cells = [c.strip() for c in line.split('\t')]
    elif _TABLE_SPACE.match(line):
        cells = [c.strip() for c in re.split(r'\s{2,}', line.strip())]
    else:
        return None
    cells = [c for c in cells if c != '']
    if not (2 <= len(cells) <= 12):
        return None
    return cells


def _collect_tables(lines):
    """返回 {start: (end_exclusive, sep)}；sep 为 'tab' 或 'space'。跳过代码围栏。"""
    n = len(lines)
    blocks = {}
    fence = False
    fence_mark = ''
    i = 0
    while i < n:
        line = lines[i]
        stripped = line.strip()
        if _FENCE_RE.match(stripped):
            if not fence:
                fence, fence_mark = True, stripped[0]
            elif stripped.startswith(fence_mark):
                fence = False
            i += 1
            continue
        if fence:
            i += 1
            continue
        cells = _table_cells(line)
        if cells is None:
            i += 1
            continue
        sep = 'tab' if '\t' in line else 'space'
        j = i + 1
        rows = [cells]
        while j < n:
            nxt = lines[j].strip()
            if not nxt or _FENCE_RE.match(nxt):
                break
            ncells = _table_cells(lines[j])
            if ncells is None or len(ncells) != len(cells) or ('\t' in lines[j]) != (sep == 'tab'):
                break
            rows.append(ncells)
            j += 1
        if len(rows) >= 2:
            blocks[i] = (j, sep)
            i = j
        else:
            i += 1
    return blocks


def _render_table(rows, cols):
    """rows: list[list[str]]；第 2 行转分隔行，列数统一。"""
    out = []
    for idx, row in enumerate(rows):
        cells = row + [''] * (cols - len(row))
        if idx == 1:
            out.append('| ' + ' | '.join(['---'] * cols) + ' |')
        out.append('| ' + ' | '.join(cells) + ' |')
    return out


def _render_toc(headings):
    lines = ['## 目录', '']
    seen = set()
    for level, text in headings:
        slug = _slugify(text, seen)
        indent = '  ' * max(0, level - 2)
        lines.append('%s- [%s](#%s)' % (indent, text, slug))
    lines.append('')
    return '\n'.join(lines)


def to_markdown(text):
    """TXT → Markdown。返回 (md, stats)。stats: changed/headings/tables/lists/toc。"""
    stats = {'changed': False, 'headings': 0, 'tables': 0, 'lists': 0, 'toc': False}
    if not text:
        return text, stats
    src = text.replace('\r\n', '\n').replace('\r', '\n')
    lines = src.split('\n')
    n = len(lines)

    tables = _collect_tables(lines)

    out = []
    headings = []
    changed = False
    fence = False
    fence_mark = ''
    i = 0
    while i < n:
        line = lines[i]
        stripped = line.strip()
        if not stripped:
            out.append('')
            i += 1
            continue
        if _FENCE_RE.match(stripped):
            out.append(line)
            if not fence:
                fence, fence_mark = True, stripped[0]
            elif stripped.startswith(fence_mark):
                fence = False
            i += 1
            continue
        if fence:
            out.append(line)
            i += 1
            continue
        if i in tables:
            end, sep = tables[i]
            block = lines[i:end]
            rows = []
            for bl in block:
                if _TABLE_TAB.match(bl):
                    rows.append([c.strip() for c in bl.split('\t') if c.strip() != ''])
                else:
                    rows.append([c.strip() for c in re.split(r'\s{2,}', bl.strip()) if c.strip() != ''])
            cols = max(len(r) for r in rows)
            out.extend(_render_table(rows, cols))
            stats['tables'] += 1
            changed = True
            i = end
            continue
        h = _heading_level(line)
        if h:
            level, htext = h
            out.append('#' * level + ' ' + htext)
            headings.append((level, htext))
            stats['headings'] += 1
            changed = True
            i += 1
            continue
        m = _LIST_BULLET.match(line)
        if m:
            out.append(m.group(1) + '- ' + line[m.end():])
            stats['lists'] += 1
            changed = True
            i += 1
            continue
        m = _LIST_CN.match(line)
        if m:
            out.append(m.group(1) + '1. ' + line[m.end():])
            stats['lists'] += 1
            changed = True
            i += 1
            continue
        m = _LIST_NUM.match(line)
        if m:
            out.append(m.group(1) + '1. ' + line[m.end():])
            stats['lists'] += 1
            changed = True
            i += 1
            continue
        # 短行独立标题：前后为空行 / 文件边界
        if i == 0 or not lines[i - 1].strip() or i + 1 == n or not lines[i + 1].strip():
            sh = _short_heading(line)
            if sh:
                level, htext = sh
                out.append('#' * level + ' ' + htext)
                headings.append((level, htext))
                stats['headings'] += 1
                changed = True
                i += 1
                continue
        out.append(line)
        i += 1

    md = '\n'.join(out).rstrip('\n') + '\n'
    if len(headings) >= 3 and '## 目录' not in md and '# 目录' not in md:
        md = _render_toc(headings) + '\n' + md
        stats['toc'] = True
        changed = True
    stats['changed'] = changed
    return md, stats