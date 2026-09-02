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
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  return new Promise((resolve, reject) => {
    if (window.ReadMDCodeMirror) { cmReady = true; resolve(); return; }
    if (cmLoading) {
      const t0 = Date.now();
      const iv = setInterval(() => {
        if (window.ReadMDCodeMirror) { clearInterval(iv); cmReady = true; cmLoading = false; resolve(); }
         else if (Date.now() - t0 > 15000) { clearInterval(iv); cmLoading = false; reject(new Error(_t('toast.editorLoadTimeout'))); }
      }, 100);
      return;
    }
    cmLoading = true;
    const s = document.createElement('script');
    s.src = '/assets/vendor/codemirror.bundle.js';
    s.onload = () => { cmReady = true; cmLoading = false; resolve(); };
   s.onerror = () => { cmLoading = false; reject(new Error(_t('toast.editorLoadFail'))); };
    document.head.appendChild(s);
  });
}

function createEditor(doc) {
  destroyEditor();
  if (!window.ReadMDCodeMirror) return false;
  const CM = window.ReadMDCodeMirror;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
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
      CM.keymap.of([
        { key: 'Alt-k', run: () => { openEditAiBar(); return true; } },
        { key: 'Ctrl-j', run: () => { openEditAiBar(); return true; } },
        CM.indentWithTab,
        ...CM.closeBracketsKeymap,
        ...CM.defaultKeymap,
        ...CM.historyKeymap,
        ...CM.completionKeymap
      ]),
      CM.EditorView.lineWrapping,
      CM.EditorView.contentAttributes.of({ 'aria-label': _t('toolbar.edit') || '' }),
      CM.EditorView.updateListener.of(u => {
        if (u.docChanged) {
          schedulePreview();
          updateDocStatistics();
          if (typeof updateUnloadGuard === 'function') updateUnloadGuard();
          if (typeof syncActiveTabDirty === 'function') syncActiveTabDirty();
        }
        if (u.selectionSet || u.docChanged) updateCmSelectionToolbar();
      }),
    ],
  });
  cmView = new CM.EditorView({ state: st, parent: $('edit-cm') });
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
    showToast(_t('toast.copiedSelection') || '', 1500);
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
  showToast(_t('toast.cutSelection') || '', 1500);
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
    showToast(_t('toast.noPasteText') || '');
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
  const headingWord = _t('editor.headingWord') || '';
  const textWord = _t('editor.textWord') || '';
  const codeWord = _t('editor.codeWord') || '';
  const descWord = _t('editor.descWord') || '';
  const taskWord = _t('editor.taskWord') || '';

  const ALL = [
    item('# ' + headingWord, '# ${' + headingWord + '}', _t('editor.h1') || '', 'markdown'),
    item('## ' + headingWord, '## ${' + headingWord + '}', _t('editor.h2') || '', 'markdown'),
    item('### ' + headingWord, '### ${' + headingWord + '}', _t('editor.h3') || '', 'markdown'),
    item('#### ' + headingWord, '#### ${' + headingWord + '}', _t('editor.h4') || '', 'markdown'),
    item('**' + (_t('editor.bold') || '') + '**', '**${' + textWord + '}**', _t('editor.bold') || '', 'markdown'),
    item('*' + (_t('editor.italic') || '') + '*', '*${' + textWord + '}*', _t('editor.italic') || '', 'markdown'),
    item('~~' + (_t('editor.strikethrough') || '') + '~~', '~~${' + textWord + '}~~', _t('editor.strikethrough') || '', 'markdown'),
    item('`' + (_t('editor.codeInline') || '') + '`', '`${' + codeWord + '}`', _t('editor.codeInline') || '', 'markdown'),
    item('```' + (_t('editor.codeBlock') || ''), '```\n${' + codeWord + '}\n```', _t('editor.codeBlock') || '', 'markdown'),
    item('[' + textWord + '](url)', '[${' + textWord + '}](url)', _t('editor.link') || '', 'markdown'),
    item('![' + descWord + '](url)', '![${' + descWord + '}](url)', _t('editor.image') || '', 'markdown'),
    item('> ' + (_t('editor.quote') || ''), '> ${' + textWord + '}', _t('editor.quote') || '', 'markdown'),
    item('$x^2$', '$x^2$', _t('editor.mathInline') || '', 'markdown'),
    item('$$...$$', '$$\n${' + textWord + '}\n$$', _t('editor.mathBlock') || '', 'markdown'),
    item('| ' + (_t('editor.table') || '') + ' |', '| Col 1 | Col 2 |\n|---|---|\n| ${' + textWord + '} |  |', _t('editor.table') || '', 'markdown'),
    item('- ' + (_t('editor.listUnordered') || ''), '- ${' + textWord + '}', _t('editor.listUnordered') || '', 'markdown'),
    item('- [ ] ' + (_t('editor.listTask') || ''), '- [ ] ${' + taskWord + '}', _t('editor.listTask') || '', 'markdown'),
    item('--- ' + (_t('editor.hr') || ''), '---', _t('editor.hr') || '', 'markdown'),
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
  const textPlaceholder = _t('editor.textWord') || '';
  const codePlaceholder = _t('editor.codeWord') || '';
  const headingPlaceholder = _t('editor.headingWord') || '';
  const quotePlaceholder = _t('editor.quote') || '';
  const itemPlaceholder = _t('editor.itemWord') || '';
  const taskPlaceholder = _t('editor.taskWord') || '';
  const descPlaceholder = _t('editor.descWord') || '';

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
    case 'codechunk': openCodeChunkModal(); return;
    case 'diagram': openDiagramModal(); return;
    case 'docimport': openDocImportModal(); return;
    case 'frontmatter': insertFrontmatterTemplate(); return;
    default: return;
  }
  if (insert === null) return;
  cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert }, selection: { anchor: cursor } });
  cmView.focus();
}


