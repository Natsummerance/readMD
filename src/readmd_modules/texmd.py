# -*- coding: utf-8 -*-
"""ReadMD High-Precision LaTeX ⇄ Markdown Bidirectional Conversion Engine.

Pure-Python, zero external dependencies (no TeXLive, Pandoc or system binaries required).
Designed for academic papers (arXiv, IEEE, ACM, Springer, Nature, Elsevier).

Key Features:
1. Macro Pre-Expansion Engine:
   - Supports \\newcommand, \\renewcommand, \\def, \\DeclareMathOperator
   - Handles multi-argument macros (#1, #2, ...) with recursive expansion and loop protection
2. Balanced-Brace AST Lexer:
   - Scans commands and arbitrary nested braces {...} and optional brackets [...]
   - Prevents nested text formatting truncation (e.g. \\textbf{\\textit{...} with \\href{...}{...}})
3. Advanced Academic Environments:
   - Full math environments: equation, align, gather, multline, flalign, split, cases, matrices
   - Math labels and tags (\\label{...}, \\tag{...})
   - Theorem-like environments: theorem, lemma, definition, proposition, corollary, proof, remark, example
   - Complex tables: tabular, booktabs (\\toprule, \\midrule, \\bottomrule), \\multicolumn
   - Figures & Captions: figure, \\includegraphics, \\caption, \\label
   - Lists: itemize, enumerate, description with custom \\item[...] labels
   - Cross-references & Citations: \\ref, \\eqref, \\pageref, \\cite, \\citep, \\citet, \\bibitem
   - Footnotes: \\footnote{...} -> [^n]
4. Publication-Ready Markdown -> LaTeX Standalone Generator:
   - Emits fully compilable, clean .tex source with standard amsmath, booktabs, hyperref, tcolorbox
"""

import re
from typing import Dict, List, Tuple, Optional, Any, Callable


# ---------------------------------------------------------------------------
# 1. Balanced-Brace & Argument Scanner
# ---------------------------------------------------------------------------

def extract_balanced(text: str, start_pos: int, open_char: str = '{', close_char: str = '}') -> Tuple[Optional[str], int]:
    """从 start_pos 开始提取匹配的开闭定界符内容，返回 (内容, 结束位置后索引)。
    
    若未在 start_pos 处找到 open_char，返回 (None, start_pos)。
    正确处理内部转义字符（如 \\{ 或 \\}）。
    """
    pos = start_pos
    # 跳过前导空白
    while pos < len(text) and text[pos].isspace():
        pos += 1

    if pos >= len(text) or text[pos] != open_char:
        return None, start_pos

    depth = 0
    start_idx = pos + 1
    i = pos
    while i < len(text):
        char = text[i]
        # 跳过转义字符
        if char == '\\':
            i += 2
            continue
        if char == open_char:
            depth += 1
        elif char == close_char:
            depth -= 1
            if depth == 0:
                return text[start_idx:i], i + 1
        i += 1

    # 若未找到闭合括号，返回剩余全部
    return text[start_idx:], len(text)


def extract_opt_arg(text: str, start_pos: int) -> Tuple[Optional[str], int]:
    """提取可选参数 [...]。"""
    return extract_balanced(text, start_pos, open_char='[', close_char=']')


def extract_mand_arg(text: str, start_pos: int) -> Tuple[Optional[str], int]:
    """提取必选参数 {...}。"""
    return extract_balanced(text, start_pos, open_char='{', close_char='}')


# ---------------------------------------------------------------------------
# 2. Macro Pre-Expansion Engine (宏预展开引擎)
# ---------------------------------------------------------------------------

