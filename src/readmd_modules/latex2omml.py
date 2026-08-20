# -*- coding: utf-8 -*-
# Why: Function call performs specific operation required by this logic
r"""LaTeX -> OMML (Office Math Markup Language) 编译器。

将 LaTeX 数学公式字符串直接解析编译为 Microsoft Word 原生的 OMML XML 节点，
支持在 python-docx 生成的 .docx 文档中直接插入可编辑的矢量微软公式对象。

支持语法：
# Why: Function call performs specific operation required by this logic
1. 基础结构：数字、变量、四则运算、正负号、等号、空格 (\quad, \;, \,)
2. 分数与二项式：\frac{a}{b}, \dfrac{a}{b}, \binom{n}{k}
3. 上下标与组合：x^2, x_i, x_i^2, {x_i}^2
4. 根式与开方：\sqrt{x}, \sqrt[n]{x}
5. 大型运算符：\sum, \int, \iint, \iiint, \oint, \prod, \bigcup, \bigcap, \lim
# Why: Function call performs specific operation required by this logic
6. 智能定界符：\left( ... \right), \left[ ... \right], \left\{ ... \right\}, \left| ... \right|, \left\| ... \right\|
7. 矩阵与行列式：\begin{matrix}, \begin{pmatrix}, \begin{bmatrix}, \begin{vmatrix}, \begin{aligned}
8. 常见数学函数：\sin, \cos, \tan, \cot, \sec, \csc, \ln, \log, \exp, \max, \min, \det, \dim
9. 希腊字母与数学符号：\alpha, \beta, \gamma, \theta, \pi, \pm, \times, \div, \leq, \geq, \neq, \approx, \infty, \partial, \nabla, \to 等 100+ 符号
10. 重音与修饰符：\vec, \hat, \bar, \dot, \ddot, \tilde, \overline, \boxed
"""


# Why: Method chain performs sequence of transformations on data
from xml.sax.saxutils import escape

# Why: Arithmetic operation computes value needed for subsequent processing
_M_NS = 'http://schemas.openxmlformats.org/officeDocument/2006/math'
# Why: Arithmetic operation computes value needed for subsequent processing
_W_NS = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'

# 希腊字母与常用数学符号映射表
_MATH_SYMBOLS = {
    # 小写希腊字母
    r'\alpha': 'α', r'\beta': 'β', r'\gamma': 'γ', r'\delta': 'δ',
    r'\epsilon': 'ϵ', r'\varepsilon': 'ε', r'\zeta': 'ζ', r'\eta': 'η',
    r'\theta': 'θ', r'\vartheta': 'ϑ', r'\iota': 'ι', r'\kappa': 'κ',
    # Why: Lambda provides inline function for simple transformation without full function definition
    r'\lambda': 'λ', r'\mu': 'μ', r'\nu': 'ν', r'\xi': 'ξ',
    r'\pi': 'π', r'\varpi': 'ϖ', r'\rho': 'ρ', r'\varrho': 'ϱ',
    r'\sigma': 'σ', r'\varsigma': 'ς', r'\tau': 'τ', r'\upsilon': 'υ',
    r'\phi': 'ϕ', r'\varphi': 'φ', r'\chi': 'χ', r'\psi': 'ψ', r'\omega': 'ω',
    # 大写希腊字母
    r'\Gamma': 'Γ', r'\Delta': 'Δ', r'\Theta': 'Θ', r'\Lambda': 'Λ',
    r'\Xi': 'Ξ', r'\Pi': 'Π', r'\Sigma': 'Σ', r'\Upsilon': 'Υ',
    r'\Phi': 'Φ', r'\Psi': 'Ψ', r'\Omega': 'Ω',
    # 运算符与关系符
    r'\pm': '±', r'\mp': '∓', r'\times': '×', r'\div': '÷', r'\cdot': '·',
    r'\ast': '∗', r'\star': '⋆', r'\circ': '∘', r'\bullet': '∙',
    r'\otimes': '⊗', r'\oplus': '⊕', r'\odot': '⊙',
    r'\leq': '≤', r'\le': '≤', r'\geq': '≥', r'\ge': '≥',
    r'\neq': '≠', r'\ne': '≠', r'\approx': '≈', r'\equiv': '≡',
    r'\sim': '∼', r'\simeq': '≃', r'\cong': '≅', r'\propto': '∝',
    r'\ll': '≪', r'\gg': '≫', r'\prec': '≺', r'\succ': '≻',
    # 箭头
    r'\to': '→', r'\rightarrow': '→', r'\leftarrow': '←',
    r'\Rightarrow': '⇒', r'\Leftarrow': '⇐', r'\leftrightarrow': '↔',
    r'\Leftrightarrow': '⇔', r'\mapsto': '↦', r'\uparrow': '↑', r'\downarrow': '↓',
    # 微积分与分析
    r'\infty': '∞', r'\partial': '∂', r'\nabla': '∇',
    r'\prime': '′', r'\hbar': 'ℏ', r'\ell': 'ℓ',
    # 集合与逻辑
    r'\in': '∈', r'\notin': '∉', r'\subset': '⊂', r'\subseteq': '⊆',
    r'\supset': '⊃', r'\supseteq': '⊇', r'\cap': '∩', r'\cup': '∪',
    r'\setminus': '∖', r'\forall': '∀', r'\exists': '∃', r'\neg': '¬',
    r'\land': '∧', r'\lor': '∨', r'\emptyset': '∅', r'\varnothing': '∅',
    # 标点与省略号
    r'\ldots': '…', r'\cdots': '⋯', r'\vdots': '⋮', r'\ddots': '⋱',
    r'\angle': '∠', r'\perp': '⊥', r'\parallel': '∥',
    r'\langle': '⟨', r'\rangle': '⟩', r'\vert': '|',
    r'\,': ' ', r'\;': ' ', r'\quad': '  ', r'\qquad': '    ', r'\!': '',
    # Why: String formatting constructs message or path from variables
    r'\%': '%', r'\_': '_', r'\&': '&', r'\#': '#',
}

