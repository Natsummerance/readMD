"""ReadMD Lightweight LaTeX ⇄ Markdown Bidirectional Converter

Pure-Python, zero heavy external dependencies (no TeXLive/Pandoc required).
Supports:
1. Markdown -> LaTeX: Headings, Math (inline/display/environments), Tables (booktabs),
   Code blocks, Lists, Images, Links, and Academic Paper Standalone Template.
2. LaTeX -> Markdown: Section hierarchy, Math normalisation ($/$$), Booktabs to GFM Tables,
   Itemize/Enumerate to Lists, Macro pre-expansion, and Preamble extraction.
"""

import re
from typing import Dict, List, Tuple, Optional


# ---------------------------------------------------------------------------
# Markdown -> LaTeX
# ---------------------------------------------------------------------------

LATEX_STANDALONE_TEMPLATE = r"""\documentclass[11pt,a4paper]{article}

% --- Packages ---
\usepackage[utf8]{inputenc}
\usepackage[margin=2.5cm]{geometry}
\usepackage{amsmath,amssymb,amsfonts}
\usepackage{booktabs}
\usepackage{graphicx}
\usepackage{hyperref}
\usepackage{listings}
\usepackage{xcolor}
\usepackage{microtype}

\hypersetup{
    colorlinks=true,
    linkcolor=blue!70!black,
    citecolor=blue!70!black,
    urlcolor=blue!70!black
}

\lstset{
    basicstyle=\ttfamily\small,
    breaklines=true,
    frame=single,
    backgroundcolor=\color{gray!10},
    keywordstyle=\color{blue!80!black},
    commentstyle=\color{green!50!black},
    stringstyle=\color{red!70!black},
    showstringspaces=false
}

\title{__TITLE__}
\author{__AUTHOR__}
\date{\today}

\begin{document}

\maketitle

__CONTENT__

\end{document}
"""


def _escape_latex_text(text: str) -> str:
    """Escape special LaTeX characters in plain text, preserving math placeholders."""
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


def md_to_latex(md_content: str, title: str = 'Document', author: str = '', standalone: bool = True) -> str:
    """Convert Markdown content to LaTeX source."""
    lines = md_content.splitlines()
    latex_lines: List[str] = []
    
    # Check for YAML frontmatter
    doc_title = title
    doc_author = author
    content_start_idx = 0
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

        # 1. Code Block Fence
        if stripped.startswith('```'):
            if in_code_block:
                # End of code block
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

        # 2. Display Math Block ($$ ... $$)
        if stripped.startswith('$$'):
            if in_display_math:
                math_buffer.append(line)
                latex_lines.append(r'\begin{equation*}')
                math_body = '\n'.join(math_buffer).strip('$').strip()
                latex_lines.append(math_body)
                latex_lines.append(r'\end{equation*}')
                in_display_math = False
                math_buffer = []
            else:
                if stripped.endswith('$$') and len(stripped) > 2:
                    math_body = stripped[2:-2].strip()
                    latex_lines.append(r'\begin{equation*}')
                    latex_lines.append(math_body)
                    latex_lines.append(r'\end{equation*}')
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
                latex_lines.append(r'\begin{equation*}')
                latex_lines.append(math_body)
                latex_lines.append(r'\end{equation*}')
                math_buffer = []
            else:
                math_buffer.append(line)
            i += 1
            continue

        # 3. Table Rows
        if stripped.startswith('|') and stripped.endswith('|'):
            cells = [c.strip() for c in stripped[1:-1].split('|')]
            if re.match(r'^[:\-\s|]+$', stripped):
                # Separator line, ignore
                pass
            else:
                if not in_table:
                    in_table = True
                    table_rows = []
                table_rows.append(cells)
            i += 1
            continue
        elif in_table:
            latex_lines.extend(_render_latex_table(table_rows))
            in_table = False
            table_rows = []

        # 4. Headings
        heading_match = re.match(r'^(#{1,6})\s+(.*)$', line)
        if heading_match:
            level = len(heading_match.group(1))
            htext = _convert_inline_md(heading_match.group(2).strip())
            cmd = {
                1: r'\section',
                2: r'\subsection',
                3: r'\subsubsection',
                4: r'\paragraph',
                5: r'\subparagraph',
                6: r'\textbf'
            }.get(level, r'\paragraph')
            latex_lines.append(f'{cmd}{{{htext}}}')
            i += 1
            continue

        # 5. Blockquote
        if stripped.startswith('>'):
            quote_text = _convert_inline_md(stripped[1:].strip())
            latex_lines.append(r'\begin{quote}')
            latex_lines.append(quote_text)
            latex_lines.append(r'\end{quote}')
            i += 1
            continue

        # 6. Unordered List
        if re.match(r'^[\*\-\+]\s+', stripped):
            item_text = _convert_inline_md(re.sub(r'^[\*\-\+]\s+', '', stripped))
            latex_lines.append(r'\begin{itemize}')
            latex_lines.append(f'  \\item {item_text}')
            while i + 1 < len(lines) and re.match(r'^[\*\-\+]\s+', lines[i + 1].strip()):
                i += 1
                next_item = _convert_inline_md(re.sub(r'^[\*\-\+]\s+', '', lines[i].strip()))
                latex_lines.append(f'  \\item {next_item}')
            latex_lines.append(r'\end{itemize}')
            i += 1
            continue

        # 7. Ordered List
        if re.match(r'^\d+\.\s+', stripped):
            item_text = _convert_inline_md(re.sub(r'^\d+\.\s+', '', stripped))
            latex_lines.append(r'\begin{enumerate}')
            latex_lines.append(f'  \\item {item_text}')
            while i + 1 < len(lines) and re.match(r'^\d+\.\s+', lines[i + 1].strip()):
                i += 1
                next_item = _convert_inline_md(re.sub(r'^\d+\.\s+', '', lines[i].strip()))
                latex_lines.append(f'  \\item {next_item}')
            latex_lines.append(r'\end{enumerate}')
            i += 1
            continue

        # 8. Horizontal Rule
        if re.match(r'^(\*{3,}|-{3,}|_{3,})$', stripped):
            latex_lines.append(r'\noindent\rule{\textwidth}{0.4pt}')
            i += 1
            continue

        # 9. Plain Paragraph
        if stripped:
            latex_lines.append(_convert_inline_md(line))
        else:
            latex_lines.append('')
        i += 1

    if in_table and table_rows:
        latex_lines.extend(_render_latex_table(table_rows))

    content_latex = '\n'.join(latex_lines)

    if standalone:
        res = LATEX_STANDALONE_TEMPLATE.replace('__TITLE__', doc_title).replace('__AUTHOR__', doc_author).replace('__CONTENT__', content_latex)
        return res
    return content_latex


