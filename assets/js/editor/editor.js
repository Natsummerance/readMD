'use strict';
/* ============================================================
   ReadMD Editor - CodeMirror 6 & Command Palette
   ============================================================ */

/* ---------------- 编辑模式（CodeMirror 6：自动补全 + 语法引用） ---------------- */

let cmView = null;
let cmReady = false;
let cmLoading = false;
let cmThemeCompartment = null;

function loadCodeMirror() {
  return new Promise((resolve, reject) => {
    if (window.ReadMDCodeMirror) { cmReady = true; resolve(); return; }
    if (cmLoading) {
      const t0 = Date.now();
      const iv = setInterval(() => {
        if (window.ReadMDCodeMirror) { clearInterval(iv); cmReady = true; cmLoading = false; resolve(); }
        else if (Date.now() - t0 > 15000) { clearInterval(iv); cmLoading = false; reject(new Error('编辑器组件加载超时')); }
      }, 100);
      return;
    }
    cmLoading = true;
    const s = document.createElement('script');
    s.src = '/assets/vendor/codemirror.bundle.js';
    s.onload = () => { cmReady = true; cmLoading = false; resolve(); };
    s.onerror = () => { cmLoading = false; reject(new Error('编辑器组件加载失败，已退回基础编辑')); };
    document.head.appendChild(s);
  });
}

function createEditor(doc) {
  destroyEditor();
  if (!window.ReadMDCodeMirror) return false;
  const CM = window.ReadMDCodeMirror;
  const dark = document.body.dataset.theme === 'dark';
  cmThemeCompartment = new CM.Compartment();
  const st = CM.EditorState.create({
    doc: doc,
    extensions: [
      CM.lineNumbers(),
      CM.highlightActiveLineGutter(),
      CM.highlightActiveLine(),
      CM.drawSelection(),
      CM.dropCursor(),
      CM.bracketMatching(),
      CM.indentOnInput(),
      CM.foldGutter(),
      CM.syntaxHighlighting(CM.defaultHighlightStyle, { fallback: true }),
      CM.history(),
      CM.markdown({ base: CM.markdownLanguage, codeLanguages: CM.languages }),
      CM.autocompletion({ override: [cmMarkdownCompletions()], activateOnTyping: true }),
      CM.closeBrackets(),
      CM.keymap.of([CM.indentWithTab, ...CM.closeBracketsKeymap, ...CM.defaultKeymap, ...CM.historyKeymap, ...CM.completionKeymap]),
      CM.EditorView.lineWrapping,
      CM.EditorView.updateListener.of(u => {
        if (u.docChanged) {
          schedulePreview();
          updateDocStatistics();
        }
        if (u.selectionSet || u.docChanged) updateCmSelectionToolbar();
      }),
    ],
  });
  cmView = new CM.EditorView({ state: st, parent: $('edit-cm') });
  cmView.dom.addEventListener('keydown', e => {
    if (e.key !== '/' || e.ctrlKey || e.metaKey || e.altKey) return;
    const pos = cmView.state.selection.main.head;
    const line = cmView.state.doc.lineAt(pos);
    if (!cmView.state.sliceDoc(line.from, pos).trim()) { e.preventDefault(); openMdCommandPalette(); }
  });
  cmView.dom.addEventListener('mouseup', () => setTimeout(updateCmSelectionToolbar, 10));
  cmView.dom.addEventListener('keyup', () => setTimeout(updateCmSelectionToolbar, 10));
  cmView.dom.addEventListener('paste', handleSmartExcelPaste);
  updateDocStatistics();
  cmView.focus();
  return true;

}

function cmUndo() {
  if (cmView && window.ReadMDCodeMirror) {
    window.ReadMDCodeMirror.undo(cmView);
  } else if ($('edit-area')) {
    document.execCommand('undo');
  }
}

function cmRedo() {
  if (cmView && window.ReadMDCodeMirror) {
    window.ReadMDCodeMirror.redo(cmView);
  } else if ($('edit-area')) {
    document.execCommand('redo');
  }
}

function hideCmSelectionToolbar() {
  const toolbar = $('cm-selection-toolbar');
  if (toolbar) toolbar.classList.add('hidden');
}