class MacroExpander:
    """LaTeX 自定义宏展开器，支持无参及带参 (\\newcommand, \\def) 递归展开。"""

    def __init__(self):
        # 内置高频数学缩写与考试命题宏
        self.macros: Dict[str, Dict[str, Any]] = {
            "R": {"num_args": 0, "body": r"\mathbb{R}"},
            "N": {"num_args": 0, "body": r"\mathbb{N}"},
            "Z": {"num_args": 0, "body": r"\mathbb{Z}"},
            "Q": {"num_args": 0, "body": r"\mathbb{Q}"},
            "C": {"num_args": 0, "body": r"\mathbb{C}"},
            "bs": {"num_args": 1, "body": r"\mathbf{#1}"},
            "degree": {"num_args": 0, "body": r"^\circ"},
            "tri": {"num_args": 0, "body": r"\triangle"},
            "i": {"num_args": 0, "body": r"\mathrm{i}"},
            "e": {"num_args": 0, "body": r"\mathrm{e}"},
        }

    def parse_preamble_macros(self, text: str) -> str:
        """扫描并提取导言区中的宏定义，返回去除宏定义语句后的文本。"""
        # 1. \newcommand{\name}[num]{body} / \newcommand*{\name}[num]{body}
        cmd_pattern = re.compile(r'\\(?:re)?newcommand\*?\s*\{?\\([a-zA-Z]+)\}?')
        pos = 0
        cleaned_parts = []
        last_pos = 0

        while True:
            m = cmd_pattern.search(text, pos)
            if not m:
                cleaned_parts.append(text[last_pos:])
                break

            name = m.group(1)
            p_end = m.end()

            # 检查是否有 [num_args] 可选参数
            num_args = 0
            opt_val, p_end = extract_opt_arg(text, p_end)
            if opt_val is not None:
                try:
                    num_args = int(opt_val.strip())
                except ValueError:
                    num_args = 0

            # 提取宏体 {body}
            body_val, p_end = extract_mand_arg(text, p_end)
            if body_val is not None:
                self.macros[name] = {"num_args": num_args, "body": body_val}
                cleaned_parts.append(text[last_pos:m.start()])
                last_pos = p_end
                pos = p_end
            else:
                pos = p_end

        text = ''.join(cleaned_parts)

        # 2. \def\name#1#2{body}
        def_pattern = re.compile(r'\\def\s*\\([a-zA-Z]+)([\s#0-9]*)')
        pos = 0
        cleaned_parts = []
        last_pos = 0

        while True:
            m = def_pattern.search(text, pos)
            if not m:
                cleaned_parts.append(text[last_pos:])
                break

            name = m.group(1)
            args_sig = m.group(2)
            # 计算 #1, #2 数量
            num_args = len(re.findall(r'#[0-9]', args_sig))
            p_end = m.end()

            body_val, p_end = extract_mand_arg(text, p_end)
            if body_val is not None:
                self.macros[name] = {"num_args": num_args, "body": body_val}
                cleaned_parts.append(text[last_pos:m.start()])
                last_pos = p_end
                pos = p_end
            else:
                pos = p_end

        text = ''.join(cleaned_parts)

        # 3. \DeclareMathOperator{\name}{op}
        op_pattern = re.compile(r'\\DeclareMathOperator\*?\s*\{?\\([a-zA-Z]+)\}?')
        pos = 0
        cleaned_parts = []
        last_pos = 0

        while True:
            m = op_pattern.search(text, pos)
            if not m:
                cleaned_parts.append(text[last_pos:])
                break

            name = m.group(1)
            p_end = m.end()
            body_val, p_end = extract_mand_arg(text, p_end)
            if body_val is not None:
                self.macros[name] = {"num_args": 0, "body": r'\operatorname{' + body_val + '}'}
                cleaned_parts.append(text[last_pos:m.start()])
                last_pos = p_end
                pos = p_end
            else:
                pos = p_end

        return ''.join(cleaned_parts)

    def expand(self, text: str, max_depth: int = 5) -> str:
        """递归展开文本中的自定义宏，带最大深度保护防止死循环。"""
        if not self.macros or max_depth <= 0:
            return text

        for _ in range(max_depth):
            changed = False
            for name, meta in self.macros.items():
                pattern = re.compile(rf'\\{name}(?![a-zA-Z])')
                pos = 0
                out = []
                last_pos = 0

                while True:
                    m = pattern.search(text, pos)
                    if not m:
                        out.append(text[last_pos:])
                        break

                    out.append(text[last_pos:m.start()])
                    cur_pos = m.end()
                    num_args = meta["num_args"]
                    args = []

                    for _ in range(num_args):
                        arg_val, cur_pos = extract_mand_arg(text, cur_pos)
                        if arg_val is not None:
                            args.append(arg_val)
                        else:
                            # 尝试获取单个非空白字符
                            while cur_pos < len(text) and text[cur_pos].isspace():
                                cur_pos += 1
                            if cur_pos < len(text):
                                args.append(text[cur_pos])
                                cur_pos += 1
                            else:
                                args.append('')

                    # 替换宏体中的 #1, #2...
                    body = meta["body"]
                    for arg_idx, arg_v in enumerate(args, 1):
                        body = body.replace(f'#{arg_idx}', arg_v)

                    out.append(body)
                    last_pos = cur_pos
                    pos = cur_pos
                    changed = True

                text = ''.join(out)
            if not changed:
                break

        return text


# ---------------------------------------------------------------------------
# 3. LaTeX -> Markdown 高精度解析引擎
# ---------------------------------------------------------------------------

