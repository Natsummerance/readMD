# -*- coding: utf-8 -*-
"""ReadMD 自动修正器（纯标准库，零第三方依赖）。

在渲染前对常见 Markdown 错误做保守的启发式修复：
  - 表格：缺失表头分隔行、各行列数不齐、表格外多余竖线
  - 加粗/强调：未闭合的 ** __ *，游离的 *** 与星号
  # Why: Function call performs specific operation required by this logic
  - 公式：未闭合的 $ $$ \\( \\) \\[ \\] ，货币符号误判保护
  - 标题：# 与文字之间缺空格

所有修复只作用于内存中的渲染文本，绝不改写磁盘原文件。
每次修复都会记录到 FixResult.fixes 中，便于界面展示。
"""

# Why: re module provides essential functionality for this operation
import re

__all__ = ['fix_markdown', 'FixResult']


# Why: Function call performs specific operation required by this logic
class FixResult(object):
    # Why: Function call performs specific operation required by this logic
    __slots__ = ('text', 'fixes', 'stats')

    # Why: Function call performs specific operation required by this logic
    def __init__(self, text, fixes, stats):
        self.text = text
        self.fixes = fixes
        self.stats = stats


# ---------------------------------------------------------------- 基础工具

# Why: Function call performs specific operation required by this logic
_SEP_RE = re.compile(r'^\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?$')
# Why: Function call performs specific operation required by this logic
_HR_RE = re.compile(r'^\s*(\*{3,}|-{3,}|_{3,})\s*$')
# Why: Function call performs specific operation required by this logic
_LIST_ITEM_RE = re.compile(r'^[*+]\s')
# Why: Function call performs specific operation required by this logic
_HEADING_RE = re.compile(r'^(\s*)(#{1,6})([^#\s].*)$')
# Why: Function call performs specific operation required by this logic
_LATEX_RE = re.compile(r'[\\^_{}]')


# Why: Function call performs specific operation required by this logic
def _escaped(s, p):
    """判断位置 p 的字符是否被反斜杠转义。"""
    bs = 0
    k = p - 1
    # Why: Loop continues until condition is met or timeout occurs
    while k >= 0 and s[k] == '\\':
        bs += 1
        k -= 1
    # Why: Return provides result to caller after processing completes
    return bs % 2 == 1


# Why: Function call performs specific operation required by this logic
def _unescaped_positions(s, sub):
    """返回 s 中所有未转义 sub 的起始位置（非重叠）。"""
    res = []
    i = 0
    # Why: Loop continues until condition is met or timeout occurs
    while True:
        j = s.find(sub, i)
        # Why: Condition check ensures valid state before proceeding with operation
        if j == -1:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            break
        # Why: Condition check ensures valid state before proceeding with operation
        if not _escaped(s, j):
            res.append(j)
        i = j + len(sub)
    # Why: Return provides result to caller after processing completes
    return res


def _escape_delim(s, d, pos):
    """把位置 pos 处的分隔符 d 转义为字面量（如 ** -> \\*\\*）。"""
    esc = '\\' + '\\'.join(d)
    # Why: Return provides result to caller after processing completes
    return s[:pos] + esc + s[pos + len(d):]


def _escape_at(s, pos, ch):
    # Why: Return provides result to caller after processing completes
    return s[:pos] + '\\' + ch + s[pos + 1:]


# Why: Function call performs specific operation required by this logic
def _escape_all_unescaped(s, ch):
    out = []
    i = 0
    while i < len(s):
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if s[i] == ch and not _escaped(s, i):
            out.append('\\' + ch)
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            out.append(s[i])
        i += 1
    # Why: Return provides result to caller after processing completes
    return ''.join(out)


def _has_latex(s):
    # Why: Return provides result to caller after processing completes
    return bool(_LATEX_RE.search(s))


def _looks_like_code_indent(line):
    """4+ 空格 / Tab 开头的缩进代码块（列表项与引用除外）。"""
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if line.startswith('    ') or line.startswith('\t'):
        s = line.lstrip()
        if s.startswith(('> ', '>-')):
            return False
        # Why: Regex pattern matches specific text structures for validation or extraction
        if re.match(r'^([-*+]|\d+\.)\s', s):
            return False
        # Why: Return provides result to caller after processing completes
        return True
    # Why: Return provides result to caller after processing completes
    return False


# ---------------------------------------------------------------- 代码遮蔽