function updateCmSelectionToolbar() {
  const toolbar = $('cm-selection-toolbar');
  if (!toolbar) return;
  if (!state.editing || !cmView) {
    toolbar.classList.add('hidden');
    return;
  }
  const sel = cmView.state.selection.main;
  if (!sel || sel.empty) {
    toolbar.classList.add('hidden');
    return;
  }
  const text = cmView.state.sliceDoc(sel.from, sel.to).trim();
  if (!text) {
    toolbar.classList.add('hidden');
    return;
  }
  const coords = cmView.coordsAtPos(sel.to) || cmView.coordsAtPos(sel.from);
  if (!coords) {
    toolbar.classList.add('hidden');
    return;
  }
  toolbar.classList.remove('hidden');
  const tbWidth = toolbar.offsetWidth || 190;
  const tbHeight = toolbar.offsetHeight || 34;
  let left = Math.max(10, Math.min(window.innerWidth - tbWidth - 10, coords.left - (tbWidth / 2)));
  let top = coords.top - tbHeight - 8;
  if (top < 50) {
    top = coords.bottom + 8;
  }
  toolbar.style.left = left + 'px';
  toolbar.style.top = top + 'px';
}

async function cmCopySelection() {
  if (!cmView) return;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = cmView.state.selection.main;
  if (sel.empty) return;
  const text = cmView.state.sliceDoc(sel.from, sel.to);
  try {
    await navigator.clipboard.writeText(text);
    showToast(_t('toast.copiedSelection') || '已复制所选文本', 1500);
  } catch (e) {
    document.execCommand('copy');
  }
  hideCmSelectionToolbar();
}

async function cmCutSelection() {
  if (!cmView) return;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = cmView.state.selection.main;
  if (sel.empty) return;
  const text = cmView.state.sliceDoc(sel.from, sel.to);
  try {
    await navigator.clipboard.writeText(text);
  } catch (e) {
    document.execCommand('copy');
  }
  cmView.dispatch({
    changes: { from: sel.from, to: sel.to, insert: '' },
    selection: { anchor: sel.from }
  });
  showToast(_t('toast.cutSelection') || '已剪切所选文本', 1500);
  hideCmSelectionToolbar();
}

async function cmPasteSelection() {
  if (!cmView) return;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  let text = '';
  try {
    if (hasPy && py.read_clipboard) {
      const clip = await py.read_clipboard(true);
      if (clip && clip.text) text = clip.text;
    }
    if (!text && navigator.clipboard && navigator.clipboard.readText) {
      text = await navigator.clipboard.readText();
    }
  } catch (e) {}
  if (!text) {
    showToast(_t('toast.noPasteText') || '剪贴板中没有可粘贴的文本');
    return;
  }
  const sel = cmView.state.selection.main;
  cmView.dispatch({
    changes: { from: sel.from, to: sel.to, insert: text },
    selection: { anchor: sel.from + text.length }
  });
  hideCmSelectionToolbar();
}


document.addEventListener('pointerdown', e => {
  const toolbar = $('cm-selection-toolbar');
  if (toolbar && !toolbar.classList.contains('hidden') && !toolbar.contains(e.target) && !e.target.closest('#edit-cm')) {
    hideCmSelectionToolbar();
  }
});

function destroyEditor() {

  hideCmSelectionToolbar();
  if (cmView) {
    try { cmView.destroy(); } catch (e) { /* ignore */ }
    cmView = null;
  }
  const c = $('edit-cm');
  if (c) c.innerHTML = '';
  cmThemeCompartment = null;
}


function applyCmTheme() {
  if (!cmView || !window.ReadMDCodeMirror || !cmThemeCompartment) return;
  const CM = window.ReadMDCodeMirror;
  const dark = document.body.dataset.theme === 'dark';
  cmView.dispatch({ effects: cmThemeCompartment.reconfigure(dark ? CM.oneDark : []) });
}