function closeMdPopups() {
  document.querySelectorAll('.md-menu, .pv-menu').forEach(el => el.classList.add('hidden'));
  const trigger = $('pv-trigger'); if (trigger) trigger.setAttribute('aria-expanded', 'false');
}

/* ============================================================
   Editor Studio PRO: Code Chunk / Diagram / Doc Import Modals
   ============================================================ */

const CODE_CHUNK_SAMPLES = {
  python: 'import matplotlib.pyplot as plt\nimport numpy as np\n\nx = np.linspace(0, 10, 100)\nplt.figure(figsize=(6, 3))\nplt.plot(x, np.sin(x), label="sin(x)", color="#3b6ef5")\nplt.legend()\nplt.grid(True)\nplt.show()',
  javascript: 'const data = [10, 25, 38, 45, 62];\nconsole.log("平均值:", data.reduce((a, b) => a + b, 0) / data.length);',
  bash: '#!/usr/bin/env bash\necho "当前目录内容:"\nls -la',
  r: 'x <- seq(0, 10, by=0.1)\ny <- sin(x)\nplot(x, y, type="l", col="blue", main="Sine Wave")',
  php: '<?php\n$items = ["ReadMD", "Markdown", "Viewer"];\necho "项目: " . implode(" - ", $items);',
  go: 'package main\nimport "fmt"\nfunc main() {\n    fmt.Println("Hello ReadMD Interactive Go!")\n}',
  ruby: 'puts (1..5).map { |n| n ** 2 }.join(", ")'
};

function openCodeChunkModal() {
  if (!state.editing) return;
  closeMdPopups();
  const langSel = $('code-chunk-lang');
  const codeArea = $('code-chunk-code');
  if (langSel && codeArea && !codeArea.value.trim()) {
    codeArea.value = CODE_CHUNK_SAMPLES[langSel.value] || CODE_CHUNK_SAMPLES.python;
  }
  $('code-chunk-modal').classList.remove('hidden');
  setTimeout(() => { if (langSel) langSel.focus(); }, 50);
}

function closeCodeChunkModal() {
  $('code-chunk-modal').classList.add('hidden');
  if (cmView) cmView.focus();
}