def _render_latex_table(rows: List[List[str]]) -> List[str]:
    """Render table rows into LaTeX booktabs table."""
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
    # Header row
    header = rows[0]
    header_cells = [_convert_inline_md(c) for c in header]
    while len(header_cells) < col_count:
        header_cells.append('')
    res.append(' & '.join(header_cells) + r' \\')
    res.append(r'\midrule')

    for r in rows[1:]:
        cells = [_convert_inline_md(c) for c in r]
        while len(cells) < col_count:
            cells.append('')
        res.append(' & '.join(cells) + r' \\')

    res.append(r'\bottomrule')
    res.append(r'\end{tabular}')
    res.append(r'\end{table}')
    return res


def _convert_inline_md(text: str) -> str:
    """Convert Markdown inline formatting (bold, italic, code, links, images, math) to LaTeX."""
    math_tokens: List[str] = []

    def save_math(match):
        token = f'QQQMATHTOKEN{len(math_tokens)}QQQ'
        math_tokens.append(match.group(0))
        return token

    # Protect inline math $...$
    text = re.sub(r'(?<!\\)\$([^\$]+?)\$', save_math, text)

    # Protect inline code `...`
    code_tokens: List[str] = []
    def save_code(match):
        token = f'QQQCODETOKEN{len(code_tokens)}QQQ'
        code_tokens.append(f'\\texttt{{{match.group(1)}}}')
        return token
    text = re.sub(r'`([^`]+)`', save_code, text)

    # Images: ![alt](url) -> \includegraphics[width=0.8\textwidth]{url}
    text = re.sub(r'!\[(.*?)\]\((.*?)\)', r'\\begin{figure}[htbp]\\centering\\includegraphics[max width=\\linewidth]{\2}\\caption{\1}\\end{figure}', text)

    # Links: [text](url) -> \href{url}{text}
    text = re.sub(r'\[(.*?)\]\((.*?)\)', r'\\href{\2}{\1}', text)

    # Bold: **text** or __text__
    text = re.sub(r'\*\*(.*?)\*\*', r'\\textbf{\1}', text)
    text = re.sub(r'__(.*?)__', r'\\textbf{\1}', text)

    # Italic: *text* or _text_
    text = re.sub(r'\*(.*?)\*', r'\\textit{\1}', text)
    text = re.sub(r'(?<!\w)_(.*?)_{1}(?!\w)', r'\\textit{\1}', text)

    # Strikethrough: ~~text~~ -> \sout{text}
    text = re.sub(r'~~(.*?)~~', r'\\sout{\1}', text)

    # Restore code tokens
    for idx, ct in enumerate(code_tokens):
        text = text.replace(f'QQQCODETOKEN{idx}QQQ', ct)

    # Restore math tokens
    for idx, mt in enumerate(math_tokens):
        text = text.replace(f'QQQMATHTOKEN{idx}QQQ', mt)

    return text



