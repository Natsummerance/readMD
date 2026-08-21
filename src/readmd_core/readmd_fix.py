# -*- coding: utf-8 -*-
"""ReadMD 自动修正器（纯标准库，零第三方依赖）。

在渲染前对常见 Markdown 错误做保守的启发式修复：
  - 表格：缺失表头分隔行、各行列数不齐、表格外多余竖线
  - 加粗/强调：未闭合的 ** __ *，游离的 *** 与星号
  - 公式：未闭合的 $ $$ \\( \\) \\[ \\] ，货币符号误判保护
  - 标题：# 与文字之间缺空格

所有修复只作用于内存中的渲染文本，绝不改写磁盘原文件。
每次修复都会记录到 FixResult.fixes 中，便于界面展示。
"""

import re

__all__ = ['fix_markdown', 'FixResult']


class FixResult(object):
    __slots__ = ('text', 'fixes', 'stats')

    def __init__(self, text, fixes, stats):
        self.text = text
        self.fixes = fixes
        self.stats = stats


# ---------------------------------------------------------------- 基础工具

_SEP_RE = re.compile(r'^\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?$')
_HR_RE = re.compile(r'^\s*(\*{3,}|-{3,}|_{3,})\s*$')
_LIST_ITEM_RE = re.compile(r'^[*+]\s')
_HEADING_RE = re.compile(r'^(\s*)(#{1,6})([^#\s].*)$')
_LATEX_RE = re.compile(r'[\\^_{}]')


def _escaped(s, p):
    """判断位置 p 的字符是否被反斜杠转义。"""
    bs = 0
    k = p - 1
    while k >= 0 and s[k] == '\\':
        bs += 1
        k -= 1
    return bs % 2 == 1


def _unescaped_positions(s, sub):
    """返回 s 中所有未转义 sub 的起始位置（非重叠）。"""
    res = []
    i = 0
    while True:
        j = s.find(sub, i)
        if j == -1:
            break
        if not _escaped(s, j):
            res.append(j)
        i = j + len(sub)
    return res


def _escape_delim(s, d, pos):
    """把位置 pos 处的分隔符 d 转义为字面量（如 ** -> \\*\\*）。"""
    esc = '\\' + '\\'.join(d)
    return s[:pos] + esc + s[pos + len(d):]


def _escape_at(s, pos, ch):
    return s[:pos] + '\\' + ch + s[pos + 1:]


def _escape_all_unescaped(s, ch):
    out = []
    i = 0
    while i < len(s):
        if s[i] == ch and not _escaped(s, i):
            out.append('\\' + ch)
        else:
            out.append(s[i])
        i += 1
    return ''.join(out)


def _has_latex(s):
    return bool(_LATEX_RE.search(s))


def _looks_like_code_indent(line):
    """4+ 空格 / Tab 开头的缩进代码块（列表项与引用除外）。"""
    if line.startswith('    ') or line.startswith('\t'):
        s = line.lstrip()
        if s.startswith(('> ', '>-')):
            return False
        if re.match(r'^([-*+]|\d+\.)\s', s):
            return False
        return True
    return False


# ---------------------------------------------------------------- 代码遮蔽

def mask_code_spans(s):
    """把行内代码 `` `...` `` 替换为占位符，避免后续修复误伤。"""
    spans = []
    out = []
    i = 0
    n = len(s)
    idx = 0
    while i < n:
        if s[i] != '`':
            j = s.find('`', i)
            if j == -1:
                out.append(s[i:])
                break
            out.append(s[i:j])
            i = j
        j = i
        while j < n and s[j] == '`':
            j += 1
        run_len = j - i
        m = j
        found = -1
        while m < n:
            k = s.find('`', m)
            if k == -1:
                break
            e = k
            while e < n and s[e] == '`':
                e += 1
            if e - k >= run_len:
                found = e
                break
            m = e
        if found != -1:
            ph = '\x1aC%d\x1a' % idx
            spans.append((ph, s[i:found]))
            out.append(ph)
            i = found
            idx += 1
        else:
            out.append(s[i:j])
            i = j
    return ''.join(out), spans


