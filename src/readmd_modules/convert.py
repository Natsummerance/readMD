# -*- coding: utf-8 -*-
"""万物转 md：核心格式专用解析（docx / pdf）+ MarkItDown 兜底 + 严格校验。

v2.1.1 质量升级：
- .docx：python-docx + lxml 专用解析 —— OMML 公式转 LaTeX、标题层级、表格、
  等宽字体代码块、图片引用；
- .pdf：PyMuPDF 专用解析 —— find_tables 还原边框表格 + 公式启发式；
- 其余格式（pptx/xlsx/html/csv/json/zip 等）走 MarkItDown；
- 专用解析异常时逐文件回退 MarkItDown，仍失败抛出带原因的异常。
"""

import os
import re

_engine = None

_MONO_FONTS = ('consolas', 'courier new', 'courier', 'menlo', 'monaco',
               'source code pro', 'cascadia code', 'fira code', 'jetbrains mono',
               'sf mono', 'liberation mono', 'dejavu sans mono')

_CODE_LANG_HINTS = (
    ('python', 'python'), ('javascript', 'javascript'), ('typescript', 'typescript'),
    ('java', 'java'), ('csharp', 'csharp'), ('c++', 'cpp'), ('cpp', 'cpp'),
    ('go', 'go'), ('rust', 'rust'), ('sql', 'sql'), ('html', 'html'),
    ('css', 'css'), ('bash', 'bash'), ('shell', 'bash'), ('json', 'json'),
    ('xml', 'xml'), ('yaml', 'yaml'), ('powershell', 'powershell'),
    ('php', 'php'), ('ruby', 'ruby'), ('vb', 'vb'), ('swift', 'swift'),
    ('kotlin', 'kotlin'), ('c#', 'csharp'), ('c', 'c'), ('cs', 'csharp'),
)

_MATH_CHARS = set('+-*/=<>^_~%∑∫√∞≈≠±×÷∂∇∏πθαβγδφψωλμνστρΔΩΦΓΛεηζξοπς')


def load():
    # 惰性加载：docx / pdf 专用解析不依赖 markitdown（Win7 版未安装）
    return _engine


def supported_hint():
    return ('支持：PDF / Word / PowerPoint / Excel / HTML / CSV / JSON / XML / '
            '邮件 / 压缩包等；图片与扫描件请用 OCR 模块')


# ---------------------------------------------------------------- 入口

def convert(path):
    """把任意支持的文件转换为 Markdown 文本（专用解析 → MarkItDown 兜底）。"""
    text, _engine_name, error = convert_verbose(path)
    if error and not text:
        raise ValueError(error)
    return text


def convert_verbose(path):
    """返回 (text, engine, error)。engine: 'docx' | 'pdf' | 'markitdown' | ''"""
    ext = os.path.splitext(path)[1].lower()
    if ext == '.docx':
        try:
            return docx2md(path), 'docx', None
        except Exception as e:  # noqa: BLE001
            try:
                return _markitdown_convert(path), 'markitdown', None
            except Exception as e2:  # noqa: BLE001
                return '', '', '%s（MarkItDown 兜底也失败：%s）' % (e, e2)
    if ext == '.pdf':
        try:
            return pdf2md(path), 'pdf', None
        except Exception as e:  # noqa: BLE001
            try:
                return _markitdown_convert(path), 'markitdown', None
            except Exception as e2:  # noqa: BLE001
                return '', '', '%s（MarkItDown 兜底也失败：%s）' % (e, e2)
    if ext in ('.tex', '.latex'):
        try:
            from . import texmd
            with open(path, 'r', encoding='utf-8', errors='replace') as f:
                tex_content = f.read()
            return texmd.latex_to_md(tex_content), 'texmd', None
        except Exception as e:  # noqa: BLE001
            return '', '', 'LaTeX 转换失败：%s' % e

    try:
        return _markitdown_convert(path), 'markitdown', None
    except Exception as e:  # noqa: BLE001
        return '', '', str(e)



def _markitdown_convert(path):
    global _engine
    if _engine is None:
        try:
            from markitdown import MarkItDown
        except Exception as e:  # noqa: BLE001
            raise ImportError('MarkItDown 未安装（%s），本格式无法转换' % e)
        _engine = MarkItDown()
    result = _engine.convert(path)
    return (result.text_content or '').strip()


