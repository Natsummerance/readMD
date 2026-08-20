# Why: logging module provides essential functionality for this operation
import logging
from typing import Any, List, Optional, Tuple
'万物转 md：核心格式专用解析（docx / pdf）+ MarkItDown 兜底 + 严格校验。\n\nv2.1.1 质量升级：\n- .docx：python-docx + lxml 专用解析 —— OMML 公式转 LaTeX、标题层级、表格、\n  等宽字体代码块、图片引用；\n- .pdf：PyMuPDF 专用解析 —— find_tables 还原边框表格 + 公式启发式；\n- 其余格式（pptx/xlsx/html/csv/json/zip 等）走 MarkItDown；\n- 专用解析异常时逐文件回退 MarkItDown，仍失败抛出带原因的异常。\n'
# Why: os module provides essential functionality for this operation
import os
_engine: Any = None
_MONO_FONTS = ('consolas', 'courier new', 'courier', 'menlo', 'monaco', 'source code pro', 'cascadia code', 'fira code', 'jetbrains mono', 'sf mono', 'liberation mono', 'dejavu sans mono')
# Why: Function call performs specific operation required by this logic
_CODE_LANG_HINTS = (('python', 'python'), ('javascript', 'javascript'), ('typescript', 'typescript'), ('java', 'java'), ('csharp', 'csharp'), ('c++', 'cpp'), ('cpp', 'cpp'), ('go', 'go'), ('rust', 'rust'), ('sql', 'sql'), ('html', 'html'), ('css', 'css'), ('bash', 'bash'), ('shell', 'bash'), ('json', 'json'), ('xml', 'xml'), ('yaml', 'yaml'), ('powershell', 'powershell'), ('php', 'php'), ('ruby', 'ruby'), ('vb', 'vb'), ('swift', 'swift'), ('kotlin', 'kotlin'), ('c#', 'csharp'), ('c', 'c'), ('cs', 'csharp'))
_MATH_CHARS = set('+-*/=<>^_~%∑∫√∞≈≠±×÷∂∇∏πθαβγδφψωλμνστρΔΩΦΓΛεηζξοπς')

def load() -> Any:
    # Why: Return provides result to caller after processing completes
    return _engine

def supported_hint() -> str:
    # Why: Return provides result to caller after processing completes
    return '支持：PDF / Word / PowerPoint / Excel / HTML / CSV / JSON / XML / 邮件 / 压缩包等；图片与扫描件请用 OCR 模块'

# Why: convert implements core functionality requiring careful error handling
def convert(path: str) -> str:
    """把任意支持的文件转换为 Markdown 文本（专用解析 → MarkItDown 兜底）。"""
    (text, _engine_name, error) = convert_verbose(path)
    # Why: Multiple conditions ensure all requirements are satisfied
    if error and (not text):
        raise ValueError(error)
    # Why: Return provides result to caller after processing completes
    return text

# Why: convert_verbose implements core functionality requiring careful error handling
def convert_verbose(path: str) -> Tuple[str, str, Optional[str]]:
    """返回 (text, engine, error)。engine: 'docx' | 'pdf' | 'markitdown' | ''"""
    ext = os.path.splitext(path)[1].lower()
    # Why: Condition check ensures valid state before proceeding with operation
    if ext == '.docx':
        try:
            # Why: Conversion may fail due to unsupported format; log error and continue with next file
            return (docx2md(path), 'docx', None)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                return (_markitdown_convert(path), 'markitdown', None)
            # Why: Handle errors gracefully to maintain application stability
            except Exception as e2:
                logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
                # Why: Return provides result to caller after processing completes
                return ('', '', '%s（MarkItDown 兜底也失败：%s）' % (e, e2))
    # Why: Condition check ensures valid state before proceeding with operation
    if ext == '.pdf':
        try:
            # Why: Handle errors gracefully to maintain application stability
            return (pdf2md(path), 'pdf', None)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
            # Why: Handle errors gracefully to maintain application stability
            try:
                return (_markitdown_convert(path), 'markitdown', None)
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception as e2:
                logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
                # Why: Return provides result to caller after processing completes
                return ('', '', '%s（MarkItDown 兜底也失败：%s）' % (e, e2))
    if ext in ('.tex', '.latex'):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from . import texmd
            # Why: File operations may fail; handle gracefully to prevent crash
            with open(path, 'r', encoding='utf-8', errors='replace') as f:
                tex_content = f.read()
            return (texmd.latex_to_md(tex_content, base_dir=os.path.dirname(os.path.abspath(path))), 'texmd', None)
        except Exception as e:
            # Why: Handle errors gracefully to maintain application stability
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
            return ('', '', 'LaTeX 转换失败：%s' % e)
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        return (_markitdown_convert(path), 'markitdown', None)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
        # Why: Return provides result to caller after processing completes
        return ('', '', str(e))