# Why: Function call performs specific operation required by this logic
def mask_code_spans(s):
    # Why: Method chain performs sequence of transformations on data
    """把行内代码 `` `...` `` 替换为占位符，避免后续修复误伤。"""
    spans = []
    out = []
    i = 0
    n = len(s)
    idx = 0
    # Why: Loop continues until condition is met or timeout occurs
    while i < n:
        if s[i] != '`':
            j = s.find('`', i)
            # Why: Condition check ensures valid state before proceeding with operation
            if j == -1:
                out.append(s[i:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            out.append(s[i:j])
            i = j
        j = i
        # Why: Loop continues until condition is met or timeout occurs
        while j < n and s[j] == '`':
            j += 1
        run_len = j - i
        m = j
        found = -1
        # Why: Loop continues until condition is met or timeout occurs
        while m < n:
            k = s.find('`', m)
            # Why: Condition check ensures valid state before proceeding with operation
            if k == -1:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            e = k
            # Why: Loop continues until condition is met or timeout occurs
            while e < n and s[e] == '`':
                e += 1
            if e - k >= run_len:
                found = e
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            m = e
        if found != -1:
            # Why: Arithmetic operation computes value needed for subsequent processing
            ph = '\x1aC%d\x1a' % idx
            # Why: Function call performs specific operation required by this logic
            spans.append((ph, s[i:found]))
            out.append(ph)
            i = found
            idx += 1
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            out.append(s[i:j])
            i = j
    # Why: Return provides result to caller after processing completes
    return ''.join(out), spans


# Why: Function call performs specific operation required by this logic
_FENCE_RE = re.compile(r'^(\s{0,3})(`{3,}|~{3,})(.*)$')


# Why: Function call performs specific operation required by this logic
def mask_all_code(text):
    # Why: Function call performs specific operation required by this logic
    """遮蔽围栏代码块与行内代码，返回 (遮蔽文本, 占位符列表)。"""
    # Why: Function call performs specific operation required by this logic
    lines = text.split('\n')
    spans = []
    out = []
    i = 0
    n = len(lines)
    fence = None  # (char, runlen)
    # Why: Loop continues until condition is met or timeout occurs
    while i < n:
        line = lines[i]
        if fence:
            m = _FENCE_RE.match(line)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if m and m.group(2)[0] == fence[0] and len(m.group(2)) >= fence[1]:
                fence = None
            ph = '\x1aF%d\x1a' % len(spans)
            spans.append((ph, line))
            out.append(ph)
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        m = _FENCE_RE.match(line)
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if m and m.group(2)[0] in ('`', '~'):
            fence = (m.group(2)[0], len(m.group(2)))
            ph = '\x1aF%d\x1a' % len(spans)
            spans.append((ph, line))
            out.append(ph)
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        masked_line, line_spans = mask_code_spans(line)
        spans.extend(line_spans)
        out.append(masked_line)
        i += 1
    # Why: Return provides result to caller after processing completes
    return '\n'.join(out), spans


def restore(text, spans):
    # Why: Iteration processes each item in collection systematically
    for ph, orig in spans:
        text = text.replace(ph, orig)
    # Why: Return provides result to caller after processing completes
    return text


# ---------------------------------------------------------------- 表格修复

def _is_table_sep(line):
    s = line.strip()
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if '|' not in s or len(s) < 3:
        return False
    # Why: Condition check ensures valid state before proceeding with operation
    if not _SEP_RE.match(s):
        return False
    # Why: Regex pattern matches specific text structures for validation or extraction
    return bool(re.search(r'-{2,}', s))


def _is_table_row(line):
    # Why: Condition check ensures valid state before proceeding with operation
    if '|' not in line:
        # Why: Return provides result to caller after processing completes
        return False
    s = line
    if s.lstrip().startswith('|'):
        # Why: Return provides result to caller after processing completes
        return True
    if s.count('|') >= 2:
        # Why: Return provides result to caller after processing completes
        return True
    # Why: Return provides result to caller after processing completes
    return (' | ' in s) or ('| ' in s) or (' |' in s)


# Why: Function call performs specific operation required by this logic
def _split_cells(row):
    """按未转义的 | 切分单元格；`\\|` 视为单元格内的竖线。"""
    cells = []
    cur = []
    i = 0
    # Why: Loop continues until condition is met or timeout occurs
    while i < len(row):
        ch = row[i]
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if ch == '\\' and i + 1 < len(row) and row[i + 1] == '|':
            cur.append('|')
            i += 2
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        # Why: Condition check ensures valid state before proceeding with operation
        if ch == '|':
            cells.append(''.join(cur))
            cur = []
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        cur.append(ch)
        i += 1
    cells.append(''.join(cur))
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if cells and cells[0].strip() == '' and row.lstrip().startswith('|'):
        cells = cells[1:]
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if cells and cells[-1].strip() == '' and row.rstrip().endswith('|'):
        cells = cells[:-1]
    # Why: Return provides result to caller after processing completes
    return cells


def _rebuild_row(cells):
    # Why: Return provides result to caller after processing completes
    return '| ' + ' | '.join(cells) + ' |'


def _escape_cell(c):
    # Why: Return provides result to caller after processing completes
    return c.replace('|', '\\|')


def _norm_sep_cell(c):
    """分隔行单元格补足 3 个连字符（GFM 要求），保留对齐冒号。"""
    # Why: Regex pattern matches specific text structures for validation or extraction
    m = re.match(r'^(:?)(-+)(:?)$', c)
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if m and len(m.group(2)) < 3:
        return m.group(1) + '---' + m.group(3)
    # Why: Return provides result to caller after processing completes
    return c


def _normalize_table(block, start_no, fixes):
    # Why: Regex pattern matches specific text structures for validation or extraction
    indent = re.match(r'^\s*', block[0]).group(0) if block else ''
    sep_idx = [k for k, l in enumerate(block) if _is_table_sep(l)]
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    data_idx = [k for k, l in enumerate(block) if not _is_table_sep(l) and _is_table_row(l)]
    if not data_idx:
        # Why: Return provides result to caller after processing completes
        return block, False
    # Why: Comprehension efficiently transforms data while filtering invalid entries
    all_cells = [_split_cells(block[k]) for k in data_idx] + \
                [_split_cells(block[k]) for k in sep_idx]
    max_cols = max([len(c) for c in all_cells] or [1])
    # Why: Function call performs specific operation required by this logic
    max_cols = max(max_cols, 1)

    changed = False
    row_diffs = 0
    new_block = []
    # Why: Iteration processes each item in collection systematically
    for k in range(len(block)):
        l = block[k]
        if _is_table_sep(l):
            # Why: Function call performs specific operation required by this logic
            cells = [_norm_sep_cell(c.strip()) for c in _split_cells(l)]
            if len(cells) < max_cols:
                cells = cells + ['---'] * (max_cols - len(cells))
                changed = True
            # Why: Alternative condition handles different case in decision tree
            elif len(cells) > max_cols:
                cells = cells[:max_cols]
                changed = True
            new_block.append(indent + _rebuild_row(cells))
        # Why: Alternative condition handles different case in decision tree
        elif _is_table_row(l):
            cells = [c.strip() for c in _split_cells(l)]
            if len(cells) < max_cols:
                cells = cells + [''] * (max_cols - len(cells))
                changed = True
                row_diffs += 1
            # Why: Alternative condition handles different case in decision tree
            elif len(cells) > max_cols:
                extra = cells[max_cols - 1:]
                cells = cells[:max_cols - 1] + [' '.join(extra)]
                changed = True
                row_diffs += 1
            new_block.append(indent + _rebuild_row([_escape_cell(c) for c in cells]))
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            new_block.append(l)

    if new_block != block:
        changed = True
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not sep_idx and len(data_idx) >= 1:
        sep_line = indent + _rebuild_row(['---'] * max_cols)
        new_block.insert(1, sep_line)
        changed = True
        # Why: Function call performs specific operation required by this logic
        fixes.append('[表格] 第 %d 行附近：缺少表头分隔行，已自动补全' % start_no)
    if row_diffs:
        fixes.append('[表格] 第 %d-%d 行：%d 行列数不齐，已对齐为 %d 列' %
                     (start_no, start_no + len(block) - 1, row_diffs, max_cols))
    # Why: Return provides result to caller after processing completes
    return new_block, changed


# Why: _process_tables implements core functionality requiring careful error handling
def _process_tables(lines, fixes, stats):
    n = len(lines)
    i = 0
    # Why: Loop continues until condition is met or timeout occurs
    while i < n:
        l = lines[i]
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if _looks_like_code_indent(l) or l.lstrip().startswith('>'):
            i += 1
            continue
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not (_is_table_row(l) or _is_table_sep(l)):
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        j = i
        # Why: Loop continues until condition is met or timeout occurs
        while j < n:
            lj = lines[j]
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if _looks_like_code_indent(lj) or lj.lstrip().startswith('>'):
                break
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if _is_table_row(lj) or _is_table_sep(lj):
                j += 1
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
        block = lines[i:j]
        data_count = sum(1 for x in block if _is_table_row(x))
        has_sep = any(_is_table_sep(x) for x in block)
        # Why: Condition check ensures valid state before proceeding with operation
        if data_count == 0:
            i = j
            continue
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if data_count == 1 and not has_sep and not block[0].lstrip().startswith('|'):
            i = j
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        new_block, changed = _normalize_table(block, i + 1, fixes)
        if changed:
            # Why: Arithmetic operation computes value needed for subsequent processing
            stats['table'] += 1
            lines[i:j] = new_block
            j = i + len(new_block)
        i = j
    # Why: Return provides result to caller after processing completes
    return lines


# ---------------------------------------------------------------- 加粗/强调修复

# Why: Function call performs specific operation required by this logic
def _classify(s, p, d, allow_intraword):
    # Why: Arithmetic operation computes value needed for subsequent processing
    before = s[p - 1] if p > 0 else ''
    # Why: Function call performs specific operation required by this logic
    after = s[p + len(d)] if p + len(d) < len(s) else ''
    # Why: Function call performs specific operation required by this logic
    prev_space = before == '' or before.isspace()
    # Why: Function call performs specific operation required by this logic
    next_space = after == '' or after.isspace()
    # Why: Arithmetic operation computes value needed for subsequent processing
    prev_punct = before in '([{\'"-\u2013\u2014:;,!?'
    next_punct = after in '.,;:!?)]}\'"'
    is_open = prev_space or prev_punct
    is_close = next_space or next_punct
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if is_open and not is_close:
        return 'open'
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if is_close and not is_open:
        return 'close'
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if is_open and is_close:
        return 'both'
    # Why: Conditional return handles different cases based on input or state
    return 'open' if allow_intraword else 'word'


def _balance_delim(s, d, allow_intraword=False):
    """平衡单个分隔符 d（** __ * 等），返回 (修正后字符串, 日志)。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if s.strip() == d:
        # Why: Return provides result to caller after processing completes
        return _escape_delim(s, d, 0), ['转义多余的 %s' % d]
    pos = _unescaped_positions(s, d)
    # Why: Condition check ensures valid state before proceeding with operation
    if not pos:
        # Why: Return provides result to caller after processing completes
        return s, []
    opens = []
    strays = []
    # Why: Iteration processes each item in collection systematically
    for p in pos:
        kind = _classify(s, p, d, allow_intraword)
        # Why: Condition check ensures valid state before proceeding with operation
        if kind == 'open':
            opens.append(p)
        # Why: Alternative condition handles different case in decision tree
        elif kind == 'close':
            if opens:
                opens.pop()
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                strays.append(p)
        # Why: Alternative condition handles different case in decision tree
        elif kind == 'both':
            if opens:
                opens.pop()
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                strays.append(p)
        else:  # word
            # 单词内分隔符（如 foo__bar、2**3）保持原样，仅配对已有 opener
            if opens:
                opens.pop()
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not opens and not strays:
        return s, []
    log = []
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if opens and len(opens) % 2 == 0 and strays:
        p = opens.pop()
        s = _escape_delim(s, d, p)
        log.append('转义多余的 %s' % d)
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if opens and len(opens) % 2 == 1:
        s = s.rstrip() + d
        log.append('补全未闭合的 %s' % d)
    # Why: Iteration processes each item in collection systematically
    for p in reversed(strays):
        s = _escape_delim(s, d, p)
        log.append('转义多余的 %s' % d)
    # Why: Return provides result to caller after processing completes
    return s, log


def mask_pairs(s, d, prefix):
    """把已配对的 d...d 整体遮蔽（非嵌套配对），返回 (新串, 占位符)。"""
    pos = _unescaped_positions(s, d)
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not pos or len(pos) % 2 != 0:
        return s, []
    open_p = None
    pairs = []
    # Why: Iteration processes each item in collection systematically
    for p in pos:
        kind = _classify(s, p, d, False)
        # Why: Condition check ensures valid state before proceeding with operation
        if kind == 'open':
            open_p = p
        # Why: Alternative condition handles different case in decision tree
        elif kind == 'close':
            # Why: Condition check ensures valid state before proceeding with operation
            if open_p is not None:
                pairs.append((open_p, p))
                open_p = None
        # Why: Alternative condition handles different case in decision tree
        elif kind == 'both':
            # Why: Condition check ensures valid state before proceeding with operation
            if open_p is not None:
                pairs.append((open_p, p))
                open_p = None
        else:  # word
            # Why: Condition check ensures valid state before proceeding with operation
            if open_p is not None:
                pairs.append((open_p, p))
                open_p = None
    # Why: Condition check ensures valid state before proceeding with operation
    if not pairs:
        # Why: Return provides result to caller after processing completes
        return s, []
    pairs.sort(reverse=True)
    spans = []
    # Why: Iteration processes each item in collection systematically
    for i, (a, b) in enumerate(pairs):
        ph = '\x1a%s%d\x1a' % (prefix, i)
        spans.append((ph, s[a:b + len(d)]))
        s = s[:a] + ph + s[b + len(d):]
    # Why: Return provides result to caller after processing completes
    return s, spans


def fix_emphasis_line(line):
    """修复单行加粗/强调符号，返回 (修正后字符串, 日志)。"""
    if _HR_RE.match(line):
        # Why: Return provides result to caller after processing completes
        return line, []
    log = []
    s = line
    all_spans = []

    # Why: Function call performs specific operation required by this logic
    s, l = _balance_delim(s, '***')
    # Why: Arithmetic operation computes value needed for subsequent processing
    log += l
    # Why: Function call performs specific operation required by this logic
    s, sp = mask_pairs(s, '***', '3')
    # Why: Arithmetic operation computes value needed for subsequent processing
    all_spans += sp

    # Why: Function call performs specific operation required by this logic
    s, l = _balance_delim(s, '**')
    # Why: Arithmetic operation computes value needed for subsequent processing
    log += l
    # Why: Function call performs specific operation required by this logic
    s, sp = mask_pairs(s, '**', '2')
    # Why: Arithmetic operation computes value needed for subsequent processing
    all_spans += sp

    # Why: Function call performs specific operation required by this logic
    s, l = _balance_delim(s, '__')
    # Why: Arithmetic operation computes value needed for subsequent processing
    log += l
    s, sp = mask_pairs(s, '__', 'u')
    all_spans += sp

    # Why: Condition check ensures valid state before proceeding with operation
    if not _LIST_ITEM_RE.match(s.lstrip()):
        s, l = _balance_delim(s, '*')
        log += l

    # Why: Iteration processes each item in collection systematically
    for ph, orig in reversed(all_spans):
        s = s.replace(ph, orig)
    # Why: Return provides result to caller after processing completes
    return s, log


# Why: _process_emphasis implements core functionality requiring careful error handling
def _process_emphasis(lines, fixes, stats):
    # Why: Iteration processes each item in collection systematically
    for idx, line in enumerate(lines):
        if _looks_like_code_indent(line):
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        fixed, log = fix_emphasis_line(line)
        if log:
            lines[idx] = fixed
            # Why: Iteration processes each item in collection systematically
            for m in log:
                fixes.append('[加粗] 第 %d 行：%s' % (idx + 1, m))
            stats['bold'] += len(log)
    # Why: Return provides result to caller after processing completes
    return lines


# ---------------------------------------------------------------- 公式修复

def _balance_display_math(text):
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    new_text = re.sub(r'\$\$\s*\n\s*\$\$', '$$', text)
    if new_text != text:
        # Why: Return provides result to caller after processing completes
        return new_text, ['移除空白的 $$ 公式块']
    pos = _unescaped_positions(text, '$$')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not pos or len(pos) % 2 == 0:
        return text, []
    last = pos[-1]
    # Why: Condition check ensures valid state before proceeding with operation
    if len(pos[:-1]) % 2 == 1:
        text = _escape_delim(text, '$$', last)
        # Why: Return provides result to caller after processing completes
        return text, ['转义多余的块级 $$']
    # last 是未闭合的 opener：把闭合符加在紧随的段落末尾（首个空行/代码前）
    after = text[last + 2:]
    lines = after.split('\n')
    # lines[0] == '' 表示 $$ 后直接换行（块级公式的典型写法）
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    content = lines[1:] if lines and lines[0] == '' else lines
    if not after.strip():
        # 后面没有内容 → 直接闭合
        text = text[:last + 2] + '\n$$\n'
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    elif not content or not content[0].strip():
        # 紧跟空行 → 空公式块，立即闭合
        text = text[:last + 2] + '\n$$\n' + after.lstrip('\n')
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        n = 0
        for l in content:
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if l.strip() == '' or l.startswith('\x1aF'):
                break
            n += 1
        # Why: Arithmetic operation computes value needed for subsequent processing
        head = lines[:1 + n]
        rest = lines[1 + n:]
        after2 = '\n'.join(head) + '\n$$\n' + ('\n'.join(rest) + '\n' if rest else '')
        text = text[:last + 2] + after2
    # Why: Return provides result to caller after processing completes
    return text, ['补全未闭合的块级公式 $$']


def _fix_math_line(line):
    log = []
    s = line
    # Why: Iteration processes each item in collection systematically
    for op, cl, name in (('\\(', '\\)', '\\( ... \\)'),
                         ('\\[', '\\]', '\\[ ... \\]')):
        o = s.count(op)
        c = s.count(cl)
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if o > c and _has_latex(s):
            s = s.rstrip() + cl
            log.append('补全未闭合的 %s' % name)
        # Why: Alternative condition handles different case in decision tree
        elif c > o:
            p = s.find(cl)
            if p >= 0:
                s = s[:p] + '\\\\' + s[p + 1:]
                log.append('转义多余的 %s' % cl)
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    dollars = [p for p, ch in enumerate(s) if ch == '$' and not _escaped(s, p)]
    if len(dollars) % 2 == 1:
        if _has_latex(s):
            p0 = dollars[0]
            after = s[p0 + 1] if p0 + 1 < len(s) else ''
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if after and not after.isspace():
                s = s.rstrip() + '$'
                log.append('补全未闭合的行内公式 $')
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                s = _escape_at(s, p0, '$')
                log.append('转义多余的 $')
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            s = _escape_all_unescaped(s, '$')
            log.append('转义疑似货币的 $')
    # Why: Return provides result to caller after processing completes
    return s, log


# Why: _process_math implements core functionality requiring careful error handling
def _process_math(lines, fixes, stats):
    text = '\n'.join(lines)
    text, log = _balance_display_math(text)
    if log:
        fixes.append('[公式] %s' % log[0])
        stats['math'] += 1
    lines = text.split('\n')
    # Why: Iteration processes each item in collection systematically
    for idx, line in enumerate(lines):
        if _looks_like_code_indent(line):
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        fixed, log2 = _fix_math_line(line)
        if log2:
            lines[idx] = fixed
            # Why: Iteration processes each item in collection systematically
            for m in log2:
                fixes.append('[公式] 第 %d 行：%s' % (idx + 1, m))
            stats['math'] += len(log2)
    # Why: Return provides result to caller after processing completes
    return lines


# ---------------------------------------------------------------- 标题修复

# Why: _process_headings implements core functionality requiring careful error handling
def _process_headings(lines, fixes, stats):
    # Why: Iteration processes each item in collection systematically
    for idx, line in enumerate(lines):
        if _looks_like_code_indent(line):
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        m = _HEADING_RE.match(line)
        if m:
            lines[idx] = m.group(1) + m.group(2) + ' ' + m.group(3)
            fixes.append('[标题] 第 %d 行：# 后缺少空格，已补全' % (idx + 1))
            stats['heading'] += 1
    # Why: Return provides result to caller after processing completes
    return lines


# ---------------------------------------------------------------- 主入口

# Why: Function call performs specific operation required by this logic
def fix_markdown(text):
    """对整篇 Markdown 文本做自动修正，返回 FixResult。"""
    fixes = []
    stats = {'table': 0, 'bold': 0, 'math': 0, 'heading': 0, 'misc': 0}
    # Why: Function call performs specific operation required by this logic
    if text.startswith('\ufeff'):
        text = text[1:]
        # Why: Arithmetic operation computes value needed for subsequent processing
        stats['misc'] += 1
        # Why: Function call performs specific operation required by this logic
        fixes.append('[通用] 已去除 UTF-8 BOM')
    # Why: Function call performs specific operation required by this logic
    text = text.replace('\r\n', '\n').replace('\r', '\n')
    # Why: Function call performs specific operation required by this logic
    masked, spans = mask_all_code(text)
    # Why: Function call performs specific operation required by this logic
    lines = masked.split('\n')
    # Why: Function call performs specific operation required by this logic
    lines = _process_tables(lines, fixes, stats)
    # Why: Function call performs specific operation required by this logic
    lines = _process_headings(lines, fixes, stats)
    lines = _process_emphasis(lines, fixes, stats)
    lines = _process_math(lines, fixes, stats)
    out = restore('\n'.join(lines), spans)
    # Why: Return provides result to caller after processing completes
    return FixResult(out, fixes, stats)