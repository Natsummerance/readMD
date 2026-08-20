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

  // 0. 优先保护 ```math ... ``` 代码块
  src = src.replace(/```math\b[^\n]*\n([\s\S]+?)```/g, (m, b) => save('$$' + repairLatex(b) + '$$'));

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

let mathObserver = null;

function renderMath(body) {
  if (!body) return;
  const html = body.innerHTML;
  if (!/\$\$|\\\(|\\\[|\$[^$\n]+\$|\\begin\{/.test(html)) return;

  function doTypeset() {
    if (!window.MathJax || !MathJax.typesetPromise) return;

    if (mathObserver) {
      mathObserver.disconnect();
      mathObserver = null;
    }

    // 检测 DOM 中包含公式的段落/块级元素数量
    const hasMathText = el => {
      const txt = el.textContent || '';
      return txt.includes('$') || txt.includes('\\(') || txt.includes('\\[') || txt.includes('\\begin');
    };

    const mathBlocks = [];
    const directChildren = body.querySelectorAll('p, div, li, td, th, blockquote, .academic-callout');
    directChildren.forEach(el => {
      if (hasMathText(el) && !el.closest('pre, code, script, style')) {
        mathBlocks.push(el);
      }
    });

    // 如果公式块数量适中（<=60），直接全量极速排版
    if (mathBlocks.length <= 60 || !window.IntersectionObserver) {
      MathJax.typesetPromise([body]).catch(err => {
        console.debug('MathJax typeset catch:', err);
      });
      return;
    }

    // 超长文档（>60 个公式块）：视口按需懒渲染，彻底杜绝主线程卡死
    const scrollContainer = document.getElementById('content') || null;
    mathObserver = new IntersectionObserver((entries, obs) => {
      const visibleBatch = [];
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          visibleBatch.push(entry.target);
          obs.unobserve(entry.target);
        }
      });
      if (visibleBatch.length > 0) {
        MathJax.typesetPromise(visibleBatch).catch(() => {});
      }
    }, {
      root: scrollContainer,
      rootMargin: '350px 0px 350px 0px',
      threshold: 0.01,
    });

    // 先立即渲染视口前 15 个块，确保即时开屏无延迟
    const immediateCount = Math.min(15, mathBlocks.length);
    const initialBatch = mathBlocks.slice(0, immediateCount);
    MathJax.typesetPromise(initialBatch).catch(() => {});

    // 其余块挂载到 Observer
    for (let i = immediateCount; i < mathBlocks.length; i++) {
      mathObserver.observe(mathBlocks[i]);
    }
  }

  if (window.MathJax) {
    try { doTypeset(); } catch (e) { /* ignore */ }
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
    try { doTypeset(); } catch (e) { /* ignore */ }
  };
  document.head.appendChild(s);
}