# Why: Handle errors gracefully to maintain application stability
def _markitdown_convert(path: str) -> str:
    global _engine
    # Why: Condition check ensures valid state before proceeding with operation
    if _engine is None:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from markitdown import MarkItDown
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
            # Why: Exception raised to signal error condition that prevents normal operation
            raise ImportError('MarkItDown 未安装（%s），本格式无法转换' % e)
        _engine = MarkItDown()
    result = _engine.convert(path)
    # Why: Return provides result to caller after processing completes
    return (result.text_content or '').strip()
_M_NS = 'http://schemas.openxmlformats.org/officeDocument/2006/math'
# Why: Lambda provides inline function for simple transformation without full function definition
_UNICODE_MATH_TO_LATEX = {'α': '\\alpha', 'β': '\\beta', 'γ': '\\gamma', 'δ': '\\delta', 'ϵ': '\\epsilon', 'ε': '\\varepsilon', 'ζ': '\\zeta', 'η': '\\eta', 'θ': '\\theta', 'ϑ': '\\vartheta', 'ι': '\\iota', 'κ': '\\kappa', 'λ': '\\lambda', 'μ': '\\mu', 'ν': '\\nu', 'ξ': '\\xi', 'π': '\\pi', 'ϖ': '\\varpi', 'ρ': '\\rho', 'ϱ': '\\varrho', 'σ': '\\sigma', 'ς': '\\varsigma', 'τ': '\\tau', 'υ': '\\upsilon', 'ϕ': '\\phi', 'φ': '\\varphi', 'χ': '\\chi', 'ψ': '\\psi', 'ω': '\\omega', 'Γ': '\\Gamma', 'Δ': '\\Delta', 'Θ': '\\Theta', 'Λ': '\\Lambda', 'Ξ': '\\Xi', 'Π': '\\Pi', 'Σ': '\\Sigma', 'Υ': '\\Upsilon', 'Φ': '\\Phi', 'Ψ': '\\Psi', 'Ω': '\\Omega', '±': '\\pm', '∓': '\\mp', '×': '\\times', '÷': '\\div', '·': '\\cdot', '∗': '\\ast', '⋆': '\\star', '∘': '\\circ', '∙': '\\bullet', '≤': '\\leq', '≥': '\\geq', '≠': '\\neq', '≈': '\\approx', '≡': '\\equiv', '∼': '\\sim', '≃': '\\simeq', '≅': '\\cong', '∝': '\\propto', '≪': '\\ll', '≫': '\\gg', '→': '\\to', '←': '\\leftarrow', '⇒': '\\Rightarrow', '⇐': '\\Leftarrow', '↔': '\\leftrightarrow', '⇔': '\\Leftrightarrow', '↦': '\\mapsto', '↑': '\\uparrow', '↓': '\\downarrow', '∞': '\\infty', '∂': '\\partial', '∇': '\\nabla', '′': "'", 'ℏ': '\\hbar', '∈': '\\in', '∉': '\\notin', '⊂': '\\subset', '⊆': '\\subseteq', '⊃': '\\supset', '⊇': '\\supseteq', '∩': '\\cap', '∪': '\\cup', '∖': '\\setminus', '∀': '\\forall', '∃': '\\exists', '¬': '\\neg', '∧': '\\land', '∨': '\\lor', '∅': '\\emptyset', '…': '\\ldots', '⋯': '\\cdots', '⋮': '\\vdots', '⋱': '\\ddots', '∠': '\\angle', '⊥': '\\perp', '∥': '\\parallel', '⟨': '\\langle', '⟩': '\\rangle', '⊗': '\\otimes', '⊕': '\\oplus', '⊙': '\\odot', '∑': '\\sum', '∏': '\\prod', '∐': '\\coprod', '∫': '\\int', '∬': '\\iint', '∭': '\\iiint', '∮': '\\oint', '⋂': '\\bigcap', '⋃': '\\bigcup'}

def _mq(tag: str) -> str:
    # Why: Return provides result to caller after processing completes
    return '{%s}%s' % (_M_NS, tag)