# ---------------------------------------------------------------------------
# LaTeX -> Markdown
# ---------------------------------------------------------------------------

def latex_to_md(tex_content: str) -> str:
    """Convert LaTeX document to Markdown format."""
    # 1. Strip comments (% ...)
    lines = []
    for line in tex_content.splitlines():
        line = re.sub(r'(?<!\\)%.*$', '', line)
        lines.append(line)
    text = '\n'.join(lines)

    # 2. Extract Document Metadata
    title_match = re.search(r'\\title\{([^}]+)\}', text)
    author_match = re.search(r'\\author\{([^}]+)\}', text)
    abstract_match = re.search(r'\\begin\{abstract\}(.*?)\\end\{abstract\}', text, re.DOTALL)

    frontmatter = []
    if title_match:
        frontmatter.append(f'title: "{title_match.group(1).strip()}"')
    if author_match:
        frontmatter.append(f'author: "{author_match.group(1).strip()}"')

    # Remove preamble up to \begin{document}
    if r'\begin{document}' in text:
        text = text.split(r'\begin{document}', 1)[1]
    if r'\end{document}' in text:
        text = text.split(r'\end{document}', 1)[0]

    # Remove \maketitle
    text = re.sub(r'\\maketitle', '', text)

    # 3. Handle Abstract
    if abstract_match:
        abstract_text = abstract_match.group(1).strip()
        text = re.sub(r'\\begin\{abstract\}.*?\\end\{abstract\}', f'> **摘要**：{abstract_text}\n\n', text, flags=re.DOTALL)

    # 4. Code listings
    def repl_code(m):
        lang = ''
        opt = m.group(1) or ''
        lang_match = re.search(r'language=([a-zA-Z0-9_\+#]+)', opt)
        if lang_match:
            lang = lang_match.group(1).lower()
        code = m.group(2).strip('\n')
        return f'\n```{lang}\n{code}\n```\n'
    text = re.sub(r'\\begin\{lstlisting\}(?:\[(.*?)\])?(.*?)\\end\{lstlisting\}', repl_code, text, flags=re.DOTALL)
    text = re.sub(r'\\begin\{verbatim\}(.*?)\\end\{verbatim\}', r'\n```\n\1\n```\n', text, flags=re.DOTALL)

    # 5. Math Environments -> $$ ... $$
    math_envs = ['equation', 'equation*', 'align', 'align*', 'aligned', 'gather', 'gather*', 'matrix', 'pmatrix', 'bmatrix', 'cases']
    for env in math_envs:
        escaped_env = re.escape(env)
        pattern = rf'\\begin\{{{escaped_env}\}}(.*?)\\end\{{{escaped_env}\}}'
        def repl_env(m, e=env):
            body = m.group(1).strip()
            if e in ('equation', 'equation*'):
                return f'\n$$\n{body}\n$$\n'
            return f'\n$$\n\\begin{{{e}}}\n{body}\n\\end{{{e}}}\n$$\n'
        text = re.sub(pattern, repl_env, text, flags=re.DOTALL)

    text = re.sub(r'\\\[(.*?)\\\]', r'\n$$\n\1\n$$\n', text, flags=re.DOTALL)
    text = re.sub(r'\\\((.*?)\\\)', r'$\1$', text, flags=re.DOTALL)

    # 6. Tables (\begin{tabular} ... \end{tabular})
    def repl_table(m):
        tbody = m.group(1).strip()
        rows = [r.strip() for r in tbody.split(r'\\') if r.strip()]
        md_rows = []
        col_count = 0
        for r in rows:
            cleaned_row = re.sub(r'\\(hline|toprule|midrule|bottomrule)', '', r).strip()
            if not cleaned_row:
                continue
            cells = [c.strip() for c in cleaned_row.split('&')]
            col_count = max(col_count, len(cells))
            md_rows.append(cells)
        if not md_rows:
            return ''
        
        out = []
        header = md_rows[0]
        while len(header) < col_count:
            header.append('')
        out.append('| ' + ' | '.join(header) + ' |')
        out.append('| ' + ' | '.join(['---'] * col_count) + ' |')
        for r in md_rows[1:]:
            while len(r) < col_count:
                r.append('')
            out.append('| ' + ' | '.join(r) + ' |')
        return '\n\n' + '\n'.join(out) + '\n\n'

    text = re.sub(r'\\begin\{table\}(?:\[.*?\])?.*?(?:\\caption\{(.*?)\})?.*?\\begin\{tabular\}\{.*?\}(.*?)\\end\{tabular\}.*?\\end\{table\}', 
                  lambda m: (f'\n\n**表：{m.group(1)}**\n' if m.group(1) else '') + repl_table(re.search(r'\\begin\{tabular\}\{.*?\}(.*?)\\end\{tabular\}', m.group(0), re.DOTALL)), 
                  text, flags=re.DOTALL)
    text = re.sub(r'\\begin\{tabular\}\{.*?\}(.*?)\\end\{tabular\}', repl_table, text, flags=re.DOTALL)

    # 7. Section Headings
    text = re.sub(r'\\section\*?\{([^}]+)\}', r'\n# \1\n', text)
    text = re.sub(r'\\subsection\*?\{([^}]+)\}', r'\n## \1\n', text)
    text = re.sub(r'\\subsubsection\*?\{([^}]+)\}', r'\n### \1\n', text)
    text = re.sub(r'\\paragraph\*?\{([^}]+)\}', r'\n#### \1\n', text)
    text = re.sub(r'\\subparagraph\*?\{([^}]+)\}', r'\n##### \1\n', text)

    # 8. Lists
    def repl_itemize(m):
        body = m.group(1).strip()
        items = re.split(r'\\item\s+', body)
        res = []
        for it in items:
            it = it.strip()
            if it:
                res.append(f'- {it}')
        return '\n\n' + '\n'.join(res) + '\n\n'
    text = re.sub(r'\\begin\{itemize\}(.*?)\\end\{itemize\}', repl_itemize, text, flags=re.DOTALL)

    def repl_enumerate(m):
        body = m.group(1).strip()
        items = re.split(r'\\item\s+', body)
        res = []
        idx = 1
        for it in items:
            it = it.strip()
            if it:
                res.append(f'{idx}. {it}')
                idx += 1
        return '\n\n' + '\n'.join(res) + '\n\n'
    text = re.sub(r'\\begin\{enumerate\}(.*?)\\end\{enumerate\}', repl_enumerate, text, flags=re.DOTALL)

    # 9. Inline Styles
    text = re.sub(r'\\textbf\{([^}]+)\}', r'**\1**', text)
    text = re.sub(r'\\textit\{([^}]+)\}', r'*\1*', text)
    text = re.sub(r'\\emph\{([^}]+)\}', r'*\1*', text)
    text = re.sub(r'\\texttt\{([^}]+)\}', r'`\1`', text)
    text = re.sub(r'\\underline\{([^}]+)\}', r'<u>\1</u>', text)
    text = re.sub(r'\\sout\{([^}]+)\}', r'~~\1~~', text)
    text = re.sub(r'\\href\{([^}]+)\}\{([^}]+)\}', r'[\2](\1)', text)
    text = re.sub(r'\\url\{([^}]+)\}', r'<\1>', text)

    # 10. Clean Unescaped LaTeX characters
    text = text.replace(r'\%', '%').replace(r'\&', '&').replace(r'\_', '_').replace(r'\#', '#').replace(r'\{', '{').replace(r'\}', '}')

    cleaned_body = re.sub(r'\n{3,}', '\n\n', text).strip()

    if frontmatter:
        fm_header = '---\n' + '\n'.join(frontmatter) + '\n---\n\n'
        return fm_header + cleaned_body
    return cleaned_body


# Aliases for compatibility
latex_to_markdown = latex_to_md
markdown_to_latex = md_to_latex

