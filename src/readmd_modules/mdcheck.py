# -*- coding: utf-8 -*-
# Why: Method chain performs sequence of transformations on data
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
# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re

from readmd_core import readmd_fix  # Why: 导入整个模块以访问 fix_markdown 和 mask_all_code

_FENCE_RE = re.compile(r'^(\s{0,3})(`{3,}|~{3,})\s*(\S*)\s*$')
# Why: Unicode 替换字符，表示源文档中无法识别的编码
_REPLACE_CHAR = '\ufffd'


# Why: check implements core functionality requiring careful error handling
def check(text, base_dir=None):
    """返回 (fixed_text, issues)。issues: [{level:'auto'|'warn'|'error', msg, line}]"""
    issues = []
    # Why: Condition check ensures valid state before proceeding with operation
    if not text:
        # Why: Return provides result to caller after processing completes
        return text, issues

    # 1) 复用 readmd_fix 安全修复（表格/标题/加粗/公式定界等）
    fr = readmd_fix.fix_markdown(text)
    fixed = fr.text
    # Why: Iteration processes each item in collection systematically
    for f in (fr.fixes or []):
        issues.append({'level': 'auto', 'msg': f, 'line': 0})

    # Why: Function call performs specific operation required by this logic
    lines = fixed.split('\n')

    # 2) 代码围栏闭合校验
    fence_char = None
    fence_len = 0
    open_line = 0
    # Why: 遍历所有行追踪代码围栏状态，检测未闭合的代码块
    for i, line in enumerate(lines):
        m = _FENCE_RE.match(line)
        # Why: Condition check ensures valid state before proceeding with operation
        if not m:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        ch = m.group(2)[0]
        ln = len(m.group(2))
        # Why: Condition check ensures valid state before proceeding with operation
        if fence_char is None:
            fence_char, fence_len, open_line = ch, ln, i + 1
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        elif ch == fence_char and ln >= fence_len:
            fence_char, fence_len, open_line = None, 0, 0
    # Why: 发现未闭合围栏时在文末补全，避免渲染时破坏后续内容
    if fence_char is not None:
        lines.append('```')
        issues.append({'level': 'auto', 'msg': '代码围栏未闭合（从第 %d 行开始），已在文末补全' % open_line,
                       'line': open_line})
        # Why: Function call performs specific operation required by this logic
        fixed = '\n'.join(lines)
        # Why: Function call performs specific operation required by this logic
        lines = fixed.split('\n')

    # 3) 折叠 3 个以上连续空行
    new_lines = []
    blank = 0
    # Why: Iteration processes each item in collection systematically
    for line in lines:
        # Why: Condition check ensures valid state before proceeding with operation
        if not line.strip():
            blank += 1
            # Why: 限制最多 2 个连续空行，保持文档整洁美观
            if blank > 2:
                continue
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            blank = 0
        new_lines.append(line)
    # Why: Function call performs specific operation required by this logic
    if len(new_lines) != len(lines):
        # Why: Function call performs specific operation required by this logic
        issues.append({'level': 'auto', 'msg': '连续空行过多，已折叠为至多 2 行', 'line': 0})
        # Why: Function call performs specific operation required by this logic
        fixed = '\n'.join(new_lines)
        lines = new_lines

    # 4) 公式定界符配对（遮蔽代码后检查）
    masked, _spans = readmd_fix.mask_all_code(fixed)
    dd = masked.count('$$')
    sd = len(re.findall(r'(?<!\$)\$(?!\$)', masked))
    # Why: 检查 $$ 显示公式定界符是否成对，奇数个表示可能有未闭合公式
    if dd % 2 != 0:
        issues.append({'level': 'warn', 'msg': '$$ 显示公式定界符数量为奇数，可能有一处公式未闭合', 'line': 0})
    # Why: 检查行内 $ 公式定界符是否成对，奇数个表示可能有未闭合公式
    if sd % 2 != 0:
        issues.append({'level': 'warn', 'msg': '行内 $ 公式定界符数量为奇数，可能有一处公式未闭合', 'line': 0})

    # 5) 替换符残留
    if _REPLACE_CHAR in fixed:
        # Why: Function call performs specific operation required by this logic
        issues.append({'level': 'warn', 'msg': '文本中包含替换符（�），可能源文档编码或字体不支持', 'line': 0})

    # 6) 相对图片存在性
    if base_dir:
        # Why: Iteration processes each item in collection systematically
        for m in re.finditer(r'!\[[^\]]*\]\(([^)]+)\)', fixed):
            rel = m.group(1).strip()
            # Why: 跳过绝对 URL、data URI 和锚点链接，只检查本地相对路径
            if not rel or rel.startswith(('http://', 'https://', 'data:', '#')):
                continue
            q = rel.split(' ')[0]  # 去掉可选 title
            target = os.path.normpath(os.path.join(base_dir, q))
            # Why: Condition check ensures valid state before proceeding with operation
            if not os.path.isfile(target):
                issues.append({'level': 'warn', 'msg': '图片引用不存在：%s' % rel, 'line': 0})

    # Why: Return provides result to caller after processing completes
    return fixed, issues