function insertCodeChunkFromModal() {
  const lang = ($('code-chunk-lang') && $('code-chunk-lang').value) || 'python';
  const isPlot = $('code-chunk-opt-plot') ? $('code-chunk-opt-plot').checked : true;
  const isHide = $('code-chunk-opt-hide') ? $('code-chunk-opt-hide').checked : false;
  const code = ($('code-chunk-code') && $('code-chunk-code').value) || '';

  const flags = ['cmd=true'];
  if (isPlot && lang === 'python') flags.push('matplotlib=true');
  if (isHide) flags.push('hide=true');

  const chunkMd = `\n\`\`\`${lang} {${flags.join(' ')}}\n${code.trim()}\n\`\`\`\n`;
  closeCodeChunkModal();

  if (cmView) {
    const sel = cmView.state.selection.main;
    cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert: chunkMd }, selection: { anchor: sel.from + chunkMd.length } });
    cmView.focus();
  }
}

const DIAGRAM_SAMPLES = {
  plantuml: '@startuml\nautonumber\nClient -> Server: 发送数据请求 (GET /api/status)\nServer -> Database: 查询记录\nDatabase --> Server: 返回数据集\nServer --> Client: 响应 200 OK\n@enduml',
  tikz: '\\begin{tikzpicture}\n\\draw[thick,->] (0,0) -- (4,0) node[anchor=north west] {x};\n\\draw[thick,->] (0,0) -- (0,3) node[anchor=south east] {y};\n\\draw[red,domain=0:3.5] plot (\\x,{0.2*\\x*\\x}) node[right] {$f(x)=\\frac{1}{5}x^2$};\n\\end{tikzpicture}',
  wavedrom: '{\n  signal: [\n    { name: "CLK",  wave: "p......" },\n    { name: "Data", wave: "x.345x.", data: ["head", "body", "tail"] },\n    { name: "Req",  wave: "0.1..0." },\n    { name: "Ack",  wave: "0..1.0." }\n  ]\n}',
  'vega-lite': '{\n  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",\n  "description": "柱状统计图",\n  "data": {\n    "values": [\n      {"类别": "A", "数值": 28}, {"类别": "B", "数值": 55},\n      {"类别": "C", "数值": 43}, {"类别": "D", "数值": 91}\n    ]\n  },\n  "mark": "bar",\n  "encoding": {\n    "x": {"field": "类别", "type": "nominal", "axis": {"labelAngle": 0}},\n    "y": {"field": "数值", "type": "quantitative"}\n  }\n}',
  graphviz: 'digraph G {\n  rankdir=LR;\n  node [shape=box, style=rounded];\n  Start -> Process -> Decision;\n  Decision -> Success [label="是"];\n  Decision -> Failure [label="否"];\n}',
  d2: 'ReadMD -> Parser: Markdown AST\nParser -> Renderer: HTML + Math\nRenderer -> Webview: DOM 呈现',
  bitfield: '{\n  reg: [\n    {bits: 8, name: "IPO", type: 8},\n    {bits: 8, name: "Payload"},\n    {bits: 16, name: "CRC32", type: 2}\n  ]\n}'
};

function openDiagramModal() {
  if (!state.editing) return;
  closeMdPopups();
  const typeSel = $('diagram-type');
  const codeArea = $('diagram-code');
  if (typeSel && codeArea) {
    codeArea.value = DIAGRAM_SAMPLES[typeSel.value] || DIAGRAM_SAMPLES.plantuml;
  }
  $('diagram-modal').classList.remove('hidden');
  setTimeout(() => { if (typeSel) typeSel.focus(); }, 50);
}

function closeDiagramModal() {
  $('diagram-modal').classList.add('hidden');
  if (cmView) cmView.focus();
}

function insertDiagramFromModal() {
  const type = ($('diagram-type') && $('diagram-type').value) || 'plantuml';
  const code = ($('diagram-code') && $('diagram-code').value) || '';
  const diagramMd = `\n\`\`\`${type}\n${code.trim()}\n\`\`\`\n`;
  closeDiagramModal();

  if (cmView) {
    const sel = cmView.state.selection.main;
    cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert: diagramMd }, selection: { anchor: sel.from + diagramMd.length } });
    cmView.focus();
  }
}

function openDocImportModal() {
  if (!state.editing) return;
  closeMdPopups();
  $('doc-import-modal').classList.remove('hidden');
  if ($('doc-import-path')) $('doc-import-path').focus();
}