# ---------------------------------------------------------------- docx 专用

_M_NS = 'http://schemas.openxmlformats.org/officeDocument/2006/math'

_UNICODE_MATH_TO_LATEX = {
    'α': r'\alpha', 'β': r'\beta', 'γ': r'\gamma', 'δ': r'\delta',
    'ϵ': r'\epsilon', 'ε': r'\varepsilon', 'ζ': r'\zeta', 'η': r'\eta',
    'θ': r'\theta', 'ϑ': r'\vartheta', 'ι': r'\iota', 'κ': r'\kappa',
    'λ': r'\lambda', 'μ': r'\mu', 'ν': r'\nu', 'ξ': r'\xi',
    'π': r'\pi', 'ϖ': r'\varpi', 'ρ': r'\rho', 'ϱ': r'\varrho',
    'σ': r'\sigma', 'ς': r'\varsigma', 'τ': r'\tau', 'υ': r'\upsilon',
    'ϕ': r'\phi', 'φ': r'\varphi', 'χ': r'\chi', 'ψ': r'\psi', 'ω': r'\omega',
    'Γ': r'\Gamma', 'Δ': r'\Delta', 'Θ': r'\Theta', 'Λ': r'\Lambda',
    'Ξ': r'\Xi', 'Π': r'\Pi', 'Σ': r'\Sigma', 'Υ': r'\Upsilon',
    'Φ': r'\Phi', 'Ψ': r'\Psi', 'Ω': r'\Omega',
    '±': r'\pm', '∓': r'\mp', '×': r'\times', '÷': r'\div', '·': r'\cdot',
    '∗': r'\ast', '⋆': r'\star', '∘': r'\circ', '∙': r'\bullet',
    '≤': r'\leq', '≥': r'\geq', '≠': r'\neq', '≈': r'\approx', '≡': r'\equiv',
    '∼': r'\sim', '≃': r'\simeq', '≅': r'\cong', '∝': r'\propto',
    '≪': r'\ll', '≫': r'\gg',
    '→': r'\to', '←': r'\leftarrow', '⇒': r'\Rightarrow', '⇐': r'\Leftarrow',
    '↔': r'\leftrightarrow', '⇔': r'\Leftrightarrow', '↦': r'\mapsto',
    '↑': r'\uparrow', '↓': r'\downarrow',
    '∞': r'\infty', '∂': r'\partial', '∇': r'\nabla', '′': r"'", 'ℏ': r'\hbar',
    '∈': r'\in', '∉': r'\notin', '⊂': r'\subset', '⊆': r'\subseteq',
    '⊃': r'\supset', '⊇': r'\supseteq', '∩': r'\cap', '∪': r'\cup',
    '∖': r'\setminus', '∀': r'\forall', '∃': r'\exists', '¬': r'\neg',
    '∧': r'\land', '∨': r'\lor', '∅': r'\emptyset',
    '…': r'\ldots', '⋯': r'\cdots', '⋮': r'\vdots', '⋱': r'\ddots',
    '∠': r'\angle', '⊥': r'\perp', '∥': r'\parallel',
    '⟨': r'\langle', '⟩': r'\rangle',
    '∑': r'\sum', '∏': r'\prod', '∐': r'\coprod',
    '∫': r'\int', '∬': r'\iint', '∭': r'\iiint', '∮': r'\oint',
    '⋂': r'\bigcap', '⋃': r'\bigcup'
}


def _mq(tag):
    return '{%s}%s' % (_M_NS, tag)


