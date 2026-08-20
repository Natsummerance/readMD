# -*- coding: utf-8 -*-
"""ReadMD 高精度 LaTeX ⇄ Markdown 双向互转引擎自动化测试套件。"""

import os
import sys
import tempfile
import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)

from src.readmd_modules.texmd import latex_to_md, md_to_latex, MacroExpander, extract_balanced
from src.readmd_modules.convert import convert_verbose
import src.readmd_modules.mdexport as MDExport


def test_balanced_brace_scanner():
    """测试平衡大括号与可选参数扫描器。"""
    text = r"\textbf{Nested {Inner {Deep}} Content} Extra"
    val, end_idx = extract_balanced(text, 7, '{', '}')
    assert val == "Nested {Inner {Deep}} Content"
    assert text[end_idx:] == " Extra"

    # 测试转义大括号
    text_esc = r"\{Escaped \{Braces\} Content\} Rest"
    val2, _ = extract_balanced(text_esc, 0, '{', '}')
    # 第一个字符是 \，不以 { 开头
    assert val2 is None


def test_macro_pre_expansion():
    """测试自定义宏预展开（无参、单参与多参）。"""
    tex_src = r"""
\documentclass{article}
\newcommand{\R}{\mathbb{R}}
\newcommand{\norm}[1]{\left\|#1\right\|}
\newcommand{\ip}[2]{\langle #1, #2 \rangle}
\def\eps{\varepsilon}
\DeclareMathOperator{\Tr}{Tr}

\begin{document}
Let $x \in \R^n$ with $\norm{x} \le 1$ and $\ip{u}{v} = 0$, where $\eps > 0$ and $\Tr(A) = 1$.
\end{document}
"""
    md = latex_to_md(tex_src)
    assert r'\mathbb{R}^n' in md
    assert r'\left\|x\right\|' in md
    assert r'\langle u, v \rangle' in md
    assert r'\varepsilon' in md
    assert r'\operatorname{Tr}(A)' in md


def test_nested_inline_formatting():
    """测试嵌套行内样式（粗体中套斜体、链接、代码等）不会发生单层正则截断。"""
    tex = r"Here is \textbf{bold text containing \textit{italic text} and a \href{https://example.com}{link}}."
    md = latex_to_md(tex)
    assert "**bold text containing *italic text* and a [link](https://example.com)**" in md


def test_math_environments_normalization():
    """测试各种多行对齐与复杂数学环境转换为标准 Markdown $$ 公式块。"""
    tex = r"""
\begin{align*}
\nabla \cdot \vec{E} &= \frac{\rho}{\epsilon_0} \label{eq:gauss} \\
\nabla \times \vec{B} &= \mu_0 \vec{J} + \mu_0 \epsilon_0 \frac{\partial \vec{E}}{\partial t}
\end{align*}

\begin{equation}
E = m c^2 \label{eq:einstein}
\end{equation}

\begin{gather}
a + b = c \\
d + e = f
\end{gather}
"""
    md = latex_to_md(tex)
    # 验证公式保留并去除了可能污染渲染器的 \label
    assert r'\begin{align*}' in md
    assert r'\nabla \cdot \vec{E}' in md
    assert r'\label{eq:gauss}' not in md
    assert '$$' in md
    assert 'E = m c^2' in md
    assert r'\begin{gather}' in md


def test_theorem_and_proof_callouts():
    """测试学术定理、引理与证明环境转换为优雅的 Callout 引用块。"""
    tex = r"""
\begin{theorem}[Cauchy-Schwarz Inequality]
For all vectors $u, v$ in an inner product space,
\[
|\langle u, v \rangle|^2 \le \langle u, u \rangle \cdot \langle v, v \rangle
\]
\end{theorem}

\begin{proof}
Consider the quadratic function $p(t) = \langle u + t v, u + t v \rangle \ge 0$.
\end{proof}
"""
    md = latex_to_md(tex)
    assert '> **定理 (Theorem) (Cauchy-Schwarz Inequality)**' in md
    assert '> **证明 (Proof)**' in md
    assert r'|\langle u, v \rangle|^2' in md
    assert 'Consider the quadratic function' in md


def test_complex_academic_table_conversion():
    """测试包含三线表与 multicolumn 的复杂学术表格解析。"""
    tex = r"""
\begin{table}[htbp]
\caption{Model Performance Comparison}
\centering
\begin{tabular}{lrr}
\toprule
\textbf{Model} & \textbf{Accuracy (\%)} & \textbf{Latency (ms)} \\
\midrule
ResNet-50 & 76.5 & 12.4 \\
Vision Transformer & 82.1 & 18.7 \\
\bottomrule
\end{tabular}
\end{table}
"""
    md = latex_to_md(tex)
    assert '**表：Model Performance Comparison**' in md
    assert '| **Model** | **Accuracy (%)** | **Latency (ms)** |' in md
    assert '| --- | --- | --- |' in md
    assert '| ResNet-50 | 76.5 | 12.4 |' in md