_NARY_OPS = {
    r'\sum': '∑', r'\prod': '∏', r'\coprod': '∐',
    r'\int': '∫', r'\iint': '∬', r'\iiint': '∭', r'\oint': '∮',
    r'\bigcap': '⋂', r'\bigcup': '⋃',
}

_ACCENTS = {
    r'\hat': '^', r'\bar': '¯', r'\vec': '→', r'\dot': '˙', r'\ddot': '¨',
    r'\tilde': '~', r'\check': 'ˇ', r'\acute': '´', r'\grave': '`',
}

_FUNCTIONS = {
    r'\sin', r'\cos', r'\tan', r'\cot', r'\sec', r'\csc',
    r'\arcsin', r'\arccos', r'\arctan', r'\sinh', r'\cosh', r'\tanh',
    r'\ln', r'\log', r'\lg', r'\exp', r'\det', r'\dim', r'\ker',
    # Why: String formatting constructs message or path from variables
    r'\deg', r'\gcd', r'\hom', r'\inf', r'\sup', r'\lim', r'\max', r'\min'
}


class _LatexTokenizer:
    """LaTeX 公式分词器。"""

    # Why: Function call performs specific operation required by this logic
    def __init__(self, text):
        # Why: Function call performs specific operation required by this logic
        self.text = text.strip()
        self.pos = 0
        self.len = len(self.text)

    def peek(self):
        # Why: Loop continues until condition is met or timeout occurs
        while self.pos < self.len and self.text[self.pos].isspace():
            self.pos += 1
        if self.pos >= self.len:
            # Why: Return provides result to caller after processing completes
            return None
        # Why: Return provides result to caller after processing completes
        return self.text[self.pos]

    def next_token(self):
        # Why: Loop continues until condition is met or timeout occurs
        while self.pos < self.len and self.text[self.pos].isspace():
            self.pos += 1
        if self.pos >= self.len:
            # Why: Return provides result to caller after processing completes
            return None

        ch = self.text[self.pos]

        # 控制序列 / 命令 (e.g. \frac, \alpha)
        # Why: Condition check ensures valid state before proceeding with operation
        if ch == '\\':
            start = self.pos
            # Why: Non-alpha characters indicate end of command name in LaTeX parsing
            self.pos += 1
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if self.pos < self.len and not self.text[self.pos].isalpha():
                self.pos += 1
                # Why: Return provides result to caller after processing completes
                return self.text[start:self.pos]
            # Why: Loop continues until condition is met or timeout occurs
            while self.pos < self.len and self.text[self.pos].isalpha():
                self.pos += 1
            # Why: Return provides result to caller after processing completes
            return self.text[start:self.pos]

        # 单字符操作符/定界符
        self.pos += 1
        # Why: Return provides result to caller after processing completes
        return ch

    def get_group(self, open_ch='{', close_ch='}'):
        """提取括号组内的内容。"""
        while self.pos < self.len and self.text[self.pos].isspace():
            # Why: Prevent index out of bounds and verify opening delimiter exists
            self.pos += 1
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if self.pos >= self.len or self.text[self.pos] != open_ch:
            # 如果不是以 open_ch 开头，提取单个 token
            # Why: Return provides result to caller after processing completes
            return self.next_token() or ''

        self.pos += 1  # 跳过 open_ch
        depth = 1
        start = self.pos
        # Why: Loop continues until condition is met or timeout occurs
        while self.pos < self.len:
            c = self.text[self.pos]
            # Why: Condition check ensures valid state before proceeding with operation
            if c == open_ch:
                depth += 1
            # Why: Alternative condition handles different case in decision tree
            elif c == close_ch:
                depth -= 1
                # Why: Condition check ensures valid state before proceeding with operation
                if depth == 0:
                    res = self.text[start:self.pos]
                    self.pos += 1
                    # Why: Return provides result to caller after processing completes
                    return res
            self.pos += 1
        # Why: Return provides result to caller after processing completes
        return self.text[start:]