def _omml_to_latex(el):
    """递归把 OMML 元素转为高质量 LaTeX 表达式。"""
    if el is None:
        return ''
    tag = el.tag
    if not isinstance(tag, str) or not tag.startswith('{'):
        return (el.text or '')
    local = tag.split('}')[1]

    def kids(e):
        if e is None:
            return ''
        return ''.join(_omml_to_latex(c) for c in e)

    if local == 't':
        txt = (el.text or '').replace('\u200b', '').replace('\u2061', '')
        # 替换 Unicode 数学符号
        out_chars = []
        for ch in txt:
            if ch in _UNICODE_MATH_TO_LATEX:
                out_chars.append(_UNICODE_MATH_TO_LATEX[ch] + ' ')
            else:
                out_chars.append(ch)
        return ''.join(out_chars)

    if local in ('r', 'oMath', 'oMathPara', 'e', 'num', 'den', 'sub',
                 'sup', 'deg', 'chr', 'fName', 'delim',
                 'phant'):
        return kids(el)

    if local == 'f':  # 分数
        num = el.find(_mq('num'))
        den = el.find(_mq('den'))
        fPr = el.find(_mq('fPr'))
        if fPr is not None:
            t_type = fPr.find(_mq('type'))
            if t_type is not None and (t_type.get(_mq('val')) == 'noBar' or t_type.get('val') == 'noBar'):
                return r'\binom{%s}{%s}' % (kids(num).strip(), kids(den).strip())
        return r'\frac{%s}{%s}' % (kids(num).strip(), kids(den).strip())

    if local == 'sSup':
        base = el.find(_mq('e'))
        sup = el.find(_mq('sup'))
        return '{%s}^{%s}' % (kids(base).strip(), kids(sup).strip())

    if local == 'sSub':
        base = el.find(_mq('e'))
        sub = el.find(_mq('sub'))
        return '{%s}_{%s}' % (kids(base).strip(), kids(sub).strip())

    if local == 'sSubSup':
        base = el.find(_mq('e'))
        sub = el.find(_mq('sub'))
        sup = el.find(_mq('sup'))
        return '{%s}_{%s}^{%s}' % (kids(base).strip(), kids(sub).strip(), kids(sup).strip())

    if local == 'rad':  # 根式
        deg = el.find(_mq('deg'))
        e_el = el.find(_mq('e'))
        inner = kids(e_el).strip()
        d = kids(deg).strip()
        if d:
            return r'\sqrt[%s]{%s}' % (d, inner)
        return r'\sqrt{%s}' % inner

    if local == 'nary':  # 求和/积分/连乘
        chr_el = el.find(_mq('naryPr'))
        c = ''
        if chr_el is not None:
            c_node = chr_el.find(_mq('chr'))
            if c_node is not None:
                c = c_node.get(_mq('val'), c_node.get('val', ''))
        if not c:
            c_node = el.find(_mq('chr'))
            if c_node is not None:
                c = c_node.get(_mq('val'), c_node.get('val', '')) or kids(c_node)
        c = c.strip()
        op = {'\u2211': r'\sum', '\u222b': r'\int', '\u220f': r'\prod',
              '\u222e': r'\oint', '\u22c2': r'\bigcap', '\u22c3': r'\bigcup',
              '∑': r'\sum', '∫': r'\int', '∏': r'\prod', '∮': r'\oint'}.get(c, c or r'\int' if 'int' in c else r'\sum')
        sub = el.find(_mq('sub'))
        sup = el.find(_mq('sup'))
        e_el = el.find(_mq('e'))
        s, p = kids(sub).strip(), kids(sup).strip()
        inner = kids(e_el).strip()
        subsup = ''
        if s and p:
            subsup = '_{%s}^{%s}' % (s, p)
        elif s:
            subsup = '_{%s}' % s
        elif p:
            subsup = '^{%s}' % p

        if inner:
            return '%s%s %s' % (op, subsup, inner)
        return '%s%s' % (op, subsup)

    if local == 'd':  # 定界符 / 括号组
        dpr = el.find(_mq('dPr'))
        beg = '('
        end = ')'
        if dpr is not None:
            beg_el = dpr.find(_mq('begChr'))
            end_el = dpr.find(_mq('endChr'))
            if beg_el is not None:
                beg = beg_el.get(_mq('val'), beg_el.get('val', '('))
            if end_el is not None:
                end = end_el.get(_mq('val'), end_el.get('val', ')'))

        delim_map = {
            '(': (r'\left(', r'\right)'),
            '[': (r'\left[', r'\right]'),
            '{': (r'\left\{', r'\right\}'),
            '|': (r'\left|', r'\right|'),
            '‖': (r'\left\|', r'\right\|'),
            '⟨': (r'\left\langle', r'\right\rangle'),
            '<': (r'\left\langle', r'\right\rangle'),
            '': (r'\left.', r'\right.')
        }
        l_beg, _ = delim_map.get(beg, (r'\left%s' % beg if beg else r'\left.', ''))
        _, r_end = delim_map.get(end, ('', r'\right%s' % end if end else r'\right.'))

        e_el = el.find(_mq('e'))
        inner = kids(e_el).strip()

        # 如果内部为矩阵，转换为标准 pmatrix / bmatrix / vmatrix
        if inner.startswith(r'\begin{matrix}') and inner.endswith(r'\end{matrix}'):
            mat_body = inner[len(r'\begin{matrix}'):-len(r'\end{matrix}')].strip()
            if beg == '(' and end == ')':
                return r'\begin{pmatrix} %s \end{pmatrix}' % mat_body
            if beg == '[' and end == ']':
                return r'\begin{bmatrix} %s \end{bmatrix}' % mat_body
            if beg == '{' and end == '}':
                return r'\begin{Bmatrix} %s \end{Bmatrix}' % mat_body
            if beg == '|' and end == '|':
                return r'\begin{vmatrix} %s \end{vmatrix}' % mat_body

        return '%s %s %s' % (l_beg, inner, r_end)

    if local == 'm':  # 矩阵
        rows = el.findall(_mq('mr'))
        if rows:
            row_strs = []
            for r in rows:
                cells = r.findall(_mq('e'))
                row_strs.append(' & '.join(kids(c).strip() for c in cells))
            return r'\begin{matrix} %s \end{matrix}' % r' \\ '.join(row_strs)
        return kids(el)

    if local == 'eqArr':  # 方程组 / 多行对齐
        rows = el.findall(_mq('e'))
        if rows:
            row_strs = [kids(r).strip() for r in rows if kids(r).strip()]
            return r'\begin{aligned} %s \end{aligned}' % r' \\ '.join(row_strs)
        return kids(el)

    if local == 'limLow':  # 下标极限 / 最小值
        e_el = el.find(_mq('e'))
        lim_el = el.find(_mq('lim'))
        e_txt = kids(e_el).strip()
        l_txt = kids(lim_el).strip()
        if e_txt.lower() in ('lim', 'max', 'min', 'inf', 'sup', 'det', 'gcd'):
            return r'\%s_{%s}' % (e_txt.lower(), l_txt)
        return '{%s}_{%s}' % (e_txt, l_txt)

    if local == 'limUpp':  # 上标极限
        e_el = el.find(_mq('e'))
        lim_el = el.find(_mq('lim'))
        e_txt = kids(e_el).strip()
        l_txt = kids(lim_el).strip()
        if e_txt.lower() in ('lim', 'max', 'min', 'inf', 'sup', 'det', 'gcd'):
            return r'\%s^{%s}' % (e_txt.lower(), l_txt)
        return '{%s}^{%s}' % (e_txt, l_txt)

    if local in ('box', 'borderBox'):  # 框选
        e_el = el.find(_mq('e'))
        return r'\boxed{%s}' % kids(e_el).strip()

    if local == 'func':  # 函数
        fname = el.find(_mq('fName'))
        e_el = el.find(_mq('e'))
        fn_str = kids(fname).strip()
        in_str = kids(e_el).strip()
        known_funcs = {'sin', 'cos', 'tan', 'cot', 'sec', 'csc', 'ln', 'log', 'lg', 'exp', 'arcsin', 'arccos', 'arctan', 'sinh', 'cosh', 'tanh'}
        if fn_str.lower() in known_funcs:
            fn_str = r'\%s' % fn_str.lower()
        if in_str:
            return r'%s(%s)' % (fn_str, in_str)
        return fn_str

    if local == 'acc':  # 帽子/箭头/重音
        acc_pr = el.find(_mq('accPr'))
        a = '^'  # 默认 hat
        if acc_pr is not None:
            chr_el = acc_pr.find(_mq('chr'))
            if chr_el is not None:
                a = chr_el.get(_mq('val'), chr_el.get('val', '^'))
        e_el = el.find(_mq('e'))
        inner = kids(e_el).strip()
        marks = {'\u02c6': r'\hat', '^': r'\hat', '\u00af': r'\bar', '¯': r'\bar',
                 '\u2192': r'\vec', '→': r'\vec', '\u02dc': r'\tilde', '~': r'\tilde',
                 '\u0307': r'\dot', '˙': r'\dot', '\u0308': r'\ddot', '¨': r'\ddot',
                 'ˇ': r'\check', '´': r'\acute', '`': r'\grave'}
        cmd = marks.get(a, r'\hat')
        return (cmd + '{%s}' % inner) if cmd else inner

    if local == 'bar':  # 上下横线
        e_el = el.find(_mq('e'))
        return r'\overline{%s}' % kids(e_el).strip()

    if local == 'groupChr':  # 括号/花括号
        chr_el = el.find(_mq('chr'))
        e_el = el.find(_mq('e'))
        c = kids(chr_el).strip()
        pairs = {'{': r'\left\{ %s \right\}', '}': r'\left\{ %s \right\}',
                 '[': r'\left[ %s \right]', ']': r'\left[ %s \right]',
                 '(': r'\left( %s \right)', ')': r'\left( %s \right)',
                 '|': r'\left| %s \right|'}
        if c in pairs:
            return pairs[c] % kids(e_el).strip()
        return kids(e_el).strip()

    if local in ('rPr', 'ctrlPr', 'argPr', 'eqArrPr', 'naryPr', 'sSupPr',
                 'sSubPr', 'sSubSupPr', 'radPr', 'fPr', 'accPr', 'barPr',
                 'delimPr', 'funcPr', 'limLowPr', 'limUppPr', 'groupChrPr',
                 'phantPr', 'boxPr', 'borderBoxPr', 'mathPr', 'wrapPr',
                 'intLim', 'naryLim', 'subHide', 'supHide', 'mPr', 'mrPr'):
        return ''

    # 未知标签：取子节点文本兜底
    return kids(el)