function closeDocImportModal() {
  $('doc-import-modal').classList.add('hidden');
  if (cmView) cmView.focus();
}

function insertDocImportFromModal() {
  const path = ($('doc-import-path') && $('doc-import-path').value.trim()) || 'chapter1.md';
  const mode = ($('doc-import-mode') && $('doc-import-mode').value) || 'markdown';
  const lines = ($('doc-import-lines') && $('doc-import-lines').value.trim()) || '';

  const opts = [];
  if (mode !== 'markdown') opts.push(`mode="${mode}"`);
  if (lines) opts.push(`lines="${lines}"`);

  const optStr = opts.length ? ` {${opts.join(' ')}}` : '';
  const importMd = `\n@import "${path}"${optStr}\n`;
  closeDocImportModal();

  if (cmView) {
    const sel = cmView.state.selection.main;
    cmView.dispatch({ changes: { from: sel.from, to: sel.to, insert: importMd }, selection: { anchor: sel.from + importMd.length } });
    cmView.focus();
  }
}

function docImportRelativePath(picked) {
  const norm = s => String(s || '').replace(/\\/g, '/');
  const target = norm(picked);
  const base = norm(state.file || '');
  const baseDir = base ? base.split('/').slice(0, -1) : [];
  const parts = target.split('/');
  let i = 0;
  while (i < baseDir.length && i < parts.length - 1 && baseDir[i].toLowerCase() === parts[i].toLowerCase()) i++;
  if (!i) return target;
  const rel = '../'.repeat(baseDir.length - i) + parts.slice(i).join('/');
  return rel.startsWith('../') ? rel : './' + rel;
}

async function browseDocImportFile() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const py = window.pywebview && window.pywebview.api;
  if (!py || typeof py.choose_file !== 'function') {
    showToast(_t('toast.browserModeHint'));
    return;
  }
  let picked = null;
  try { picked = await py.choose_file(); } catch (e) { picked = null; }
  if (!picked) return;
  const input = $('doc-import-path');
  if (input) { input.value = docImportRelativePath(picked); input.focus(); }
}

function openFrontmatterModal() {
  if (!state.editing) return;
  closeMdPopups();
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const modal = $('frontmatter-modal');
  if (!modal) return;

  if ($('fm-input-title')) {
    const defTitle = (state.mode === 'file' && state.file) ? state.file.split(/[\\/]/).pop().replace(/\.[^.]+$/, '') : (_t('editor.docTitleDefault') || '');
    $('fm-input-title').value = defTitle;
  }
  modal.classList.remove('hidden');
  setTimeout(() => { if ($('fm-input-title')) $('fm-input-title').focus(); }, 50);
}

function closeFrontmatterModal() {
  const modal = $('frontmatter-modal');
  if (modal) modal.classList.add('hidden');
  if (cmView) cmView.focus();
}

function insertFrontmatterFromModal() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const title = ($('fm-input-title') && $('fm-input-title').value.trim()) || (_t('editor.docTitleDefault') || '');
  const author = ($('fm-input-author') && $('fm-input-author').value.trim()) || 'ReadMD User';
  const theme = ($('fm-select-theme') && $('fm-select-theme').value) || 'black';
  const transition = ($('fm-select-transition') && $('fm-select-transition').value) || 'slide';

  const frontmatter = `---\ntitle: "${title}"\nauthor: "${author}"\npresentation:\n  theme: "${theme}"\n  transition: "${transition}"\n---\n\n`;
  closeFrontmatterModal();

  if (cmView) {
    const currentDoc = cmView.state.doc.toString();
    if (currentDoc.startsWith('---')) {
      const secondDivider = currentDoc.indexOf('\n---', 3);
      if (secondDivider !== -1) {
        const endOfFm = currentDoc.indexOf('\n', secondDivider + 4);
        const replaceLen = (endOfFm !== -1 ? endOfFm + 1 : secondDivider + 4);
        cmView.dispatch({ changes: { from: 0, to: replaceLen, insert: frontmatter }, selection: { anchor: frontmatter.length } });
        cmView.focus();
        showToast(_t('toast.frontmatterUpdated') || '');
        return;
      }
    }
    cmView.dispatch({ changes: { from: 0, to: 0, insert: frontmatter }, selection: { anchor: frontmatter.length } });
    cmView.focus();
    showToast(_t('toast.frontmatterInserted') || '');
  }
}

