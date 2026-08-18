# -*- coding: utf-8 -*-
"""Markdown -> 块级 AST 解析器（导出专用，支持 GFM 常用子集 + 数学公式）。

块类型: heading / paragraph / table / code / list / quote / hr / image /
         math(display) / html
行内节点: {t:text|bold|italic|code|strike|link|image|math, ...}
"""

import re

# 行内正则（懒惰匹配，避免吞并）
_IMG_RE = re.compile(r'!\[([^\]]*)\]\(([^)\s]+)(?:\s+["\']([^"\']*)["\'])?\)')
_LINK_RE = re.compile(r'\[([^\]]+)\]\(([^)\s]+)(?:\s+["\']([^"\']*)["\'])?\)')
_CODE_RE = re.compile(r'`([^`]+)`')
_MATH_RE = re.compile(r'\$([^$\n]+?)\$')
_BOLD_RE = re.compile(r'\*\*([^*]+?)\*\*')
_UNDER_RE = re.compile(r'__([^_]+?)__')
_STRIKE_RE = re.compile(r'~~([^~]+?)~~')
_EM_RE = re.compile(r'\*([^*\n]+?)\*|_([^_\n]+?)_')

_HR_RE = re.compile(r'^\s*(?:---+|\*\*\*+|___+)\s*$')
_HEAD_RE = re.compile(r'^(#{1,6})\s+(.*)$')
_FENCE_RE = re.compile(r'^(`{3,}|~{3,})(.*)$')
_LIST_RE = re.compile(r'^(\s*)([-*+]|\d+\.)\s+(.*)$')
_TASK_RE = re.compile(r'^\[([ xX])\]\s+(.*)$')
_TBL_SEP_RE = re.compile(r'^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$')
_TBL_CELL_RE = re.compile(r'^\s*:?-+:?\s*$')


def parse_inline(text):
    """把行内 Markdown 解析为节点列表。"""
    nodes = []
    buf = []
    i, n = 0, len(text)

    def flush():
        if buf:
            nodes.append({'t': 'text', 'v': ''.join(buf)})
            del buf[:]

    while i < n:
        if text.startswith('![', i):
            m = _IMG_RE.match(text, i)
            if m:
                flush(); nodes.append({'t': 'image', 'alt': m.group(1),
                                       'src': m.group(2), 'title': m.group(3) or ''})
                i = m.end(); continue
        if text[i] == '[':
            m = _LINK_RE.match(text, i)
            if m:
                flush()
                nodes.append({'t': 'link', 'text': parse_inline(m.group(1)),
                              'href': m.group(2), 'title': m.group(3) or ''})
                i = m.end(); continue
        if text[i] == '`':
            m = _CODE_RE.match(text, i)
            if m:
                flush(); nodes.append({'t': 'code', 'v': m.group(1)})
                i = m.end(); continue
        if text[i] == '$':
            m = _MATH_RE.match(text, i)
            if m:
                flush(); nodes.append({'t': 'math', 'latex': m.group(1), 'display': False})
                i = m.end(); continue
        if text.startswith('**', i):
            m = _BOLD_RE.match(text, i)
            if m:
                flush(); nodes.append({'t': 'bold', 'v': m.group(1)})
                i = m.end(); continue
        if text.startswith('__', i):
            m = _UNDER_RE.match(text, i)
            if m:
                flush(); nodes.append({'t': 'bold', 'v': m.group(1)})
                i = m.end(); continue
        if text.startswith('~~', i):
            m = _STRIKE_RE.match(text, i)
            if m:
                flush(); nodes.append({'t': 'strike', 'v': m.group(1)})
                i = m.end(); continue
        if text[i] in '*_':
            m = _EM_RE.match(text, i)
            if m:
                flush(); nodes.append({'t': 'italic', 'v': m.group(1) or m.group(2)})
                i = m.end(); continue
        buf.append(text[i])
        i += 1
    flush()
    return nodes


def inline_text(nodes):
    """行内节点 -> 纯文本（用于 TOC / 回退）。"""
    out = []
    for nd in nodes:
        t = nd['t']
        if t == 'text' or t == 'code':
            out.append(nd['v'])
        elif t in ('bold', 'italic', 'strike'):
            out.append(inline_text(nd['v']) if isinstance(nd['v'], list) else nd['v'])
        elif t == 'link':
            out.append(inline_text(nd['text']))
        elif t == 'image':
            out.append(nd['alt'])
        elif t == 'math':
            out.append(nd['latex'])
    return ''.join(out)


