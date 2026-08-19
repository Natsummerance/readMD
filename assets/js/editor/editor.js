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
        if (u.docChanged) schedulePreview();
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
  const sel = cmView.state.selection.main;
  if (sel.empty) return;
  const text = cmView.state.sliceDoc(sel.from, sel.to);
  try {
    await navigator.clipboard.writeText(text);
    showToast('已复制所选文本', 1500);
  } catch (e) {
    document.execCommand('copy');
  }
  hideCmSelectionToolbar();
}

async function cmCutSelection() {
  if (!cmView) return;
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
  showToast('已剪切所选文本', 1500);
  hideCmSelectionToolbar();
}

async function cmPasteSelection() {
  if (!cmView) return;
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
    showToast('剪贴板中没有可粘贴的文本');
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
  const item = (label, snippetText, detail, type) => ({
    label, detail, type, apply: CM.snippet(snippetText),
  });
  const ALL = [
    item("# 标题", "# ${标题}", "一级标题", "markdown"),
    item("## 标题", "## ${标题}", "二级标题", "markdown"),
    item("### 标题", "### ${标题}", "三级标题", "markdown"),
    item("#### 标题", "#### ${标题}", "四级标题", "markdown"),
    item("**加粗**", "**${文本}**", "加粗", "markdown"),
    item("*斜体*", "*${文本}*", "斜体", "markdown"),
    item("~~删除线~~", "~~${文本}~~", "删除线", "markdown"),
    item("`行内代码`", "`${代码}`", "行内代码", "markdown"),
    item("```代码块", "```\n${代码}\n```", "代码块", "markdown"),
    item("[链接文本](url)", "[${文本}](url)", "链接", "markdown"),
    item("![图片描述](url)", "![${描述}](url)", "图片", "markdown"),
    item("> 引用", "> ${引用}", "引用块", "markdown"),
    item("$公式$", "$x^2$", "行内公式", "markdown"),
    item("$$公式$$", "$$\n${公式}\n$$", "块级公式", "markdown"),
    item("| 表格 |", "| 列1 | 列2 |\n|---|---|\n| ${值} |  |", "表格", "markdown"),
    item("- 列表项", "- ${项目}", "无序列表", "markdown"),
    item("- [ ] 任务", "- [ ] ${任务}", "任务列表", "markdown"),
    item("--- 分隔线", "---", "分隔线", "markdown"),
  ];  return context => {
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
  const sel = cmView.state.selection.main;
  const selected = cmView.state.sliceDoc(sel.from, sel.to);
  let insert = null;
  let cursor = sel.from;
  const wrap = (b, d, a) => {
    insert = b + (selected || d) + a;
    cursor = sel.from + b.length + (selected || d).length;
  };
  switch (kind) {
    case 'bold': wrap('**', '文本', '**'); break;
    case 'italic': wrap('*', '文本', '*'); break;
    case 'strike': wrap('~~', '文本', '~~'); break;
    case 'code': wrap('`', '代码', '`'); break;
    case 'math': wrap('$', 'x^2', '$'); break;
    case 'mathblock': insert = '$$\n' + (selected || 'x^2') + '\n$$'; cursor = sel.from + insert.length - 3; break;
    case 'h2': insert = '## ' + (selected || '标题'); cursor = sel.from + insert.length; break;
    case 'quote': insert = '> ' + (selected || '引用'); cursor = sel.from + insert.length; break;
    case 'list': insert = '- ' + (selected || '项目'); cursor = sel.from + insert.length; break;
    case 'ordered': insert = '1. ' + (selected || '项目'); cursor = sel.from + insert.length; break;
    case 'task': insert = '- [ ] ' + (selected || '任务'); cursor = sel.from + insert.length; break;
    case 'link': insert = '[' + (selected || '文本') + '](url)'; cursor = sel.from + 1 + (selected || '文本').length; break;
    case 'image': insert = '![' + (selected || '描述') + '](url)'; cursor = sel.from + 2 + (selected || '描述').length; break;
    case 'codeblock': insert = '```\n' + (selected || '代码') + '\n```'; cursor = sel.from + 4 + (selected || '代码').length; break;
    case 'table': insert = '| 列1 | 列2 |\n|---|---|\n| ' + (selected || '值') + ' |  |'; cursor = sel.from + insert.length; break;
    case 'hr': insert = '\n---\n'; cursor = sel.from + insert.length; break;
    default: return;
  }
  if (insert === null) return;
  cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert }, selection: { anchor: cursor } });
  cmView.focus();
}