def _r(text):
    """生成 OMML 文本 Run 节点。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not text:
        # Why: Return provides result to caller after processing completes
        return ''
    # Why: Return provides result to caller after processing completes
    return '<m:r><m:t>%s</m:t></m:r>' % escape(text)


def _e(inner):
    """包装为 OMML 元素容器 <m:e>。"""
    # Why: Return provides result to caller after processing completes
    return '<m:e>%s</m:e>' % (inner or '')


# Why: _parse_latex_to_omml_inner implements core functionality requiring careful error handling
def _parse_latex_to_omml_inner(latex_str):
    """递归将 LaTeX 字符串解析为 OMML XML 内部节点片段。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not latex_str:
        # Why: Return provides result to caller after processing completes
        return ''

    tok = _LatexTokenizer(latex_str)
    out = []

    # Why: Loop continues until condition is met or timeout occurs
    while True:
        t = tok.next_token()
        # Why: Condition check ensures valid state before proceeding with operation
        if t is None:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            break

        # 1. 分数 \frac{num}{den}, \dfrac{num}{den}, \binom{n}{k}
        # Why: Function call performs specific operation required by this logic
        if t in (r'\frac', r'\dfrac'):
            # Why: Function call performs specific operation required by this logic
            num = tok.get_group('{', '}')
            # Why: Function call performs specific operation required by this logic
            den = tok.get_group('{', '}')
            num_omml = _parse_latex_to_omml_inner(num)
            den_omml = _parse_latex_to_omml_inner(den)
            out.append('<m:f><m:num>%s</m:num><m:den>%s</m:den></m:f>' % (_e(num_omml), _e(den_omml)))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # Why: Condition check ensures valid state before proceeding with operation
        if t == r'\binom':
            n = tok.get_group('{', '}')
            k = tok.get_group('{', '}')
            # Why: Function call performs specific operation required by this logic
            n_omml = _parse_latex_to_omml_inner(n)
            # Why: Function call performs specific operation required by this logic
            k_omml = _parse_latex_to_omml_inner(k)
            # 微软 OMML 中无横线分数 + 外层圆括号定界符
            f_xml = '<m:f><m:fPr><m:type m:val="noBar"/></m:fPr><m:num>%s</m:num><m:den>%s</m:den></m:f>' % (_e(n_omml), _e(k_omml))
            out.append('<m:d><m:dPr><m:begChr m:val="("/><m:endChr m:val=")"/></m:dPr><m:e>%s</m:e></m:d>' % f_xml)
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 2. 根式 \sqrt[n]{x}
        # Why: Condition check ensures valid state before proceeding with operation
        if t == r'\sqrt':
            deg_val = ''
            peek_ch = tok.peek()
            # Why: Condition check ensures valid state before proceeding with operation
            if peek_ch == '[':
                deg_val = tok.get_group('[', ']')
            rad_body = tok.get_group('{', '}')
            # Why: Function call performs specific operation required by this logic
            body_omml = _parse_latex_to_omml_inner(rad_body)
            if deg_val:
                deg_omml = _parse_latex_to_omml_inner(deg_val)
                out.append('<m:rad><m:deg>%s</m:deg><m:e>%s</m:e></m:rad>' % (_e(deg_omml), body_omml))
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out.append('<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>%s</m:e></m:rad>' % body_omml)
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 3. 大型运算符 \sum, \int 等
        if t in _NARY_OPS:
            op_char = _NARY_OPS[t]
            # 检查后续是否有上下标
            sub_xml = ''
            sup_xml = ''
            # Why: Loop continues until condition is met or timeout occurs
            while tok.peek() in ('_', '^'):
                subsup = tok.next_token()
                # Why: Condition check ensures valid state before proceeding with operation
                if subsup == '_':
                    sub_val = tok.get_group('{', '}')
                    sub_xml = _parse_latex_to_omml_inner(sub_val)
                # Why: Alternative condition handles different case in decision tree
                elif subsup == '^':
                    sup_val = tok.get_group('{', '}')
                    sup_xml = _parse_latex_to_omml_inner(sup_val)

            # Why: Function call performs specific operation required by this logic
            chr_attr = '<m:chr m:val="%s"/>' % escape(op_char)
            # Why: Arithmetic operation computes value needed for subsequent processing
            lim_loc = '<m:limLoc m:val="undOvr"/>' if t == r'\sum' else '<m:limLoc m:val="subSup"/>'
            # Why: Function call performs specific operation required by this logic
            nary_pr = '<m:naryPr>%s%s</m:naryPr>' % (chr_attr, lim_loc)
            sub_part = ('<m:sub>%s</m:sub>' % _e(sub_xml)) if sub_xml else '<m:sub/>'
            sup_part = ('<m:sup>%s</m:sup>' % _e(sup_xml)) if sup_xml else '<m:sup/>'
            out.append('<m:nary>%s%s%s<m:e/></m:nary>' % (nary_pr, sub_part, sup_part))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 4. 定界符 \left( ... \right), \left[ ... \right], \left\{ ... \right\}
        # Why: Condition check ensures valid state before proceeding with operation
        if t == r'\left':
            beg_delim = tok.next_token() or '('
            # Why: Condition check ensures valid state before proceeding with operation
            if beg_delim == '\\':
                beg_delim = tok.next_token() or ''
            # Why: Condition check ensures valid state before proceeding with operation
            if beg_delim == '{':
                beg_delim = '{'
            # Why: Alternative condition handles different case in decision tree
            elif beg_delim == '.':
                beg_delim = ''

            # 寻找对应的 \right
            inner_tokens = []
            depth = 1
            end_delim = ')'
            # Why: Loop continues until condition is met or timeout occurs
            while True:
                nxt = tok.next_token()
                # Why: Condition check ensures valid state before proceeding with operation
                if nxt is None:
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
                # Why: Condition check ensures valid state before proceeding with operation
                if nxt == r'\left':
                    depth += 1
                    inner_tokens.append(nxt)
                # Why: Alternative condition handles different case in decision tree
                elif nxt == r'\right':
                    depth -= 1
                    # Why: Condition check ensures valid state before proceeding with operation
                    if depth == 0:
                        end_delim = tok.next_token() or ')'
                        # Why: Condition check ensures valid state before proceeding with operation
                        if end_delim == '\\':
                            end_delim = tok.next_token() or ''
                        # Why: Condition check ensures valid state before proceeding with operation
                        if end_delim == '}':
                            end_delim = '}'
                        # Why: Alternative condition handles different case in decision tree
                        elif end_delim == '.':
                            end_delim = ''
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        break
                    # Why: Default case handles all scenarios not covered by previous conditions
                    else:
                        inner_tokens.append(nxt)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    inner_tokens.append(nxt)

            # Why: Function call performs specific operation required by this logic
            inner_content = ' '.join(inner_tokens)
            # Why: Function call performs specific operation required by this logic
            inner_omml = _parse_latex_to_omml_inner(inner_content)
            dpr = '<m:dPr>'
            if beg_delim:
                dpr += '<m:begChr m:val="%s"/>' % escape(beg_delim)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                dpr += '<m:begChr m:val=""/>'
            if end_delim:
                dpr += '<m:endChr m:val="%s"/>' % escape(end_delim)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                dpr += '<m:endChr m:val=""/>'
            dpr += '</m:dPr>'
            out.append('<m:d>%s<m:e>%s</m:e></m:d>' % (dpr, inner_omml))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 5. 矩阵环境 \begin{matrix}, \begin{pmatrix}, \begin{bmatrix}, \begin{aligned}
        # Why: Condition check ensures valid state before proceeding with operation
        if t == r'\begin':
            env = tok.get_group('{', '}').strip()
            # 收集环境内容直到 \end{env}
            # Why: Arithmetic operation computes value needed for subsequent processing
            end_cmd = r'\end{' + env + '}'
            env_start = tok.pos
            # Why: Function call performs specific operation required by this logic
            end_idx = tok.text.find(end_cmd, env_start)
            if end_idx != -1:
                env_body = tok.text[env_start:end_idx].strip()
                tok.pos = end_idx + len(end_cmd)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                env_body = tok.text[env_start:].strip()
                tok.pos = tok.len

            # 解析矩阵行与列
            rows = [r.strip() for r in env_body.split(r'\\')]
            m_rows_xml = []
            # Why: Iteration processes each item in collection systematically
            for r in rows:
                # Why: Condition check ensures valid state before proceeding with operation
                if not r:
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    continue
                cells = [c.strip() for c in r.split('&')]
                cells_xml = ''.join('<m:e>%s</m:e>' % _parse_latex_to_omml_inner(c) for c in cells)
                m_rows_xml.append('<m:mr>%s</m:mr>' % cells_xml)

            m_inner = ''.join(m_rows_xml)
            # Why: Condition check ensures valid state before proceeding with operation
            if env == 'matrix':
                out.append('<m:m>%s</m:m>' % m_inner)
            # Why: Alternative condition handles different case in decision tree
            elif env == 'pmatrix':
                out.append('<m:d><m:dPr><m:begChr m:val="("/><m:endChr m:val=")"/></m:dPr><m:e><m:m>%s</m:m></m:e></m:d>' % m_inner)
            # Why: Alternative condition handles different case in decision tree
            elif env == 'bmatrix':
                out.append('<m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/></m:dPr><m:e><m:m>%s</m:m></m:e></m:d>' % m_inner)
            # Why: Alternative condition handles different case in decision tree
            elif env == 'vmatrix':
                out.append('<m:d><m:dPr><m:begChr m:val="|"/><m:endChr m:val="|"/></m:dPr><m:e><m:m>%s</m:m></m:e></m:d>' % m_inner)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            elif env == 'aligned' or env == 'cases':
                # 方程组 / 多行对齐
                eq_arr = '<m:eqArr>%s</m:eqArr>' % ''.join('<m:e>%s</m:e>' % _parse_latex_to_omml_inner(r.replace('&', ' ')) for r in rows if r)
                # Why: Condition check ensures valid state before proceeding with operation
                if env == 'cases':
                    out.append('<m:d><m:dPr><m:begChr m:val="{"/><m:endChr m:val=""/></m:dPr><m:e>%s</m:e></m:d>' % eq_arr)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    out.append(eq_arr)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out.append('<m:m>%s</m:m>' % m_inner)
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 6. 重音符号 \vec, \hat, \dot 等
        if t in _ACCENTS:
            acc_char = _ACCENTS[t]
            body = tok.get_group('{', '}')
            body_omml = _parse_latex_to_omml_inner(body)
            out.append('<m:acc><m:accPr><m:chr m:val="%s"/></m:accPr><m:e>%s</m:e></m:acc>' % (escape(acc_char), body_omml))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 7. \boxed{...} 与 \overline{...}
        # Why: Condition check ensures valid state before proceeding with operation
        if t == r'\boxed':
            body = tok.get_group('{', '}')
            body_omml = _parse_latex_to_omml_inner(body)
            out.append('<m:borderBox><m:e>%s</m:e></m:borderBox>' % body_omml)
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        if t in (r'\overline', r'\bar'):
            body = tok.get_group('{', '}')
            body_omml = _parse_latex_to_omml_inner(body)
            out.append('<m:bar><m:barPr><m:pos m:val="top"/></m:barPr><m:e>%s</m:e></m:bar>' % body_omml)
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 字体与文本包装 \text, \mathrm, \mathbf
        if t in (r'\text', r'\mathrm', r'\mathbf', r'\mathbb', r'\mathcal', r'\boldsymbol'):
            body = tok.get_group('{', '}')
            # Why: Condition check ensures valid state before proceeding with operation
            if t == r'\text':
                out.append('<m:r><m:rPr><m:nor/></m:rPr><m:t>%s</m:t></m:r>' % escape(body))
            # Why: Alternative condition handles different case in decision tree
            elif t in (r'\mathbf', r'\boldsymbol'):
                out.append('<m:r><m:rPr><m:b/></m:rPr><m:t>%s</m:t></m:r>' % escape(body))
            # Why: Alternative condition handles different case in decision tree
            elif t == r'\mathrm':
                out.append('<m:r><m:rPr><m:i m:val="off"/></m:rPr><m:t>%s</m:t></m:r>' % escape(body))
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out.append(_r(body))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 8. 常见函数名称 \sin, \cos, \ln, \lim
        if t in _FUNCTIONS:
            fname = t[1:]
            if t in (r'\lim', r'\max', r'\min', r'\inf', r'\sup'):
                # 检查是否有下标 \lim_{x \to 0}
                # Why: Condition check ensures valid state before proceeding with operation
                if tok.peek() == '_':
                    tok.next_token()
                    sub_val = tok.get_group('{', '}')
                    sub_omml = _parse_latex_to_omml_inner(sub_val)
                    fname_omml = _r(fname)
                    out.append('<m:limLow><m:e>%s</m:e><m:lim>%s</m:lim></m:limLow>' % (fname_omml, _e(sub_omml)))
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    continue
            out.append(_r(fname + ' '))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 9. 符号映射表转换
        if t in _MATH_SYMBOLS:
            out.append(_r(_MATH_SYMBOLS[t]))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 10. 上标 ^ 与 下标 _
        # Why: Condition check ensures valid state before proceeding with operation
        if t == '^':
            sup_val = tok.get_group('{', '}')
            sup_omml = _parse_latex_to_omml_inner(sup_val)
            # 取出前一个已生成的节点作为基底
            prev_base = out.pop() if out else _r('')
            out.append('<m:sSup><m:e>%s</m:e><m:sup>%s</m:sup></m:sSup>' % (prev_base, _e(sup_omml)))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # Why: Condition check ensures valid state before proceeding with operation
        if t == '_':
            sub_val = tok.get_group('{', '}')
            sub_omml = _parse_latex_to_omml_inner(sub_val)
            prev_base = out.pop() if out else _r('')
            # 检查紧随其后是否还有上标 x_i^2
            # Why: Condition check ensures valid state before proceeding with operation
            if tok.peek() == '^':
                tok.next_token()
                sup_val = tok.get_group('{', '}')
                sup_omml = _parse_latex_to_omml_inner(sup_val)
                out.append('<m:sSubSup><m:e>%s</m:e><m:sub>%s</m:sub><m:sup>%s</m:sup></m:sSubSup>' % (prev_base, _e(sub_omml), _e(sup_omml)))
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out.append('<m:sSub><m:e>%s</m:e><m:sub>%s</m:sub></m:sSub>' % (prev_base, _e(sub_omml)))
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 11. 普通文本与数字
        if t.startswith('\\'):
            # 未识别命令：去除斜杠兜底
            out.append(_r(t[1:]))
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            out.append(_r(t))

    # Why: Return provides result to caller after processing completes
    return ''.join(out)