def _split_row(line):
    line = line.strip()
    if line.startswith('|'):
        line = line[1:]
    if line.endswith('|'):
        line = line[:-1]
    return [c.strip().replace('\\|', '|') for c in line.split('|')]


def _split_align(line):
    cells = _split_row(line)
    aligns = []
    for c in cells:
        c = c.strip()
        if c.startswith(':') and c.endswith(':'):
            aligns.append('center')
        elif c.endswith(':'):
            aligns.append('right')
        elif c.startswith(':'):
            aligns.append('left')
        else:
            aligns.append('')
    return aligns


def _is_block_start(line):
    s = line.strip()
    if not s:
        return True
    if s.startswith('```') or s.startswith('~~~'):
        return True
    if s.startswith('$$'):
        return True
    if _HEAD_RE.match(line) or _HR_RE.match(s):
        return True
    if s.startswith('>'):
        return True
    if _LIST_RE.match(line):
        return True
    if '|' in line and _TBL_SEP_RE.match(line):
        return True
    return False


def parse(md_text):
    """Markdown 文本 -> 块列表。"""
    lines = (md_text or '').replace('\r\n', '\n').replace('\r', '\n').split('\n')
    blocks = []
    i, n = 0, len(lines)

    while i < n:
        line = lines[i]
        stripped = line.strip()
        if not stripped:
            i += 1
            continue

        # 围栏代码块
        m = _FENCE_RE.match(stripped)
        if m:
            fence, lang = m.group(1), m.group(2).strip()
            code = []
            i += 1
            while i < n:
                if lines[i].strip().startswith(fence[0] * len(fence)):
                    i += 1
                    break
                code.append(lines[i])
                i += 1
            blocks.append({'type': 'code', 'lang': lang, 'content': '\n'.join(code)})
            continue

        # 展示型公式 $$...$$
        if stripped.startswith('$$'):
            m2 = re.match(r'^\$\$(.*?)\$\$$', stripped)
            if m2:
                blocks.append({'type': 'math', 'display': True, 'latex': m2.group(1).strip()})
                i += 1
                continue
            buf = []
            i += 1
            while i < n:
                if '$$' in lines[i]:
                    buf.append(lines[i].replace('$$', ''))
                    i += 1
                    break
                buf.append(lines[i])
                i += 1
            blocks.append({'type': 'math', 'display': True, 'latex': '\n'.join(buf).strip()})
            continue

        # 标题
        m = _HEAD_RE.match(line)
        if m:
            blocks.append({'type': 'heading', 'level': len(m.group(1)),
                           'text': parse_inline(m.group(2).strip())})
            i += 1
            continue

        # 表格
        if '|' in line and i + 1 < n and _TBL_SEP_RE.match(lines[i + 1]):
            header = [parse_inline(c) for c in _split_row(line)]
            aligns = _split_align(lines[i + 1])
            rows = []
            i += 2
            while i < n and '|' in lines[i] and lines[i].strip() and not _is_block_start(lines[i]):
                rows.append([parse_inline(c) for c in _split_row(lines[i])])
                i += 1
            blocks.append({'type': 'table', 'header': header, 'rows': rows, 'aligns': aligns})
            continue

        # 分割线
        if _HR_RE.match(stripped):
            blocks.append({'type': 'hr'})
            i += 1
            continue

        # 引用
        if stripped.startswith('>'):
            q = []
            while i < n and lines[i].strip().startswith('>'):
                q.append(re.sub(r'^\s*>\s?', '', lines[i]))
                i += 1
            blocks.append({'type': 'quote', 'blocks': parse('\n'.join(q))})
            continue

        # 列表
        m = _LIST_RE.match(line)
        if m:
            items = []
            while i < n:
                lm = _LIST_RE.match(lines[i])
                if not lm:
                    break
                marker, rest = lm.group(2), lm.group(3)
                ordered = marker[0].isdigit()
                task = False
                checked = False
                tm = _TASK_RE.match(rest)
                if tm:
                    task = True
                    checked = tm.group(1).lower() == 'x'
                    rest = tm.group(2)
                items.append({'text': parse_inline(rest.strip()),
                              'task': task, 'checked': checked, 'ordered': ordered})
                i += 1
            blocks.append({'type': 'list', 'items': items})
            continue

        # 段落（聚合到下一个块起点）
        buf = [line]
        i += 1
        while i < n and lines[i].strip() and not _is_block_start(lines[i]):
            buf.append(lines[i])
            i += 1
        blocks.append({'type': 'paragraph',
                       'text': parse_inline(' '.join(x.strip() for x in buf))})

    return blocks