def _run_font_lower(r):
    try:
        name = (r.font.name or '') or ''
    except Exception:  # noqa: BLE001
        name = ''
    if not name:
        rPr = r._r.find('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}rPr')
        if rPr is not None:
            rf = rPr.find('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}rFonts')
            if rf is not None:
                name = (rf.get('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}ascii')
                        or rf.get('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}hAnsi')
                        or '')
    return (name or '').lower()


def _para_has_mono(p):
    return any(_run_font_lower(r) in _MONO_FONTS for r in p.runs if (r.text or '').strip())


def _para_plain(p):
    return ''.join((r.text or '') for r in p.runs)


def _para_inline(p):
    parts = []
    for r in p.runs:
        t = r.text or ''
        if not t:
            continue
        if _run_font_lower(r) in _MONO_FONTS:
            t = '`' + t + '`'
        if r.bold:
            t = '**' + t + '**'
        if r.italic:
            t = '*' + t + '*'
        parts.append(t)
    return ''.join(parts)


def _lang_hint(text):
    low = (text or '').lower()
    for key, lang in _CODE_LANG_HINTS:
        if key in low:
            return lang
    return ''


def _para_inline_with_math(p, doc=None):
    """按段落子节点物理先后顺序提取文字、格式、超链接与 OMML 公式。"""
    from docx.text.run import Run
    parts = []

    for child in p._p:
        tag = child.tag
        if not isinstance(tag, str):
            continue
        local = tag.split('}')[-1]

        if local == 'r':
            r = Run(child, p)
            t = r.text or ''
            if not t:
                continue
            if _run_font_lower(r) in _MONO_FONTS:
                t = '`' + t + '`'
            if r.bold:
                t = '**' + t + '**'
            if r.italic:
                t = '*' + t + '*'
            parts.append(t)
        elif local == 'hyperlink':
            r_id = child.get('{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id')
            url = ''
            if r_id and doc is not None and hasattr(doc, 'part') and r_id in doc.part.rels:
                try:
                    url = doc.part.rels[r_id].target_ref
                except Exception:
                    url = ''
            link_text = ''.join((c.text or '') for c in child.findall('.//{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t'))
            if url and link_text:
                parts.append('[%s](%s)' % (link_text, url))
            elif link_text:
                parts.append(link_text)
        elif local == 'oMath':
            latex = _omml_to_latex(child).strip()
            if latex:
                parts.append('$%s$' % latex)
        elif local == 'oMathPara':
            for om in child.findall(_mq('oMath')):
                latex = _omml_to_latex(om).strip()
                if latex:
                    parts.append('$$%s$$' % latex)

    res = ''.join(parts).strip()
    # 如果段落内只有一个 $...$，且包含复杂结构，提升为独立公式块 $$...$$
    if res.startswith('$') and res.endswith('$') and not res.startswith('$$') and not res.endswith('$$'):
        inner_m = res[1:-1].strip()
        if len(inner_m) >= 60 or r'\begin{' in inner_m or r'\frac' in inner_m or r'\sum' in inner_m or r'\int' in inner_m or r'\aligned' in inner_m:
            res = '$$%s$$' % inner_m
    return res