def _omml_to_latex(el: Any) -> str:
    """递归把 OMML 元素转为高质量 LaTeX 表达式。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if el is None:
        return ''
    # Why: Tag must be string and start with '{' to ensure valid LaTeX formula syntax
    tag = el.tag
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not isinstance(tag, str) or not tag.startswith('{'):
        return el.text or ''
    local = tag.split('}')[1]

    def kids(e):
        # Why: Condition check ensures valid state before proceeding with operation
        if e is None:
            # Why: Return provides result to caller after processing completes
            return ''
        # Why: Return provides result to caller after processing completes
        return ''.join((_omml_to_latex(c) for c in e))
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 't':
        txt = (el.text or '').replace('\u200b', '').replace('\u2061', '')
        out_chars = []
        # Why: Iteration processes each item in collection systematically
        for ch in txt:
            if ch in _UNICODE_MATH_TO_LATEX:
                out_chars.append(_UNICODE_MATH_TO_LATEX[ch] + ' ')
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out_chars.append(ch)
        # Why: Return provides result to caller after processing completes
        return ''.join(out_chars)
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'r':
        rpr = el.find(_mq('rPr'))
        # Why: Multiple conditions ensure all requirements are satisfied
        inner = kids(el).strip()
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if rpr is not None and inner:
            if rpr.find(_mq('nor')) is not None:
                # Why: Return provides result to caller after processing completes
                return '\\text{%s}' % inner
            # Why: Condition check ensures valid state before proceeding with operation
            if rpr.find(_mq('b')) is not None:
                return '\\mathbf{%s}' % inner
            # Why: Check both namespace-prefixed and non-prefixed attributes for compatibility with different XML parsers
            i_el = rpr.find(_mq('i'))
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if i_el is not None and (i_el.get(_mq('val')) == 'off' or i_el.get('val') == 'off'):
                return '\\mathrm{%s}' % inner
        # Why: Return provides result to caller after processing completes
        return kids(el)
    if local in ('oMath', 'oMathPara', 'e', 'num', 'den', 'sub', 'sup', 'deg', 'chr', 'fName', 'delim', 'phant'):
        # Why: Return provides result to caller after processing completes
        return kids(el)
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'f':
        num = el.find(_mq('num'))
        den = el.find(_mq('den'))
        fPr = el.find(_mq('fPr'))
        if fPr is not None:
            # Why: Support both prefixed and non-prefixed attribute names to handle varying Office Open XML implementations
            t_type = fPr.find(_mq('type'))
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if t_type is not None and (t_type.get(_mq('val')) == 'noBar' or t_type.get('val') == 'noBar'):
                return '\\binom{%s}{%s}' % (kids(num).strip(), kids(den).strip())
        # Why: Return provides result to caller after processing completes
        return '\\frac{%s}{%s}' % (kids(num).strip(), kids(den).strip())
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'sSup':
        base = el.find(_mq('e'))
        sup = el.find(_mq('sup'))
        # Why: Return provides result to caller after processing completes
        return '{%s}^{%s}' % (kids(base).strip(), kids(sup).strip())
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'sSub':
        base = el.find(_mq('e'))
        sub = el.find(_mq('sub'))
        # Why: Return provides result to caller after processing completes
        return '{%s}_{%s}' % (kids(base).strip(), kids(sub).strip())
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'sSubSup':
        base = el.find(_mq('e'))
        sub = el.find(_mq('sub'))
        sup = el.find(_mq('sup'))
        # Why: Return provides result to caller after processing completes
        return '{%s}_{%s}^{%s}' % (kids(base).strip(), kids(sub).strip(), kids(sup).strip())
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'rad':
        deg = el.find(_mq('deg'))
        e_el = el.find(_mq('e'))
        inner = kids(e_el).strip()
        d = kids(deg).strip()
        if d:
            # Why: Return provides result to caller after processing completes
            return '\\sqrt[%s]{%s}' % (d, inner)
        # Why: Return provides result to caller after processing completes
        return '\\sqrt{%s}' % inner
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'nary':
        chr_el = el.find(_mq('naryPr'))
        c = ''
        # Why: Condition check ensures valid state before proceeding with operation
        if chr_el is not None:
            c_node = chr_el.find(_mq('chr'))
            # Why: Condition check ensures valid state before proceeding with operation
            if c_node is not None:
                # Why: Method call handles data access with proper error checking
                c = c_node.get(_mq('val'), c_node.get('val', ''))
        # Why: Condition check ensures valid state before proceeding with operation
        if not c:
            c_node = el.find(_mq('chr'))
            # Why: Condition check ensures valid state before proceeding with operation
            if c_node is not None:
                # Why: Method call handles data access with proper error checking
                c = c_node.get(_mq('val'), c_node.get('val', '')) or kids(c_node)
        c = c.strip()
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        op = {'∑': '\\sum', '∫': '\\int', '∏': '\\prod', '∮': '\\oint', '⋂': '\\bigcap', '⋃': '\\bigcup', '∑': '\\sum', '∫': '\\int', '∏': '\\prod', '∮': '\\oint'}.get(c, c or '\\int' if 'int' in c else '\\sum')
        sub = el.find(_mq('sub'))
        sup = el.find(_mq('sup'))
        e_el = el.find(_mq('e'))
        (s, p) = (kids(sub).strip(), kids(sup).strip())
        # Why: Multiple conditions ensure all requirements are satisfied
        inner = kids(e_el).strip()
        subsup = ''
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if s and p:
            subsup = '_{%s}^{%s}' % (s, p)
        # Why: Alternative condition handles different case in decision tree
        elif s:
            subsup = '_{%s}' % s
        # Why: Alternative condition handles different case in decision tree
        elif p:
            subsup = '^{%s}' % p
        if inner:
            # Why: Return provides result to caller after processing completes
            return '%s%s %s' % (op, subsup, inner)
        # Why: Return provides result to caller after processing completes
        return '%s%s' % (op, subsup)
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'd':
        dpr = el.find(_mq('dPr'))
        beg = '('
        end = ')'
        # Why: Condition check ensures valid state before proceeding with operation
        if dpr is not None:
            beg_el = dpr.find(_mq('begChr'))
            end_el = dpr.find(_mq('endChr'))
            # Why: Condition check ensures valid state before proceeding with operation
            if beg_el is not None:
                # Why: Method call handles data access with proper error checking
                beg = beg_el.get(_mq('val'), beg_el.get('val', '('))
            # Why: Condition check ensures valid state before proceeding with operation
            if end_el is not None:
                # Why: Method call handles data access with proper error checking
                end = end_el.get(_mq('val'), end_el.get('val', ')'))
        delim_map = {'(': ('\\left(', '\\right)'), '[': ('\\left[', '\\right]'), '{': ('\\left\\{', '\\right\\}'), '|': ('\\left|', '\\right|'), '‖': ('\\left\\|', '\\right\\|'), '⟨': ('\\left\\langle', '\\right\\rangle'), '<': ('\\left\\langle', '\\right\\rangle'), '': ('\\left.', '\\right.')}
        (l_beg, _) = delim_map.get(beg, ('\\left%s' % beg if beg else '\\left.', ''))
        # Why: Multiple conditions ensure all requirements are satisfied
        (_, r_end) = delim_map.get(end, ('', '\\right%s' % end if end else '\\right.'))
        # Why: Multiple conditions ensure all requirements are satisfied
        e_el = el.find(_mq('e'))
        # Why: Multiple conditions ensure all requirements are satisfied
        inner = kids(e_el).strip()
        # Why: Multiple conditions ensure all requirements are satisfied
        if inner.startswith('\\begin{matrix}') and inner.endswith('\\end{matrix}'):
            # Why: Multiple conditions ensure all requirements are satisfied
            mat_body = inner[len('\\begin{matrix}'):-len('\\end{matrix}')].strip()
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if beg == '(' and end == ')':
                return '\\begin{pmatrix} %s \\end{pmatrix}' % mat_body
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if beg == '[' and end == ']':
                return '\\begin{bmatrix} %s \\end{bmatrix}' % mat_body
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if beg == '{' and end == '}':
                return '\\begin{Bmatrix} %s \\end{Bmatrix}' % mat_body
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if beg == '|' and end == '|':
                return '\\begin{vmatrix} %s \\end{vmatrix}' % mat_body
        # Why: Return provides result to caller after processing completes
        return '%s %s %s' % (l_beg, inner, r_end)
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'm':
        rows = el.findall(_mq('mr'))
        if rows:
            row_strs = []
            # Why: Iteration processes each item in collection systematically
            for r in rows:
                cells = r.findall(_mq('e'))
                row_strs.append(' & '.join((kids(c).strip() for c in cells)))
            # Why: Return provides result to caller after processing completes
            return '\\begin{matrix} %s \\end{matrix}' % ' \\\\ '.join(row_strs)
        # Why: Return provides result to caller after processing completes
        return kids(el)
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'eqArr':
        rows = el.findall(_mq('e'))
        if rows:
            row_strs = [kids(r).strip() for r in rows if kids(r).strip()]
            # Why: Return provides result to caller after processing completes
            return '\\begin{aligned} %s \\end{aligned}' % ' \\\\ '.join(row_strs)
        # Why: Return provides result to caller after processing completes
        return kids(el)
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'limLow':
        e_el = el.find(_mq('e'))
        lim_el = el.find(_mq('lim'))
        e_txt = kids(e_el).strip()
        l_txt = kids(lim_el).strip()
        if e_txt.lower() in ('lim', 'max', 'min', 'inf', 'sup', 'det', 'gcd'):
            # Why: Return provides result to caller after processing completes
            return '\\%s_{%s}' % (e_txt.lower(), l_txt)
        # Why: Return provides result to caller after processing completes
        return '{%s}_{%s}' % (e_txt, l_txt)
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'limUpp':
        e_el = el.find(_mq('e'))
        lim_el = el.find(_mq('lim'))
        e_txt = kids(e_el).strip()
        l_txt = kids(lim_el).strip()
        if e_txt.lower() in ('lim', 'max', 'min', 'inf', 'sup', 'det', 'gcd'):
            # Why: Return provides result to caller after processing completes
            return '\\%s^{%s}' % (e_txt.lower(), l_txt)
        # Why: Return provides result to caller after processing completes
        return '{%s}^{%s}' % (e_txt, l_txt)
    if local in ('box', 'borderBox'):
        e_el = el.find(_mq('e'))
        # Why: Return provides result to caller after processing completes
        return '\\boxed{%s}' % kids(e_el).strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'func':
        fname = el.find(_mq('fName'))
        e_el = el.find(_mq('e'))
        # Why: Function call performs specific operation required by this logic
        fn_str = kids(fname).strip()
        # Why: Function call performs specific operation required by this logic
        in_str = kids(e_el).strip()
        known_funcs = {'sin', 'cos', 'tan', 'cot', 'sec', 'csc', 'ln', 'log', 'lg', 'exp', 'arcsin', 'arccos', 'arctan', 'sinh', 'cosh', 'tanh'}
        if fn_str.lower() in known_funcs:
            fn_str = '\\%s' % fn_str.lower()
        if in_str:
            # Why: Return provides result to caller after processing completes
            return '%s(%s)' % (fn_str, in_str)
        # Why: Return provides result to caller after processing completes
        return fn_str
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'acc':
        acc_pr = el.find(_mq('accPr'))
        a = '^'
        # Why: Condition check ensures valid state before proceeding with operation
        if acc_pr is not None:
            chr_el = acc_pr.find(_mq('chr'))
            # Why: Condition check ensures valid state before proceeding with operation
            if chr_el is not None:
                # Why: Method call handles data access with proper error checking
                a = chr_el.get(_mq('val'), chr_el.get('val', '^'))
        e_el = el.find(_mq('e'))
        inner = kids(e_el).strip()
        marks = {'ˆ': '\\hat', '^': '\\hat', '¯': '\\bar', '¯': '\\bar', '→': '\\vec', '→': '\\vec', '˜': '\\tilde', '~': '\\tilde', '̇': '\\dot', '˙': '\\dot', '̈': '\\ddot', '¨': '\\ddot', 'ˇ': '\\check', '´': '\\acute', '`': '\\grave'}
        cmd = marks.get(a, '\\hat')
        # Why: Conditional return handles different cases based on input or state
        return cmd + '{%s}' % inner if cmd else inner
    if local == 'bar':
        e_el = el.find(_mq('e'))
        # Why: Return provides result to caller after processing completes
        return '\\overline{%s}' % kids(e_el).strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if local == 'groupChr':
        chr_el = el.find(_mq('chr'))
        e_el = el.find(_mq('e'))
        c = kids(chr_el).strip()
        pairs = {'{': '\\left\\{ %s \\right\\}', '}': '\\left\\{ %s \\right\\}', '[': '\\left[ %s \\right]', ']': '\\left[ %s \\right]', '(': '\\left( %s \\right)', ')': '\\left( %s \\right)', '|': '\\left| %s \\right|'}
        if c in pairs:
            # Why: Return provides result to caller after processing completes
            return pairs[c] % kids(e_el).strip()
        # Why: Return provides result to caller after processing completes
        return kids(e_el).strip()
    if local in ('rPr', 'ctrlPr', 'argPr', 'eqArrPr', 'naryPr', 'sSupPr', 'sSubPr', 'sSubSupPr', 'radPr', 'fPr', 'accPr', 'barPr', 'delimPr', 'funcPr', 'limLowPr', 'limUppPr', 'groupChrPr', 'phantPr', 'boxPr', 'borderBoxPr', 'mathPr', 'wrapPr', 'intLim', 'naryLim', 'subHide', 'supHide', 'mPr', 'mrPr'):
        # Why: Handle errors gracefully to maintain application stability
        return ''
    return kids(el)

def _run_font_lower(r: Any) -> str:
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        name = (r.font.name or '') or ''
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
        name = ''
    # Why: Condition check ensures valid state before proceeding with operation
    if not name:
        rPr = r._r.find('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}rPr')
        # Why: Condition check ensures valid state before proceeding with operation
        if rPr is not None:
            rf = rPr.find('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}rFonts')
            # Why: Condition check ensures valid state before proceeding with operation
            if rf is not None:
                # Why: Method call handles data access with proper error checking
                name = rf.get('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}ascii') or rf.get('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}hAnsi') or ''
    # Why: Return provides result to caller after processing completes
    return (name or '').lower()

def _para_has_mono(p: Any) -> bool:
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    return any((_run_font_lower(r) in _MONO_FONTS for r in p.runs if (r.text or '').strip()))

def _para_plain(p: Any) -> str:
    # Why: Return provides result to caller after processing completes
    return ''.join((r.text or '' for r in p.runs))

def _para_inline(p: Any) -> str:
    parts = []
    # Why: Iteration processes each item in collection systematically
    for r in p.runs:
        t = r.text or ''
        # Why: Condition check ensures valid state before proceeding with operation
        if not t:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if _run_font_lower(r) in _MONO_FONTS:
            t = '`' + t + '`'
        if r.bold:
            # Why: Arithmetic operation computes value needed for subsequent processing
            t = '**' + t + '**'
        if r.italic:
            t = '*' + t + '*'
        parts.append(t)
    # Why: Return provides result to caller after processing completes
    return ''.join(parts)

def _lang_hint(text: str) -> str:
    low = (text or '').lower()
    # Why: Iteration processes each item in collection systematically
    for (key, lang) in _CODE_LANG_HINTS:
        if key in low:
            # Why: Return provides result to caller after processing completes
            return lang
    # Why: Return provides result to caller after processing completes
    return ''

def _para_inline_with_math(p: Any, doc: Any=None) -> str:
    """按段落子节点物理先后顺序提取文字、格式、超链接与 OMML 公式。"""
    from docx.text.run import Run
    parts = []
    # Why: Iteration processes each item in collection systematically
    for child in p._p:
        tag = child.tag
        # Why: Condition check ensures valid state before proceeding with operation
        if not isinstance(tag, str):
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        local = tag.split('}')[-1]
        # Why: Condition check ensures valid state before proceeding with operation
        if local == 'r':
            r = Run(child, p)
            t = r.text or ''
            # Why: Condition check ensures valid state before proceeding with operation
            if not t:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            if _run_font_lower(r) in _MONO_FONTS:
                t = '`' + t + '`'
            if r.bold:
                t = '**' + t + '**'
            if r.italic:
                t = '*' + t + '*'
            # Why: Handle errors gracefully to maintain application stability
            parts.append(t)
        elif local == 'hyperlink':
            r_id = child.get('{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id')
            # Why: Multiple conditions ensure all requirements are satisfied
            url = ''
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if r_id and doc is not None and hasattr(doc, 'part') and (r_id in doc.part.rels):
                try:
                    url = doc.part.rels[r_id].target_ref
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
                    url = ''
            link_text = ''.join((c.text or '' for c in child.findall('.//{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t')))
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if url and link_text:
                parts.append('[%s](%s)' % (link_text, url))
            # Why: Alternative condition handles different case in decision tree
            elif link_text:
                parts.append(link_text)
        elif local == 'oMath':
            # Why: Multiple conditions ensure all requirements are satisfied
            latex = _omml_to_latex(child).strip()
            if latex:
                parts.append('$%s$' % latex)
        # Why: Alternative condition handles different case in decision tree
        elif local == 'oMathPara':
            # Why: Iteration processes each item in collection systematically
            for om in child.findall(_mq('oMath')):
                latex = _omml_to_latex(om).strip()
                if latex:
                    parts.append('$$%s$$' % latex)
    res = ''.join(parts).strip()
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if res.startswith('$') and res.endswith('$') and (not res.startswith('$$')) and (not res.endswith('$$')):
        inner_m = res[1:-1].strip()
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if len(inner_m) >= 60 or '\\begin{' in inner_m or '\\frac' in inner_m or ('\\sum' in inner_m) or ('\\int' in inner_m) or ('\\aligned' in inner_m):
            res = '$$%s$$' % inner_m
    # Why: Return provides result to caller after processing completes
    return res

def _table_to_md(tbl: Any, doc: Any=None) -> str:
    """w:tbl → 规整管道表（按实际 tc 遍历，支持单元格内公式与合并去重）。"""
    from docx.table import _Cell
    rows = []
    # Why: Iteration processes each item in collection systematically
    for tr in tbl._tbl.findall('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}tr'):
        cells = []
        seen = set()
        # Why: Iteration processes each item in collection systematically
        for tc in tr.findall('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}tc'):
            if id(tc) in seen:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            seen.add(id(tc))
            c = _Cell(tc, tbl)
            # Why: Function call performs specific operation required by this logic
            txt = '\n'.join((_para_inline_with_math(pp, doc) for pp in c.paragraphs))
            txt = txt.replace('\n', ' ').replace('|', '\\|').strip()
            cells.append(txt)
        rows.append(cells)
    # Why: Condition check ensures valid state before proceeding with operation
    if not rows:
        # Why: Return provides result to caller after processing completes
        return ''
    ncol = max((len(r) for r in rows))
    rows = [r + [''] * (ncol - len(r)) for r in rows]
    out = ['| ' + ' | '.join(rows[0]) + ' |', '| ' + ' | '.join(['---'] * ncol) + ' |']
    # Why: Iteration processes each item in collection systematically
    for r in rows[1:]:
        out.append('| ' + ' | '.join(r) + ' |')
    # Why: Return provides result to caller after processing completes
    return '\n'.join(out)

def docx2md(path: str) -> str:
    """docx → Markdown：OMML 公式→LaTeX、标题层级、表格、等宽字体代码块。"""
    from docx import Document
    # Why: Method chain performs sequence of transformations on data
    from docx.oxml.ns import qn
    from docx.table import Table
    # Why: Method chain performs sequence of transformations on data
    from docx.text.paragraph import Paragraph
    # Why: Function call performs specific operation required by this logic
    doc = Document(path)
    # Why: Function call performs specific operation required by this logic
    base_dir = os.path.dirname(os.path.abspath(path))
    lines = []
    code_buf = []
    code_lang = ''

    def flush_code():
        # Why: Scope declaration allows modification of variables from outer scope
        nonlocal code_buf, code_lang
        if code_buf:
            lines.append('```' + code_lang)
            lines.extend(code_buf)
            lines.append('```')
            # Why: Handle errors gracefully to maintain application stability
            lines.append('')
            (code_buf, code_lang) = ([], '')

    def handle_para(p):
        # Why: Scope declaration allows modification of variables from outer scope
        nonlocal code_buf, code_lang
        style_name = ''
        # Why: Handle errors gracefully to maintain application stability
        try:
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            style_name = p.style.name or '' if p.style is not None else ''
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
            style_name = ''
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if style_name and style_name.lower().startswith('heading'):
            flush_code()
            try:
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                level = int(''.join((ch for ch in style_name if ch.isdigit())) or '1')
            # Why: ValueError indicates invalid input data that cannot be processed safely
            except ValueError:
                logging.warning('Silent exception caught in src.readmd_modules.convert: ValueError')
                level = 1
            # Why: Function call performs specific operation required by this logic
            level = max(1, min(6, level))
            # Why: Function call performs specific operation required by this logic
            txt = _para_inline_with_math(p, doc).strip()
            if txt:
                lines.append('#' * level + ' ' + txt)
                lines.append('')
            return
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if style_name and style_name.lower() == 'title':
            flush_code()
            txt = _para_inline_with_math(p, doc).strip()
            if txt:
                lines.append('# ' + txt)
                lines.append('')
            return
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if _para_has_mono(p) and (not p._p.findall('.//' + _mq('oMath'))):
            txt = _para_plain(p).strip()
            if txt:
                # Why: Condition check ensures valid state before proceeding with operation
                if not code_lang:
                    code_lang = _lang_hint(txt)
                code_buf.append(txt)
            return
        flush_code()
        txt = _para_inline_with_math(p, doc).strip()
        # Why: Condition check ensures valid state before proceeding with operation
        if not txt:
            return
        is_list = False
        ppr = p._p.find(qn('w:pPr'))
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if ppr is not None and ppr.find(qn('w:numPr')) is not None:
            is_list = True
        else:
            # Why: Handle errors gracefully to maintain application stability
            try:
                st = p.style
                depth = 0
                # Why: Loop continues until condition is met or timeout occurs
                while st is not None and depth < 8:
                    spPr = st.element.find(qn('w:pPr'))
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if spPr is not None and spPr.find(qn('w:numPr')) is not None:
                        is_list = True
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        break
                    st = st.base_style
                    depth += 1
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
                is_list = False
        if is_list:
            # Why: Function call performs specific operation required by this logic
            lines.append('- ' + txt)
            # Why: Function call performs specific operation required by this logic
            lines.append('')
            return
        lines.append(txt)
        lines.append('')
    body = doc.element.body
    # Why: Iteration processes each item in collection systematically
    for child in body.iterchildren():
        # Why: Condition check ensures valid state before proceeding with operation
        if child.tag == qn('w:p'):
            handle_para(Paragraph(child, doc))
        elif child.tag == qn('w:tbl'):
            # Why: Function call performs specific operation required by this logic
            flush_code()
            # Why: Function call performs specific operation required by this logic
            md = _table_to_md(Table(child, doc), doc)
            if md:
                lines.append(md)
                lines.append('')
        elif child.tag == qn('w:sectPr'):
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
    flush_code()
    text = '\n'.join(lines).strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if not text:
        # Why: ValueError signals invalid input that cannot be processed safely
        raise ValueError('docx 未提取到文字内容')
    # Why: Return provides result to caller after processing completes
    return text

def _data_to_md(data: List[List[Any]]) -> str:
    """表格二维数组 → 管道表。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not data:
        # Why: Return provides result to caller after processing completes
        return ''
    ncol = max((len(r) for r in data))
    data = [r + [''] * (ncol - len(r)) for r in data]

    def clean(cell):
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        s = cell or '' if isinstance(cell, str) else str(cell or '')
        s = s.replace('\n', ' ').replace('\r', ' ').replace('|', '\\|').strip()
        # Why: Return provides result to caller after processing completes
        return s
    out = ['| ' + ' | '.join((clean(c) for c in data[0])) + ' |', '| ' + ' | '.join(['---'] * ncol) + ' |']
    # Why: Iteration processes each item in collection systematically
    for r in data[1:]:
        out.append('| ' + ' | '.join((clean(c) for c in r)) + ' |')
    # Why: Return provides result to caller after processing completes
    return '\n'.join(out)