def test_figures_and_captions():
    """测试 Figure 与图片路径解析。"""
    tex = r"""
\begin{figure}[htbp]
\centering
\includegraphics[width=0.8\textwidth]{figures/architecture.png}
\caption{Overall Deep Neural Network Architecture}
\label{fig:arch}
\end{figure}
"""
    md = latex_to_md(tex)
    assert '![Overall Deep Neural Network Architecture](figures/architecture.png)' in md


def test_citations_and_references():
    """测试文献引用与 thebibliography 解析。"""
    tex = r"""
As demonstrated in previous work \cite{vaswani2017attention, devlin2018bert}.

\begin{thebibliography}{99}
\bibitem{vaswani2017attention} Vaswani et al., "Attention Is All You Need", NeurIPS 2017.
\bibitem{devlin2018bert} Devlin et al., "BERT: Pre-training of Deep Bidirectional Transformers", NAACL 2019.
\end{thebibliography}
"""
    md = latex_to_md(tex)
    assert '[@vaswani2017attention; @devlin2018bert]' in md
    assert '## 参考文献' in md
    assert '**[@vaswani2017attention]** Vaswani et al.' in md


def test_academic_algorithms_and_subfigures():
    """测试算法伪代码与子图环境解析。"""
    tex = r"""
\begin{algorithm}
\caption{Model Training}
\begin{algorithmic}
\REQUIRE Dataset $D$
\ENSURE Model $\theta$
\FOR{$epoch = 1$ \TO $N$}
\STATE $\theta \leftarrow \theta - \alpha \nabla L$
\ENDFOR
\RETURN $\theta$
\end{algorithmic}
\end{algorithm}

\begin{subfigure}{0.45\textwidth}
\includegraphics{img1.png}
\caption{Sub-figure 1}
\end{subfigure}
"""
    md = latex_to_md(tex)
    assert '**算法：Model Training**' in md
    assert '```pseudocode' in md
    assert '**Input:** Dataset $D$' in md
    assert '![Sub-figure 1](img1.png)' in md


def test_md_to_latex_export_compilable():
    """测试 Markdown 导出为可编译的标准 LaTeX 独立文档。"""
    md = r"""---
title: "Deep Learning Foundations"
author: "Antigravity Research Team"
---

# Introduction

Deep neural networks approximate arbitrary non-linear functions:

$$f(x) = W_2 \sigma(W_1 x + b_1) + b_2$$

| Metric | Baseline | Ours |
| --- | --- | --- |
| F1-Score | 0.84 | 0.92 |

- First advantage
- Second advantage
"""
    tex = md_to_latex(md)
    assert r'\documentclass[11pt,a4paper]{article}' in tex
    assert r'\title{Deep Learning Foundations}' in tex
    assert r'\author{Antigravity Research Team}' in tex
    assert r'\begin{document}' in tex
    assert r'\section{Introduction}' in tex
    assert r'\begin{equation*}' in tex
    assert r'\begin{tabular}{lll}' in tex
    assert r'\toprule' in tex
    assert r'\end{document}' in tex


def test_roundtrip_latex_md_latex():
    """测试 LaTeX ➔ Markdown ➔ LaTeX 双向回环完整性。"""
    original_tex = r"""\documentclass{article}
\title{Quantum Mechanics Note}
\author{Alice}
\begin{document}
\section{Wave Function}
The state vector satisfies the Schrodinger equation:
\begin{equation*}
i\hbar \frac{\partial}{\partial t}\psi = H\psi
\end{equation*}
\end{document}"""

    md = latex_to_md(original_tex)
    assert 'Wave Function' in md
    assert r'i\hbar \frac{\partial}{\partial t}\psi = H\psi' in md

    re_tex = md_to_latex(md)
    assert r'\section{Wave Function}' in re_tex
    assert r'i\hbar \frac{\partial}{\partial t}\psi = H\psi' in re_tex


def test_convert_verbose_integration():
    """测试 convert_verbose 对 .tex 文件直接导入。"""
    with tempfile.NamedTemporaryFile(suffix='.tex', mode='w', encoding='utf-8', delete=False) as f:
        f.write(r"""\documentclass{article}
\title{Sample Experiment}
\begin{document}
\section{Methodology}
Result is $E = mc^2$.
\end{document}""")
        tex_path = f.name

    try:
        text, engine, err = convert_verbose(tex_path)
        assert err is None
        assert engine == 'texmd'
        assert '# Methodology' in text
        assert '$E = mc^2$' in text
    finally:
        if os.path.exists(tex_path):
            os.remove(tex_path)


def test_export_tex_integration():
    """测试 MDExport.export('tex', ...) 与 MDExport.export('latex', ...)。"""
    with tempfile.TemporaryDirectory() as tmpdir:
        out_tex = os.path.join(tmpdir, "exported.tex")
        res = MDExport.export('tex', "# Hello LaTeX\n\n$$\\int_0^1 x dx = \\frac{1}{2}$$", tmpdir, out_tex)
        assert res.get('ok') is True
        assert os.path.exists(out_tex)
        with open(out_tex, 'r', encoding='utf-8') as f:
            content = f.read()
        assert r'\section{Hello LaTeX}' in content
        assert r'\int_0^1 x dx = \frac{1}{2}' in content