function insertFrontmatterTemplate() {
  openFrontmatterModal();
}

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
   Editor Studio PRO: 表格设计器 & 统计（Zen 由 reader/render.js 统一管理）
   ============================================================ */

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
  const defaultCol = _t('editor.table') || '';
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
  const headerPrefix = _t('editor.tableHeaderPrefix') || '';
  const cellWord = _t('editor.tableCell') || '';
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

/* ============================================================
   Editor Studio PRO: AI Assistant & Inline Completion (Alt+K)
   ============================================================ */

let editAiCurrentResult = '';
let editAiSelectionRange = null;

function openEditAiBar() {
  if (!state.editing || !cmView) return;
  const bar = $('edit-ai-bar');
  if (!bar) return;

  const sel = cmView.state.selection.main;
  if (sel && !sel.empty) {
    editAiSelectionRange = { from: sel.from, to: sel.to, text: cmView.state.sliceDoc(sel.from, sel.to) };
  } else {
    const cursorPos = sel ? sel.from : cmView.state.doc.length;
    const contextBefore = cmView.state.sliceDoc(Math.max(0, cursorPos - 1200), cursorPos);
    editAiSelectionRange = { from: cursorPos, to: cursorPos, text: '', context: contextBefore };
  }

  bar.classList.remove('hidden');
  const input = $('edit-ai-input');
  if (input) {
    input.value = '';
    setTimeout(() => input.focus(), 30);
  }
}

function closeEditAiBar() {
  const bar = $('edit-ai-bar');
  if (bar) bar.classList.add('hidden');
  const preview = $('edit-ai-preview');
  if (preview) preview.classList.add('hidden');
  editAiCurrentResult = '';
  editAiSelectionRange = null;
  if (cmView) cmView.focus();
}

async function runEditAiAction(act, customPrompt = '') {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const preview = $('edit-ai-preview');
  const previewContent = $('edit-ai-preview-content');
  const statusEl = $('edit-ai-status');
  if (preview) preview.classList.remove('hidden');
  if (previewContent) previewContent.innerHTML = '';
  if (statusEl) statusEl.textContent = _t('editai.generating') || '';

  const range = editAiSelectionRange || { from: 0, to: 0, text: '' };
  const skillByAction = { complete: 'readmd-continue', polish: 'readmd-polish', fix: 'readmd-format-fix', translate: 'readmd-translate' };
  const skillId = skillByAction[act] || 'readmd-polish';
  let userMessage = '';
  const sourceText = range.text || (cmView ? cmView.state.doc.toString() : '');

  if (act === 'complete') {
    userMessage = customPrompt || '';
  } else if (act === 'polish') {
    userMessage = customPrompt || '';
  } else if (act === 'fix') {
    userMessage = customPrompt || '';
  } else if (act === 'translate') {
    userMessage = customPrompt || '';
  } else {
    userMessage = customPrompt || '';
  }

  editAiCurrentResult = '';

  try {
    const connection = typeof ensureAiConfigured === 'function'
      ? await ensureAiConfigured()
      : (typeof resolveSharedAiConnection === 'function' ? await resolveSharedAiConnection() : null);
    if (!connection) {
      if (statusEl) statusEl.textContent = _t('toast.noApiKeyNotice');
      return;
    }
    const res = await apiFetch('/api/ai/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        provider: connection.provider,
        credential_id: connection.credential_id,
        model: connection.model,
        base_url: connection.base_url,
        mode: connection.mode,
        endpoint_mode: connection.endpoint_mode,
        headers: connection.headers,
        skill_id: skillId,
        skill_variables: {
          document: range.context || sourceText,
          selection: sourceText,
          request: userMessage,
          language: (window.i18n && window.i18n.locale) || document.documentElement.lang || 'en',
          context: range.context || '',
          output_format: 'Markdown'
        },
        messages: [{ role: 'user', content: userMessage }],
        stream: false
      })
    });

    if (!res.ok) {
      const errText = await res.text().catch(() => '');
      let errMsg = 'HTTP ' + res.status;
      try {
        const errJson = JSON.parse(errText);
        if (errJson.error) errMsg = errJson.error;
      } catch (e) {
        if (errText) errMsg = errText.slice(0, 100);
      }
      throw new Error(errMsg);
    }

    let resultText = '';
    const contentType = res.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
      const data = await res.json();
      resultText = data.content || (data.choices && data.choices[0] && data.choices[0].message && data.choices[0].message.content) || '';
    } else {
      const rawText = await res.text();
      const lines = rawText.split('\n');
      const chunks = [];
      for (const line of lines) {
        if (line.startsWith('data:')) {
          const jsonStr = line.slice(5).trim();
          if (jsonStr && jsonStr !== '[DONE]') {
            try {
              const d = JSON.parse(jsonStr);
              if (d.d) chunks.push(d.d);
              else if (d.content) chunks.push(d.content);
            } catch (e) {}
          }
        }
      }
      resultText = chunks.length ? chunks.join('') : rawText;
    }

    editAiCurrentResult = resultText;
    if (previewContent) {
      previewContent.textContent = resultText;
    }
    if (statusEl) statusEl.textContent = _t('editai.title') || '';
  } catch (err) {
    if (statusEl) statusEl.textContent = (_t('ai.reqFailMsg') || '') + err.message;
    if (previewContent) previewContent.textContent = err.message;
  }
}