def latex_to_md(tex_content: str) -> str:
    """将 LaTeX 文档/片段高质量转换为 GitHub Flavored Markdown。"""
    if not tex_content or not tex_content.strip():
        return ''

    # 1. 预处理：保护注释与剔除行尾注释
    lines = []
    for line in tex_content.splitlines():
        # 保护转义 \%
        line = re.sub(r'(?<!\\)%.*$', '', line)
        lines.append(line)
    text = '\n'.join(lines)

    # 2. 宏提取与预展开
    macro_engine = MacroExpander()
    text = macro_engine.parse_preamble_macros(text)
    text = macro_engine.expand(text)

    # 3. 提取文档元数据
    title_match = re.search(r'\\title(?:\[[^\]]*\])?\{', text)
    title_val = ''
    if title_match:
        val, _ = extract_mand_arg(text, title_match.end() - 1)
        title_val = (val or '').strip()

    author_val = ''
    author_match = re.search(r'\\author(?:\[[^\]]*\])?\{', text)
    if author_match:
        val, _ = extract_mand_arg(text, author_match.end() - 1)
        author_val = (val or '').strip()

    date_val = ''
    date_match = re.search(r'\\date(?:\[[^\]]*\])?\{', text)
    if date_match:
        val, _ = extract_mand_arg(text, date_match.end() - 1)
        date_val = (val or '').strip()

    # 提取 Abstract
    abstract_val = ''
    abs_match = re.search(r'\\begin\{abstract\}', text)
    if abs_match:
        abs_end = text.find(r'\end{abstract}', abs_match.end())
        if abs_end != -1:
            abstract_val = text[abs_match.end():abs_end].strip()
            text = text[:abs_match.start()] + text[abs_end + len(r'\end{abstract}'):]

    # 4. 截取正文主体（若包含 \begin{document}）
    if r'\begin{document}' in text:
        text = text.split(r'\begin{document}', 1)[1]
    if r'\end{document}' in text:
        text = text.split(r'\end{document}', 1)[0]

    # 去除 \maketitle, \tableofcontents, \newpage, \clearpage
    text = re.sub(r'\\(?:maketitle|tableofcontents|newpage|clearpage|cleardoublepage)', '', text)

    # 5. 代码块处理 (\begin{lstlisting}, \begin{verbatim}, \begin{minted})
    def _repl_lstlisting(m):
        opt = m.group(1) or ''
        body = m.group(2)
        lang = ''
        lang_m = re.search(r'language\s*=\s*([a-zA-Z0-9_\+#]+)', opt, re.IGNORECASE)
        if lang_m:
            lang = lang_m.group(1).lower()
        return f'\n```{lang}\n{body.strip()}\n```\n'

    text = re.sub(r'\\begin\{lstlisting\}(?:\[(.*?)\])?(.*?)\\end\{lstlisting\}', _repl_lstlisting, text, flags=re.DOTALL)
    text = re.sub(r'\\begin\{minted\}(?:\[.*?\])?\{([a-zA-Z0-9_\+#]+)\}(.*?)\\end\{minted\}', r'\n```\1\n\2\n```\n', text, flags=re.DOTALL)
    text = re.sub(r'\\begin\{verbatim\}(.*?)\\end\{verbatim\}', r'\n```\n\1\n```\n', text, flags=re.DOTALL)

    # 6. 数学环境规范化
    # 块级独立数学环境
    math_block_envs = [
        'equation', 'equation*', 'align', 'align*', 'aligned',
        'gather', 'gather*', 'multline', 'multline*', 'flalign', 'flalign*',
        'split', 'alignat', 'alignat*'
    ]
    for env in math_block_envs:
        escaped_env = re.escape(env)
        pattern = rf'\\begin\{{{escaped_env}\}}(?:\[.*?\])?(.*?)\\end\{{{escaped_env}\}}'
        def make_repl_math(e_name):
            def _repl_m(m):
                m_body = m.group(1).strip()
                # 去除 \label{...} 并保留干净公式
                m_body = re.sub(r'\\label\{[^}]*\}', '', m_body).strip()
                if e_name in ('equation', 'equation*'):
                    return f'\n\n$$\n{m_body}\n$$\n\n'
                return f'\n\n$$\n\\begin{{{e_name}}}\n{m_body}\n\\end{{{e_name}}}\n$$\n\n'
            return _repl_m
        text = re.sub(pattern, make_repl_math(env), text, flags=re.DOTALL)

    # 特殊块级定界符 \[ ... \]
    text = re.sub(r'\\\[(.*?)\\\]', r'\n\n$$\n\1\n$$\n\n', text, flags=re.DOTALL)
    # 行内定界符 \( ... \)
    text = re.sub(r'\\\)\s*\\\(', r'\) \(', text)
    text = re.sub(r'\\\((.*?)\\\)', r'$\1$', text, flags=re.DOTALL)

    # 7. 列表环境优先解析
    def _repl_itemize(m):
        body = m.group(1).strip()
        items = re.split(r'\\item(?:\[(.*?)\])?(?:\s+|(?=[\\$]))', body)
        res = []
        i = 1
        while i < len(items):
            opt_tag = items[i]
            it_text = items[i + 1].strip() if (i + 1 < len(items)) else ''
            if opt_tag:
                res.append(f'- **{opt_tag}** {it_text}')
            else:
                res.append(f'- {it_text}')
            i += 2
        return '\n\n' + '\n'.join(res) + '\n\n'

    text = re.sub(r'\\begin\{itemize\}(.*?)\\end\{itemize\}', _repl_itemize, text, flags=re.DOTALL)

    def _repl_enumerate(m):
        body = m.group(1).strip()
        items = re.split(r'\\item(?:\[(.*?)\])?(?:\s+|(?=[\\$]))', body)
        res = []
        idx = 1
        i = 1
        while i < len(items):
            opt_tag = items[i]
            it_text = items[i + 1].strip() if (i + 1 < len(items)) else ''
            if opt_tag:
                res.append(f'{idx}. **{opt_tag}** {it_text}')
            else:
                res.append(f'{idx}. {it_text}')
            idx += 1
            i += 2
        return '\n\n' + '\n'.join(res) + '\n\n'

    text = re.sub(r'\\begin\{enumerate\}(.*?)\\end\{enumerate\}', _repl_enumerate, text, flags=re.DOTALL)
    text = re.sub(r'\\begin\{description\}(.*?)\\end\{description\}', _repl_itemize, text, flags=re.DOTALL)

    # 圆圈序号列表 \begin{circlelist}
    def _repl_circlelist(m):
        body = m.group(1).strip()
        items = re.split(r'\\item(?:\s+|(?=[\\$]))', body)
        res = []
        circle_nums = ['①', '②', '③', '④', '⑤', '⑥', '⑦', '⑧', '⑨', '⑩']
        idx = 0
        for it in items:
            it = it.strip()
            if it:
                c_num = circle_nums[idx] if idx < len(circle_nums) else f'({idx+1})'
                res.append(f'- **{c_num}** {it}')
                idx += 1
        return '\n\n' + '\n'.join(res) + '\n\n'

    text = re.sub(r'\\begin\{circlelist\*?\}(.*?)\\end\{circlelist\*?\}', _repl_circlelist, text, flags=re.DOTALL)

    # 兜底清理外部孤立 \item
    text = re.sub(r'\\item(?:\[(.*?)\])?(?:\s+|(?=[\\$]))', lambda m: f'- **{m.group(1)}** ' if m.group(1) else '- ', text)

    # 8. 学术定理与证明环境 (Theorem, Lemma, Proof -> Callout Blockquotes)
    theorem_map = {
        'theorem': '定理 (Theorem)',
        'lemma': '引理 (Lemma)',
        'definition': '定义 (Definition)',
        'proposition': '命题 (Proposition)',
        'corollary': '推论 (Corollary)',
        'conjecture': '猜想 (Conjecture)',
        'proof': '证明 (Proof)',
        'remark': '注记 (Remark)',
        'example': '示例 (Example)'
    }
    for thm_env, thm_title in theorem_map.items():
        pattern = rf'\\begin\{{{thm_env}\}}(?:\[(.*?)\])?(.*?)\\end\{{{thm_env}\}}'
        def make_repl_thm(title_default):
            def _repl_th(m):
                opt_title = m.group(1)
                th_body = m.group(2).strip()
                header = f'**{title_default}**'
                if opt_title:
                    header = f'**{title_default} ({opt_title.strip()})**'
                # 转换为引用块
                quoted_lines = '\n'.join(f'> {l}' for l in th_body.splitlines())
                return f'\n\n> {header}\n>\n{quoted_lines}\n\n'
            return _repl_th
        text = re.sub(pattern, make_repl_thm(thm_title), text, flags=re.DOTALL | re.IGNORECASE)

    # 9. 题目、选项、答案与解析环境 (Exam & Problem Sets)
    def _repl_choices(t_in: str) -> str:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile(r'\\choices(?:five|four|three|six|two)?(?![a-zA-Z])')
        labels = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H']
        while True:
            m = pattern.search(t_in, pos)
            if not m:
                out_chunks.append(t_in[last_pos:])
                break
            out_chunks.append(t_in[last_pos:m.start()])
            cur_pos = m.end()
            opts = []
            while True:
                # 探测下一个必选参数 {...}
                test_pos = cur_pos
                while test_pos < len(t_in) and t_in[test_pos].isspace():
                    test_pos += 1
                if test_pos < len(t_in) and t_in[test_pos] == '{':
                    opt_val, cur_pos = extract_mand_arg(t_in, test_pos)
                    if opt_val is not None:
                        opts.append(opt_val.strip())
                    else:
                        break
                else:
                    break
            if opts:
                choice_lines = []
                for idx, opt_text in enumerate(opts):
                    lbl = labels[idx] if idx < len(labels) else str(idx + 1)
                    choice_lines.append(f'- **{lbl}.** {opt_text}')
                out_chunks.append('\n\n' + '\n'.join(choice_lines) + '\n\n')
                last_pos = cur_pos
                pos = cur_pos
            else:
                pos = cur_pos
        return ''.join(out_chunks)

    text = _repl_choices(text)

    # 题目、答案、解析
    text = re.sub(r'\\begin\{problem\*?\}(?:\[.*?\])?(.*?)\\end\{problem\*?\}',
                  lambda m: f'\n\n#### 【题目】\n\n{m.group(1).strip()}\n\n', text, flags=re.DOTALL)
    text = re.sub(r'\\begin\{answer\*?\}(?:\[.*?\])?(.*?)\\end\{answer\*?\}',
                  lambda m: f'\n\n> **【答案】** {m.group(1).strip()}\n\n', text, flags=re.DOTALL)
    text = re.sub(r'\\begin\{solution\*?\}(?:\[.*?\])?(.*?)\\end\{solution\*?\}',
                  lambda m: '\n\n> **【解析】**\n>\n' + '\n'.join(f'> {l}' for l in m.group(1).strip().splitlines()) + '\n\n',
                  text, flags=re.DOTALL)

    # 10. 图形与多媒体及填空题标记 (\begin{figure} ... \includegraphics ...)
    text = re.sub(r'\\fillinblank(?:\{[^}]*\})?', ' ______ ', text)
    text = re.sub(r'\\blank(?:\{[^}]*\})?', ' ______ ', text)
    text = re.sub(r'\\solutionfigure\{\\bitmapfigure(?:\[.*?\])?\{([^}]+)\}\}', r'\n\n![解析配图](\1)\n\n', text)
    text = re.sub(r'\\bitmapfigure(?:\[.*?\])?\{([^}]+)\}', r'\n\n![题目配图](\1)\n\n', text)
    text = re.sub(r'\\Figure(?:Layout|Trim)Declare\{[^}]*\}\{[^}]*\}\{[^}]*\}', '', text)
    text = re.sub(r'\\Figure(?:Layout|Trim)Declare\{[^}]*\}\{[^}]*\}', '', text)

    def _repl_figure(m):
        f_body = m.group(1)
        cap_m = re.search(r'\\caption\{([^}]+)\}', f_body)
        caption = cap_m.group(1).strip() if cap_m else ''
        img_m = re.search(r'\\includegraphics(?:\[.*?\])?\{([^}]+)\}', f_body)
        img_path = img_m.group(1).strip() if img_m else ''
        if img_path:
            return f'\n\n![{caption}]({img_path})\n\n'
        return ''

    text = re.sub(r'\\begin\{figure\*?\}(?:\[.*?\])?(.*?)\\end\{figure\*?\}', _repl_figure, text, flags=re.DOTALL)

    # 11. 复杂学术表格 (\begin{table}, \begin{tabular})
    def _parse_tabular(tbody: str) -> str:
        rows = [r.strip() for r in tbody.split(r'\\') if r.strip()]
        md_table_rows = []
        max_cols = 0

        for r in rows:
            # 清理宏包分隔线
            cleaned_row = re.sub(r'\\(hline|toprule|midrule|bottomrule|cline\{[^}]*\})', '', r).strip()
            if not cleaned_row:
                continue
            # 分解单元格并处理 \multicolumn{n}{align}{content}
            raw_cells = [c.strip() for c in cleaned_row.split('&')]
            processed_cells = []
            for cell in raw_cells:
                mc_m = re.match(r'\\multicolumn\{(\d+)\}\{[^}]*\}\{(.*)\}', cell)
                if mc_m:
                    span = int(mc_m.group(1))
                    content = mc_m.group(2).strip()
                    processed_cells.append(content)
                    for _ in range(span - 1):
                        processed_cells.append('')
                else:
                    processed_cells.append(cell)

            max_cols = max(max_cols, len(processed_cells))
            md_table_rows.append(processed_cells)

        if not md_table_rows or max_cols == 0:
            return ''

        # 构造 Markdown 管道表格
        out = []
        header = md_table_rows[0]
        while len(header) < max_cols:
            header.append('')
        out.append('| ' + ' | '.join(header) + ' |')
        out.append('| ' + ' | '.join(['---'] * max_cols) + ' |')

        for r in md_table_rows[1:]:
            while len(r) < max_cols:
                r.append('')
            out.append('| ' + ' | '.join(r) + ' |')

        return '\n\n' + '\n'.join(out) + '\n\n'

    def _repl_table_env(m):
        tbl_full = m.group(0)
        cap_m = re.search(r'\\caption\{([^}]+)\}', tbl_full)
        caption_text = f'\n\n**表：{cap_m.group(1).strip()}**\n' if cap_m else ''
        tab_m = re.search(r'\\begin\{tabular\*?\}(?:\{[^}]*\})?\{([^}]*)\}(.*?)\\end\{tabular\*?\}', tbl_full, re.DOTALL)
        if tab_m:
            return caption_text + _parse_tabular(tab_m.group(2))
        return ''

    text = re.sub(r'\\begin\{table\*?\}(?:\[.*?\])?(.*?)\\end\{table\*?\}', _repl_table_env, text, flags=re.DOTALL)
    # 单独的 tabular
    text = re.sub(r'\\begin\{tabular\*?\}(?:\{[^}]*\})?\{([^}]*)\}(.*?)\\end\{tabular\*?\}', lambda m: _parse_tabular(m.group(2)), text, flags=re.DOTALL)

    # 12. 章节标题解析 (Section Hierarchies)
    sec_commands = [
        (r'\\part\*?', '# '),
        (r'\\chapter\*?', '# '),
        (r'\\section\*?', '# '),
        (r'\\subsection\*?', '## '),
        (r'\\subsubsection\*?', '### '),
        (r'\\paragraph\*?', '#### '),
        (r'\\subparagraph\*?', '##### ')
    ]
    for cmd_regex, md_prefix in sec_commands:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile(cmd_regex + r'(?:\[[^\]]*\])?')
        while True:
            m = pattern.search(text, pos)
            if not m:
                out_chunks.append(text[last_pos:])
                break
            out_chunks.append(text[last_pos:m.start()])
            p_end = m.end()
            heading_val, p_end = extract_mand_arg(text, p_end)
            if heading_val is not None:
                # 剔除可能内嵌的 \label{...}
                heading_val = re.sub(r'\\label\{[^}]*\}', '', heading_val).strip()
                out_chunks.append(f'\n\n{md_prefix}{heading_val}\n\n')
                last_pos = p_end
                pos = p_end
            else:
                pos = p_end
        text = ''.join(out_chunks)

    # 13. 交叉引用与学术引用
    # \eqref{eq:1} -> (1), \ref{sec:1} -> [sec:1]
    text = re.sub(r'\\eqref\{([^}]+)\}', r'(\1)', text)
    text = re.sub(r'\\ref\{([^}]+)\}', r'[\1]', text)
    text = re.sub(r'\\pageref\{([^}]+)\}', r'[p.\1]', text)

    # \cite{ref1, ref2} -> [@ref1; @ref2]
    def _repl_cite(m):
        keys = [k.strip() for k in m.group(1).split(',') if k.strip()]
        return '[' + '; '.join(f'@{k}' for k in keys) + ']'
    text = re.sub(r'\\(?:cite|citep|citet|parencite|textcite)(?:\[.*?\])*\{([^}]+)\}', _repl_cite, text)

    # 13. 递归内联排版解析（使用平衡括号，彻底杜绝单层正则截断）
    inline_map = [
        (r'\\textbf', lambda c: f'**{c}**'),
        (r'\\textbf\*', lambda c: f'**{c}**'),
        (r'\\textit', lambda c: f'*{c}*'),
        (r'\\emph', lambda c: f'*{c}*'),
        (r'\\texttt', lambda c: f'`{c}`'),
        (r'\\underline', lambda c: f'<u>{c}</u>'),
        (r'\\sout', lambda c: f'~~{c}~~'),
        (r'\\textsc', lambda c: f'<span style="font-variant: small-caps;">{c}</span>'),
        (r'\\footnote', lambda c: f' [^{c}]')
    ]

    for cmd_tag, formatter in inline_map:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile(cmd_tag + r'(?![a-zA-Z])')
        while True:
            m = pattern.search(text, pos)
            if not m:
                out_chunks.append(text[last_pos:])
                break
            out_chunks.append(text[last_pos:m.start()])
            p_end = m.end()
            arg_val, p_end = extract_mand_arg(text, p_end)
            if arg_val is not None:
                out_chunks.append(formatter(arg_val))
                last_pos = p_end
                pos = p_end
            else:
                pos = p_end
        text = ''.join(out_chunks)

    # \href{url}{text} 与 \url{url}
    def _repl_href(t_in: str) -> str:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile(r'\\href(?![a-zA-Z])')
        while True:
            m = pattern.search(t_in, pos)
            if not m:
                out_chunks.append(t_in[last_pos:])
                break
            out_chunks.append(t_in[last_pos:m.start()])
            p_end = m.end()
            url_val, p_end = extract_mand_arg(t_in, p_end)
            text_val, p_end = extract_mand_arg(t_in, p_end)
            if url_val is not None and text_val is not None:
                out_chunks.append(f'[{text_val}]({url_val})')
                last_pos = p_end
                pos = p_end
            else:
                pos = p_end
        return ''.join(out_chunks)

    text = _repl_href(text)
    text = re.sub(r'\\url\{([^}]+)\}', r'<\1>', text)

    # 14. 参考文献环境 (\begin{thebibliography} ... \bibitem)
    def _repl_bib(m):
        b_body = m.group(1).strip()
        items = re.split(r'\\bibitem(?:\[(.*?)\])?\{([^}]+)\}', b_body)
        bib_lines = ['\n\n## 参考文献\n']
        i = 1
        while i < len(items):
            label = items[i]
            key = items[i + 1]
            desc = items[i + 2].strip() if (i + 2 < len(items)) else ''
            prefix = f'[{label}]' if label else f'[@{key}]'
            bib_lines.append(f'- {prefix} {desc}')
            i += 3
        return '\n'.join(bib_lines) + '\n\n'

    text = re.sub(r'\\begin\{thebibliography\}(?:\{[^}]*\})?(.*?)\\end\{thebibliography\}', _repl_bib, text, flags=re.DOTALL)

    # 15. 保护数学公式并清理普通文本中的 LaTeX 转义字符（精确切分，零 Token 泄漏风险）
    def _unescape_plain_text(t: str) -> str:
        pattern = re.compile(r'(\$\$.*?\$\$|(?<!\\)\$.*?(?<!\\)\$)', re.DOTALL)
        parts = pattern.split(t)
        out = []
        for i, p in enumerate(parts):
            if i % 2 == 1:
                out.append(p)
            else:
                p = p.replace(r'\%', '%').replace(r'\&', '&').replace(r'\_', '_').replace(r'\#', '#').replace(r'\{', '{').replace(r'\}', '}').replace('~', ' ')
                out.append(p)
        return ''.join(out)

    text = _unescape_plain_text(text)

    # 16. 构建 Frontmatter
    frontmatter = []
    if title_val:
        frontmatter.append(f'title: "{title_val}"')
    if author_val:
        frontmatter.append(f'author: "{author_val}"')
    if date_val:
        frontmatter.append(f'date: "{date_val}"')

    cleaned_body = re.sub(r'\n{3,}', '\n\n', text).strip()

    res_parts = []
    if frontmatter:
        res_parts.append('---\n' + '\n'.join(frontmatter) + '\n---\n')
    if abstract_val:
        res_parts.append(f'> **摘要**：{abstract_val}\n')
    res_parts.append(cleaned_body)

    return '\n\n'.join(res_parts).strip()