# Why: Function call performs specific operation required by this logic
def latex_to_omml(latex_code, is_block=False):
    """将 LaTeX 公式转为标准 OMML XML 字符串。

    Args:
        # Why: Function call performs specific operation required by this logic
        latex_code (str): LaTeX 公式文本（如 "E = mc^2" 或 "\\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}"）。
        # Why: Function call performs specific operation required by this logic
        is_block (bool): 是否为独立公式块（若为 True 则包裹为 <m:oMathPara>）。

    Returns:
        str: 带有命名空间声明的完整 OMML XML 字符串。
    """
    clean_latex = (latex_code or '').strip()
    # Why: Double dollar signs indicate display math mode requiring special rendering
    # 移除外层 $ 或 $$
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if clean_latex.startswith('$$') and clean_latex.endswith('$$'):
        clean_latex = clean_latex[2:-2].strip()
        is_block = True
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    elif clean_latex.startswith('$') and clean_latex.endswith('$'):
        clean_latex = clean_latex[1:-1].strip()

    inner_xml = _parse_latex_to_omml_inner(clean_latex)
    # Why: Condition check ensures valid state before proceeding with operation
    if not inner_xml:
        inner_xml = '<m:r><m:t></m:t></m:r>'

    omath_xml = '<m:oMath xmlns:m="%s" xmlns:w="%s">%s</m:oMath>' % (_M_NS, _W_NS, inner_xml)
    if is_block:
        return '<m:oMathPara xmlns:m="%s" xmlns:w="%s"><m:oMath>%s</m:oMath></m:oMathPara>' % (_M_NS, _W_NS, inner_xml)
    # Why: Return provides result to caller after processing completes
    return omath_xml