def _table_to_md(tbl, doc=None):
    """w:tbl → 规整管道表（按实际 tc 遍历，支持单元格内公式与合并去重）。"""
    from docx.table import _Cell
    rows = []
    for tr in tbl._tbl.findall('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}tr'):
        cells = []
        seen = set()
        for tc in tr.findall('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}tc'):
            if id(tc) in seen:
                continue
            seen.add(id(tc))
            c = _Cell(tc, tbl)
            txt = '\n'.join(_para_inline_with_math(pp, doc) for pp in c.paragraphs)
            txt = txt.replace('\n', ' ').replace('|', '\\|').strip()
            cells.append(txt)
        rows.append(cells)
    if not rows:
        return ''
    ncol = max(len(r) for r in rows)
    rows = [r + [''] * (ncol - len(r)) for r in rows]
    out = ['| ' + ' | '.join(rows[0]) + ' |',
           '| ' + ' | '.join(['---'] * ncol) + ' |']
    for r in rows[1:]:
        out.append('| ' + ' | '.join(r) + ' |')
    return '\n'.join(out)


def docx2md(path):
    """docx → Markdown：OMML 公式→LaTeX、标题层级、表格、等宽字体代码块。"""
    from docx import Document
    from docx.oxml.ns import qn
    from docx.table import Table
    from docx.text.paragraph import Paragraph

    doc = Document(path)
    base_dir = os.path.dirname(os.path.abspath(path))
    lines = []
    code_buf = []
    code_lang = ''

    def flush_code():
        nonlocal code_buf, code_lang
        if code_buf:
            lines.append('```' + code_lang)
            lines.extend(code_buf)
            lines.append('```')
            lines.append('')
            code_buf, code_lang = [], ''

    def handle_para(p):
        nonlocal code_buf, code_lang
        style_name = ''
        try:
            style_name = (p.style.name or '') if p.style is not None else ''
        except Exception:  # noqa: BLE001
            style_name = ''
        if style_name and style_name.lower().startswith('heading'):
            flush_code()
            try:
                level = int(''.join(ch for ch in style_name if ch.isdigit()) or '1')
            except ValueError:
                level = 1
            level = max(1, min(6, level))
            txt = _para_inline_with_math(p, doc).strip()
            if txt:
                lines.append('#' * level + ' ' + txt)
                lines.append('')
            return
        if style_name and style_name.lower() == 'title':
            flush_code()
            txt = _para_inline_with_math(p, doc).strip()
            if txt:
                lines.append('# ' + txt)
                lines.append('')
            return
        if _para_has_mono(p) and not p._p.findall('.//' + _mq('oMath')):
            txt = _para_plain(p).strip()
            if txt:
                if not code_lang:
                    code_lang = _lang_hint(txt)
                code_buf.append(txt)
            return
        flush_code()
        txt = _para_inline_with_math(p, doc).strip()
        if not txt:
            return
        # 列表：段落级或样式级 numPr → "- "
        is_list = False
        ppr = p._p.find(qn('w:pPr'))
        if ppr is not None and ppr.find(qn('w:numPr')) is not None:
            is_list = True
        else:
            try:
                st = p.style
                depth = 0
                while st is not None and depth < 8:
                    spPr = st.element.find(qn('w:pPr'))
                    if spPr is not None and spPr.find(qn('w:numPr')) is not None:
                        is_list = True
                        break
                    st = st.base_style
                    depth += 1
            except Exception:  # noqa: BLE001
                is_list = False
        if is_list:
            lines.append('- ' + txt)
            lines.append('')
            return
        lines.append(txt)
        lines.append('')

    body = doc.element.body
    for child in body.iterchildren():
        if child.tag == qn('w:p'):
            handle_para(Paragraph(child, doc))
        elif child.tag == qn('w:tbl'):
            flush_code()
            md = _table_to_md(Table(child, doc), doc)
            if md:
                lines.append(md)
                lines.append('')
        elif child.tag == qn('w:sectPr'):
            continue
    flush_code()
    text = '\n'.join(lines).strip()
    if not text:
        raise ValueError('docx 未提取到文字内容')
    return text


