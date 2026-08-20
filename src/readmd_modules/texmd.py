# Why: logging module provides essential functionality for this operation
import logging
'ReadMD High-Precision LaTeX ⇄ Markdown Bidirectional Conversion Engine.\n\nPure-Python, zero external dependencies (no TeXLive, Pandoc or system binaries required).\nDesigned for academic papers (arXiv, IEEE, ACM, Springer, Nature, Elsevier).\n\nKey Features:\n1. Macro Pre-Expansion Engine:\n   - Supports \\newcommand, \\renewcommand, \\def, \\DeclareMathOperator\n   - Handles multi-argument macros (#1, #2, ...) with recursive expansion and loop protection\n2. Balanced-Brace AST Lexer:\n   - Scans commands and arbitrary nested braces {...} and optional brackets [...]\n   - Prevents nested text formatting truncation (e.g. \\textbf{\\textit{...} with \\href{...}{...}})\n3. Advanced Academic Environments:\n   - Full math environments: equation, align, gather, multline, flalign, split, cases, matrices\n   - Math labels and tags (\\label{...}, \\tag{...})\n   - Theorem-like environments: theorem, lemma, definition, proposition, corollary, proof, remark, example\n   - Complex tables: tabular, booktabs (\\toprule, \\midrule, \\bottomrule), \\multicolumn\n   - Figures & Captions: figure, \\includegraphics, \\caption, \\label\n   - Lists: itemize, enumerate, description with custom \\item[...] labels\n   - Cross-references & Citations: \\ref, \\eqref, \\pageref, \\cite, \\citep, \\citet, \\bibitem\n   - Footnotes: \\footnote{...} -> [^n]\n4. Publication-Ready Markdown -> LaTeX Standalone Generator:\n   - Emits fully compilable, clean .tex source with standard amsmath, booktabs, hyperref, tcolorbox\n'
# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re
from typing import Dict, List, Tuple, Optional, Any

# Why: Function call performs specific operation required by this logic
def extract_balanced(text: str, start_pos: int, open_char: str='{', close_char: str='}') -> Tuple[Optional[str], int]:
    # Why: Function call performs specific operation required by this logic
    """从 start_pos 开始提取匹配的开闭定界符内容，返回 (内容, 结束位置后索引)。
    
    # Why: Function call performs specific operation required by this logic
    若未在 start_pos 处找到 open_char，返回 (None, start_pos)。
    正确处理内部转义字符（如 \\{ 或 \\}）。
    """
    pos = start_pos
    while pos < len(text) and text[pos].isspace():
        # Why: Prevent index out of bounds when parsing LaTeX delimiters
        pos += 1
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if pos >= len(text) or text[pos] != open_char:
        return (None, start_pos)
    depth = 0
    start_idx = pos + 1
    i = pos
    # Why: Loop continues until condition is met or timeout occurs
    while i < len(text):
        char = text[i]
        # Why: Condition check ensures valid state before proceeding with operation
        if char == '\\':
            i += 2
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        # Why: Condition check ensures valid state before proceeding with operation
        if char == open_char:
            depth += 1
        # Why: Alternative condition handles different case in decision tree
        elif char == close_char:
            depth -= 1
            # Why: Condition check ensures valid state before proceeding with operation
            if depth == 0:
                # Why: Return provides result to caller after processing completes
                return (text[start_idx:i], i + 1)
        i += 1
    # Why: Return provides result to caller after processing completes
    return (text[start_idx:], len(text))

def extract_opt_arg(text: str, start_pos: int) -> Tuple[Optional[str], int]:
    # Why: Method chain performs sequence of transformations on data
    """提取可选参数 [...]。"""
    # Why: Function call performs specific operation required by this logic
    return extract_balanced(text, start_pos, open_char='[', close_char=']')

# Why: Function call performs specific operation required by this logic
def extract_mand_arg(text: str, start_pos: int) -> Tuple[Optional[str], int]:
    # Why: Method chain performs sequence of transformations on data
    """提取必选参数 {...}。"""
    # Why: Function call performs specific operation required by this logic
    return extract_balanced(text, start_pos, open_char='{', close_char='}')

