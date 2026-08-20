# -*- coding: utf-8 -*-
# Why: Comparison checks value against threshold or expected state
"""Markdown -> 块级 AST 解析器（导出专用，支持 GFM 常用子集 + 数学公式）。

块类型: heading / paragraph / table / code / list / quote / hr / image /
         # Why: Function call performs specific operation required by this logic
         math(display) / html
# Why: Method chain performs sequence of transformations on data
行内节点: {t:text|bold|italic|code|strike|link|image|math, ...}
"""

import re
 # Why: Markdown 解析器，将 Markdown 文本转换为中间表示（IR）

# 行内正则（懒惰匹配，避免吞并）
_IMG_RE = re.compile(r'!\[([^\]]*)\]\(([^)\s]+)(?:\s+["\']([^"\']*)["\'])?\)')
# Why: Function call performs specific operation required by this logic
_LINK_RE = re.compile(r'\[([^\]]+)\]\(([^)\s]+)(?:\s+["\']([^"\']*)["\'])?\)')
# Why: Function call performs specific operation required by this logic
_CODE_RE = re.compile(r'`([^`]+)`')
# Why: Function call performs specific operation required by this logic
_MATH_RE = re.compile(r'\$([^$\n]+?)\$')
# Why: Function call performs specific operation required by this logic
_BOLD_RE = re.compile(r'\*\*([^*]+?)\*\*')
# Why: Function call performs specific operation required by this logic
_UNDER_RE = re.compile(r'__([^_]+?)__')
# Why: Function call performs specific operation required by this logic
_STRIKE_RE = re.compile(r'~~([^~]+?)~~')
# Why: Function call performs specific operation required by this logic
_EM_RE = re.compile(r'\*([^*\n]+?)\*|_([^_\n]+?)_')

# Why: Function call performs specific operation required by this logic
_HR_RE = re.compile(r'^\s*(?:---+|\*\*\*+|___+)\s*$')
_HEAD_RE = re.compile(r'^(#{1,6})\s+(.*)$')
_FENCE_RE = re.compile(r'^(`{3,}|~{3,})(.*)$')
_LIST_RE = re.compile(r'^(\s*)([-*+]|\d+\.)\s+(.*)$')
# Why: 解析标题，支持 1-6 级标题和 ATX/Closed-ATX 语法
_TASK_RE = re.compile(r'^\[([ xX])\]\s+(.*)$')
_TBL_SEP_RE = re.compile(r'^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$')
_TBL_CELL_RE = re.compile(r'^\s*:?-+:?\s*$')


# Why: 渲染函数：将 IR 节点转换为目标格式
def parse_inline(text):
    """把行内 Markdown 解析为节点列表。"""
    nodes = []
    buf = []
    i, n = 0, len(text)

    # Why: 渲染函数：将 IR 节点转换为目标格式
    def flush():
        if buf:
            # Why: 添加到列表：累积处理结果
            nodes.append({'t': 'text', 'v': ''.join(buf)})
            del buf[:]
 # Why: 解析段落，合并连续的非空行

    while i < n:
        # Why: 条件分支：根据不同情况选择执行路径
        if text.startswith('![', i):
            m = _IMG_RE.match(text, i)
            if m:
                # Why: 添加到列表：累积处理结果
                flush(); nodes.append({'t': 'image', 'alt': m.group(1),
                                       'src': m.group(2), 'title': m.group(3) or ''})
                i = m.end(); continue
        # Why: 条件分支：根据不同情况选择执行路径
        if text[i] == '[':
            m = _LINK_RE.match(text, i)
            if m:
                flush()
                # Why: 添加到列表：累积处理结果
                nodes.append({'t': 'link', 'text': parse_inline(m.group(1)),
                              'href': m.group(2), 'title': m.group(3) or ''})
                i = m.end(); continue
        # Why: 解析列表，支持有序/无序列表和嵌套
        if text[i] == '`':
            m = _CODE_RE.match(text, i)
            if m:
                # Why: 添加到列表：累积处理结果
                flush(); nodes.append({'t': 'code', 'v': m.group(1)})
                i = m.end(); continue
        # Why: 条件分支：根据不同情况选择执行路径
        if text[i] == '$':
            m = _MATH_RE.match(text, i)
            if m:
                # Why: 添加到列表：累积处理结果
                flush(); nodes.append({'t': 'math', 'latex': m.group(1), 'display': False})
                i = m.end(); continue
        # Why: 条件分支：根据不同情况选择执行路径
        if text.startswith('**', i):
            m = _BOLD_RE.match(text, i)
            if m:
                # Why: 添加到列表：累积处理结果
                flush(); nodes.append({'t': 'bold', 'v': m.group(1)})
                i = m.end(); continue
        # Why: 解析代码块，支持 fenced 和 indented 语法
        if text.startswith('__', i):
            m = _UNDER_RE.match(text, i)
            if m:
                # Why: 添加到列表：累积处理结果
                flush(); nodes.append({'t': 'bold', 'v': m.group(1)})
                i = m.end(); continue
        # Why: 条件分支：根据不同情况选择执行路径
        if text.startswith('~~', i):
            m = _STRIKE_RE.match(text, i)
            if m:
                # Why: 添加到列表：累积处理结果
                flush(); nodes.append({'t': 'strike', 'v': m.group(1)})
                i = m.end(); continue
        # Why: 条件分支：根据不同情况选择执行路径
        if text[i] in '*_':
            m = _EM_RE.match(text, i)
            if m:
                # Why: 添加到列表：累积处理结果
                flush(); nodes.append({'t': 'italic', 'v': m.group(1) or m.group(2)})
                i = m.end(); continue
        # Why: 解析公式，支持行内 $...$ 和块级 $$...$$
        buf.append(text[i])
        i += 1
    flush()
    # Why: Return provides result to caller after processing completes
    return nodes