_FENCE_RE = re.compile(r'^(\s{0,3})(`{3,}|~{3,})(.*)$')


def mask_all_code(text):
    """遮蔽围栏代码块与行内代码，返回 (遮蔽文本, 占位符列表)。"""
    lines = text.split('\n')
    spans = []
    out = []
    i = 0
    n = len(lines)
    fence = None  # (char, runlen)
    while i < n:
        line = lines[i]
        if fence:
            m = _FENCE_RE.match(line)
            if m and m.group(2)[0] == fence[0] and len(m.group(2)) >= fence[1]:
                fence = None
            ph = '\x1aF%d\x1a' % len(spans)
            spans.append((ph, line))
            out.append(ph)
            i += 1
            continue
        m = _FENCE_RE.match(line)
        if m and m.group(2)[0] in ('`', '~'):
            fence = (m.group(2)[0], len(m.group(2)))
            ph = '\x1aF%d\x1a' % len(spans)
            spans.append((ph, line))
            out.append(ph)
            i += 1
            continue
        masked_line, line_spans = mask_code_spans(line)
        spans.extend(line_spans)
        out.append(masked_line)
        i += 1
    return '\n'.join(out), spans


def restore(text, spans):
    for ph, orig in spans:
        text = text.replace(ph, orig)
    return text


# ---------------------------------------------------------------- 表格修复

def _is_table_sep(line):
    s = line.strip()
    if '|' not in s or len(s) < 3:
        return False
    if not _SEP_RE.match(s):
        return False
    return bool(re.search(r'-{2,}', s))


def _is_table_row(line):
    if '|' not in line:
        return False
    s = line
    if s.lstrip().startswith('|'):
        return True
    if s.count('|') >= 2:
        return True
    return (' | ' in s) or ('| ' in s) or (' |' in s)


def _split_cells(row):
    """按未转义的 | 切分单元格；`\\|` 视为单元格内的竖线。"""
    cells = []
    cur = []
    i = 0
    while i < len(row):
        ch = row[i]
        if ch == '\\' and i + 1 < len(row) and row[i + 1] == '|':
            cur.append('|')
            i += 2
            continue
        if ch == '|':
            cells.append(''.join(cur))
            cur = []
            i += 1
            continue
        cur.append(ch)
        i += 1
    cells.append(''.join(cur))
    if cells and cells[0].strip() == '' and row.lstrip().startswith('|'):
        cells = cells[1:]
    if cells and cells[-1].strip() == '' and row.rstrip().endswith('|'):
        cells = cells[:-1]
    return cells


def _rebuild_row(cells):
    return '| ' + ' | '.join(cells) + ' |'


def _escape_cell(c):
    return c.replace('|', '\\|')


def _norm_sep_cell(c):
    """分隔行单元格补足 3 个连字符（GFM 要求），保留对齐冒号。"""
    m = re.match(r'^(:?)(-+)(:?)$', c)
    if m and len(m.group(2)) < 3:
        return m.group(1) + '---' + m.group(3)
    return c