# ---------------------------------------------------------------------------
# 4. Markdown -> LaTeX 高质量独立学术文档生成器
# ---------------------------------------------------------------------------

LATEX_ARTICLE_TEMPLATE = r"""\documentclass[11pt,a4paper]{article}

% --- 核心数学与学术宏包 ---
\usepackage[utf8]{inputenc}
\usepackage[margin=2.5cm]{geometry}
\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}
\usepackage{booktabs,tabularx,multirow}
\usepackage{graphicx}
\usepackage{hyperref}
\usepackage{listings}
\usepackage{xcolor}
\usepackage{tcolorbox}
\usepackage{microtype}

% --- 超链接与主题色彩 ---
\hypersetup{
    colorlinks=true,
    linkcolor=blue!70!black,
    citecolor=blue!70!black,
    urlcolor=blue!70!black
}

% --- 代码块样式 ---
\lstset{
    basicstyle=\ttfamily\small,
    breaklines=true,
    frame=single,
    backgroundcolor=\color{gray!8},
    keywordstyle=\color{blue!80!black},
    commentstyle=\color{green!50!black},
    stringstyle=\color{red!70!black},
    showstringspaces=false
}

% --- 引用块与提示框 ---
\tcolorboxenvironment{quote}{
    colback=gray!5,
    colframe=gray!40,
    arc=2mm,
    left=3mm,
    right=3mm,
    top=2mm,
    bottom=2mm
}

\title{__TITLE__}
\author{__AUTHOR__}
\date{__DATE__}

\begin{document}

\maketitle

__CONTENT__

\end{document}
"""