/* Markdown 自动补全（基于 GitHub 开源 @codemirror/autocomplete） */
function cmMarkdownCompletions() {
  const CM = window.ReadMDCodeMirror;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const item = (label, snippetText, detail, type) => ({
    label, detail, type, apply: CM.snippet(snippetText),
  });
  const headingWord = _t('editor.headingWord') || 'Heading';
  const textWord = _t('editor.textWord') || 'text';
  const codeWord = _t('editor.codeWord') || 'code';
  const descWord = _t('editor.descWord') || 'desc';
  const taskWord = _t('editor.taskWord') || 'task';

  const ALL = [
    item('# ' + headingWord, '# ${' + headingWord + '}', _t('editor.h1') || '一级标题', 'markdown'),
    item('## ' + headingWord, '## ${' + headingWord + '}', _t('editor.h2') || '二级标题', 'markdown'),
    item('### ' + headingWord, '### ${' + headingWord + '}', _t('editor.h3') || '三级标题', 'markdown'),
    item('#### ' + headingWord, '#### ${' + headingWord + '}', _t('editor.h4') || '四级标题', 'markdown'),
    item('**' + (_t('editor.bold') || '加粗') + '**', '**${' + textWord + '}**', _t('editor.bold') || '加粗', 'markdown'),
    item('*' + (_t('editor.italic') || '斜体') + '*', '*${' + textWord + '}*', _t('editor.italic') || '斜体', 'markdown'),
    item('~~' + (_t('editor.strikethrough') || '删除线') + '~~', '~~${' + textWord + '}~~', _t('editor.strikethrough') || '删除线', 'markdown'),
    item('`' + (_t('editor.codeInline') || '行内代码') + '`', '`${' + codeWord + '}`', _t('editor.codeInline') || '行内代码', 'markdown'),
    item('```' + (_t('editor.codeBlock') || '代码块'), '```\n${' + codeWord + '}\n```', _t('editor.codeBlock') || '代码块', 'markdown'),
    item('[' + textWord + '](url)', '[${' + textWord + '}](url)', _t('editor.link') || '链接', 'markdown'),
    item('![' + descWord + '](url)', '![${' + descWord + '}](url)', _t('editor.image') || '图片', 'markdown'),
    item('> ' + (_t('editor.quote') || '引用'), '> ${' + textWord + '}', _t('editor.quote') || '引用块', 'markdown'),
    item('$x^2$', '$x^2$', _t('editor.mathInline') || '行内公式', 'markdown'),
    item('$$...$$', '$$\n${' + textWord + '}\n$$', _t('editor.mathBlock') || '块级公式', 'markdown'),
    item('| ' + (_t('editor.table') || '表格') + ' |', '| Col 1 | Col 2 |\n|---|---|\n| ${' + textWord + '} |  |', _t('editor.table') || '表格', 'markdown'),
    item('- ' + (_t('editor.listUnordered') || '列表项'), '- ${' + textWord + '}', _t('editor.listUnordered') || '无序列表', 'markdown'),
    item('- [ ] ' + (_t('editor.listTask') || '任务'), '- [ ] ${' + taskWord + '}', _t('editor.listTask') || '任务列表', 'markdown'),
    item('--- ' + (_t('editor.hr') || '分隔线'), '---', _t('editor.hr') || '分隔线', 'markdown'),
  ];
  return context => {
    const before = context.matchBefore(/[\w#*_`\[!>|\$~:]{0,8}/);
    if (!before) return null;
    const w = before.text.toLowerCase();
    const matched = ALL.filter(c => c.label.toLowerCase().startsWith(w) || c.label.toLowerCase().includes(w));
    if (!matched.length) return null;
    return { from: before.from, options: matched.slice(0, 12) };
  };
}


/* 语法引用 / 插入工具栏 */
function cmInsertSyntax(kind) {
  if (!cmView) return;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = cmView.state.selection.main;
  const selected = cmView.state.sliceDoc(sel.from, sel.to);
  let insert = null;
  let cursor = sel.from;
  const textPlaceholder = _t('editor.textWord') || 'text';
  const codePlaceholder = _t('editor.codeWord') || 'code';
  const headingPlaceholder = _t('editor.headingWord') || 'Heading';
  const quotePlaceholder = _t('editor.quote') || 'Quote';
  const itemPlaceholder = _t('editor.itemWord') || 'item';
  const taskPlaceholder = _t('editor.taskWord') || 'task';
  const descPlaceholder = _t('editor.descWord') || 'desc';

  const wrap = (b, d, a) => {
    insert = b + (selected || d) + a;
    cursor = sel.from + b.length + (selected || d).length;
  };
  switch (kind) {
    case 'bold': wrap('**', textPlaceholder, '**'); break;
    case 'italic': wrap('*', textPlaceholder, '*'); break;
    case 'strike': wrap('~~', textPlaceholder, '~~'); break;
    case 'code': wrap('`', codePlaceholder, '`'); break;
    case 'math': wrap('$', 'x^2', '$'); break;
    case 'mathblock': insert = '$$\n' + (selected || 'x^2') + '\n$$'; cursor = sel.from + insert.length - 3; break;
    case 'h2': insert = '## ' + (selected || headingPlaceholder); cursor = sel.from + insert.length; break;
    case 'quote': insert = '> ' + (selected || quotePlaceholder); cursor = sel.from + insert.length; break;
    case 'list': insert = '- ' + (selected || itemPlaceholder); cursor = sel.from + insert.length; break;
    case 'ordered': insert = '1. ' + (selected || itemPlaceholder); cursor = sel.from + insert.length; break;
    case 'task': insert = '- [ ] ' + (selected || taskPlaceholder); cursor = sel.from + insert.length; break;
    case 'link': insert = '[' + (selected || textPlaceholder) + '](url)'; cursor = sel.from + 1 + (selected || textPlaceholder).length; break;
    case 'image': insert = '![' + (selected || descPlaceholder) + '](url)'; cursor = sel.from + 2 + (selected || descPlaceholder).length; break;
    case 'codeblock': insert = '```\n' + (selected || codePlaceholder) + '\n```'; cursor = sel.from + 4 + (selected || codePlaceholder).length; break;
    case 'table': insert = '| Col 1 | Col 2 |\n|---|---|\n| ' + (selected || textPlaceholder) + ' |  |'; cursor = sel.from + insert.length; break;
    case 'hr': insert = '\n---\n'; cursor = sel.from + insert.length; break;
    default: return;
  }
  if (insert === null) return;
  cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert }, selection: { anchor: cursor } });
  cmView.focus();
}