def _normalize_table(block, start_no, fixes):
    indent = re.match(r'^\s*', block[0]).group(0) if block else ''
    sep_idx = [k for k, l in enumerate(block) if _is_table_sep(l)]
    data_idx = [k for k, l in enumerate(block) if not _is_table_sep(l) and _is_table_row(l)]
    if not data_idx:
        return block, False
    all_cells = [_split_cells(block[k]) for k in data_idx] + \
                [_split_cells(block[k]) for k in sep_idx]
    max_cols = max([len(c) for c in all_cells] or [1])
    max_cols = max(max_cols, 1)

    changed = False
    row_diffs = 0
    new_block = []
    for k in range(len(block)):
        l = block[k]
        if _is_table_sep(l):
            cells = [_norm_sep_cell(c.strip()) for c in _split_cells(l)]
            if len(cells) < max_cols:
                cells = cells + ['---'] * (max_cols - len(cells))
                changed = True
            elif len(cells) > max_cols:
                cells = cells[:max_cols]
                changed = True
            new_block.append(indent + _rebuild_row(cells))
        elif _is_table_row(l):
            cells = [c.strip() for c in _split_cells(l)]
            if len(cells) < max_cols:
                cells = cells + [''] * (max_cols - len(cells))
                changed = True
                row_diffs += 1
            elif len(cells) > max_cols:
                extra = cells[max_cols - 1:]
                cells = cells[:max_cols - 1] + [' '.join(extra)]
                changed = True
                row_diffs += 1
            new_block.append(indent + _rebuild_row([_escape_cell(c) for c in cells]))
        else:
            new_block.append(l)

    if new_block != block:
        changed = True
    if not sep_idx and len(data_idx) >= 1:
        sep_line = indent + _rebuild_row(['---'] * max_cols)
        new_block.insert(1, sep_line)
        changed = True
        fixes.append('[表格] 第 %d 行附近：缺少表头分隔行，已自动补全' % start_no)
    if row_diffs:
        fixes.append('[表格] 第 %d-%d 行：%d 行列数不齐，已对齐为 %d 列' %
                     (start_no, start_no + len(block) - 1, row_diffs, max_cols))
    return new_block, changed


def _process_tables(lines, fixes, stats):
    n = len(lines)
    i = 0
    while i < n:
        l = lines[i]
        if _looks_like_code_indent(l) or l.lstrip().startswith('>'):
            i += 1
            continue
        if not (_is_table_row(l) or _is_table_sep(l)):
            i += 1
            continue
        j = i
        while j < n:
            lj = lines[j]
            if _looks_like_code_indent(lj) or lj.lstrip().startswith('>'):
                break
            if _is_table_row(lj) or _is_table_sep(lj):
                j += 1
            else:
                break
        block = lines[i:j]
        data_count = sum(1 for x in block if _is_table_row(x))
        has_sep = any(_is_table_sep(x) for x in block)
        if data_count == 0:
            i = j
            continue
        if data_count == 1 and not has_sep and not block[0].lstrip().startswith('|'):
            i = j
            continue
        new_block, changed = _normalize_table(block, i + 1, fixes)
        if changed:
            stats['table'] += 1
            lines[i:j] = new_block
            j = i + len(new_block)
        i = j
    return lines


# ---------------------------------------------------------------- 加粗/强调修复

_PUNCT_CHARS = set('([{\'"-\u2013\u2014:;,!?/\\|~`@#$%^&*+=<>~（）【】《》“”‘’，。、；：？！…—·「」『』')


def _is_punct_or_space(ch):
    if not ch or ch.isspace():
        return True
    if ch in _PUNCT_CHARS:
        return True
    import unicodedata
    cat = unicodedata.category(ch)
    return cat.startswith(('P', 'S', 'Z'))


def _classify(s, p, d, allow_intraword):
    before = s[p - 1] if p > 0 else ''
    after = s[p + len(d)] if p + len(d) < len(s) else ''
    prev_boundary = _is_punct_or_space(before)
    next_boundary = _is_punct_or_space(after)
    is_open = prev_boundary and not (after and after.isspace())
    is_close = next_boundary and not (before and before.isspace())
    if is_open and not is_close:
        return 'open'
    if is_close and not is_open:
        return 'close'
    if is_open and is_close:
        return 'both'
    return 'open' if allow_intraword else 'word'