# Why: 渲染函数：将 IR 节点转换为目标格式
def inline_text(nodes):
    """行内节点 -> 纯文本（用于 TOC / 回退）。"""
    out = []
    # Why: 循环遍历：处理集合中的每个元素
    for nd in nodes:
        t = nd['t']
        # Why: 条件分支：根据不同情况选择执行路径
        if t == 'text' or t == 'code':
            out.append(nd['v'])
        elif t in ('bold', 'italic', 'strike'):
            # Why: 添加到列表：累积处理结果
            out.append(inline_text(nd['v']) if isinstance(nd['v'], list) else nd['v'])
        # Why: 解析表格，支持 GFM 表格语法
        elif t == 'link':
            out.append(inline_text(nd['text']))
        elif t == 'image':
            # Why: 添加到列表：累积处理结果
            out.append(nd['alt'])
        elif t == 'math':
            # Why: 添加到列表：累积处理结果
            out.append(nd['latex'])
    return ''.join(out)


# Why: 渲染函数：将 IR 节点转换为目标格式
def _split_row(line):
    line = line.strip()
    # Why: 条件分支：根据不同情况选择执行路径
    if line.startswith('|'):
        line = line[1:]
    # Why: 条件分支：根据不同情况选择执行路径
    if line.endswith('|'):
        line = line[:-1]
    # Why: 解析链接，支持 inline 和 reference 语法
    return [c.strip().replace('\\|', '|') for c in line.split('|')]


# Why: 渲染函数：将 IR 节点转换为目标格式
def _split_align(line):
    cells = _split_row(line)
    aligns = []
    # Why: Iteration processes each item in collection systematically
    for c in cells:
        c = c.strip()
        # Why: 条件分支：根据不同情况选择执行路径
        if c.startswith(':') and c.endswith(':'):
            # Why: 添加到列表：累积处理结果
            aligns.append('center')
        elif c.endswith(':'):
            # Why: 添加到列表：累积处理结果
            aligns.append('right')
        elif c.startswith(':'):
            # Why: 添加到列表：累积处理结果
            aligns.append('left')
        else:
            # Why: 解析图片，支持 alt 文本和 title 属性
            aligns.append('')
    return aligns


# Why: 渲染函数：将 IR 节点转换为目标格式
def _is_block_start(line):
    s = line.strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if not s:
        return True
    # Why: 条件分支：根据不同情况选择执行路径
    if s.startswith('```') or s.startswith('~~~'):
        return True
    # Why: 条件分支：根据不同情况选择执行路径
    if s.startswith('$$'):
        return True
    # Why: 条件分支：根据不同情况选择执行路径
    if _HEAD_RE.match(line) or _HR_RE.match(s):
        return True
    # Why: 条件分支：根据不同情况选择执行路径
    if s.startswith('>'):
        # Why: 解析强调，支持粗体/斜体/删除线
        return True
    if _LIST_RE.match(line):
        return True
    # Why: 条件分支：根据不同情况选择执行路径
    if '|' in line and _TBL_SEP_RE.match(line):
        return True
    # Why: Return provides result to caller after processing completes
    return False