const MD_COMMANDS = [
  ['加粗', 'bold', '**文本**'], ['斜体', 'italic', '*文本*'], ['删除线', 'strike', '~~文本~~'],
  ['二级标题', 'h2', '## 标题'], ['引用', 'quote', '> 引用'], ['无序列表', 'list', '- 项目'],
  ['有序列表', 'ordered', '1. 项目'], ['任务列表', 'task', '- [ ] 任务'], ['链接', 'link', '[文本](url)'],
  ['图片', 'image', '本地图片或 URL'], ['行内代码', 'code', '`代码`'], ['代码块', 'codeblock', '```'],
  ['表格', 'table', '| 列1 | 列2 |'], ['分隔线', 'hr', '---'], ['行内公式', 'math', '$x^2$'], ['块级公式', 'mathblock', '$$…$$'],
];
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
  const rows = MD_COMMANDS.filter(c => !q || (c[0] + ' ' + c[2]).toLowerCase().includes(q));
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
  ['运算','乘除','times divide','a\\times b\\div c'], ['运算','正负','plus minus pm','a\\pm b'], ['运算','点乘','dot','a\\cdot b'],
  ['关系','小于等于','less equal','a\\le b'], ['关系','大于等于','greater equal','a\\ge b'], ['关系','不等于','not equal','a\\ne b'], ['关系','约等于','approx','a\\approx b'],
  ['箭头','右箭头','right arrow','A\\rightarrow B'], ['箭头','双向箭头','leftright arrow','A\\leftrightarrow B'], ['箭头','推出','implies','A\\Rightarrow B'],
  ['函数','正弦','sin','\\sin x'], ['函数','对数','log','\\log_{a}x'], ['函数','指数','exp','e^{x}'],
  ['结构','求和','sum','\\sum_{i=1}^{n} x_i'], ['结构','积分','integral','\\int_{a}^{b} f(x)\\,dx'], ['结构','极限','limit','\\lim_{x\\to 0} f(x)'], ['结构','矩阵','matrix','\\begin{bmatrix}a&b\\\\c&d\\end{bmatrix}'], ['结构','分段函数','cases','f(x)=\\begin{cases}x,&x\\ge0\\\\-x,&x<0\\end{cases}'],
];
let formulaCategory = '常用';

function openFormulaModal(mode) { if (!state.editing) return; closeMdPopups(); $('formula-mode').value = mode || 'inline'; $('formula-modal').classList.remove('hidden'); $('formula-search').value = ''; renderFormulaPicker(); setTimeout(() => $('formula-search').focus(), 0); }
function closeFormulaModal() { $('formula-modal').classList.add('hidden'); if (cmView) cmView.focus(); }
function renderFormulaPicker() {
  const cats = [...new Set(FORMULAS.map(f => f[0]))]; const catBox = $('formula-cats'); catBox.innerHTML = '';
  cats.forEach(c => { const b = document.createElement('button'); b.textContent = c; b.classList.toggle('active', c === formulaCategory); b.addEventListener('click', () => { formulaCategory = c; renderFormulaPicker(); }); catBox.appendChild(b); });
  const q = $('formula-search').value.trim().toLowerCase(); const rows = FORMULAS.filter(f => (q ? (f.join(' ').toLowerCase().includes(q)) : f[0] === formulaCategory));
  const list = $('formula-list'); list.innerHTML = '';
  rows.forEach(f => { const b = document.createElement('button'); b.className = 'formula-item'; b.innerHTML = '<span></span><small></small>'; b.querySelector('span').textContent = f[1]; b.querySelector('small').textContent = f[3]; b.addEventListener('mouseenter', () => previewFormula(f[3])); b.addEventListener('focus', () => previewFormula(f[3])); b.addEventListener('click', () => insertFormula(f[3])); list.appendChild(b); });
}
function previewFormula(tex) { const p = $('formula-preview'); p.textContent = '$$' + tex + '$$'; renderMath(p); }
function insertFormula(tex) { const mode = $('formula-mode').value; closeFormulaModal(); if (!cmView) return; const sel = cmView.state.selection.main; const selected = cmView.state.sliceDoc(sel.from, sel.to); const body = selected || tex; const insert = mode === 'block' ? '\n$$\n' + body + '\n$$\n' : '$' + body + '$'; cmView.dispatch({changes:{from:sel.from,to:sel.to,insert},selection:{anchor:sel.from+insert.length}}); cmView.focus(); }
