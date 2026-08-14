# -*- coding: utf-8 -*-
"""转换后严格校验 + 安全自动修复（v2.1.1）。

自动修复（安全项）：
- 表格列数对齐 / 标题空行 / 常见 MD 错误（复用 readmd_fix）
- 代码围栏未闭合 → 文末补全
- 多余空行折叠
警告（不自动改）：
- $ / $$ 定界符数量奇偶不配对
- 替换符（�）残留
- 相对图片引用文件不存在
"""
import os
import re

import readmd_fix

_FENCE_RE = re.compile(r'^(\s{0,3})(`{3,}|~{3,})\s*(\S*)\s*$')
_REPLACE_CHAR = '\ufffd'


def check(text, base_dir=None):
    """返回 (fixed_text, issues)。issues: [{level:'auto'|'warn'|'error', msg, line}]"""
    issues = []
    if not text:
        return text, issues

    # 1) 复用 readmd_fix 安全修复（表格/标题/加粗/公式定界等）
    fr = readmd_fix.fix_markdown(text)
    fixed = fr.text
    for f in (fr.fixes or []):
        issues.append({'level': 'auto', 'msg': f, 'line': 0})

    lines = fixed.split('\n')

    # 2) 代码围栏闭合校验
    fence_char = None
    fence_len = 0
    open_line = 0
    for i, line in enumerate(lines):
        m = _FENCE_RE.match(line)
        if not m:
            continue
        ch = m.group(2)[0]
        ln = len(m.group(2))
        if fence_char is None:
            fence_char, fence_len, open_line = ch, ln, i + 1
        elif ch == fence_char and ln >= fence_len:
            fence_char, fence_len, open_line = None, 0, 0
    if fence_char is not None:
        lines.append('```')
        issues.append({'level': 'auto', 'msg': '代码围栏未闭合（从第 %d 行开始），已在文末补全' % open_line,
                       'line': open_line})
        fixed = '\n'.join(lines)
        lines = fixed.split('\n')

    # 3) 折叠 3 个以上连续空行
    new_lines = []
    blank = 0
    for line in lines:
        if not line.strip():
            blank += 1
            if blank > 2:
                continue
        else:
            blank = 0
        new_lines.append(line)
    if len(new_lines) != len(lines):
        issues.append({'level': 'auto', 'msg': '连续空行过多，已折叠为至多 2 行', 'line': 0})
        fixed = '\n'.join(new_lines)
        lines = new_lines

    # 4) 公式定界符配对（遮蔽代码后检查）
    masked, _spans = readmd_fix.mask_all_code(fixed)
    dd = masked.count('$$')
    sd = len(re.findall(r'(?<!\$)\$(?!\$)', masked))
    if dd % 2 != 0:
        issues.append({'level': 'warn', 'msg': '$$ 显示公式定界符数量为奇数，可能有一处公式未闭合', 'line': 0})
    if sd % 2 != 0:
        issues.append({'level': 'warn', 'msg': '行内 $ 公式定界符数量为奇数，可能有一处公式未闭合', 'line': 0})

    # 5) 替换符残留
    if _REPLACE_CHAR in fixed:
        issues.append({'level': 'warn', 'msg': '文本中包含替换符（�），可能源文档编码或字体不支持', 'line': 0})

    # 6) 相对图片存在性
    if base_dir:
        for m in re.finditer(r'!\[[^\]]*\]\(([^)]+)\)', fixed):
            rel = m.group(1).strip()
            if not rel or rel.startswith(('http://', 'https://', 'data:', '#')):
                continue
            q = rel.split(' ')[0]  # 去掉可选 title
            target = os.path.normpath(os.path.join(base_dir, q))
            if not os.path.isfile(target):
                issues.append({'level': 'warn', 'msg': '图片引用不存在：%s' % rel, 'line': 0})

    return fixed, issues