function getMdCommands() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  return [
    [_t('editor.bold') || '加粗', 'bold', '**' + (_t('editor.text') || '文本') + '**'],
    [_t('editor.italic') || '斜体', 'italic', '*' + (_t('editor.text') || '文本') + '*'],
    [_t('editor.strikethrough') || '删除线', 'strike', '~~' + (_t('editor.text') || '文本') + '~~'],
    [_t('editor.h2') || '二级标题', 'h2', '## ' + (_t('editor.h2') || '标题')],
    [_t('editor.quote') || '引用', 'quote', '> ' + (_t('editor.quote') || '引用')],
    [_t('editor.ul') || '无序列表', 'list', '- ' + (_t('editor.text') || '项目')],
    [_t('editor.ol') || '有序列表', 'ordered', '1. ' + (_t('editor.text') || '项目')],
    [_t('editor.taskList') || '任务列表', 'task', '- [ ] ' + (_t('editor.taskList') || '任务')],
    [_t('editor.link') || '链接', 'link', '[' + (_t('editor.text') || '文本') + '](url)'],
    [_t('editor.image') || '图片', 'image', _t('img.title') || '本地图片或 URL'],
    [_t('editor.codeInline') || '行内代码', 'code', '`' + (_t('editor.codeInline') || '代码') + '`'],
    [_t('editor.codeBlock') || '代码块', 'codeblock', '```'],
    [_t('editor.table') || '表格', 'table', '| ' + (_t('editor.table') || '列1') + ' | ' + (_t('editor.table') || '列2') + ' |'],
    [_t('editor.hr') || '分隔线', 'hr', '---'],
    [_t('formula.inline') || '行内公式', 'math', '$x^2$'],
    [_t('formula.block') || '块级公式', 'mathblock', '$$…$$'],
  ];
}
let mdCommandIndex = 0;

function closeMdPopups() {
  document.querySelectorAll('.md-menu, .pv-menu').forEach(el => el.classList.add('hidden'));
  const trigger = $('pv-trigger'); if (trigger) trigger.setAttribute('aria-expanded', 'false');
}

function openMdCommandPalette() {
  if (!state.editing) return;
  closeMdPopups();
  $('md-command-modal').classList.remove('hidden');
  $('md-command-search').value = '';
  mdCommandIndex = 0; renderMdCommands();
  setTimeout(() => $('md-command-search').focus(), 0);
}

function closeMdCommandPalette() { $('md-command-modal').classList.add('hidden'); if (cmView) cmView.focus(); }