class MacroExpander:
    # Why: Function call performs specific operation required by this logic
    """LaTeX 自定义宏展开器，支持无参及带参 (\\newcommand, \\def) 递归展开。"""

    def __init__(self):
        self.macros: Dict[str, Dict[str, Any]] = {'R': {'num_args': 0, 'body': '\\mathbb{R}'}, 'N': {'num_args': 0, 'body': '\\mathbb{N}'}, 'Z': {'num_args': 0, 'body': '\\mathbb{Z}'}, 'Q': {'num_args': 0, 'body': '\\mathbb{Q}'}, 'C': {'num_args': 0, 'body': '\\mathbb{C}'}, 'bs': {'num_args': 1, 'body': '\\mathbf{#1}'}, 'degree': {'num_args': 0, 'body': '^\\circ'}, 'tri': {'num_args': 0, 'body': '\\triangle'}, 'i': {'num_args': 0, 'body': '\\mathrm{i}'}, 'e': {'num_args': 0, 'body': '\\mathrm{e}'}}

    # Why: parse_preamble_macros implements core functionality requiring careful error handling
    def parse_preamble_macros(self, text: str) -> str:
        """扫描并提取导言区中的宏定义，返回去除宏定义语句后的文本。"""
        cmd_pattern = re.compile('\\\\(?:re)?newcommand\\*?\\s*\\{?\\\\([a-zA-Z]+)\\}?')
        pos = 0
        cleaned_parts = []
        last_pos = 0
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            m = cmd_pattern.search(text, pos)
            # Why: Condition check ensures valid state before proceeding with operation
            if not m:
                cleaned_parts.append(text[last_pos:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            name = m.group(1)
            p_end = m.end()
            num_args = 0
            (opt_val, p_end) = extract_opt_arg(text, p_end)
            # Why: Condition check ensures valid state before proceeding with operation
            if opt_val is not None:
                try:
                    # Why: Integer conversion may fail on non-numeric input; handle gracefully
                    num_args = int(opt_val.strip())
                # Why: ValueError indicates invalid input data that cannot be processed safely
                except ValueError:
                    logging.warning('Silent exception caught in src.readmd_modules.texmd: ValueError')
                    num_args = 0
            (body_val, p_end) = extract_mand_arg(text, p_end)
            # Why: Condition check ensures valid state before proceeding with operation
            if body_val is not None:
                self.macros[name] = {'num_args': num_args, 'body': body_val}
                cleaned_parts.append(text[last_pos:m.start()])
                last_pos = p_end
                pos = p_end
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                pos = p_end
        text = ''.join(cleaned_parts)
        # Why: Function call performs specific operation required by this logic
        def_pattern = re.compile('\\\\def\\s*\\\\([a-zA-Z]+)([\\s#0-9]*)')
        pos = 0
        cleaned_parts = []
        last_pos = 0
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            m = def_pattern.search(text, pos)
            # Why: Condition check ensures valid state before proceeding with operation
            if not m:
                cleaned_parts.append(text[last_pos:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            name = m.group(1)
            args_sig = m.group(2)
            num_args = len(re.findall('#[0-9]', args_sig))
            p_end = m.end()
            (body_val, p_end) = extract_mand_arg(text, p_end)
            # Why: Condition check ensures valid state before proceeding with operation
            if body_val is not None:
                self.macros[name] = {'num_args': num_args, 'body': body_val}
                cleaned_parts.append(text[last_pos:m.start()])
                last_pos = p_end
                pos = p_end
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                cleaned_parts.append(text[last_pos:p_end])
                last_pos = p_end
                pos = p_end
        # Why: Function call performs specific operation required by this logic
        text = ''.join(cleaned_parts)
        # Why: Function call performs specific operation required by this logic
        op_pattern = re.compile('\\\\DeclareMathOperator\\*?\\s*\\{?\\\\([a-zA-Z]+)\\}?')
        pos = 0
        cleaned_parts = []
        last_pos = 0
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            m = op_pattern.search(text, pos)
            # Why: Condition check ensures valid state before proceeding with operation
            if not m:
                cleaned_parts.append(text[last_pos:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            name = m.group(1)
            p_end = m.end()
            (body_val, p_end) = extract_mand_arg(text, p_end)
            # Why: Condition check ensures valid state before proceeding with operation
            if body_val is not None:
                self.macros[name] = {'num_args': 0, 'body': '\\operatorname{' + body_val + '}'}
                cleaned_parts.append(text[last_pos:m.start()])
                last_pos = p_end
                pos = p_end
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                cleaned_parts.append(text[last_pos:p_end])
                last_pos = p_end
                pos = p_end
        # Why: Return provides result to caller after processing completes
        return ''.join(cleaned_parts)

    def expand(self, text: str, max_depth: int=5) -> str:
        # Why: Prevent infinite recursion in macro expansion by limiting depth
        """递归展开文本中的自定义宏，带最大深度保护防止死循环。"""
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not self.macros or max_depth <= 0:
            return text
        # Why: Iteration processes each item in collection systematically
        for _ in range(max_depth):
            changed = False
            # Why: Iteration processes each item in collection systematically
            for (name, meta) in self.macros.items():
                pattern = re.compile('\\\\%s(?![a-zA-Z])' % name)
                pos = 0
                out = []
                last_pos = 0
                # Why: Loop continues until condition is met or timeout occurs
                while True:
                    m = pattern.search(text, pos)
                    # Why: Condition check ensures valid state before proceeding with operation
                    if not m:
                        out.append(text[last_pos:])
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        break
                    out.append(text[last_pos:m.start()])
                    cur_pos = m.end()
                    num_args = meta['num_args']
                    args = []
                    # Why: Iteration processes each item in collection systematically
                    for _ in range(num_args):
                        (arg_val, cur_pos) = extract_mand_arg(text, cur_pos)
                        # Why: Condition check ensures valid state before proceeding with operation
                        if arg_val is not None:
                            args.append(arg_val)
                        # Why: Default case handles all scenarios not covered by previous conditions
                        else:
                            # Why: Loop continues until condition is met or timeout occurs
                            while cur_pos < len(text) and text[cur_pos].isspace():
                                cur_pos += 1
                            if cur_pos < len(text):
                                args.append(text[cur_pos])
                                cur_pos += 1
                            # Why: Default case handles all scenarios not covered by previous conditions
                            else:
                                args.append('')
                    body = meta['body']
                    # Why: Iteration processes each item in collection systematically
                    for (arg_idx, arg_v) in enumerate(args, 1):
                        body = body.replace('#{}'.format(arg_idx), arg_v)
                    out.append(body)
                    last_pos = cur_pos
                    pos = cur_pos
                    changed = True
                text = ''.join(out)
            # Why: Condition check ensures valid state before proceeding with operation
            if not changed:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
        # Why: Return provides result to caller after processing completes
        return text

def latex_to_md(tex_content: str, base_dir: str='') -> str:
    # Why: Skip empty TeX content to avoid processing invalid formulas
    """将 LaTeX 文档/片段高质量转换为 GitHub Flavored Markdown。"""
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not tex_content or not tex_content.strip():
        return ''
    lines = []
    for line in tex_content.splitlines():
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        # Only strip % comments when preceded by whitespace or at start (not inline like 88.5%)
        line = re.sub('(?<!\\\\)(?<![a-zA-Z0-9.])%.*$', '', line)
        lines.append(line)
    # Why: Verify base directory exists before attempting file operations
    text = '\n'.join(lines)
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if base_dir and os.path.exists(base_dir):

        def _expand_inputs(t: str, cur_dir: str, depth: int=0) -> str:
            if depth > 10:
                # Why: Return provides result to caller after processing completes
                return t

            def _repl_input(m):
                fname = m.group(1).strip()
                candidates = [os.path.join(cur_dir, fname), os.path.join(cur_dir, fname + '.tex'), os.path.join(base_dir, fname), os.path.join(base_dir, fname + '.tex')]
                # Why: Iteration processes each item in collection systematically
                for target in candidates:
                    if os.path.isfile(target):
                        # Why: Try block protects against runtime errors in operations that may fail
                        try:
                            # Why: Context manager ensures proper resource cleanup even if errors occur
                            with open(target, 'rb') as f:
                                # Why: Method call handles data access with proper error checking
                                raw_b = f.read()
                            try:
                                # Why: File encoding may not be UTF-8; try alternative encodings for compatibility
                                sub_content = raw_b.decode('utf-8')
                            # Why: Exception handling prevents crashes and provides meaningful error messages to users
                            except UnicodeDecodeError:
                                logging.warning('Silent exception caught in src.readmd_modules.texmd: UnicodeDecodeError')
                                sub_content = raw_b.decode('latin-1')
                            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                            sub_lines = [re.sub('(?<!\\\\)%.*$', '', l) for l in sub_content.splitlines()]
                            sub_text = '\n'.join(sub_lines)
                            if '\\begin{document}' in sub_text:
                                sub_text = sub_text.split('\\begin{document}', 1)[1]
                            if '\\end{document}' in sub_text:
                                sub_text = sub_text.split('\\end{document}', 1)[0]
                            # Why: Unexpected errors during TeX processing should not crash the entire application
                            return '\n\n' + _expand_inputs(sub_text, os.path.dirname(target), depth + 1) + '\n\n'
                        # Why: Exception handling prevents crashes and provides meaningful error messages to users
                        except Exception:
                            logging.warning('Silent exception caught in src.readmd_modules.texmd: Exception')
                return ''
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            return re.sub('\\\\(?:input|include|subfile)\\{([^}]+)\\}', _repl_input, t)
        text = _expand_inputs(text, base_dir)
    macro_engine = MacroExpander()
    # Why: Function call performs specific operation required by this logic
    text = macro_engine.parse_preamble_macros(text)
    text = macro_engine.expand(text)

    def _clean_metadata_text(val: str) -> str:
        # Why: Condition check ensures valid state before proceeding with operation
        if not val:
            return ''
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\(?:thanks|footnote|email|inst|affil|corref|fnmark|authornote)\\{[^}]*\\}', '', val)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\footnotemark(?:\\[[^\\]]*\\])?', '', val)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\(?:hspace|vspace)\\*?\\{[^}]*\\}', '', val)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\color\\{[^}]*\\}', '', val)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\(?:texttt|textbf|textit|textsf|textsc|emph)\\{([^}]*)\\}', '\\1', val)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\url\\{([^}]*)\\}', '\\1', val)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\href\\{[^}]*\\}\\{([^}]*)\\}', '\\1', val)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\\\(?:And|AND|and)\\b', ' & ', val)
        val = val.replace('\\\\', ' ')
        val = val.replace('\\', '')
        val = val.replace('{', '').replace('}', '')
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('\\s+', ' ', val).strip()
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        val = re.sub('^(&\\s*)+|(\\s*&)+$', '', val).strip()
        val = val.replace('"', "'")
        return val
    # Why: Regex pattern matches specific text structures for validation or extraction
    title_match = re.search('\\\\title(?:\\[[^\\]]*\\])?\\{', text)
    title_val = ''
    if title_match:
        (val, _) = extract_mand_arg(text, title_match.end() - 1)
        title_val = _clean_metadata_text(val)
    author_val = ''
    # Why: Regex pattern matches specific text structures for validation or extraction
    author_match = re.search('\\\\author(?:\\[[^\\]]*\\])?\\{', text)
    if author_match:
        (val, _) = extract_mand_arg(text, author_match.end() - 1)
        author_val = _clean_metadata_text(val)
    date_val = ''
    # Why: Regex pattern matches specific text structures for validation or extraction
    date_match = re.search('\\\\date(?:\\[[^\\]]*\\])?\\{', text)
    if date_match:
        (val, _) = extract_mand_arg(text, date_match.end() - 1)
        date_val = _clean_metadata_text(val)
    if '\\begin{document}' in text:
        (preamble, doc_body) = text.split('\\begin{document}', 1)
        # Why: Regex pattern matches specific text structures for validation or extraction
        abs_match = re.search('\\\\begin\\{abstract\\}(.*?)\\\\end\\{abstract\\}', preamble, flags=re.DOTALL)
        if abs_match:
            doc_body = abs_match.group(0) + '\n\n' + doc_body
        text = doc_body
    if '\\end{document}' in text:
        text = text.split('\\end{document}', 1)[0]
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\bgroup\\b', '{', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\egroup\\b', '}', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\textcolor\\{[^}]*\\}\\{([^}]*)\\}', '\\1', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\color\\{[^}]*\\}', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('``([^`\\n]*?)(\'\'|\\")', '“\\1”', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('`([^`\\n]*?)(\'|\\")', '‘\\1’', text)

    def _repl_verb(m):
        delim = m.group(1)
        body = m.group(2)
        return '`{}`'.format(body)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\verb(.)(.*?)\\1', _repl_verb, text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\textbackslash(?![a-zA-Z])', '\\\\', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\textbar(?![a-zA-Z])', '|', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\textless(?![a-zA-Z])', '<', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\textgreater(?![a-zA-Z])', '>', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\textasciitilde(?![a-zA-Z])', '~', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\textasciicircum(?![a-zA-Z])', '^', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:newline|linebreak)(?![a-zA-Z])', '\n', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\centerline\\{([^}]+)\\}', '\\1', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\cormark(?:\\[.*?\\])?', '*', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\cortext(?:\\[.*?\\])?\\{([^}]+)\\}', '\\n\\n* \\1\\n\\n', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\printcredits(?![a-zA-Z])', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\bio(?:\\{.*?\\})?(.*?)\\\\endbio', '\\n\\n**作者简介：**\\n\\n\\1\\n\\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\ead(?:\\[.*?\\])?\\{([^}]+)\\}', '<\\1>', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\{\\\\bf\\s+([^}]+)\\}', '**\\1**', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\{\\\\it\\s+([^}]+)\\}', '*\\1*', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\{\\\\em\\s+([^}]+)\\}', '*\\1*', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\{\\\\tt\\s+([^}]+)\\}', '`\\1`', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\{\\\\sc\\s+([^}]+)\\}', '<span style="font-variant: small-caps;">\\1</span>', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\{\\\\rm\\s+([^}]+)\\}', '\\1', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\{\\\\sf\\s+([^}]+)\\}', '\\1', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\bf\\s+([^\\n\\\\{]+)', '**\\1**', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\it\\s+([^\\n\\\\{]+)', '*\\1*', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\tt\\s+([^\\n\\\\{]+)', '`\\1`', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:rm|sf|em|sc)\\s+([^\\n\\\\{]+)', '\\1', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:maketitle|tableofcontents|newpage|clearpage|cleardoublepage)', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:vspace\\*?|hspace\\*?)\\{[^}]*\\}', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:vskip|hskip|kern)\\s*[-+]?\\d+(?:\\.\\d+)?[a-zA-Z]+', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:centering|raggedright|raggedleft|noindent|indent|vfill|hfill|smallskip|medskip|bigskip)(?![a-zA-Z])', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:Huge|huge|LARGE|Large|large|normalsize|small|footnotesize|scriptsize|tiny)(?![a-zA-Z])', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\rule\\{[^}]*\\}\\{[^}]*\\}', '\n\n---\n\n', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\hrule(?![a-zA-Z])', '\n\n---\n\n', text)

    def _repl_lstlisting(m):
        opt = m.group(1) or ''
        body = m.group(2)
        lang = ''
        # Why: Regex pattern matches specific text structures for validation or extraction
        lang_m = re.search('language\\s*=\\s*([a-zA-Z0-9_\\+#]+)', opt, re.IGNORECASE)
        if lang_m:
            lang = lang_m.group(1).lower()
        return '\n```{}\n{}\n```\n'.format(lang, body.strip())
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{lstlisting\\}(?:\\[(.*?)\\])?(.*?)\\\\end\\{lstlisting\\}', _repl_lstlisting, text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{minted\\}(?:\\[.*?\\])?\\{([a-zA-Z0-9_\\+#]+)\\}(.*?)\\\\end\\{minted\\}', '\\n```\\1\\n\\2\\n```\\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:verbatim\\*?|stdout|session|shell|console|terminal|alltt|code)\\}(.*?)\\\\end\\{(?:verbatim\\*?|stdout|session|shell|console|terminal|alltt|code)\\}', '\\n```\\n\\1\\n```\\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\verb([^a-zA-Z0-9\\s])(.*?)\\1', '`\\2`', text)

    def _unwrap_boxes(t_in: str) -> str:
        # Why: Iteration processes each item in collection systematically
        for cmd in ['\\\\resizebox\\*?', '\\\\scalebox\\*?', '\\\\parbox\\*?']:
            pattern = re.compile(cmd + '(?![a-zA-Z])')
            # Why: Loop continues until condition is met or timeout occurs
            while True:
                m = pattern.search(t_in)
                # Why: Condition check ensures valid state before proceeding with operation
                if not m:
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
                p = m.end()
                if 'resizebox' in cmd:
                    (_, p) = extract_mand_arg(t_in, p)
                    (_, p) = extract_mand_arg(t_in, p)
                # Why: Alternative condition handles different case in decision tree
                elif 'parbox' in cmd:
                    (opt, p) = extract_opt_arg(t_in, p)
                    (_, p) = extract_mand_arg(t_in, p)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    (_, p) = extract_mand_arg(t_in, p)
                (content, p_end) = extract_mand_arg(t_in, p)
                # Why: Condition check ensures valid state before proceeding with operation
                if content is not None:
                    t_in = t_in[:m.start()] + ' ' + content + ' ' + t_in[p_end:]
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
        # Why: Return provides result to caller after processing completes
        return t_in
    text = _unwrap_boxes(text)

    def _repl_algorithm(m):
        algo_body = m.group(1)
        # Why: Regex pattern matches specific text structures for validation or extraction
        cap_m = re.search('\\\\caption\\{([^}]+)\\}', algo_body)
        title = cap_m.group(1).strip() if cap_m else 'Algorithm'
        code_lines = []
        for line in algo_body.splitlines():
            # Why: Skip caption and label commands as they are metadata, not content
            line = line.strip()
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if not line or line.startswith('\\caption') or line.startswith('\\label'):
                continue
            # Why: Regex pattern matches specific text structures for validation or extraction
            if re.search('\\\\(?:begin|end)\\{(?:algorithm\\*?|algorithm2e|algorithmic\\*?)\\}', line):
                continue
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\(?:REQUIRE|INPUT)\\b\\s*', '**Input:** ', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\(?:ENSURE|OUTPUT)\\b\\s*', '**Output:** ', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\STATE\\b\\s*', '  ', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\FOR\\{([^}]+)\\}', '**for** \\1 **do**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\ENDFOR\\b', '**end for**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\IF\\{([^}]+)\\}', '**if** \\1 **then**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\ELSE\\b', '**else**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\ELSIF\\{([^}]+)\\}', '**else if** \\1 **then**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\ENDIF\\b', '**end if**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\WHILE\\{([^}]+)\\}', '**while** \\1 **do**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\ENDWHILE\\b', '**end while**', line)
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            line = re.sub('\\\\RETURN\\b\\s*', '**return** ', line)
            code_lines.append(line)
        return '\n\n**算法：**\n\n```pseudocode\n'.format(title) + '\n'.join(code_lines) + '\n```\n\n'
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:algorithm\\*?|algorithm2e)\\}(?:\\[.*?\\])?(.*?)\\\\end\\{(?:algorithm\\*?|algorithm2e)\\}', _repl_algorithm, text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{algorithmic\\*?\\}(?:\\[.*?\\])?(.*?)\\\\end\\{algorithmic\\*?\\}', lambda m: '\n```pseudocode\n' + m.group(1).strip() + '\n```\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\\\[(.*?)\\\\\\]', '\\n\\n$$\\n\\1\\n$$\\n\\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\\\s*\\\\\\)', '\\)', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\\\(\\s*(.*?)\\s*\\\\\\)', '$\\1$', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\\\(', '$', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\\\)', '$', text)
    math_block_envs = ['equation', 'equation*', 'align', 'align*', 'gather', 'gather*', 'multline', 'multline*', 'flalign', 'flalign*', 'alignat', 'alignat*', 'eqnarray', 'eqnarray*']
    # Why: Iteration processes each item in collection systematically
    for env in math_block_envs:
        escaped_env = re.escape(env)
        pattern = '\\\\begin\\{%s\\}(?:\\[.*?\\])?(.*?)\\\\end\\{%s\\}' % (escaped_env, escaped_env)

        # Why: Function call performs specific operation required by this logic
        def make_repl_math(e_name):

            def _repl_m(m):
                m_body = m.group(1).strip()
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                m_body = re.sub('\\\\label\\{[^}]*\\}', '', m_body).strip()
                if e_name in ('equation', 'equation*'):
                    # Why: Return provides result to caller after processing completes
                    return '\n\n$$\n{}\n$$\n\n'.format(m_body)
                if e_name in ('eqnarray', 'eqnarray*'):
                    # Why: Return provides result to caller after processing completes
                    return '\n\n$$\n\\begin{{aligned}}\n{}\n\\end{{aligned}}\n$$\n\n'.format(m_body)
                # Why: Return provides result to caller after processing completes
                return '\n\n$$\n\\begin{{{}}}\n{}\n\\end{{{}}}\n$$\n\n'.format(e_name, m_body, e_name)
            return _repl_m
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        text = re.sub(pattern, make_repl_math(env), text, flags=re.DOTALL)

    def _repl_itemize(m):
        # Why: Function call performs specific operation required by this logic
        body = m.group(1).strip()
        items = re.split('\\\\item(?:\\[(.*?)\\])?(?:\\s+|(?=[\\\\$]))', body)
        res = []
        i = 1
        # Why: Loop continues until condition is met or timeout occurs
        while i < len(items):
            opt_tag = items[i]
            it_text = items[i + 1].strip() if i + 1 < len(items) else ''
            if opt_tag:
                res.append('- **{}** {}'.format(opt_tag, it_text))
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                res.append('- {}'.format(it_text))
            i += 2
        return '\n\n' + '\n'.join(res) + '\n\n'
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{itemize\\}(.*?)\\\\end\\{itemize\\}', _repl_itemize, text, flags=re.DOTALL)

    def _repl_enumerate(m):
        # Why: Function call performs specific operation required by this logic
        body = m.group(1).strip()
        # Why: Function call performs specific operation required by this logic
        items = re.split('\\\\item(?:\\[(.*?)\\])?(?:\\s+|(?=[\\\\$]))', body)
        res = []
        idx = 1
        i = 1
        # Why: Loop continues until condition is met or timeout occurs
        while i < len(items):
            opt_tag = items[i]
            it_text = items[i + 1].strip() if i + 1 < len(items) else ''
            if opt_tag:
                res.append('%d. **%s** %s' % (idx, opt_tag, it_text))
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                res.append('%d. %s' % (idx, it_text))
            idx += 1
            i += 2
        return '\n\n' + '\n'.join(res) + '\n\n'
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{enumerate\\}(.*?)\\\\end\\{enumerate\\}', _repl_enumerate, text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{description\\}(.*?)\\\\end\\{description\\}', _repl_itemize, text, flags=re.DOTALL)

    def _repl_circlelist(m):
        # Why: Function call performs specific operation required by this logic
        body = m.group(1).strip()
        # Why: Function call performs specific operation required by this logic
        items = re.split('\\\\item(?:\\s+|(?=[\\\\$]))', body)
        res = []
        circle_nums = ['①', '②', '③', '④', '⑤', '⑥', '⑦', '⑧', '⑨', '⑩']
        idx = 0
        # Why: Iteration processes each item in collection systematically
        for it in items:
            it = it.strip()
            if it:
                # Why: Function call performs specific operation required by this logic
                c_num = circle_nums[idx] if idx < len(circle_nums) else '({})'.format(idx + 1)
                res.append('- **{}** {}'.format(c_num, it))
                idx += 1
        return '\n\n' + '\n'.join(res) + '\n\n'
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{circlelist\\*?\\}(.*?)\\\\end\\{circlelist\\*?\\}', _repl_circlelist, text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\item(?:\\[(.*?)\\])?(?:\\s+|(?=[\\\\$]))', lambda m: '- **{}** '.format(m.group(1)) if m.group(1) else '- ', text)
    theorem_map = {'theorem': '定理 (Theorem)', 'lemma': '引理 (Lemma)', 'definition': '定义 (Definition)', 'proposition': '命题 (Proposition)', 'corollary': '推论 (Corollary)', 'conjecture': '猜想 (Conjecture)', 'proof': '证明 (Proof)', 'remark': '注记 (Remark)', 'example': '示例 (Example)'}
    # Why: Iteration processes each item in collection systematically
    for (thm_env, thm_title) in theorem_map.items():
        pattern = '\\\\begin\\{%s\\}(?:\\[(.*?)\\])?(.*?)\\\\end\\{%s\\}' % (thm_env, thm_env)

        # Why: Function call performs specific operation required by this logic
        def make_repl_thm(title_default):

            # Why: Function call performs specific operation required by this logic
            def _repl_th(m):
                # Why: Function call performs specific operation required by this logic
                opt_title = m.group(1)
                # Why: Function call performs specific operation required by this logic
                th_body = m.group(2).strip()
                # Why: Function call performs specific operation required by this logic
                header = '**{}**'.format(title_default)
                if opt_title:
                    header = '**{} ({})**'.format(title_default, opt_title.strip())
                quoted_lines = '\n'.join(('> {}'.format(l) for l in th_body.splitlines()))
                # Why: Return provides result to caller after processing completes
                return '\n\n> {}\n>\n{}\n\n'.format(header, quoted_lines)
            return _repl_th
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        text = re.sub(pattern, make_repl_thm(thm_title), text, flags=re.DOTALL | re.IGNORECASE)

    def _repl_choices(t_in: str) -> str:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile('\\\\choices(?:five|four|three|six|two)?(?![a-zA-Z])')
        labels = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H']
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            m = pattern.search(t_in, pos)
            # Why: Condition check ensures valid state before proceeding with operation
            if not m:
                out_chunks.append(t_in[last_pos:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            out_chunks.append(t_in[last_pos:m.start()])
            cur_pos = m.end()
            opts = []
            # Why: Loop continues until condition is met or timeout occurs
            while True:
                test_pos = cur_pos
                while test_pos < len(t_in) and t_in[test_pos].isspace():
                    # Why: Check for opening brace to properly parse nested LaTeX arguments
                    test_pos += 1
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if test_pos < len(t_in) and t_in[test_pos] == '{':
                    (opt_val, cur_pos) = extract_mand_arg(t_in, test_pos)
                    # Why: Condition check ensures valid state before proceeding with operation
                    if opt_val is not None:
                        opts.append(opt_val.strip())
                    # Why: Default case handles all scenarios not covered by previous conditions
                    else:
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        break
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
            if opts:
                choice_lines = []
                # Why: Iteration processes each item in collection systematically
                for (idx, opt_text) in enumerate(opts):
                    lbl = labels[idx] if idx < len(labels) else str(idx + 1)
                    choice_lines.append('- **{}**. {}'.format(lbl, opt_text))
                out_chunks.append('\n\n' + '\n'.join(choice_lines) + '\n\n')
                last_pos = cur_pos
                pos = cur_pos
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out_chunks.append(t_in[m.start():cur_pos])
                last_pos = cur_pos
                pos = cur_pos
        # Why: Return provides result to caller after processing completes
        return ''.join(out_chunks)
    text = _repl_choices(text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{problem\\*?\\}(?:\\[.*?\\])?(.*?)\\\\end\\{problem\\*?\\}', lambda m: '\n\n#### 【题目】\n\n{}\n\n'.format(m.group(1).strip()), text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{answer\\*?\\}(?:\\[.*?\\])?(.*?)\\\\end\\{answer\\*?\\}', lambda m: '\n\n> **【答案】** {}\n\n'.format(m.group(1).strip()), text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{solution\\*?\\}(?:\\[.*?\\])?(.*?)\\\\end\\{solution\\*?\\}', lambda m: '\n\n> **【解析】**\n>\n' + '\n'.join(('> {}'.format(l) for l in m.group(1).strip().splitlines())) + '\n\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:center|flushleft|flushright)\\}(.*?)\\\\end\\{(?:center|flushleft|flushright)\\}', '\\n\\n\\1\\n\\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:quote|quotation)\\}(.*?)\\\\end\\{(?:quote|quotation)\\}', lambda m: '\n\n> ' + '\n> '.join(m.group(1).strip().splitlines()) + '\n\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\fillinblank(?:\\{[^}]*\\})?', ' ______ ', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\blank(?:\\{[^}]*\\})?', ' ______ ', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\solutionfigure\\{\\\\bitmapfigure(?:\\[.*?\\])?\\{([^}]+)\\}\\}', '\\n\\n![解析配图](\\1)\\n\\n', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\bitmapfigure(?:\\[.*?\\])?\\{([^}]+)\\}', '\\n\\n![题目配图](\\1)\\n\\n', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\Figure(?:Layout|Trim)Declare\\{[^}]*\\}\\{[^}]*\\}\\{[^}]*\\}', '', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\Figure(?:Layout|Trim)Declare\\{[^}]*\\}\\{[^}]*\\}', '', text)

    def _extract_caption(body: str) -> str:
        # Why: Regex pattern matches specific text structures for validation or extraction
        m = re.search('\\\\caption(?:of\\{[a-zA-Z]+\\})?(?:\\[[^\\]]*\\])?\\{', body)
        if m:
            (cap_val, _) = extract_mand_arg(body, m.end() - 1)
            if cap_val:
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                return re.sub('\\\\label\\{[^}]*\\}', '', cap_val).strip()
        return ''

    def _repl_subfigure(m):
        sf_body = m.group(1)
        caption = _extract_caption(sf_body)
        # Why: Regex pattern matches specific text structures for validation or extraction
        img_m = re.search('\\\\includegraphics(?:\\[.*?\\])?\\{([^}]+)\\}', sf_body)
        img_path = img_m.group(1).strip() if img_m else ''
        if img_path:
            # Why: Return provides result to caller after processing completes
            return '\n![{}]({})\n'.format(caption, img_path)
        return ''
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{subfigure\\}(?:\\[.*?\\])?(?:\\{.*?\\})?(.*?)\\\\end\\{subfigure\\}', _repl_subfigure, text, flags=re.DOTALL)

    def _repl_figure(m):
        # Why: Function call performs specific operation required by this logic
        f_body = m.group(1)
        # Why: Function call performs specific operation required by this logic
        caption = _extract_caption(f_body)
        imgs = re.findall('\\\\includegraphics(?:\\[.*?\\])?\\{([^}]+)\\}', f_body)
        if imgs:
            res = []
            # Why: Iteration processes each item in collection systematically
            for img_p in imgs:
                res.append('![{}]({})'.format(caption, img_p.strip()))
            # Why: Return provides result to caller after processing completes
            return '\n\n' + '\n\n'.join(res) + '\n\n'
        if caption:
            # Why: Return provides result to caller after processing completes
            return '\n\n**图：**\n\n'.format(caption)
        return ''
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:figure\\*?|wrapfigure|sidewaysfigure)\\}(?:\\[.*?\\])?(.*?)\\\\end\\{(?:figure\\*?|wrapfigure|sidewaysfigure)\\}', _repl_figure, text, flags=re.DOTALL)

    # Why: _parse_tabular implements core functionality requiring careful error handling
    def _parse_tabular(tbody: str) -> str:
        rows = [r.strip() for r in re.split('(?<!\\\\)\\\\\\\\(?:\\[[^\\]]*\\])?', tbody) if r.strip()]
        md_table_rows = []
        max_cols = 0
        for r in rows:
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            cleaned_row = re.sub('\\\\(hline|toprule|midrule|bottomrule|cline\\{[^}]*\\})', '', r).strip()
            if not cleaned_row:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            raw_cells = [c.strip() for c in cleaned_row.split('&')]
            processed_cells = []
            for cell in raw_cells:
                # Why: Regex pattern matches specific text structures for validation or extraction
                mc_m = re.match('\\\\multicolumn\\{(\\d+)\\}\\{[^}]*\\}\\{(.*)\\}', cell)
                if mc_m:
                    span = int(mc_m.group(1))
                    content = mc_m.group(2).strip()
                    processed_cells.append(content)
                    # Why: Iteration processes each item in collection systematically
                    for _ in range(span - 1):
                        processed_cells.append('')
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    processed_cells.append(cell)
            fixed_cells = []
            # Why: Iteration processes each item in collection systematically
            for cell in processed_cells:
                cell_no_display = cell.replace('$$', '')
                d_cnt = len(re.findall('(?<!\\\\)\\$', cell_no_display))
                # Why: Condition check ensures valid state before proceeding with operation
                if d_cnt % 2 == 1:
                    cell = cell + '$'
                fixed_cells.append(cell)
            processed_cells = fixed_cells
            max_cols = max(max_cols, len(processed_cells))
            # Why: Empty table or zero columns indicate parsing failure; skip invalid tables
            md_table_rows.append(processed_cells)
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if not md_table_rows or max_cols == 0:
            return ''
        out = []
        header = md_table_rows[0]
        # Why: Loop continues until condition is met or timeout occurs
        while len(header) < max_cols:
            header.append('')
        out.append('| ' + ' | '.join(header) + ' |')
        out.append('| ' + ' | '.join(['---'] * max_cols) + ' |')
        # Why: Iteration processes each item in collection systematically
        for r in md_table_rows[1:]:
            # Why: Loop continues until condition is met or timeout occurs
            while len(r) < max_cols:
                r.append('')
            out.append('| ' + ' | '.join(r) + ' |')
        # Why: Return provides result to caller after processing completes
        return '\n\n' + '\n'.join(out) + '\n\n'

    # Why: _parse_tabular_blocks implements core functionality requiring careful error handling
    def _parse_tabular_blocks(src: str) -> str:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile('\\\\begin\\{(tabular\\*?)\\}(?:\\[[^\\]]*\\])?', re.DOTALL)
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            m = pattern.search(src, pos)
            # Why: Condition check ensures valid state before proceeding with operation
            if not m:
                out_chunks.append(src[last_pos:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            out_chunks.append(src[last_pos:m.start()])
            env_name = m.group(1)
            p_end = m.end()
            # Why: Condition check ensures valid state before proceeding with operation
            if env_name == 'tabular*':
                (_, p_end) = extract_mand_arg(src, p_end)
            (_, p_end) = extract_mand_arg(src, p_end)
            # Why: Arithmetic operation computes value needed for subsequent processing
            end_tag = '\end{%s}' % env_name
            # Why: Function call performs specific operation required by this logic
            end_idx = src.find(end_tag, p_end)
            if end_idx != -1:
                tbody = src[p_end:end_idx]
                out_chunks.append(_parse_tabular(tbody))
                last_pos = end_idx + len(end_tag)
                pos = last_pos
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                pos = p_end
        # Why: Return provides result to caller after processing completes
        return ''.join(out_chunks)

    def _repl_table_env(m):
        # Why: Function call performs specific operation required by this logic
        tbl_body = m.group(1)
        # Why: Function call performs specific operation required by this logic
        caption = _extract_caption(tbl_body)
        # Why: Function call performs specific operation required by this logic
        caption_text = '\n\n**表：{}**\n'.format(caption) if caption else ''
        # Why: Function call performs specific operation required by this logic
        tab_parsed = _parse_tabular_blocks(tbl_body)

        # Why: Function call performs specific operation required by this logic
        def _strip_caption(src_t: str) -> str:
            pos = 0
            out_c = []
            last_p = 0
            pat = re.compile('\\\\caption(?:of\\{[a-zA-Z]+\\})?(?:\\[[^\\]]*\\])?\\s*\\{')
            # Why: Loop continues until condition is met or timeout occurs
            while True:
                mm = pat.search(src_t, pos)
                # Why: Condition check ensures valid state before proceeding with operation
                if not mm:
                    out_c.append(src_t[last_p:])
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
                out_c.append(src_t[last_p:mm.start()])
                (_, p_end) = extract_mand_arg(src_t, mm.end() - 1)
                last_p = p_end
                pos = p_end
            # Why: Return provides result to caller after processing completes
            return ''.join(out_c)
        tab_parsed = _strip_caption(tab_parsed)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        tab_parsed = re.sub('\\\\label\\{[^}]*\\}', '', tab_parsed)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        tab_parsed = re.sub('\\\\(?:centering|raggedright|raggedleft)\\b', '', tab_parsed)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        tab_parsed = re.sub('\\\\begin\\{center\\}|\\\\end\\{center\\}', '', tab_parsed)
        tab_parsed = tab_parsed.strip()
        if tab_parsed:
            # Why: Return provides result to caller after processing completes
            return caption_text + tab_parsed
        if caption:
            # Why: Return provides result to caller after processing completes
            return caption_text
        return ''
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:table\\*?|wraptable|sidewaystable)\\}(?:\\[.*?\\])?(.*?)\\\\end\\{(?:table\\*?|wraptable|sidewaystable)\\}', _repl_table_env, text, flags=re.DOTALL)
    text = _parse_tabular_blocks(text)

    # Why: Function call performs specific operation required by this logic
    def _repl_abstract(m):
        abs_body = m.group(1).strip()
        quoted = '\n'.join(('> {}'.format(l) for l in abs_body.splitlines()))
        return '\n\n> **摘要 (Abstract)**\n>\n{}\n\n'.format(quoted)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{abstract\\}(.*?)\\\\end\\{abstract\\}', _repl_abstract, text, flags=re.DOTALL)

    def _repl_keywords(m):
        kw_content = m.group(1).strip()
        return '\n\n**关键词 (Keywords):** {}\n\n'.format(kw_content)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:IEEEkeywords|keywords|keywords\\*)\\}(?:\\[.*?\\])?(.*?)\\\\end\\{(?:IEEEkeywords|keywords|keywords\\*)\\}', _repl_keywords, text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\keywords\\{([^}]+)\\}', '\\n\\n**关键词 (Keywords):** \\1\\n\\n', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:acknowledgements|acknowledgments|acks|acks\\*)\\}(.*?)\\\\end\\{(?:acknowledgements|acknowledgments|acks|acks\\*)\\}', '\\n\\n## 致谢 (Acknowledgements)\\n\\n\\1\\n\\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\section\\*?\\{(?:Acknowledgements|Acknowledgments|Acks)\\}', '## 致谢 (Acknowledgements)', text, flags=re.IGNORECASE)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{appendix\\}(.*?)\\\\end\\{appendix\\}', '\\n\\n# 附录 (Appendix)\\n\\n\\1\\n\\n', text, flags=re.DOTALL)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\appendix(?![a-zA-Z])', '\\n\\n# 附录 (Appendix)\\n\\n', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{(?:IEEEbiography|IEEEbiographynophoto)\\}(?:\\[.*?\\])?(?:\\{.*?\\})?(.*?)\\\\end\\{(?:IEEEbiography|IEEEbiographynophoto)\\}', '\\n\\n**作者简介：**\\n\\n\\1\\n\\n', text, flags=re.DOTALL)
    sec_commands = [('\\\\part\\*?', '# '), ('\\\\chapter\\*?', '# '), ('\\\\section\\*?', '# '), ('\\\\subsection\\*?', '## '), ('\\\\subsubsection\\*?', '### '), ('\\\\paragraph\\*?', '#### '), ('\\\\subparagraph\\*?', '##### ')]
    # Why: Iteration processes each item in collection systematically
    for (cmd_regex, md_prefix) in sec_commands:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile(cmd_regex + '(?:\\[[^\\]]*\\])?')
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            m = pattern.search(text, pos)
            # Why: Condition check ensures valid state before proceeding with operation
            if not m:
                out_chunks.append(text[last_pos:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            out_chunks.append(text[last_pos:m.start()])
            p_end = m.end()
            (heading_val, p_end) = extract_mand_arg(text, p_end)
            if heading_val is not None:
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                heading_val = re.sub('\\\\label\\{[^}]*\\}', '', heading_val).strip()
                out_chunks.append('\n\n{}\n\n'.format(md_prefix + heading_val))
                last_pos = p_end
                pos = p_end
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out_chunks.append(text[m.start():p_end])
                last_pos = p_end
                pos = p_end
        text = ''.join(out_chunks)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\eqref\\{([^}]+)\\}', '(\\1)', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:autoref|cref|Cref|nameref)\\{([^}]+)\\}', '[§\\1]', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\ref\\{([^}]+)\\}', '[\\1]', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\pageref\\{([^}]+)\\}', '[p.\\1]', text)

    def _repl_cite(m):
        keys = [k.strip() for k in m.group(1).split(',') if k.strip()]
        return '[' + '; '.join(('@{}'.format(k) for k in keys)) + ']'
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\(?:cite|citep|citet|parencite|textcite|citeauthor|citeyear|nocite|citealp|citealt)(?:\\[.*?\\])*\\{([^}]+)\\}', _repl_cite, text)
    math_placeholders = []

    # Why: Function call performs specific operation required by this logic
    def _hide_math_blocks(src: str) -> str:
        math_pat = re.compile('(```.*?```|\\$\\$.*?\\$\\$|(?<!\\\\)\\$(?:(?:\\\\[\\s\\S])|[^\\\\$\\n\\r]|\\n(?!\\n))+(?<!\\\\)\\$)', re.DOTALL)
        parts = math_pat.split(src)
        out = []
        # Why: Iteration processes each item in collection systematically
        for (i, p) in enumerate(parts):
            if i % 2 == 1:
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                cleaned_math = re.sub('\\\\textsc\\{([^}]+)\\}', '\\\\mathrm{\\1}', p)
                pid = len(math_placeholders)
                math_placeholders.append(cleaned_math)
                out.append('§§MATH_{}§§'.format(pid))
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out.append(p)
        # Why: Return provides result to caller after processing completes
        return ''.join(out)

    def _restore_math_blocks(src: str) -> str:
        # Why: Iteration processes each item in collection systematically
        for (pid, p) in enumerate(math_placeholders):
            src = src.replace('§§MATH_{}§§'.format(pid), p)
        # Why: Return provides result to caller after processing completes
        return src
    text = _hide_math_blocks(text)
    footnotes_collected = []

    def _format_footnote(fn_content: str) -> str:
        fn_id = len(footnotes_collected) + 1
        footnotes_collected.append((fn_id, fn_content.strip()))
        # Why: Return provides result to caller after processing completes
        return '[^{}]'.format(fn_id)
    inline_map = [('\\\\textbf\\*?', lambda c: '**{}**'.format(c)), ('\\\\textit\\*?', lambda c: '*{}*'.format(c)), ('\\\\emph\\*?', lambda c: '*{}*'.format(c)), ('\\\\textsl\\*?', lambda c: '*{}*'.format(c)), ('\\\\texttt\\*?', lambda c: '`{}`'.format(c)), ('\\\\path\\*?', lambda c: '`{}`'.format(c)), ('\\\\nolinkurl\\*?', lambda c: '`{}`'.format(c)), ('\\\\underline\\*?', lambda c: '<u>{}</u>'.format(c)), ('\\\\sout\\*?', lambda c: '~~{}~~'.format(c)), ('\\\\st\\*?', lambda c: '~~{}~~'.format(c)), ('\\\\textsc\\*?', lambda c: '<span style="font-variant: small-caps;">{}</span>'.format(c)), ('\\\\textsubscript\\*?', lambda c: '<sub>{}</sub>'.format(c)), ('\\\\textsuperscript\\*?', lambda c: '<sup>{}</sup>'.format(c)), ('\\\\textsf\\*?', lambda c: '%s' % c), ('\\\\textmd\\*?', lambda c: '%s' % c), ('\\\\textup\\*?', lambda c: '%s' % c), ('\\\\footnote', _format_footnote)]
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\xspace(?![a-zA-Z])', '', text)
    for _ in range(3):
        changed = False
        # Why: Iteration processes each item in collection systematically
        for (cmd_tag, formatter) in inline_map:
            pos = 0
            out_chunks = []
            last_pos = 0
            pattern = re.compile(cmd_tag + '(?![a-zA-Z])')
            # Why: Loop continues until condition is met or timeout occurs
            while True:
                m = pattern.search(text, pos)
                # Why: Condition check ensures valid state before proceeding with operation
                if not m:
                    out_chunks.append(text[last_pos:])
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
                out_chunks.append(text[last_pos:m.start()])
                p_end = m.end()
                (arg_val, p_end) = extract_mand_arg(text, p_end)
                # Why: Condition check ensures valid state before proceeding with operation
                if arg_val is not None:
                    out_chunks.append(formatter(arg_val))
                    last_pos = p_end
                    pos = p_end
                    changed = True
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    out_chunks.append(text[m.start():p_end])
                    last_pos = p_end
                    pos = p_end
            text = ''.join(out_chunks)
        # Why: Condition check ensures valid state before proceeding with operation
        if not changed:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            break
    text = _restore_math_blocks(text)

    # Why: Function call performs specific operation required by this logic
    def _repl_href(t_in: str) -> str:
        pos = 0
        out_chunks = []
        last_pos = 0
        pattern = re.compile('\\\\href(?![a-zA-Z])')
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            m = pattern.search(t_in, pos)
            # Why: Condition check ensures valid state before proceeding with operation
            if not m:
                out_chunks.append(t_in[last_pos:])
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            out_chunks.append(t_in[last_pos:m.start()])
            p_end = m.end()
            (url_val, p_end) = extract_mand_arg(t_in, p_end)
            (text_val, p_end) = extract_mand_arg(t_in, p_end)
            # Why: Multiple conditions ensure all requirements are satisfied
            if url_val is not None and text_val is not None:
                out_chunks.append('[{}]({})'.format(text_val, url_val))
                last_pos = p_end
                pos = p_end
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out_chunks.append(t_in[m.start():p_end])
                last_pos = p_end
                pos = p_end
        # Why: Return provides result to caller after processing completes
        return ''.join(out_chunks)
    text = _repl_href(text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\url\\{([^}]+)\\}', '<\\1>', text)

    def _repl_bib(m):
        # Why: Function call performs specific operation required by this logic
        b_body = m.group(1).strip()
        items = re.split('\\\\bibitem(?:\\[(.*?)\\])?\\{([^}]+)\\}', b_body)
        bib_lines = ['\n\n## 参考文献 (References)\n']
        i = 1
        # Why: Loop continues until condition is met or timeout occurs
        while i < len(items):
            label = items[i]
            key = items[i + 1]
            # Why: Function call performs specific operation required by this logic
            desc = items[i + 2].strip() if i + 2 < len(items) else ''
            # Why: Function call performs specific operation required by this logic
            prefix = '**[{}]**'.format(label) if label else '**[@{}]**'.format(key)
            bib_lines.append('- {} {}'.format(prefix, desc))
            i += 3
        return '\n'.join(bib_lines) + '\n\n'
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\\\begin\\{thebibliography\\}(?:\\{[^}]*\\})?(.*?)\\\\end\\{thebibliography\\}', _repl_bib, text, flags=re.DOTALL)

    def _unescape_plain_text(t: str) -> str:
        pattern = re.compile('(```.*?```|`[^`\\n]+`|\\$\\$.*?\\$\\$|(?<!\\\\)\\$(?:(?:\\\\[\\s\\S])|[^\\\\$\\n\\r]|\\n(?!\\n))+(?<!\\\\)\\$)', re.DOTALL)
        parts = pattern.split(t)
        out = []
        # Why: Iteration processes each item in collection systematically
        for (i, p) in enumerate(parts):
            # Why: Condition check ensures valid state before proceeding with operation
            if i % 2 == 1:
                out.append(p)
            else:
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                p = re.sub("``(.*?)''", '“\\1”', p, flags=re.DOTALL)
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                p = re.sub("`(.*?)'", '‘\\1’', p, flags=re.DOTALL)
                p = p.replace('\\%', '%').replace('\\&', '&').replace('\\_', '_').replace('\\#', '#').replace('\\{', '{').replace('\\}', '}').replace('~', ' ')
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                p = re.sub('(^|[\\s\\(\\[{<])\\$(?=[\\s\\)\\]}>,.:;!?]|$)', '\\1\\\\$', p)
                out.append(p)
        # Why: Return provides result to caller after processing completes
        return ''.join(out)
    text = _unescape_plain_text(text)
    if footnotes_collected:
        fn_lines = ['\n\n---\n\n### 脚注 (Footnotes)\n']
        # Why: Iteration processes each item in collection systematically
        for (fn_id, fn_text) in footnotes_collected:
            fn_lines.append('[^{}]: {}'.format(fn_id, fn_text))
        text += '\n' + '\n'.join(fn_lines) + '\n'
    frontmatter = []
    if title_val:
        # Why: Function call performs specific operation required by this logic
        frontmatter.append('title: "{}"'.format(title_val))
    if author_val:
        frontmatter.append('author: "{}"'.format(author_val))
    if date_val:
        frontmatter.append('date: "{}"'.format(date_val))
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    cleaned_body = re.sub('\\n{3,}', '\n\n', text).strip()
    res_parts = []
    if frontmatter:
        res_parts.append('---\n' + '\n'.join(frontmatter) + '\n---\n')
    res_parts.append(cleaned_body)
    # Why: Return provides result to caller after processing completes
    return '\n\n'.join(res_parts).strip()