def _balance_delim(s, d, allow_intraword=False):
    """平衡单个分隔符 d（** __ * 等），返回 (修正后字符串, 日志)。"""
    if s.strip() == d:
        return _escape_delim(s, d, 0), ['转义多余的 %s' % d]
    pos = _unescaped_positions(s, d)
    if not pos:
        return s, []
    opens = []
    strays = []
    for p in pos:
        kind = _classify(s, p, d, allow_intraword)
        if kind == 'open':
            opens.append(p)
        elif kind == 'close':
            if opens:
                opens.pop()
            else:
                strays.append(p)
        elif kind == 'both':
            if opens:
                opens.pop()
            else:
                strays.append(p)
        else:  # word
            # 单词内分隔符（如 foo__bar、2**3）保持原样，仅配对已有 opener
            if opens:
                opens.pop()
    if not opens and not strays:
        return s, []
    log = []
    if opens and len(opens) % 2 == 0 and strays:
        p = opens.pop()
        s = _escape_delim(s, d, p)
        log.append('转义多余的 %s' % d)
    if opens and len(opens) % 2 == 1:
        s = s.rstrip() + d
        log.append('补全未闭合的 %s' % d)
    for p in reversed(strays):
        s = _escape_delim(s, d, p)
        log.append('转义多余的 %s' % d)
    return s, log


def mask_pairs(s, d, prefix):
    """把已配对的 d...d 整体遮蔽（非嵌套配对），返回 (新串, 占位符)。"""
    pos = _unescaped_positions(s, d)
    if not pos or len(pos) % 2 != 0:
        return s, []
    open_p = None
    pairs = []
    for p in pos:
        kind = _classify(s, p, d, False)
        if kind == 'open':
            open_p = p
        elif kind == 'close':
            if open_p is not None:
                pairs.append((open_p, p))
                open_p = None
        elif kind == 'both':
            if open_p is not None:
                pairs.append((open_p, p))
                open_p = None
        else:  # word
            if open_p is not None:
                pairs.append((open_p, p))
                open_p = None
    if not pairs:
        return s, []
    pairs.sort(reverse=True)
    spans = []
    for i, (a, b) in enumerate(pairs):
        ph = '\x1a%s%d\x1a' % (prefix, i)
        spans.append((ph, s[a:b + len(d)]))
        s = s[:a] + ph + s[b + len(d):]
    return s, spans


def fix_emphasis_line(line):
    """修复单行加粗/强调符号，返回 (修正后字符串, 日志)。"""
    if _HR_RE.match(line):
        return line, []
    log = []
    s = line
    all_spans = []

    s, l = _balance_delim(s, '***')
    log += l
    s, sp = mask_pairs(s, '***', '3')
    all_spans += sp

    s, l = _balance_delim(s, '**')
    log += l
    s, sp = mask_pairs(s, '**', '2')
    all_spans += sp

    s, l = _balance_delim(s, '__')
    log += l
    s, sp = mask_pairs(s, '__', 'u')
    all_spans += sp

    if not _LIST_ITEM_RE.match(s.lstrip()):
        s, l = _balance_delim(s, '*')
        log += l

    for ph, orig in reversed(all_spans):
        s = s.replace(ph, orig)
    return s, log


def _process_emphasis(lines, fixes, stats):
    for idx, line in enumerate(lines):
        if _looks_like_code_indent(line):
            continue
        fixed, log = fix_emphasis_line(line)
        if log:
            lines[idx] = fixed
            for m in log:
                fixes.append('[加粗] 第 %d 行：%s' % (idx + 1, m))
            stats['bold'] += len(log)
    return lines


# ---------------------------------------------------------------- 公式修复

