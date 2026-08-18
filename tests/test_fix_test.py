# -*- coding: utf-8 -*-
"""ReadMD 修正器单元测试：python test_fix_test.py"""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from src.readmd_fix import fix_markdown

QUIET = False
FAIL = 0
PASS = 0


def check(name, inp, expects, not_expects=None):
    global FAIL, PASS
    r = fix_markdown(inp)
    ok = True
    for e in expects:
        if e not in r.text:
            ok = False
    for e in (not_expects or []):
        if e in r.text:
            ok = False
    if ok:
        PASS += 1
    else:
        FAIL += 1
        if not QUIET:
            print('FAIL %-32s' % name)
            print('   in : %r' % inp)
            print('   out: %r' % r.text)
            print('   fix: %r' % r.fixes)


def run_tests(quiet=False):
    global QUIET, FAIL, PASS
    QUIET = quiet
    FAIL = 0
    PASS = 0
    # ---------- 表格 ----------
    check('表格:缺分隔行', '| A | B |\n| 1 | 2 |',
          ['| A | B |', '| --- | --- |', '| 1 | 2 |'], ['| 1 | 2 |\n'])
    check('表格:列数不齐', '| A | B | C |\n|---|---|\n| 1 | 2 |',
          ['| --- | --- | --- |', '| 1 | 2 |  |'])
    check('表格:无外竖线', 'A | B\n--|--\n1 | 2',
          ['| A | B |', '| --- | --- |', '| 1 | 2 |'])
    check('表格:转义竖线', '| a \\| b | c |\n|---|---|',
          ['| a \\| b | c |'])
    check('表格:对齐分隔行保留', '| a | b |\n|:--|--:|\n| 1 | 2 |',
          ['| :--- | ---: |'])
    check('表格:两列散文不动', 'a|b\nc|d', ['a|b\nc|d'])
    check('表格:单行带竖线不动', '价格|折扣', ['价格|折扣'])
    check('表格:代码块内不动', '```\n| A | B |\n| 1 | 2 |\n```',
          ['| A | B |\n| 1 | 2 |'], ['| --- | --- |'])

    # ---------- 加粗 ----------
    check('加粗:未闭合', '**bold', ['**bold**'])
    check('加粗:结尾游离', 'bold**', ['bold\\*\\*'])
    check('加粗:混合', '**a** **b', ['**a** **b**'])
    check('加粗:双未闭合', '**a** and **b** and **c', ['**c**'])
    check('加粗:下划线', '__under', ['__under__'])
    check('加粗:斜体未闭合', '*italic', ['*italic*'])
    check('加粗:乘号', '2 * 3', ['2 \\* 3'])
    check('加粗:列表项不动', '* item\n* item2', ['* item\n* item2'])
    check('加粗:分隔线不动', '***\n---\n___', ['***\n---\n___'])
    check('加粗:代码内不动', '`**x` 和 `*y`', ['`**x` 和 `*y`'])
    check('加粗:单词内下划线不动', 'foo_bar foo__bar', ['foo_bar foo__bar'])
    check('加粗:三段粗体', '***bold', ['***bold***'])
    check('加粗:孤立符号', '**', ['\\*\\*'])

    # ---------- 公式 ----------
    check('公式:行内未闭合', '$x^2$ 和 $y', ['$x^2$ 和 $y$'])
    check('公式:货币转义', '价格 $5', ['价格 \\$5'])
    check('公式:块级未闭合', '$$\nE=mc^2', ['$$\nE=mc^2\n$$'])
    check('公式:圆括号', '\\(x^2', ['\\(x^2\\)'])
    check('公式:方括号', '\\[x^2', ['\\[x^2\\]'])
    check('公式:空块移除', '$$\n\n$$\n正文', ['正文'], ['$$\n\n$$'])
    check('公式:代码内不动', '`$x$`', ['`$x$`'])
    check('公式:完整不动', '$a$ 和 $b$', ['$a$ 和 $b$'])
    check('公式:$$inline$$', '$$x^2$$ 文本', ['$$x^2$$ 文本'])

    # ---------- 标题 ----------
    check('标题:补空格', '#标题', ['# 标题'])
    check('标题:多井号', '##标题', ['## 标题'])
    check('标题:七井号不动', '####### x', ['####### x'])
    check('标题:已有空格不动', '# 标题', ['# 标题'])
    check('标题:单独井号不动', '#\n##', ['#\n##'])

    # ---------- 通用 ----------
    check('通用:BOM', '\ufeff# 标题', ['# 标题'])
    check('通用:CRLF', '# a\r\n# b\r\n', ['# a\n# b\n'])

    if not QUIET:
        print('passed: %d, failed: %d' % (PASS, FAIL))
    return FAIL


if __name__ == '__main__':
    sys.exit(1 if run_tests() else 0)