# ---------------------------------------------------------------- pdf 专用

def _data_to_md(data):
    """表格二维数组 → 管道表。"""
    if not data:
        return ''
    ncol = max(len(r) for r in data)
    data = [r + [''] * (ncol - len(r)) for r in data]

    def clean(cell):
        s = (cell or '') if isinstance(cell, str) else str(cell or '')
        s = s.replace('\n', ' ').replace('\r', ' ').replace('|', '\\|').strip()
        return s

    out = ['| ' + ' | '.join(clean(c) for c in data[0]) + ' |',
           '| ' + ' | '.join(['---'] * ncol) + ' |']
    for r in data[1:]:
        out.append('| ' + ' | '.join(clean(c) for c in r) + ' |')
    return '\n'.join(out)


def _looks_like_formula(line):
    s = line.strip()
    if not s or len(s) < 2 or len(s) > 160:
        return False
    if any('\u4e00' <= ch <= '\u9fff' for ch in s):
        return False
    sig = sum(1 for ch in s if ch in _MATH_CHARS or ch.isdigit() or ch in '()[]{},.')
    return sig / max(len(s), 1) >= 0.45


def _page_to_md(page):
    try:
        import fitz
    except Exception:  # noqa: BLE001
        return ''
    tables = []
    try:
        tables = list(page.find_tables().tables)
    except Exception:  # noqa: BLE001
        tables = []
    tbl_boxes = []
    for t in tables:
        try:
            tbl_boxes.append(fitz.Rect(t.bbox))
        except Exception:  # noqa: BLE001
            pass

    def in_table(r):
        try:
            area = r.get_area()
            if area <= 0:
                return False
            for tb in tbl_boxes:
                inter = r & tb
                if not inter.is_empty and inter.get_area() > 0.5 * area:
                    return True
        except Exception:  # noqa: BLE001
            return False
        return False

    items = []
    seq = 0
    for t in tables:
        try:
            md = _data_to_md(t.extract())
        except Exception:  # noqa: BLE001
            md = ''
        if md:
            items.append((t.bbox[1], seq, 'table', md))
            seq += 1
    try:
        d = page.get_text('dict')
        for block in d.get('blocks', []):
            if block.get('type') != 0:
                continue
            for line in block.get('lines', []):
                bbox = fitz.Rect(line['bbox'])
                if in_table(bbox):
                    continue
                text = ''.join((sp.get('text') or '') for sp in line.get('spans', []))
                text = text.rstrip()
                if not text.strip():
                    continue
                items.append((bbox.y0, seq, 'text', text))
                seq += 1
    except Exception:  # noqa: BLE001
        pass
    items.sort(key=lambda it: (round(it[0], 1), it[1]))
    out = []
    for _y0, _seq, kind, payload in items:
        if kind == 'table':
            out.append(payload)
            out.append('')
        else:
            if _looks_like_formula(payload):
                out.append('$' + payload.strip() + '$')
            else:
                out.append(payload)
            out.append('')
    return '\n'.join(out).strip()


def pdf2md(path):
    import fitz
    doc = fitz.open(path)
    parts = []
    try:
        for page in doc:
            p = _page_to_md(page)
            if p.strip():
                parts.append(p)
    finally:
        doc.close()
    text = '\n\n'.join(parts).strip()
    if not text:
        raise ValueError('pdf 未提取到文字内容（可能是扫描件，请用 OCR）')
    return text