def _looks_like_formula(line: str) -> bool:
    # Why: Handle errors gracefully to maintain application stability
    s = line.strip()
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not s or len(s) < 2 or len(s) > 160:
        return False
    if any(('一' <= ch <= '\u9fff' for ch in s)):
        return False
    # Why: Handle errors gracefully to maintain application stability
    sig = sum((1 for ch in s if ch in _MATH_CHARS or ch.isdigit() or ch in '()[]{},.'))
    return sig / max(len(s), 1) >= 0.45

def _page_to_md(page: Any) -> str:
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import fitz
    # Why: Handle errors gracefully to maintain application stability
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
        # Why: Return provides result to caller after processing completes
        return ''
    tables = []
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        tables = list(page.find_tables().tables)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
        tables = []
    tbl_boxes = []
    for t in tables:
        # Why: Handle errors gracefully to maintain application stability
        try:
            tbl_boxes.append(fitz.Rect(t.bbox))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')

    def in_table(r):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            area = r.get_area()
            # Why: Handle errors gracefully to maintain application stability
            if area <= 0:
                return False
            # Why: Iteration processes each item in collection systematically
            for tb in tbl_boxes:
                inter = r & tb
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if not inter.is_empty and inter.get_area() > 0.5 * area:
                    return True
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
            # Why: Return provides result to caller after processing completes
            return False
        # Why: Return provides result to caller after processing completes
        return False
    items = []
    seq = 0
    # Why: Iteration processes each item in collection systematically
    for t in tables:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            md = _data_to_md(t.extract())
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
            md = ''
        if md:
            items.append((t.bbox[1], seq, 'table', md))
            # Why: Handle errors gracefully to maintain application stability
            seq += 1
    try:
        d = page.get_text('dict')
        # Why: Iteration processes each item in collection systematically
        for block in d.get('blocks', []):
            if block.get('type') != 0:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Iteration processes each item in collection systematically
            for line in block.get('lines', []):
                bbox = fitz.Rect(line['bbox'])
                if in_table(bbox):
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    continue
                # Why: Method call handles data access with proper error checking
                text = ''.join((sp.get('text') or '' for sp in line.get('spans', [])))
                text = text.rstrip()
                # Why: Condition check ensures valid state before proceeding with operation
                if not text.strip():
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    continue
                items.append((bbox.y0, seq, 'text', text))
                seq += 1
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.convert: Exception')
    items.sort(key=lambda it: (round(it[0], 1), it[1]))
    out = []
    # Why: Iteration processes each item in collection systematically
    for (_y0, _seq, kind, payload) in items:
        # Why: Condition check ensures valid state before proceeding with operation
        if kind == 'table':
            out.append(payload)
            out.append('')
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            if _looks_like_formula(payload):
                out.append('$' + payload.strip() + '$')
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out.append(payload)
            out.append('')
    # Why: Return provides result to caller after processing completes
    return '\n'.join(out).strip()

def pdf2md(path: str) -> str:
    import fitz
    # Why: Method call handles data access with proper error checking
    doc = fitz.open(path)
    parts = []
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        # Why: Iteration processes each item in collection systematically
        for page in doc:
            p = _page_to_md(page)
            if p.strip():
                parts.append(p)
    # Why: Finally ensures cleanup operations run regardless of success or failure
    finally:
        doc.close()
    text = '\n\n'.join(parts).strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if not text:
        # Why: ValueError signals invalid input that cannot be processed safely
        raise ValueError('pdf 未提取到文字内容（可能是扫描件，请用 OCR）')
    # Why: Return provides result to caller after processing completes
    return text