def _escape_latex_plain_text(text: str) -> str:
    """对纯文本中的特殊 LaTeX 字符转义，不干扰已被占位的数学公式。"""
    chars = {
        '&': r'\&',
        '%': r'\%',
        '$': r'\$',
        '#': r'\#',
        '_': r'\_',
        '{': r'\{',
        '}': r'\}',
        '~': r'\textasciitilde{}',
        '^': r'\textasciicircum{}',
    }
    pattern = re.compile('|'.join(re.escape(k) for k in chars.keys()))
    return pattern.sub(lambda m: chars[m.group(0)], text)


def _convert_inline_md_to_latex(text: str) -> str:
    """将 Markdown 行内样式转换为 LaTeX 语法。"""
    math_tokens = []
    def _save_math(m):
        t = f'QQQMATHTOKEN{len(math_tokens)}QQQ'
        math_tokens.append(m.group(0))
        return t

    # 保护行内公式 $...$
    text = re.sub(r'(?<!\\)\$([^\$]+?)\$', _save_math, text)

    # 保护行内代码 `...`
    code_tokens = []
    def _save_code(m):
        t = f'QQQCODETOKEN{len(code_tokens)}QQQ'
        code_tokens.append(f'\\texttt{{{_escape_latex_plain_text(m.group(1))}}}')
        return t
    text = re.sub(r'`([^`]+)`', _save_code, text)

    # 图片: ![alt](url) -> \includegraphics
    text = re.sub(r'!\[(.*?)\]\((.*?)\)', r'\\begin{figure}[htbp]\\centering\\includegraphics[max width=\\linewidth]{\2}\\caption{\1}\\end{figure}', text)

    # 链接: [text](url) -> \href{url}{text}
    text = re.sub(r'\[(.*?)\]\((.*?)\)', r'\\href{\2}{\1}', text)

    # 粗体: **text**
    text = re.sub(r'\*\*(.*?)\*\*', r'\\textbf{\1}', text)
    text = re.sub(r'__(.*?)__', r'\\textbf{\1}', text)

    # 斜体: *text*
    text = re.sub(r'\*(.*?)\*', r'\\textit{\1}', text)
    text = re.sub(r'(?<!\w)_(.*?)_{1}(?!\w)', r'\\textit{\1}', text)

    # 删除线: ~~text~~ -> \sout{text}
    text = re.sub(r'~~(.*?)~~', r'\\sout{\1}', text)

    # 恢复 code 与 math tokens
    for idx, ct in enumerate(code_tokens):
        text = text.replace(f'QQQCODETOKEN{idx}QQQ', ct)
    for idx, mt in enumerate(math_tokens):
        text = text.replace(f'QQQMATHTOKEN{idx}QQQ', mt)

    return text