function applyEditAiResult() {
  if (!cmView || !editAiCurrentResult) {
    closeEditAiBar();
    return;
  }
  const range = editAiSelectionRange || { from: cmView.state.selection.main.from, to: cmView.state.selection.main.to };
  cmView.dispatch({
    changes: { from: range.from, to: range.to, insert: editAiCurrentResult },
    selection: { anchor: range.from + editAiCurrentResult.length }
  });
  closeEditAiBar();
}

function insertEditAiResult() {
  if (!cmView || !editAiCurrentResult) {
    closeEditAiBar();
    return;
  }
  const sel = cmView.state.selection.main;
  const pos = sel ? sel.to : cmView.state.doc.length;
  cmView.dispatch({
    changes: { from: pos, to: pos, insert: '\n' + editAiCurrentResult + '\n' },
    selection: { anchor: pos + editAiCurrentResult.length + 2 }
  });
  closeEditAiBar();
}

function discardEditAiResult() {
  closeEditAiBar();
}

function bindEditorAiEvents() {
  const btnAssistant = $('btn-edit-ai-assistant');
  const cmSelAi = $('cm-sel-ai');
  const closeBtn = $('edit-ai-close');
  const submitBtn = $('edit-ai-submit');
  const inputEl = $('edit-ai-input');
  const applyBtn = $('edit-ai-apply');
  const insertBtn = $('edit-ai-insert');
  const discardBtn = $('edit-ai-discard');

  if (btnAssistant) btnAssistant.addEventListener('click', openEditAiBar);
  if (cmSelAi) cmSelAi.addEventListener('click', () => { hideCmSelectionToolbar(); openEditAiBar(); });
  if (closeBtn) closeBtn.addEventListener('click', closeEditAiBar);
  if (discardBtn) discardBtn.addEventListener('click', discardEditAiResult);
  if (applyBtn) applyBtn.addEventListener('click', applyEditAiResult);
  if (insertBtn) insertBtn.addEventListener('click', insertEditAiResult);

  if (submitBtn && inputEl) {
    submitBtn.addEventListener('click', () => {
      const val = inputEl.value.trim();
      if (val) runEditAiAction('custom', val);
    });
    inputEl.addEventListener('keydown', e => {
      if (e.key === 'Enter') {
        e.preventDefault();
        const val = inputEl.value.trim();
        if (val) runEditAiAction('custom', val);
      } else if (e.key === 'Escape') {
        closeEditAiBar();
      }
    });
  }

  document.querySelectorAll('.edit-ai-act-chip').forEach(btn => {
    btn.addEventListener('click', () => {
      const act = btn.dataset.act;
      if (act) runEditAiAction(act);
    });
  });
}
