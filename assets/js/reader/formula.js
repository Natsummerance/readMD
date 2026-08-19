'use strict';
/* ============================================================
   ReadMD Reader - Mathematical Formulas & LaTeX Picker
   ============================================================ */

/* ---------------- 数学公式与 LaTeX 兼容自修复 ---------------- */

function repairLatex(latex) {
  if (!latex) return '';
  let t = latex.trim();

  // 1. HTML 实体还原
  t = t.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'");

  // 2. 常见 Unicode 数学符号自动转标准 LaTeX 指令
  const uMap = {
    '×': '\\times ', '÷': '\\div ', '±': '\\pm ', '∓': '\\mp ',
    '≤': '\\le ', '≥': '\\ge ', '≠': '\\ne ', '≈': '\\approx ',
    '≡': '\\equiv ', '∞': '\\infty ', '∑': '\\sum ', '∏': '\\prod ',
    '∫': '\\int ', '√': '\\sqrt', '∈': '\\in ', '∉': '\\notin ',
    '⊂': '\\subset ', '⊆': '\\subseteq ', '∪': '\\cup ', '∩': '\\cap ',
    '∀': '\\forall ', '∃': '\\exists ', '∇': '\\nabla ', '∂': '\\partial ',
    'α': '\\alpha ', 'β': '\\beta ', 'γ': '\\gamma ', 'δ': '\\delta ',
    'ε': '\\varepsilon ', 'θ': '\\theta ', 'λ': '\\lambda ', 'μ': '\\mu ',
    'π': '\\pi ', 'σ': '\\sigma ', 'τ': '\\tau ', 'φ': '\\varphi ',
    'ω': '\\omega ', 'Δ': '\\Delta ', 'Ω': '\\Omega '
  };
  for (const [u, r] of Object.entries(uMap)) {
    if (t.includes(u)) t = t.replaceAll(u, r);
  }

  // 3. 修复在 Markdown 解析中被破坏的 \\ 换行转义（例如在 cases / align / matrix 中）
  t = t.replace(/(\\\s*\n|\s*\\\\(?!\n))\s*/g, ' \\\\ \n');

  // 4. 自动配平未闭合的花括号 {}
  let openBraces = 0;
  let i = 0;
  while (i < t.length) {
    if (t[i] === '\\') { i += 2; continue; }
    if (t[i] === '{') openBraces++;
    else if (t[i] === '}') { if (openBraces > 0) openBraces--; }
    i++;
  }
  if (openBraces > 0) {
    t += '}'.repeat(openBraces);
  }

  return t;
}

function protectMath(src) {
  const saved = [];
  const save = m => {
    saved.push(m);
    return '\x01M' + (saved.length - 1) + '\x01';
  };
  const looksMath = body => /[\\^_{}]/.test(body) || (/[A-Za-z\u0391-\u03C9]/.test(body) && !/\s/.test(body));

  // 1. 优先保护标准多行块级 $$...$$
  src = src.replace(/\$\$([\s\S]+?)\$\$/g, (m, b) => save('$$' + repairLatex(b) + '$$'));

  // 2. 保护未包裹在 $$ 里的裸 LaTeX 多行环境（\begin{cases}...\end{cases}, align, matrix, equation, gather 等）
  const envPattern = /\\begin\{(cases|align\*?|aligned|matrix|pmatrix|bmatrix|vmatrix|Vmatrix|equation\*?|gather\*?|array|split|smallmatrix)\}([\s\S]*?)\\end\{\1\}/g;
  src = src.replace(envPattern, (m, env, body) => save('$$\\begin{' + env + '}' + repairLatex(body) + '\\end{' + env + '}$$'));

  // 3. 保护 \(...\) 与 \[...\]
  src = src.replace(/\\\(([\s\S]+?)\\\)/g, (m, b) => save('\\(' + repairLatex(b) + '\\)'));
  src = src.replace(/\\\[([\s\S]+?)\\\]/g, (m, b) => save('\\[' + repairLatex(b) + '\\]'));

  // 4. 保护行内 $...$ 公式
  src = src.replace(/(^|[^\\$A-Za-z0-9])\$([^$\n]+?)\$/g, (m, pre, b) => {
    if (looksMath(b)) {
      return pre + save('$' + repairLatex(b) + '$');
    }
    return m;
  });

  return { src, saved };
}

function restoreMath(html, saved) {
  return html.replace(/\x01M(\d+)\x01/g, (m, i) => saved[+i] || m);
}

function renderMath(body) {
  const html = body.innerHTML;
  if (!/\$\$|\\\(|\\\[|\$[^$\n]+\$|\\begin\{/.test(html)) return;
  if (window.MathJax) {
    try {
      MathJax.typesetPromise([body]).catch(err => {
        console.warn('MathJax typeset catch:', err);
      });
    } catch (e) { /* ignore */ }
    return;
  }
  window.MathJax = {
    tex: {
      inlineMath: [['$', '$'], ['\\(', '\\)']],
      displayMath: [['$$', '$$'], ['\\[', '\\]']],
      packages: {'[+]': ['cases', 'ams', 'color', 'html']},
      formatError: (jax, err) => {
        console.warn('TeX format error:', err);
        return jax.formatError(err);
      }
    },
    options: {
      skipHtmlTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code']
    },
    startup: { typeset: false },
  };
  const s = document.createElement('script');
  s.src = '/assets/vendor/mathjax/tex-svg.js';
  s.onload = () => {
    try {
      MathJax.typesetPromise([body]).catch(() => {});
    } catch (e) { /* ignore */ }
  };
  document.head.appendChild(s);
}