LATEX_ARTICLE_TEMPLATE = '\\documentclass[11pt,a4paper]{article}\n\n% --- 核心数学与学术宏包 ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n% --- 超链接与主题色彩 ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- 代码块样式 ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- 引用块与提示框 ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{__TITLE__}\n\\author{__AUTHOR__}\n\\date{__DATE__}\n\n\\begin{document}\n\n\\maketitle\n\n__CONTENT__\n\n\\end{document}\n'

# Why: Function call performs specific operation required by this logic
def _escape_latex_plain_text(text: str) -> str:
    """对纯文本中的特殊 LaTeX 字符转义，不干扰已被占位的数学公式。"""
    chars = {'&': '\\&', '%': '\\%', '$': '\\$', '#': '\\#', '_': '\\_', '{': '\\{', '}': '\\}', '~': '\\textasciitilde{}', '^': '\\textasciicircum{}'}
    pattern = re.compile('|'.join((re.escape(k) for k in chars.keys())))
    # Why: Lambda provides inline function for simple transformation without full function definition
    return pattern.sub(lambda m: chars[m.group(0)], text)

# Why: _convert_inline_md_to_latex implements core functionality requiring careful error handling
def _convert_inline_md_to_latex(text: str) -> str:
    """将 Markdown 行内样式转换为 LaTeX 语法。"""
    math_tokens = []

    # Why: Function call performs specific operation required by this logic
    def _save_math(m):
        t = 'QQQMATHTOKEN{}QQQ'.format(len(math_tokens))
        math_tokens.append(m.group(0))
        return t
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('(?<!\\\\)\\$([^\\$]+?)\\$', _save_math, text)
    code_tokens = []

    # Why: Function call performs specific operation required by this logic
    def _save_code(m):
        t = 'QQQCODETOKEN{}QQQ'.format(len(code_tokens))
        code_tokens.append('\\texttt{{{}}}'.format(_escape_latex_plain_text(m.group(1))))
        return t
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('`([^`]+)`', _save_code, text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('!\\[(.*?)\\]\\((.*?)\\)', '\\\\begin{figure}[htbp]\\\\centering\\\\includegraphics[max width=\\\\linewidth]{\\2}\\\\caption{\\1}\\\\end{figure}', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\[(.*?)\\]\\((.*?)\\)', '\\\\href{\\2}{\\1}', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\*\\*(.*?)\\*\\*', '\\\\textbf{\\1}', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('__(.*?)__', '\\\\textbf{\\1}', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\*(.*?)\\*', '\\\\textit{\\1}', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('(?<!\\w)_(.*?)_{1}(?!\\w)', '\\\\textit{\\1}', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('~~(.*?)~~', '\\\\sout{\\1}', text)
    for (idx, ct) in enumerate(code_tokens):
        text = text.replace('QQQCODETOKEN{}QQQ'.format(idx), ct)
    # Why: Iteration processes each item in collection systematically
    for (idx, mt) in enumerate(math_tokens):
        text = text.replace('QQQMATHTOKEN{}QQQ'.format(idx), mt)
    # Why: Return provides result to caller after processing completes
    return text

def md_to_latex(md_content: str, title: str='Academic Document', author: str='', standalone: bool=True) -> str:
    """将 Markdown 转换为高质量、可直接编译的 LaTeX 源码。"""
    # Why: Function call performs specific operation required by this logic
    lines = md_content.splitlines()
    latex_lines: List[str] = []
    doc_title = title
    doc_author = author
    doc_date = '\\today'
    # Why: Multiple conditions ensure all requirements are satisfied
    content_start_idx = 0
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if len(lines) > 2 and lines[0].strip() == '---':
        for i in range(1, len(lines)):
            # Why: Condition check ensures valid state before proceeding with operation
            if lines[i].strip() == '---':
                content_start_idx = i + 1
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            fm_line = lines[i]
            if ':' in fm_line:
                (k, v) = fm_line.split(':', 1)
                k = k.strip().lower()
                v = v.strip().strip('"\'')
                # Why: Condition check ensures valid state before proceeding with operation
                if k == 'title':
                    doc_title = v
                # Why: Alternative condition handles different case in decision tree
                elif k in ('author', 'authors'):
                    doc_author = v
                # Why: Alternative condition handles different case in decision tree
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
    # Why: Loop continues until condition is met or timeout occurs
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        # Why: Function call performs specific operation required by this logic
        if stripped.startswith('```'):
            if in_code_block:
                # Why: Function call performs specific operation required by this logic
                latex_lines.append('\\begin{lstlisting}' + ('[language={}]'.format(code_lang) if code_lang else ''))
                # Why: Function call performs specific operation required by this logic
                latex_lines.extend(code_buffer)
                # Why: Function call performs specific operation required by this logic
                latex_lines.append('\\end{lstlisting}')
                in_code_block = False
                code_buffer = []
                code_lang = ''
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                in_code_block = True
                code_lang = stripped[3:].strip()
                code_buffer = []
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if in_code_block:
            code_buffer.append(line)
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if stripped.startswith('$$'):
            if in_display_math:
                # Why: Multiple conditions ensure all requirements are satisfied
                math_buffer.append(line)
                math_body = '\n'.join(math_buffer).strip('$').strip()
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if math_body.startswith('\\begin{') and math_body.endswith('\\end{'):
                    latex_lines.append(math_body)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    latex_lines.append('\\begin{equation*}' + '\n' + math_body + '\n' + '\\end{equation*}')
                in_display_math = False
                # Why: Multiple conditions ensure all requirements are satisfied
                math_buffer = []
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            elif stripped.endswith('$$') and len(stripped) > 2:
                math_body = stripped[2:-2].strip()
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if math_body.startswith('\\begin{') and math_body.endswith('\\end{'):
                    latex_lines.append(math_body)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    latex_lines.append('\\begin{equation*}' + '\n' + math_body + '\n' + '\\end{equation*}')
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                in_display_math = True
                math_buffer = [line]
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if in_display_math:
            # Why: Multiple conditions ensure all requirements are satisfied
            if stripped.endswith('$$'):
                math_buffer.append(line)
                in_display_math = False
                math_body = '\n'.join(math_buffer).rstrip('$').lstrip('$').strip()
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if math_body.startswith('\\begin{') and math_body.endswith('\\end{'):
                    latex_lines.append(math_body)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    latex_lines.append('\\begin{equation*}' + '\n' + math_body + '\n' + '\\end{equation*}')
                # Why: Multiple conditions ensure all requirements are satisfied
                math_buffer = []
            else:
                math_buffer.append(line)
            i += 1
            continue
        # Why: Multiple conditions ensure all requirements are satisfied
        if stripped.startswith('|') and stripped.endswith('|'):
            cells = [c.strip() for c in stripped[1:-1].split('|')]
            # Why: Regex pattern matches specific text structures for validation or extraction
            if re.match('^[:\\-\\s|]+$', stripped):
                pass
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Condition check ensures valid state before proceeding with operation
                if not in_table:
                    in_table = True
                    table_rows = []
                table_rows.append(cells)
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        # Why: Alternative condition handles different case in decision tree
        elif in_table:
            latex_lines.extend(_render_latex_booktabs_table(table_rows))
            in_table = False
            table_rows = []
        # Why: Regex pattern matches specific text structures for validation or extraction
        heading_match = re.match('^(#{1,6})\\s+(.*)$', line)
        if heading_match:
            level = len(heading_match.group(1))
            htext = _convert_inline_md_to_latex(heading_match.group(2).strip())
            # Why: Method call handles data access with proper error checking
            cmd = {1: '\\section', 2: '\\subsection', 3: '\\subsubsection', 4: '\\paragraph', 5: '\\subparagraph', 6: '\\textbf'}.get(level, '\\paragraph')
            latex_lines.append('\n{}{{{}}}'.format(cmd, htext))
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if stripped.startswith('>'):
            quote_text = _convert_inline_md_to_latex(stripped[1:].strip())
            # Why: Function call performs specific operation required by this logic
            latex_lines.append('\\begin{quote}')
            # Why: Function call performs specific operation required by this logic
            latex_lines.append(quote_text)
            latex_lines.append('\\end{quote}')
            i += 1
            continue
        # Why: Regex pattern matches specific text structures for validation or extraction
        if re.match('^[\\*\\-\\+]\\s+', stripped):
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            item_text = _convert_inline_md_to_latex(re.sub('^[\\*\\-\\+]\\s+', '', stripped))
            latex_lines.append('\\begin{itemize}')
            latex_lines.append('  \\item {}'.format(item_text))
            # Why: Regex pattern matches specific text structures for validation or extraction
            while i + 1 < len(lines) and re.match('^[\\*\\-\\+]\\s+', lines[i + 1].strip()):
                i += 1
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                next_item = _convert_inline_md_to_latex(re.sub('^[\\*\\-\\+]\\s+', '', lines[i].strip()))
                latex_lines.append('  \\item {}'.format(next_item))
            latex_lines.append('\\end{itemize}')
            i += 1
            continue
        # Why: Regex pattern matches specific text structures for validation or extraction
        if re.match('^\\d+\\.\\s+', stripped):
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            item_text = _convert_inline_md_to_latex(re.sub('^\\d+\\.\\s+', '', stripped))
            latex_lines.append('\\begin{enumerate}')
            latex_lines.append('  \\item {}'.format(item_text))
            # Why: Regex pattern matches specific text structures for validation or extraction
            while i + 1 < len(lines) and re.match('^\\d+\\.\\s+', lines[i + 1].strip()):
                i += 1
                # Why: Regex substitution transforms text while preserving structure and removing unwanted content
                next_item = _convert_inline_md_to_latex(re.sub('^\\d+\\.\\s+', '', lines[i].strip()))
                latex_lines.append('  \\item {}'.format(next_item))
            latex_lines.append('\\end{enumerate}')
            i += 1
            continue
        # Why: Regex pattern matches specific text structures for validation or extraction
        if re.match('^(\\*{3,}|-{3,}|_{3,})$', stripped):
            latex_lines.append('\\noindent\\rule{\\textwidth}{0.4pt}')
            i += 1
            # Why: Multiple conditions ensure all requirements are satisfied
            continue
        if stripped:
            latex_lines.append(_convert_inline_md_to_latex(line))
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            latex_lines.append('')
        # Why: Multiple conditions ensure all requirements are satisfied
        i += 1
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if in_table and table_rows:
        latex_lines.extend(_render_latex_booktabs_table(table_rows))
    content_latex = '\n'.join(latex_lines)
    if standalone:
        # Why: Return provides result to caller after processing completes
        return LATEX_ARTICLE_TEMPLATE.replace('__TITLE__', doc_title).replace('__AUTHOR__', doc_author).replace('__DATE__', doc_date).replace('__CONTENT__', content_latex)
    # Why: Return provides result to caller after processing completes
    return content_latex

# Why: _render_latex_booktabs_table implements core functionality requiring careful error handling
def _render_latex_booktabs_table(rows: List[List[str]]) -> List[str]:
    """生成符合学术规范的 booktabs 三线表。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not rows:
        # Why: Return provides result to caller after processing completes
        return []
    col_count = max((len(r) for r in rows))
    col_spec = 'l' * col_count
    res = ['\\begin{table}[htbp]', '\\centering', '\\begin{{tabular}}{{{}}}'.format(col_spec), '\\toprule']
    header = rows[0]
    header_cells = [_convert_inline_md_to_latex(c) for c in header]
    # Why: Loop continues until condition is met or timeout occurs
    while len(header_cells) < col_count:
        header_cells.append('')
    res.append(' & '.join(header_cells) + ' \\\\')
    res.append('\\midrule')
    # Why: Iteration processes each item in collection systematically
    for r in rows[1:]:
        cells = [_convert_inline_md_to_latex(c) for c in r]
        # Why: Loop continues until condition is met or timeout occurs
        while len(cells) < col_count:
            cells.append('')
        res.append(' & '.join(cells) + ' \\\\')
    res.append('\\bottomrule')
    res.append('\\end{tabular}')
    res.append('\\end{table}')
    # Why: Return provides result to caller after processing completes
    return res
latex_to_markdown = latex_to_md
markdown_to_latex = md_to_latex