function renderMdCommands() {
  const q = $('md-command-search').value.trim().toLowerCase();
  const commands = getMdCommands();
  const rows = commands.filter(c => !q || (c[0] + ' ' + c[2]).toLowerCase().includes(q));
  mdCommandIndex = Math.max(0, Math.min(mdCommandIndex, rows.length - 1));
  const list = $('md-command-list'); list.innerHTML = '';
  rows.forEach((c, i) => {
    const b = document.createElement('button'); b.className = 'command-item' + (i === mdCommandIndex ? ' active' : '');
    b.innerHTML = '<span></span><small></small>'; b.querySelector('span').textContent = c[0]; b.querySelector('small').textContent = c[2];
    b.addEventListener('click', () => runMdCommand(c[1])); list.appendChild(b);
  });
}

function runMdCommand(kind) { closeMdCommandPalette(); if (kind === 'image') openImgModal(); else if (kind === 'math' || kind === 'mathblock') openFormulaModal(kind === 'mathblock' ? 'block' : 'inline'); else cmInsertSyntax(kind); }

const FORMULAS = [
  ['常用','平方根','sqrt root','\\sqrt{x}'], ['常用','分式','fraction frac','\\frac{a}{b}'], ['常用','幂与下标','power subscript','x^{n}_{i}'], ['常用','二次公式','quadratic','x=\\frac{-b\\pm\\sqrt{b^2-4ac}}{2a}'],
  ['希腊','阿尔法','alpha','\\alpha'], ['希腊','贝塔','beta','\\beta'], ['希腊','伽马','gamma','\\gamma'], ['希腊','派','pi','\\pi'], ['希腊','西塔','theta','\\theta'], ['希腊','欧米伽','omega','\\omega'],
  ['运算','加减','plus minus','\\pm'], ['运算','乘号','times multiply','\\times'], ['运算','除号','divide','\\div'],
  ['关系','小于等于','less equal','\\le'], ['关系','大于等于','greater equal','\\ge'], ['关系','不等于','not equal','\\ne'], ['关系','约等于','approx','\\approx'],
  ['箭头','右箭头','right arrow','A\\rightarrow B'], ['箭头','双向箭头','leftright arrow','A\\leftrightarrow B'], ['箭头','推出','implies','A\\Rightarrow B'],
  ['函数','正弦','sin','\\sin x'], ['函数','对数','log','\\log_{a}x'], ['函数','指数','exp','e^{x}'],
  ['结构','求和','sum','\\sum_{i=1}^{n} x_i'], ['结构','积分','integral','\\int_{a}^{b} f(x)\\,dx'], ['结构','极限','limit','\\lim_{x\\to 0} f(x)'], ['结构','矩阵','matrix','\\begin{bmatrix}a&b\\\\c&d\\end{bmatrix}'], ['结构','分段函数','cases','f(x)=\\begin{cases}x,&x\\ge0\\\\-x,&x<0\\end{cases}'],
];
const FORMULA_CAT_NAMES = {
  '常用': 'formula.catCommon',
  '希腊': 'formula.catGreek',
  '运算': 'formula.catCalc',
  '关系': 'formula.catRel',
  '箭头': 'formula.catArrows',
  '函数': 'formula.catFuncs',
  '结构': 'formula.catStruct'
};

const FORMULA_ITEM_KEYS = {
  '平方根': 'formula.sqrt', '分式': 'formula.frac', '幂与下标': 'formula.powerSub', '二次公式': 'formula.quadratic',
  '阿尔法': 'formula.alpha', '贝塔': 'formula.beta', '伽马': 'formula.gamma', '派': 'formula.pi', '西塔': 'formula.theta', '欧米伽': 'formula.omega',
  '加减': 'formula.plusMinus', '乘号': 'formula.times', '除号': 'formula.divide',
  '小于等于': 'formula.le', '大于等于': 'formula.ge', '不等于': 'formula.ne', '约等于': 'formula.approx',
  '右箭头': 'formula.rightArrow', '双向箭头': 'formula.bothArrow', '推出': 'formula.implies',
  '正弦': 'formula.sin', '对数': 'formula.log', '指数': 'formula.exp',
  '求和': 'formula.sum', '积分': 'formula.integral', '极限': 'formula.limit', '矩阵': 'formula.matrix', '分段函数': 'formula.cases'
};
let formulaCategory = '常用';