def _balance_display_math(text):
    new_text = re.sub(r'\$\$\s*\n\s*\$\$', '$$', text)
    if new_text != text:
        return new_text, ['移除空白的 $$ 公式块']
    pos = _unescaped_positions(text, '$$')
    if not pos or len(pos) % 2 == 0:
        return text, []
    last = pos[-1]
    if len(pos[:-1]) % 2 == 1:
        text = _escape_delim(text, '$$', last)
        return text, ['转义多余的块级 $$']
    # last 是未闭合的 opener：把闭合符加在紧随的段落末尾（首个空行/代码前）
    after = text[last + 2:]
    lines = after.split('\n')
    # lines[0] == '' 表示 $$ 后直接换行（块级公式的典型写法）
    content = lines[1:] if lines and lines[0] == '' else lines
    if not after.strip():
        # 后面没有内容 → 直接闭合
        text = text[:last + 2] + '\n$$\n'
    elif not content or not content[0].strip():
        # 紧跟空行 → 空公式块，立即闭合
        text = text[:last + 2] + '\n$$\n' + after.lstrip('\n')
    else:
        n = 0
        for l in content:
            if l.strip() == '' or l.startswith('\x1aF'):
                break
            n += 1
        head = lines[:1 + n]
        rest = lines[1 + n:]
        after2 = '\n'.join(head) + '\n$$\n' + ('\n'.join(rest) + '\n' if rest else '')
        text = text[:last + 2] + after2
    return text, ['补全未闭合的块级公式 $$']


def _fix_math_line(line):
    log = []
    s = line
    for op, cl, name in (('\\(', '\\)', '\\( ... \\)'),
                         ('\\[', '\\]', '\\[ ... \\]')):
        o = s.count(op)
        c = s.count(cl)
        if o > c and _has_latex(s):
            s = s.rstrip() + cl
            log.append('补全未闭合的 %s' % name)
        elif c > o:
            p = s.find(cl)
            if p >= 0:
                s = s[:p] + '\\\\' + s[p + 1:]
                log.append('转义多余的 %s' % cl)
    dollars = [p for p, ch in enumerate(s) if ch == '$' and not _escaped(s, p)]
    if len(dollars) % 2 == 1:
        if _has_latex(s):
            p0 = dollars[0]
            after = s[p0 + 1] if p0 + 1 < len(s) else ''
            if after and not after.isspace():
                s = s.rstrip() + '$'
                log.append('补全未闭合的行内公式 $')
            else:
                s = _escape_at(s, p0, '$')
                log.append('转义多余的 $')
        else:
            s = _escape_all_unescaped(s, '$')
            log.append('转义疑似货币的 $')
    return s, log


def _process_math(lines, fixes, stats):
    text = '\n'.join(lines)
    text, log = _balance_display_math(text)
    if log:
        fixes.append('[公式] %s' % log[0])
        stats['math'] += 1
    lines = text.split('\n')
    for idx, line in enumerate(lines):
        if _looks_like_code_indent(line):
            continue
        fixed, log2 = _fix_math_line(line)
        if log2:
            lines[idx] = fixed
            for m in log2:
                fixes.append('[公式] 第 %d 行：%s' % (idx + 1, m))
            stats['math'] += len(log2)
    return lines


# ---------------------------------------------------------------- 标题修复

def _process_headings(lines, fixes, stats):
    for idx, line in enumerate(lines):
        if _looks_like_code_indent(line):
            continue
        m = _HEADING_RE.match(line)
        if m:
            lines[idx] = m.group(1) + m.group(2) + ' ' + m.group(3)
            fixes.append('[标题] 第 %d 行：# 后缺少空格，已补全' % (idx + 1))
            stats['heading'] += 1
    return lines


# ---------------------------------------------------------------- 主入口

def fix_markdown(text):
    """对整篇 Markdown 文本做自动修正，返回 FixResult。"""
    fixes = []
    stats = {'table': 0, 'bold': 0, 'math': 0, 'heading': 0, 'misc': 0}
    if text.startswith('\ufeff'):
        text = text[1:]
        stats['misc'] += 1
        fixes.append('[通用] 已去除 UTF-8 BOM')
    text = text.replace('\r\n', '\n').replace('\r', '\n')
    masked, spans = mask_all_code(text)
    lines = masked.split('\n')
    lines = _process_tables(lines, fixes, stats)
    lines = _process_headings(lines, fixes, stats)
    lines = _process_emphasis(lines, fixes, stats)
    lines = _process_math(lines, fixes, stats)
    out = restore('\n'.join(lines), spans)
    return FixResult(out, fixes, stats)