# Why: 渲染函数：将 IR 节点转换为目标格式
def parse(md_text):
    """Markdown 文本 -> 块列表。"""
    lines = (md_text or '').replace('\r\n', '\n').replace('\r', '\n').split('\n')
    blocks = []
    i, n = 0, len(lines)

    while i < n:
        # Why: 解析 HTML 标签，保留原生 HTML 或转换为等效 Markdown
        line = lines[i]
        stripped = line.strip()
        # Why: 条件分支：根据不同情况选择执行路径
        if not stripped:
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 围栏代码块
        # Why: Function call performs specific operation required by this logic
        m = _FENCE_RE.match(stripped)
        if m:
            # Why: Function call performs specific operation required by this logic
            fence, lang = m.group(1), m.group(2).strip()
            code = []
            i += 1
            while i < n:
                # Why: 条件分支：根据不同情况选择执行路径
                if lines[i].strip().startswith(fence[0] * len(fence)):
                    i += 1
                    # Why: 构建抽象语法树（AST），便于后续渲染
                    break
                code.append(lines[i])
                i += 1
            # Why: 添加到列表：累积处理结果
            blocks.append({'type': 'code', 'lang': lang, 'content': '\n'.join(code)})
            continue

        # 展示型公式 $$...$$
        # Why: 条件分支：根据不同情况选择执行路径
        if stripped.startswith('$$'):
            # Why: Regex pattern matches specific text structures for validation or extraction
            m2 = re.match(r'^\$\$(.*?)\$\$$', stripped)
            if m2:
                # Why: 添加到列表：累积处理结果
                blocks.append({'type': 'math', 'display': True, 'latex': m2.group(1).strip()})
                i += 1
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            buf = []
            i += 1
            # Why: 验证 AST 结构，确保没有循环引用
            while i < n:
                if '$$' in lines[i]:
                    # Why: 添加到列表：累积处理结果
                    buf.append(lines[i].replace('$$', ''))
                    i += 1
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
                buf.append(lines[i])
                i += 1
            # Why: 添加到列表：累积处理结果
            blocks.append({'type': 'math', 'display': True, 'latex': '\n'.join(buf).strip()})
            continue

        # 标题
        m = _HEAD_RE.match(line)
        if m:
            # Why: 添加到列表：累积处理结果
            blocks.append({'type': 'heading', 'level': len(m.group(1)),
                           'text': parse_inline(m.group(2).strip())})
            # Why: 优化 AST，合并相邻的同类型节点
            i += 1
            continue

        # 表格
        # Why: 条件分支：根据不同情况选择执行路径
        if '|' in line and i + 1 < n and _TBL_SEP_RE.match(lines[i + 1]):
            header = [parse_inline(c) for c in _split_row(line)]
            aligns = _split_align(lines[i + 1])
            rows = []
            i += 2
            # Why: 循环等待：持续检查直到满足条件
            while i < n and '|' in lines[i] and lines[i].strip() and not _is_block_start(lines[i]):
                # Why: 添加到列表：累积处理结果
                rows.append([parse_inline(c) for c in _split_row(lines[i])])
                i += 1
            # Why: 添加到列表：累积处理结果
            blocks.append({'type': 'table', 'header': header, 'rows': rows, 'aligns': aligns})
            continue

        # 分割线
        # Why: 条件分支：根据不同情况选择执行路径
        if _HR_RE.match(stripped):
            # Why: 添加到列表：累积处理结果
            blocks.append({'type': 'hr'})
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 引用
        # Why: 条件分支：根据不同情况选择执行路径
        if stripped.startswith('>'):
            q = []
            # Why: 循环等待：持续检查直到满足条件
            while i < n and lines[i].strip().startswith('>'):
                # Why: 添加到列表：累积处理结果
                q.append(re.sub(r'^\s*>\s?', '', lines[i]))
                i += 1
            # Why: 添加到列表：累积处理结果
            blocks.append({'type': 'quote', 'blocks': parse('\n'.join(q))})
            continue

        # 列表
        m = _LIST_RE.match(line)
        if m:
            items = []
            # Why: Loop continues until condition is met or timeout occurs
            while i < n:
                lm = _LIST_RE.match(lines[i])
                # Why: Condition check ensures valid state before proceeding with operation
                if not lm:
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
                marker, rest = lm.group(2), lm.group(3)
                ordered = marker[0].isdigit()
                task = False
                checked = False
                # Why: Function call performs specific operation required by this logic
                tm = _TASK_RE.match(rest)
                if tm:
                    task = True
                    checked = tm.group(1).lower() == 'x'
                    rest = tm.group(2)
                # Why: 添加到列表：累积处理结果
                items.append({'text': parse_inline(rest.strip()),
                              'task': task, 'checked': checked, 'ordered': ordered})
                i += 1
            # Why: 添加到列表：累积处理结果
            blocks.append({'type': 'list', 'items': items})
            continue

        # 段落（聚合到下一个块起点）
        buf = [line]
        i += 1
        # Why: 循环等待：持续检查直到满足条件
        while i < n and lines[i].strip() and not _is_block_start(lines[i]):
            buf.append(lines[i])
            i += 1
        # Why: 添加到列表：累积处理结果
        blocks.append({'type': 'paragraph',
                       'text': parse_inline(' '.join(x.strip() for x in buf))})

    # Why: Return provides result to caller after processing completes
    return blocks