function openFormulaModal(mode) { if (!state.editing) return; closeMdPopups(); $('formula-mode').value = mode || 'inline'; $('formula-modal').classList.remove('hidden'); $('formula-search').value = ''; renderFormulaPicker(); setTimeout(() => $('formula-search').focus(), 0); }
function closeFormulaModal() { $('formula-modal').classList.add('hidden'); if (cmView) cmView.focus(); }
function renderFormulaPicker() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const cats = [...new Set(FORMULAS.map(f => f[0]))]; const catBox = $('formula-cats'); catBox.innerHTML = '';
  cats.forEach(c => {
    const b = document.createElement('button');
    const labelKey = FORMULA_CAT_NAMES[c] || c;
    b.textContent = _t(labelKey) || c;
    b.classList.toggle('active', c === formulaCategory);
    b.addEventListener('click', () => { formulaCategory = c; renderFormulaPicker(); });
    catBox.appendChild(b);
  });
  const q = $('formula-search').value.trim().toLowerCase(); const rows = FORMULAS.filter(f => (q ? (f.join(' ').toLowerCase().includes(q)) : f[0] === formulaCategory));
  const list = $('formula-list'); list.innerHTML = '';
  rows.forEach(f => {
    const b = document.createElement('button');
    b.className = 'formula-item';
    b.innerHTML = '<span></span><small></small>';
    const itemKey = FORMULA_ITEM_KEYS[f[1]];
    b.querySelector('span').textContent = (itemKey ? _t(itemKey) : null) || f[1];
    b.querySelector('small').textContent = f[3];
    b.addEventListener('mouseenter', () => previewFormula(f[3]));
    b.addEventListener('focus', () => previewFormula(f[3]));
    b.addEventListener('click', () => insertFormula(f[3]));
    list.appendChild(b);
  });
}

function previewFormula(tex) { const p = $('formula-preview'); p.textContent = '$$' + tex + '$$'; renderMath(p); }
function insertFormula(tex) { const mode = $('formula-mode').value; closeFormulaModal(); if (!cmView) return; const sel = cmView.state.selection.main; const selected = cmView.state.sliceDoc(sel.from, sel.to); const body = selected || tex; const insert = mode === 'block' ? '\n$$\n' + body + '\n$$\n' : '$' + body + '$'; cmView.dispatch({changes:{from:sel.from,to:sel.to,insert},selection:{anchor:sel.from+insert.length}}); cmView.focus(); }

/* ============================================================
   Editor Studio PRO: Zen Mode (禅模式) & 表格设计器 & 统计
   ============================================================ */

let isZenMode = false;

function toggleZenMode(enable) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (typeof enable === 'boolean') {
    isZenMode = enable;
  } else {
    isZenMode = !isZenMode;
  }
  document.body.classList.toggle('zen-mode', isZenMode);
  if (isZenMode) {
    showToast(_t('toast.zenEntered') || '已进入禅模式 (按 F11 或 Esc 退出)', 2000);
    if (cmView) cmView.focus();
  }
}


document.addEventListener('keydown', e => {
  if (e.key === 'F11' && state.editing) {
    e.preventDefault();
    toggleZenMode();
  } else if (e.key === 'Escape' && isZenMode) {
    toggleZenMode(false);
  }
});

/* 实时文档统计与阅读时长 */
function updateDocStatistics() {
  const statsEl = $('edit-doc-stats');
  if (!statsEl) return;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : `${p.words} 字 · 阅读约 ${p.min} 分钟`;
  const docText = typeof getEditContent === 'function' ? getEditContent() : (cmView ? cmView.state.doc.toString() : ($('edit-area') && $('edit-area').value || ''));
  if (!docText) {
    statsEl.textContent = _t('editor.statsFormat', { words: 0, min: 1 });
    return;
  }

  const chars = docText.length;
  // 中文字符 + 西文字数
  const cjk = (docText.match(/[\u4e00-\u9fa5]/g) || []).length;
  const nonCjk = docText.replace(/[\u4e00-\u9fa5]/g, ' ').trim().split(/\s+/).filter(Boolean).length;
  const totalWords = cjk + nonCjk;
  const minutes = Math.max(1, Math.ceil(totalWords / 300));
  statsEl.textContent = _t('editor.statsFormat', { words: totalWords, min: minutes });
}


