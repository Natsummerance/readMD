# -*- coding: utf-8 -*-
"""Tests for LaTeX (.tex) to Markdown conversion in ReadMD."""

import os
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import convert


class TestConvertLatex(unittest.TestCase):
    """Test pure-Python zero-dependency LaTeX parser and converter."""

    def test_enabled_plaintext_plugin_does_not_strip_markdown_or_math(self):
        with tempfile.TemporaryDirectory() as directory:
            path = os.path.join(directory, 'paper.tex')
            with open(path, 'w', encoding='utf-8') as f:
                f.write(r'\section{Main} Text with $x^2$ and \textbf{bold}.')
            with patch('src.readmd_modules.plugin_manager.is_plugin_enabled', return_value=True):
                text, engine, error = convert.convert_verbose(path)
            self.assertIsNone(error)
            self.assertIn('# Main', text)
            self.assertIn('$x^2$', text)
            self.assertIn('**bold**', text)

    def test_basic_metadata_and_sections(self):
        sample_tex = r"""\documentclass{article}
\title{Quantum Computing Foundations}
\author{Dr. Alice \and Bob}
\date{\today}
\begin{document}
\maketitle

\section{Introduction}
This is the introduction to quantum algorithms.

\subsection{Qubits and Superposition}
A qubit can exist in state $|\psi\rangle = \alpha |0\rangle + \beta |1\rangle$.

\subsubsection{Entanglement}
Bell states represent maximally entangled states.
\end{document}
"""
        md = convert._convert_latex_native(sample_tex)
        self.assertIn('title: "Quantum Computing Foundations"', md)
        self.assertIn("# Introduction", md)
        self.assertIn("## Qubits and Superposition", md)
        self.assertIn("### Entanglement", md)
        self.assertIn(r"|\psi\rangle = \alpha |0\rangle + \beta |1\rangle", md)
        self.assertNotIn(r"\documentclass", md)
        self.assertNotIn(r"\maketitle", md)
        self.assertNotIn(r"\begin{document}", md)

    def test_text_formatting_and_lists(self):
        sample_tex = r"""\begin{document}
Here is \textbf{bold text}, \textit{italicized text}, \emph{emphasized text}, and \texttt{inline code}.

\begin{itemize}
  \item First bullet point
  \item Second bullet point with \textbf{bold}
\end{itemize}

\begin{enumerate}
  \item First numbered item
  \item Second numbered item
\end{enumerate}
\end{document}
"""
        md = convert._convert_latex_native(sample_tex)
        self.assertIn("**bold text**", md)
        self.assertIn("*italicized text*", md)
        self.assertIn("*emphasized text*", md)
        self.assertIn("`inline code`", md)
        self.assertIn("- First bullet point", md)
        self.assertIn("- Second bullet point with **bold**", md)
        self.assertIn("1. First numbered item", md)
        self.assertIn("2. Second numbered item", md)

    def test_math_environments(self):
        sample_tex = r"""\begin{document}
Euler's identity is given by $e^{i\pi} + 1 = 0$.

\begin{equation}
\int_{-\infty}^{\infty} e^{-x^2} dx = \sqrt{\pi}
\end{equation}

\begin{align}
f(x) &= 2x + 1 \\
g(x) &= x^2
\end{align}
\end{document}
"""
        md = convert._convert_latex_native(sample_tex)
        # Inline math preserved
        self.assertIn(r"$e^{i\pi} + 1 = 0$", md)
        # Display equations wrapped in $$
        self.assertIn("$$\n\\int_{-\\infty}^{\\infty} e^{-x^2} dx = \\sqrt{\\pi}\n$$", md)
        self.assertIn("\\begin{align}\nf(x) &= 2x + 1 \\\\\ng(x) &= x^2\n\\end{align}", md)
        self.assertNotIn(r"\begin{equation}", md)

    def test_tabular_conversion(self):
        sample_tex = r"""\begin{document}
\begin{tabular}{|l|c|r|}
\hline
Name & Age & Role \\
\hline
Alice & 28 & Engineer \\
Bob & 34 & Scientist \\
\hline
\end{tabular}
\end{document}
"""
        md = convert._convert_latex_native(sample_tex)
        self.assertIn("| Name | Age | Role |", md)
        self.assertIn("| Alice | 28 | Engineer |", md)
        self.assertIn("| Bob | 34 | Scientist |", md)
        self.assertNotIn(r"\begin{tabular}", md)
        self.assertNotIn(r"\hline", md)

    def test_comments_and_escapes(self):
        sample_tex = r"""\begin{document}
Visible text % This is a comment that should be stripped
This has a 50\% discount!
\end{document}
"""
        md = convert._convert_latex_native(sample_tex)
        self.assertIn("Visible text", md)
        self.assertNotIn("This is a comment that should be stripped", md)
        self.assertIn("50% discount", md)

    def test_convert_dispatcher_with_tex_file(self):
        with tempfile.NamedTemporaryFile(suffix=".tex", mode="w", encoding="utf-8", delete=False) as f:
            f.write(r"""\documentclass{article}
\title{Paper Title}
\begin{document}
\section{Main}
Content here.
\end{document}
""")
            tmp_path = f.name

        try:
            res = convert.convert(tmp_path)
            self.assertIn('title: "Paper Title"', res)
            self.assertIn("# Main", res)
            self.assertIn("Content here.", res)
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)


if __name__ == "__main__":
    unittest.main()