def md_to_latex(md_content: str, title: str = 'Academic Document', author: str = '', standalone: bool = True) -> str:
    """将 Markdown 转换为高质量、可直接编译的 LaTeX 源码。"""
    lines = md_content.splitlines()
    latex_lines: List[str] = []

    doc_title = title
    doc_author = author
    doc_date = r'\today'
    content_start_idx = 0

    # 1. 解析 YAML Frontmatter
    if len(lines) > 2 and lines[0].strip() == '---':
        for i in range(1, len(lines)):
            if lines[i].strip() == '---':
                content_start_idx = i + 1
                break
            fm_line = lines[i]
            if ':' in fm_line:
                k, v = fm_line.split(':', 1)
                k = k.strip().lower()
                v = v.strip().strip('"\'')
                if k == 'title':
                    doc_title = v
                elif k in ('author', 'authors'):
                    doc_author = v
                elif k == 'date':
                    doc_date = v

    in_code_block = False
    code_lang = ''
    code_buffer: List[str] = []

    in_table = False
    table_rows: List[List[str]] = []

    in_display_math = False
    math_buffer: List[str] = []

    i = content_start_idx
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # 1. 代码块 ```
        if stripped.startswith('```'):
            if in_code_block:
                latex_lines.append(r'\begin{lstlisting}' + (f'[language={code_lang}]' if code_lang else ''))
                latex_lines.extend(code_buffer)
                latex_lines.append(r'\end{lstlisting}')
                in_code_block = False
                code_buffer = []
                code_lang = ''
            else:
                in_code_block = True
                code_lang = stripped[3:].strip()
                code_buffer = []
            i += 1
            continue

        if in_code_block:
            code_buffer.append(line)
            i += 1
            continue

        # 2. 独立公式块 $$ ... $$
        if stripped.startswith('$$'):
            if in_display_math:
                math_buffer.append(line)
                math_body = '\n'.join(math_buffer).strip('$').strip()
                # 检查公式内部是否已经包含了环境
                if math_body.startswith(r'\begin{') and math_body.endswith(r'\end{'):
                    latex_lines.append(math_body)
                else:
                    latex_lines.append(r'\begin{equation*}' + '\n' + math_body + '\n' + r'\end{equation*}')
                in_display_math = False
                math_buffer = []
            else:
                if stripped.endswith('$$') and len(stripped) > 2:
                    math_body = stripped[2:-2].strip()
                    if math_body.startswith(r'\begin{') and math_body.endswith(r'\end{'):
                        latex_lines.append(math_body)
                    else:
                        latex_lines.append(r'\begin{equation*}' + '\n' + math_body + '\n' + r'\end{equation*}')
                else:
                    in_display_math = True
                    math_buffer = [line]
            i += 1
            continue

        if in_display_math:
            if stripped.endswith('$$'):
                math_buffer.append(line)
                in_display_math = False
                math_body = '\n'.join(math_buffer).rstrip('$').lstrip('$').strip()
                if math_body.startswith(r'\begin{') and math_body.endswith(r'\end{'):
                    latex_lines.append(math_body)
                else:
                    latex_lines.append(r'\begin{equation*}' + '\n' + math_body + '\n' + r'\end{equation*}')
                math_buffer = []
            else:
                math_buffer.append(line)
            i += 1
            continue

        # 3. 管道表格 | ... |
        if stripped.startswith('|') and stripped.endswith('|'):
            cells = [c.strip() for c in stripped[1:-1].split('|')]
            if re.match(r'^[:\-\s|]+$', stripped):
                pass
            else:
                if not in_table:
                    in_table = True
                    table_rows = []
                table_rows.append(cells)
            i += 1
            continue
        elif in_table:
            latex_lines.extend(_render_latex_booktabs_table(table_rows))
            in_table = False
            table_rows = []

        # 4. 标题 (Headings)
        heading_match = re.match(r'^(#{1,6})\s+(.*)$', line)
        if heading_match:
            level = len(heading_match.group(1))
            htext = _convert_inline_md_to_latex(heading_match.group(2).strip())
            cmd = {
                1: r'\section',
                2: r'\subsection',
                3: r'\subsubsection',
                4: r'\paragraph',
                5: r'\subparagraph',
                6: r'\textbf'
            }.get(level, r'\paragraph')
            latex_lines.append(f'\n{cmd}{{{htext}}}')
            i += 1
            continue

        # 5. 引用块 (Blockquote)
        if stripped.startswith('>'):
            quote_text = _convert_inline_md_to_latex(stripped[1:].strip())
            latex_lines.append(r'\begin{quote}')
            latex_lines.append(quote_text)
            latex_lines.append(r'\end{quote}')
            i += 1
            continue

        # 6. 无序列表 (Unordered List)
        if re.match(r'^[\*\-\+]\s+', stripped):
            item_text = _convert_inline_md_to_latex(re.sub(r'^[\*\-\+]\s+', '', stripped))
            latex_lines.append(r'\begin{itemize}')
            latex_lines.append(f'  \\item {item_text}')
            while i + 1 < len(lines) and re.match(r'^[\*\-\+]\s+', lines[i + 1].strip()):
                i += 1
                next_item = _convert_inline_md_to_latex(re.sub(r'^[\*\-\+]\s+', '', lines[i].strip()))
                latex_lines.append(f'  \\item {next_item}')
            latex_lines.append(r'\end{itemize}')
            i += 1
            continue

        # 7. 有序列表 (Ordered List)
        if re.match(r'^\d+\.\s+', stripped):
            item_text = _convert_inline_md_to_latex(re.sub(r'^\d+\.\s+', '', stripped))
            latex_lines.append(r'\begin{enumerate}')
            latex_lines.append(f'  \\item {item_text}')
            while i + 1 < len(lines) and re.match(r'^\d+\.\s+', lines[i + 1].strip()):
                i += 1
                next_item = _convert_inline_md_to_latex(re.sub(r'^\d+\.\s+', '', lines[i].strip()))
                latex_lines.append(f'  \\item {next_item}')
            latex_lines.append(r'\end{enumerate}')
            i += 1
            continue

        # 8. 分割线
        if re.match(r'^(\*{3,}|-{3,}|_{3,})$', stripped):
            latex_lines.append(r'\noindent\rule{\textwidth}{0.4pt}')
            i += 1
            continue

        # 9. 正文段落
        if stripped:
            latex_lines.append(_convert_inline_md_to_latex(line))
        else:
            latex_lines.append('')
        i += 1

    if in_table and table_rows:
        latex_lines.extend(_render_latex_booktabs_table(table_rows))

    content_latex = '\n'.join(latex_lines)

    if standalone:
        return (LATEX_ARTICLE_TEMPLATE
                .replace('__TITLE__', doc_title)
                .replace('__AUTHOR__', doc_author)
                .replace('__DATE__', doc_date)
                .replace('__CONTENT__', content_latex))
    return content_latex


def _render_latex_booktabs_table(rows: List[List[str]]) -> List[str]:
    """生成符合学术规范的 booktabs 三线表。"""
    if not rows:
        return []
    col_count = max(len(r) for r in rows)
    col_spec = 'l' * col_count
    res = [
        r'\begin{table}[htbp]',
        r'\centering',
        f'\\begin{{tabular}}{{{col_spec}}}',
        r'\toprule'
    ]
    # 表头
    header = rows[0]
    header_cells = [_convert_inline_md_to_latex(c) for c in header]
    while len(header_cells) < col_count:
        header_cells.append('')
    res.append(' & '.join(header_cells) + r' \\')
    res.append(r'\midrule')

    for r in rows[1:]:
        cells = [_convert_inline_md_to_latex(c) for c in r]
        while len(cells) < col_count:
            cells.append('')
        res.append(' & '.join(cells) + r' \\')

    res.append(r'\bottomrule')
    res.append(r'\end{tabular}')
    res.append(r'\end{table}')
    return res


# 兼容别名
latex_to_markdown = latex_to_md
markdown_to_latex = md_to_latex