/* 智能 Excel / CSV 粘贴转 Markdown 表格 */
function handleSmartExcelPaste(e) {
  if (!e.clipboardData) return;
  const text = e.clipboardData.getData('text/plain');
  if (!text || !text.includes('\t') || !text.includes('\n')) return;

  const lines = text.trim().split(/\r?\n/).map(l => l.split('\t'));
  if (lines.length < 2 || lines[0].length < 2) return;

  // 确认为多行多列表格数据
  e.preventDefault();
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const colCount = Math.max(...lines.map(r => r.length));
  const mdRows = [];
  
  // 表头
  const defaultCol = _t('editor.table') || '列';
  const headers = lines[0].map(c => c.trim() || defaultCol);
  while (headers.length < colCount) headers.push(defaultCol + (headers.length + 1));
  mdRows.push('| ' + headers.join(' | ') + ' |');
  mdRows.push('| ' + headers.map(() => '---').join(' | ') + ' |');

  // 表体
  for (let i = 1; i < lines.length; i++) {
    const row = lines[i].map(c => c.trim().replace(/\|/g, '\\|'));
    while (row.length < colCount) row.push('');
    mdRows.push('| ' + row.join(' | ') + ' |');
  }

  const tableMd = '\n' + mdRows.join('\n') + '\n';
  if (cmView) {
    const sel = cmView.state.selection.main;
    cmView.dispatch({
      changes: { from: sel.from, to: sel.to, insert: tableMd },
      selection: { anchor: sel.from + tableMd.length }
    });
    cmView.focus();
  } else if ($('edit-area')) {
    document.execCommand('insertText', false, tableMd);
  }
  showToast(_t('toast.tableConverted', { count: lines.length }) || `已将剪贴板中 ${lines.length} 行表格转为 Markdown 表格`, 2000);
}

/* 交互式表格设计器 */
let selectedRows = 3;
let selectedCols = 3;

function openTableModal() {
  if (!state.editing) return;
  closeMdPopups();
  const modal = $('table-modal');
  if (!modal) return;
  modal.classList.remove('hidden');
  initTableGridPicker();
}

function closeTableModal() {
  const modal = $('table-modal');
  if (modal) modal.classList.add('hidden');
  if (cmView) cmView.focus();
}

function initTableGridPicker() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const picker = $('table-grid-picker');
  const label = $('table-grid-label');
  if (!picker) return;
  picker.innerHTML = '';
  selectedRows = 3;
  selectedCols = 3;

  for (let r = 1; r <= 10; r++) {
    for (let c = 1; c <= 10; c++) {
      const cell = document.createElement('div');
      cell.className = 'table-grid-cell' + (r <= 3 && c <= 3 ? ' highlight' : '');
      cell.dataset.row = r;
      cell.dataset.col = c;
      cell.addEventListener('mouseenter', () => {
        selectedRows = r;
        selectedCols = c;
        if (label) label.textContent = _t('editor.tableDimensions', { rows: r, cols: c }) || `${r} 行 × ${c} 列 表格`;
        picker.querySelectorAll('.table-grid-cell').forEach(el => {
          const er = +el.dataset.row;
          const ec = +el.dataset.col;
          el.classList.toggle('highlight', er <= r && ec <= c);
        });
      });
      cell.addEventListener('click', () => {
        insertCustomTable(selectedRows, selectedCols);
        closeTableModal();
      });
      picker.appendChild(cell);
    }
  }
}

function insertCustomTable(rows, cols) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const headerPrefix = _t('editor.tableHeaderPrefix') || 'Header';
  const cellWord = _t('editor.tableCell') || 'Cell';
  const headers = Array.from({ length: cols }, (_, i) => `${headerPrefix} ${i + 1}`);
  const sep = Array.from({ length: cols }, () => '---');
  const mdLines = [
    '| ' + headers.join(' | ') + ' |',
    '| ' + sep.join(' | ') + ' |'
  ];
  for (let r = 0; r < rows; r++) {
    const row = Array.from({ length: cols }, () => cellWord);
    mdLines.push('| ' + row.join(' | ') + ' |');
  }
  const tableMd = '\n' + mdLines.join('\n') + '\n';

  if (cmView) {
    const sel = cmView.state.selection.main;
    cmView.dispatch({
      changes: { from: sel.from, to: sel.to, insert: tableMd },
      selection: { anchor: sel.from + tableMd.length }
    });
    cmView.focus();
  }
}
