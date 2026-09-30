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

/* Editor view preferences (line numbers / focus / typewriter), kept in the
   browser profile so they survive restarts without touching shared settings. */
const EDITOR_PREFS_KEY = 'readmd-editor-prefs';
const editorPrefs = (() => {
  const base = { lineNumbers: false, focus: false, typewriter: false };
  try { return Object.assign(base, JSON.parse(localStorage.getItem(EDITOR_PREFS_KEY) || '{}')); }
  catch (e) { return base; }
})();
function saveEditorPrefs() {
  try { localStorage.setItem(EDITOR_PREFS_KEY, JSON.stringify(editorPrefs)); } catch (e) { /* ignore */ }
}

let cmCompartments = null;
let cmPointerDown = false;

function createEditor(doc) {
  destroyEditor();
  if (!window.ReadMDCodeMirror) return false;
  const CM = window.ReadMDCodeMirror;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const C = cmCompartments = {
    theme: new CM.Compartment(), gutter: new CM.Compartment(),
    focus: new CM.Compartment(), typewriter: new CM.Compartment(),
  };
  cmThemeCompartment = C.theme;
  const fmt = kind => () => { cmInsertSyntax(kind); return true; };
  const st = CM.EditorState.create({
    doc: doc,
    extensions: [
      C.gutter.of(cmGutterExtension()),
      CM.highlightActiveLine(),
      CM.highlightSpecialChars(),
      CM.drawSelection({ cursorBlinkRate: 1100 }),
      CM.dropCursor(),
      CM.EditorState.allowMultipleSelections.of(true),
      CM.rectangularSelection(),
      CM.crosshairCursor(),
      CM.bracketMatching(),
      CM.indentOnInput(),
      CM.history(),
      CM.markdown({ base: CM.markdownLanguage, codeLanguages: CM.languages }),
      // Markdown prose: pair brackets, double quotes, backticks and CJK brackets
      // (single quotes stay unpaired: they are apostrophes in prose).
      CM.markdownLanguage.data.of({ closeBrackets: { brackets: ['(', '[', '{', '"', '`', '（', '【', '「', '《', '“'] } }),
      CM.syntaxHighlighting(cmHighlightStyle()),
      cmMarkdownDecorations(),
      C.focus.of(editorPrefs.focus ? cmFocusExtension() : []),
      C.typewriter.of(editorPrefs.typewriter ? cmTypewriterExtension() : []),
      CM.autocompletion({ override: [cmMarkdownCompletions()], activateOnTyping: false, icons: false }),
      CM.closeBrackets(),
      CM.Prec.highest(CM.keymap.of(cmPriorityKeymap())),
      CM.keymap.of([
        { key: 'Alt-k', run: () => { openEditAiBar(); return true; } },
        { key: 'Ctrl-j', run: () => { openEditAiBar(); return true; } },
        { key: 'Mod-b', run: fmt('bold'), preventDefault: true },
        { key: 'Mod-i', run: fmt('italic'), preventDefault: true },
        { key: 'Mod-k', run: fmt('link'), preventDefault: true },
        // Ctrl+E toggles edit mode globally; inside the editor it is inline code.
        { key: 'Mod-e', run: fmt('code'), preventDefault: true, stopPropagation: true },
        { key: 'Mod-Shift-x', run: fmt('strike'), preventDefault: true },
        { key: 'Mod-Shift-8', run: fmt('list'), preventDefault: true },
        { key: 'Mod-Shift-7', run: fmt('ordered'), preventDefault: true },
        { key: 'Mod-Shift-9', run: fmt('task'), preventDefault: true },
        { key: 'Mod-Shift-.', run: fmt('quote'), preventDefault: true },
        ...[0, 1, 2, 3, 4, 5, 6].map(n => ({ key: 'Mod-' + n, run: fmt(n ? 'h' + n : 'para'), preventDefault: true, stopPropagation: true })),
        { key: 'Mod-/', run: v => { openSlashMenu(v, { typed: false }); return true; }, preventDefault: true },
        CM.indentWithTab,
        ...CM.closeBracketsKeymap,
        ...CM.defaultKeymap,
        ...CM.historyKeymap,
        ...CM.completionKeymap
      ]),
      C.theme.of(cmThemeFor(document.body.dataset.theme)),
      CM.EditorView.lineWrapping,
      CM.placeholder(_t('editor.placeholder')),
      CM.EditorView.contentAttributes.of({ 'aria-label': _t('toolbar.edit') || '', 'aria-multiline': 'true' }),
      CM.Prec.high(CM.EditorView.inputHandler.of(cmSmartInput)),
      CM.Prec.highest(CM.EditorView.domEventHandlers({ paste: cmHandlePaste })),
      CM.EditorView.domEventHandlers({ mousedown: cmTaskMarkerClick }),
      CM.EditorView.updateListener.of(u => {
        if (u.docChanged) {
          schedulePreview();
          scheduleDocStatistics();
          if (typeof updateUnloadGuard === 'function') updateUnloadGuard();
          if (typeof syncActiveTabDirty === 'function') syncActiveTabDirty();
        }
        slashOnUpdate(u);
        if (u.selectionSet && !u.docChanged) scheduleDocStatistics();
        if (u.selectionSet || u.docChanged) updateCmSelectionToolbar();
        if (u.geometryChanged && !slash.open) repositionCmSelectionToolbar();
      }),
    ],
  });
  cmView = new CM.EditorView({ state: st, parent: $('edit-cm') });
  window.cmView = cmView;
  applyEditorViewClasses();
  cmView.dom.addEventListener('pointerdown', () => { cmPointerDown = true; hideCmSelectionToolbar(); });
  cmView.dom.addEventListener('keyup', () => setTimeout(updateCmSelectionToolbar, 10));
  cmView.scrollDOM.addEventListener('scroll', () => {
    if (slash.open) positionSlashMenu();
    repositionCmSelectionToolbar();
  }, { passive: true });
  updateDocStatistics();
  cmView.focus();
  return true;

}

document.addEventListener('pointerup', () => {
  if (!cmPointerDown) return;
  cmPointerDown = false;
  setTimeout(updateCmSelectionToolbar, 10);
});

/* vendor 包只导出 historyKeymap，不直接导出 undo / redo：从键位表里取命令 */
function cmHistoryCommand(key) {
  const CM = window.ReadMDCodeMirror;
  if (!CM) return null;
  if (typeof CM[key] === 'function') return CM[key];
  const want = key === 'undo' ? 'Mod-z' : 'Mod-y';
  const b = (CM.historyKeymap || []).find(x => x.key === want);
  return b && typeof b.run === 'function' ? b.run : null;
}

function cmUndo() {
  const run = cmView && cmHistoryCommand('undo');
  if (run) { run(cmView); cmView.focus(); }
  else if ($('edit-area')) document.execCommand('undo');
}

function cmRedo() {
  const run = cmView && cmHistoryCommand('redo');
  if (run) { run(cmView); cmView.focus(); }
  else if ($('edit-area')) document.execCommand('redo');
}

function hideCmSelectionToolbar() {
  const toolbar = $('cm-selection-toolbar');
  if (toolbar) toolbar.classList.add('hidden');
}

/* Selection toolbar buttons -> toolbar command (same code path as #md-tool). */
const CM_SEL_COMMANDS = {
  'cm-sel-bold': 'bold', 'cm-sel-italic': 'italic', 'cm-sel-strike': 'strike',
  'cm-sel-code': 'code', 'cm-sel-link': 'link', 'cm-sel-heading': 'h2', 'cm-sel-quote': 'quote',
};
const CM_SEL_NODES = { bold: 'StrongEmphasis', italic: 'Emphasis', strike: 'Strikethrough', code: 'InlineCode', link: 'Link' };

function cmSelectionMarks(view, from, to) {
  const CM = window.ReadMDCodeMirror;
  const active = new Set();
  if (!CM || !CM.syntaxTree) return active;
  const tree = CM.syntaxTree(view.state);
  const names = Object.values(CM_SEL_NODES);
  for (let n = tree.resolveInner(from, 1); n; n = n.parent) {
    if (names.includes(n.name) && n.from <= from && n.to >= to) active.add(n.name);
  }
  return active;
}

function updateCmSelectionToolbar() {
  const toolbar = $('cm-selection-toolbar');
  if (!toolbar) return;
  if (!state.editing || !cmView || cmPointerDown || slash.open) {
    toolbar.classList.add('hidden');
    return;
  }
  const sel = cmView.state.selection.main;
  if (!sel || sel.empty || !cmView.hasFocus && !toolbar.contains(document.activeElement)) {
    toolbar.classList.add('hidden');
    return;
  }
  const text = cmView.state.sliceDoc(sel.from, sel.to).trim();
  if (!text) {
    toolbar.classList.add('hidden');
    return;
  }
  const a = cmView.coordsAtPos(sel.from, 1);
  const b = cmView.coordsAtPos(sel.to, -1) || a;
  if (!a) {
    toolbar.classList.add('hidden');
    return;
  }
  const marks = cmSelectionMarks(cmView, sel.from, sel.to);
  toolbar.querySelectorAll('[data-cmd]').forEach(btn => {
    const node = CM_SEL_NODES[btn.dataset.cmd];
    if (node) btn.setAttribute('aria-pressed', marks.has(node) ? 'true' : 'false');
  });
  const wasHidden = toolbar.classList.contains('hidden');
  toolbar.classList.remove('hidden');
  if (wasHidden) toolbar.classList.remove('is-settled');
  const tbWidth = toolbar.offsetWidth || 320;
  const tbHeight = toolbar.offsetHeight || 36;
  const scroller = cmView.scrollDOM.getBoundingClientRect();
  const sameLine = Math.abs(a.top - b.top) < 4;
  const anchorX = sameLine ? (a.left + b.right) / 2 : a.left + Math.min(160, (scroller.right - a.left) / 2);
  const minX = Math.max(8, scroller.left + 4);
  const maxX = Math.min(window.innerWidth - 8, scroller.right - 4) - tbWidth;
  const left = Math.max(minX, Math.min(maxX, anchorX - tbWidth / 2));
  // Touch devices show the native selection menu above: sit below instead.
  const below = window.matchMedia && window.matchMedia('(pointer: coarse)').matches;
  let top = below ? b.bottom + 12 : a.top - tbHeight - 10;
  if (!below && top < scroller.top + 4) top = b.bottom + 10;
  if (top + tbHeight > window.innerHeight - 8) top = Math.max(scroller.top + 4, a.top - tbHeight - 10);
  toolbar.style.left = Math.round(left) + 'px';
  toolbar.style.top = Math.round(top) + 'px';
  if (wasHidden) requestAnimationFrame(() => toolbar.classList.add('is-settled'));
}

let cmSelRepositionFrame = 0;
function repositionCmSelectionToolbar() {
  const toolbar = $('cm-selection-toolbar');
  if (!toolbar || toolbar.classList.contains('hidden') || cmSelRepositionFrame) return;
  cmSelRepositionFrame = requestAnimationFrame(() => { cmSelRepositionFrame = 0; updateCmSelectionToolbar(); });
}

/* The AI hook: other modules listen for `readmd:ai-inline` (bubbles to document
   and window) and call preventDefault() to take over.  Unclaimed events fall
   back to the built-in edit AI bar. */
function cmRequestInlineAi(source) {
  if (!cmView) return;
  const sel = cmView.state.selection.main;
  const detail = { selection: cmView.state.sliceDoc(sel.from, sel.to), from: sel.from, to: sel.to, source: source || 'selection-toolbar' };
  const ev = new CustomEvent('readmd:ai-inline', { detail, bubbles: true, cancelable: true, composed: true });
  cmView.dom.dispatchEvent(ev);
  hideCmSelectionToolbar();
  if (!ev.defaultPrevented) openEditAiBar();
}
window.cmRequestInlineAi = cmRequestInlineAi;

function bindCmSelectionToolbar() {
  const toolbar = $('cm-selection-toolbar');
  if (!toolbar || toolbar.dataset.bound) return;
  toolbar.dataset.bound = '1';
  // Keep the editor selection while clicking toolbar buttons.
  toolbar.addEventListener('mousedown', e => { if (e.target.closest('button')) e.preventDefault(); });
  Object.keys(CM_SEL_COMMANDS).forEach(id => {
    const btn = $(id);
    if (btn) btn.addEventListener('click', () => { cmInsertSyntax(CM_SEL_COMMANDS[id]); updateCmSelectionToolbar(); });
  });
  toolbar.addEventListener('keydown', e => {
    const buttons = [...toolbar.querySelectorAll('button')].filter(b => b.offsetParent !== null);
    const at = buttons.indexOf(document.activeElement);
    if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
      e.preventDefault();
      const next = buttons[(at + (e.key === 'ArrowRight' ? 1 : -1) + buttons.length) % buttons.length];
      if (next) next.focus();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      if (cmView) cmView.focus();
    }
  });
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
  if (slash.open && slash.el && !slash.el.contains(e.target)) closeSlashMenu();
});

function destroyEditor() {

  hideCmSelectionToolbar();
  closeSlashMenu();
  if (cmView) {
    try { cmView.destroy(); } catch (e) { /* ignore */ }
    cmView = null;
  }
  window.cmView = null;
  const c = $('edit-cm');
  if (c) c.innerHTML = '';
  cmThemeCompartment = null;
  cmCompartments = null;
}


/* CodeMirror theme: every colour is a CSS token (tokens.css), so one theme
   object serves light / dark / sepia and re-evaluates when body[data-theme]
   changes. The `dark` flag only steers CodeMirror's own base styles. */
const cmThemeCache = {};
function cmThemeFor(theme) {
  const CM = window.ReadMDCodeMirror;
  const dark = theme === 'dark';
  const key = dark ? 'dark' : 'light';
  if (cmThemeCache[key]) return cmThemeCache[key];
  if (!CM.EditorView || typeof CM.EditorView.theme !== 'function') return [];
  cmThemeCache[key] = CM.EditorView.theme({
    '&': { backgroundColor: 'var(--color-surface)', color: 'var(--color-fg)' },
    '.cm-content': { caretColor: 'var(--editor-caret)' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--editor-caret)', borderLeftWidth: '2px' },
    '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': { backgroundColor: 'var(--editor-selection-bg)' },
    '.cm-selectionBackground': { backgroundColor: 'var(--editor-selection-blur)' },
    '.cm-activeLine': { backgroundColor: 'transparent' },
    '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--color-fg-subtle)', border: 'none' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-fg)' },
    '.cm-matchingBracket, &.cm-focused .cm-matchingBracket': { backgroundColor: 'var(--color-accent-soft)', outline: '1px solid color-mix(in srgb, var(--color-accent) 40%, transparent)' },
    '.cm-nonmatchingBracket, &.cm-focused .cm-nonmatchingBracket': { backgroundColor: 'transparent', color: 'var(--color-danger)' },
    '.cm-placeholder': { color: 'var(--color-fg-subtle)' },
  }, { dark });
  return cmThemeCache[key];
}

/* Markdown source that reads like a document: headings sized and weighted,
   emphasis rendered, syntax marks dimmed, code in the code face. */
let cmHighlightCache = null;
function cmHighlightStyle() {
  const CM = window.ReadMDCodeMirror;
  if (cmHighlightCache) return cmHighlightCache;
  if (!CM.HighlightStyle || !CM.tags) return CM.defaultHighlightStyle;
  const t = CM.tags;
  cmHighlightCache = CM.HighlightStyle.define([
    { tag: t.heading1, class: 'cmt-h cmt-h1' },
    { tag: t.heading2, class: 'cmt-h cmt-h2' },
    { tag: t.heading3, class: 'cmt-h cmt-h3' },
    { tag: [t.heading4, t.heading5, t.heading6], class: 'cmt-h cmt-h4' },
    { tag: t.heading, class: 'cmt-h' },
    { tag: t.strong, class: 'cmt-strong' },
    { tag: t.emphasis, class: 'cmt-em' },
    { tag: t.strikethrough, class: 'cmt-strike' },
    { tag: t.link, class: 'cmt-link' },
    { tag: t.url, class: 'cmt-url' },
    { tag: t.monospace, class: 'cmt-code' },
    { tag: t.quote, class: 'cmt-quote' },
    { tag: t.list, class: 'cmt-list' },
    { tag: t.contentSeparator, class: 'cmt-hr' },
    { tag: t.processingInstruction, class: 'cmt-mark' },
    { tag: t.labelName, class: 'cmt-label' },
    { tag: t.atom, class: 'cmt-atom' },
    { tag: t.escape, class: 'cmt-mark' },
    { tag: [t.comment, t.blockComment, t.lineComment], class: 'cmt-comment' },
    { tag: t.meta, class: 'cmt-meta' },
    // Fenced code languages
    { tag: [t.keyword, t.controlKeyword, t.operatorKeyword, t.definitionKeyword, t.moduleKeyword], class: 'cmt-kw' },
    { tag: [t.string, t.special(t.string), t.regexp], class: 'cmt-str' },
    { tag: [t.number, t.bool, t.null], class: 'cmt-num' },
    { tag: [t.function(t.variableName), t.function(t.propertyName)], class: 'cmt-fn' },
    { tag: [t.typeName, t.className, t.namespace], class: 'cmt-type' },
    { tag: [t.propertyName, t.attributeName], class: 'cmt-prop' },
    { tag: [t.tagName, t.angleBracket], class: 'cmt-tag' },
    { tag: [t.definition(t.variableName)], class: 'cmt-def' },
    { tag: t.invalid, class: 'cmt-invalid' },
  ]);
  return cmHighlightCache;
}

/* Line-level decorations: heading lines, fenced code blocks, quotes,
   callouts, tables, horizontal rules, front matter, task checkboxes. */
function cmMarkdownDecorations() {
  const CM = window.ReadMDCodeMirror;
  if (!CM.ViewPlugin || !CM.Decoration || !CM.syntaxTree) return [];
  const D = CM.Decoration;
  const lineDeco = {};
  const line = cls => lineDeco[cls] || (lineDeco[cls] = D.line({ class: cls }));
  class TaskBox extends CM.WidgetType {
    constructor(checked) { super(); this.checked = checked; }
    eq(o) { return o.checked === this.checked; }
    toDOM() {
      const el = document.createElement('span');
      el.className = 'cm-task-box' + (this.checked ? ' is-checked' : '');
      el.setAttribute('aria-hidden', 'true');
      return el;
    }
    ignoreEvent() { return false; }
  }
  const CALLOUT_RE = /^\s{0,3}>\s?\[!(note|tip|important|warning|caution|info|success|danger|abstract|question|quote|example|bug)\][+-]?/i;
  function build(view) {
    const b = new CM.RangeSetBuilder();
    const doc = view.state.doc;
    const tree = CM.syntaxTree(view.state);
    const marks = [];
    for (const { from, to } of view.visibleRanges) {
      tree.iterate({
        from, to,
        enter: node => {
          const n = node.name;
          if (n === 'FencedCode' || n === 'CodeBlock') {
            const first = doc.lineAt(node.from).number, last = doc.lineAt(node.to).number;
            for (let i = first; i <= last; i++) {
              const l = doc.line(i);
              const cls = 'cm-md-code' + (i === first ? ' cm-md-code-first' : '') + (i === last ? ' cm-md-code-last' : '');
              marks.push([l.from, line(cls)]);
            }
            return false;
          }
          if (/^(ATX|Setext)Heading(\d)$/.test(n)) {
            const lvl = n.slice(-1);
            marks.push([doc.lineAt(node.from).from, line('cm-md-h cm-md-h' + lvl)]);
          } else if (n === 'Blockquote') {
            const first = doc.lineAt(node.from);
            const callout = CALLOUT_RE.exec(first.text);
            const kind = callout ? callout[1].toLowerCase() : '';
            const last = doc.lineAt(node.to).number;
            for (let i = first.number; i <= last; i++) {
              const l = doc.line(i);
              const cls = (kind ? 'cm-md-callout cm-md-callout-' + kind : 'cm-md-quote') + (i === first.number ? ' cm-md-quote-first' : '') + (i === last ? ' cm-md-quote-last' : '');
              marks.push([l.from, line(cls)]);
            }
            return false;
          } else if (n === 'Table') {
            const first = doc.lineAt(node.from).number, last = doc.lineAt(node.to).number;
            for (let i = first; i <= last; i++) marks.push([doc.line(i).from, line('cm-md-table' + (i === first ? ' cm-md-table-head' : ''))]);
          } else if (n === 'HorizontalRule') {
            marks.push([doc.lineAt(node.from).from, line('cm-md-hr')]);
          } else if (n === 'TaskMarker') {
            const checked = /x/i.test(doc.sliceString(node.from, node.to));
            marks.push([node.from, D.replace({ widget: new TaskBox(checked) }), node.to]);
            const l = doc.lineAt(node.from);
            if (checked) marks.push([l.from, line('cm-md-task-done')]);
          }
        },
      });
    }
    // Front matter (--- ... --- at the very top)
    if (doc.lines > 1 && doc.line(1).text === '---' && view.visibleRanges.length && view.visibleRanges[0].from === 0) {
      for (let i = 2; i <= Math.min(doc.lines, 200); i++) {
        if (/^(---|\.\.\.)\s*$/.test(doc.line(i).text)) {
          for (let k = 1; k <= i; k++) marks.push([doc.line(k).from, line('cm-md-frontmatter')]);
          break;
        }
      }
    }
    marks.sort((x, y) => x[0] - y[0] || (x[2] === undefined ? -1 : 1) - (y[2] === undefined ? -1 : 1));
    let lastLine = -1, lastCls = '';
    for (const m of marks) {
      if (m[2] === undefined) {
        // Merge multiple line classes on the same line into one decoration.
        if (m[0] === lastLine) { lastCls += ' ' + m[1].spec.class; continue; }
        if (lastLine >= 0) b.add(lastLine, lastLine, line(lastCls));
        lastLine = m[0]; lastCls = m[1].spec.class;
      } else {
        if (lastLine >= 0 && lastLine <= m[0]) { b.add(lastLine, lastLine, line(lastCls)); lastLine = -1; }
        b.add(m[0], m[2], m[1]);
      }
    }
    if (lastLine >= 0) b.add(lastLine, lastLine, line(lastCls));
    return b.finish();
  }
  return CM.ViewPlugin.fromClass(class {
    constructor(view) { this.decorations = build(view); }
    update(u) {
      if (u.docChanged || u.viewportChanged || CM.syntaxTree(u.startState) !== CM.syntaxTree(u.state)) this.decorations = build(u.view);
    }
  }, { decorations: v => v.decorations });
}

/* Click on a rendered task checkbox toggles [ ] / [x] (one undo step). */
function cmTaskMarkerClick(e, view) {
  const box = e.target && e.target.closest && e.target.closest('.cm-task-box');
  if (!box) return false;
  const pos = view.posAtDOM(box);
  const text = view.state.sliceDoc(pos, pos + 3);
  if (!/^\[[ xX]\]$/.test(text)) return false;
  e.preventDefault();
  view.dispatch({ changes: { from: pos + 1, to: pos + 2, insert: text[1] === ' ' ? 'x' : ' ' }, userEvent: 'input.toggle' });
  return true;
}

/* Gutter: hidden by default (prose). Line numbers + folding appear when
   the user turns them on. */
function cmGutterExtension() {
  const CM = window.ReadMDCodeMirror;
  if (!editorPrefs.lineNumbers) return [];
  return [CM.lineNumbers(), CM.highlightActiveLineGutter(), CM.foldGutter({ openText: '⌄', closedText: '›' })];
}

/* Focus mode: every paragraph except the one holding the cursor is dimmed. */
function cmFocusExtension() {
  const CM = window.ReadMDCodeMirror;
  const dim = CM.Decoration.line({ class: 'cm-focus-dim' });
  function build(view) {
    const b = new CM.RangeSetBuilder();
    const doc = view.state.doc;
    const head = view.state.selection.main.head;
    const cur = doc.lineAt(head).number;
    let a = cur, z = cur;
    const blank = n => !doc.line(n).text.trim();
    if (!blank(cur)) {
      while (a > 1 && !blank(a - 1)) a--;
      while (z < doc.lines && !blank(z + 1)) z++;
    }
    for (const { from, to } of view.visibleRanges) {
      for (let pos = from; pos <= to;) {
        const l = doc.lineAt(pos);
        if (l.number < a || l.number > z) b.add(l.from, l.from, dim);
        pos = l.to + 1;
      }
    }
    return b.finish();
  }
  return CM.ViewPlugin.fromClass(class {
    constructor(view) { this.decorations = build(view); }
    update(u) { if (u.docChanged || u.selectionSet || u.viewportChanged) this.decorations = build(u.view); }
  }, { decorations: v => v.decorations });
}

/* Typewriter mode: the cursor line stays at the vertical centre. */
function cmTypewriterExtension() {
  const CM = window.ReadMDCodeMirror;
  return [
    CM.EditorView.scrollMargins.of(view => {
      const h = view.scrollDOM.clientHeight;
      return { top: h * 0.45, bottom: h * 0.45 };
    }),
    CM.EditorView.updateListener.of(u => {
      if (!u.selectionSet || !u.view.hasFocus) return;
      if (!u.transactions.some(tr => tr.isUserEvent('input') || tr.isUserEvent('delete') || tr.isUserEvent('select') || tr.isUserEvent('move'))) return;
      const head = u.state.selection.main.head;
      requestAnimationFrame(() => {
        if (!cmView || cmView !== u.view) return;
        u.view.dispatch({ effects: CM.EditorView.scrollIntoView(head, { y: 'center' }) });
      });
    }),
  ];
}

function applyEditorViewClasses() {
  const wrap = $('edit-wrap');
  if (wrap) {
    wrap.classList.toggle('has-gutter', !!editorPrefs.lineNumbers);
    wrap.classList.toggle('is-focus-mode', !!editorPrefs.focus);
    wrap.classList.toggle('is-typewriter', !!editorPrefs.typewriter);
  }
  const map = { lineNumbers: 'edit-view-lines', focus: 'edit-view-focus', typewriter: 'edit-view-typewriter' };
  Object.entries(map).forEach(([k, id]) => {
    const el = $(id);
    if (el) el.setAttribute('aria-checked', editorPrefs[k] ? 'true' : 'false');
  });
}

function setEditorPref(key, value) {
  editorPrefs[key] = value === undefined ? !editorPrefs[key] : !!value;
  saveEditorPrefs();
  applyEditorViewClasses();
  if (!cmView || !cmCompartments) return;
  const effects = [];
  if (key === 'lineNumbers') effects.push(cmCompartments.gutter.reconfigure(cmGutterExtension()));
  if (key === 'focus') effects.push(cmCompartments.focus.reconfigure(editorPrefs.focus ? cmFocusExtension() : []));
  if (key === 'typewriter') {
    effects.push(cmCompartments.typewriter.reconfigure(editorPrefs.typewriter ? cmTypewriterExtension() : []));
    if (editorPrefs.typewriter) effects.push(window.ReadMDCodeMirror.EditorView.scrollIntoView(cmView.state.selection.main.head, { y: 'center' }));
  }
  cmView.dispatch({ effects });
}
window.setEditorPref = setEditorPref;

function applyCmTheme() {
  if (!cmView || !window.ReadMDCodeMirror || !cmThemeCompartment) return;
  cmView.dispatch({ effects: cmThemeCompartment.reconfigure(cmThemeFor(document.body.dataset.theme)) });
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
  switch (kind) {
    case 'codechunk': openCodeChunkModal(); return;
    case 'diagram': openDiagramModal(); return;
    case 'docimport': openDocImportModal(); return;
    case 'frontmatter': insertFrontmatterTemplate(); return;
    default: break;
  }
  if (!window.ReadMDTransforms) return;
  const sel = cmView.state.selection.main;
  const ph = {
    text: _t('editor.textWord') || 'text',
    code: _t('editor.codeWord') || 'code',
    heading: _t('editor.headingWord') || 'Heading',
    quote: _t('editor.quote') || 'Quote',
    item: _t('editor.itemWord') || 'Item',
    task: _t('editor.taskWord') || 'Task',
    desc: _t('editor.descWord') || 'image',
  };
  const e = window.ReadMDTransforms.computeSyntaxEdit(cmView.state.doc.toString(), sel.from, sel.to, kind, ph);
  if (!e) return;
  // 一次 dispatch = 一步撤销
  cmView.dispatch({ changes: e.changes, selection: e.selection, scrollIntoView: true, userEvent: 'input.syntax' });
  cmView.focus();
}


function closeMdPopups() {
  document.querySelectorAll('.md-menu, .pv-menu').forEach(el => el.classList.add('hidden'));
  const trigger = $('pv-trigger'); if (trigger) trigger.setAttribute('aria-expanded', 'false');
  document.querySelectorAll('#edit-bar [data-menu]').forEach(b => b.setAttribute('aria-expanded', 'false'));
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
  chart: '{\n  "type": "bar",\n  "data": {\n    "labels": ["A", "B", "C", "D"],\n    "datasets": [{ "label": "ReadMD", "data": [28, 55, 43, 91] }]\n  },\n  "options": { "responsive": true, "plugins": { "legend": { "display": true } } }\n}',
  graphviz: 'digraph G {\n  rankdir=LR;\n  node [shape=box, style=rounded];\n  Start -> Process -> Decision;\n  Decision -> Success [label="是"];\n  Decision -> Failure [label="否"];\n}',
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

/* 实时文档统计与阅读时长：words (CJK characters count as words), visible
   characters, reading time; a selection shows its own word count. */
let docStatsTimer = null;
function scheduleDocStatistics() {
  if (docStatsTimer) return;
  docStatsTimer = setTimeout(() => { docStatsTimer = null; updateDocStatistics(); }, 160);
}

function updateDocStatistics() {
  const statsEl = $('edit-doc-stats');
  if (!statsEl) return;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const T = window.ReadMDTransforms;
  const docText = typeof getEditContent === 'function' ? getEditContent() : (cmView ? cmView.state.doc.toString() : ($('edit-area') && $('edit-area').value || ''));
  const stats = T && T.textStats ? T.textStats(docText) : { words: 0, chars: docText.length, minutes: 0 };
  const nf = n => { try { return Number(n).toLocaleString((window.i18n && window.i18n.locale) || undefined); } catch (e) { return String(n); } };
  let selWords = 0;
  if (cmView && T && T.textStats) {
    const ranges = cmView.state.selection.ranges.filter(r => !r.empty);
    if (ranges.length) selWords = T.textStats(ranges.map(r => cmView.state.sliceDoc(r.from, r.to)).join('\n')).words;
  }
  const params = { words: nf(stats.words), chars: nf(stats.chars), min: nf(Math.max(stats.minutes, stats.words ? 1 : 0)), total: nf(stats.words) };
  statsEl.textContent = selWords
    ? _t('editor.statsSelection', { words: nf(selWords), total: params.total })
    : _t('editor.statsLine', params);
  statsEl.title = _t('editor.statsLine', params);
}


/* 智能 Excel / CSV 粘贴转 Markdown 表格 */
function handleSmartExcelPaste(e) {
  if (!e.clipboardData) return;

  // 0. 剪贴板图片粘贴 -> 自动上传并插入 Markdown 语法
  const items = e.clipboardData.items;
  let imageFile = null;
  if (items && items.length > 0) {
    for (let i = 0; i < items.length; i++) {
      if (items[i].type && items[i].type.startsWith('image/')) {
        imageFile = items[i].getAsFile();
        break;
      }
    }
  }
  if (!imageFile && e.clipboardData.files && e.clipboardData.files.length > 0) {
    for (let i = 0; i < e.clipboardData.files.length; i++) {
      const f = e.clipboardData.files[i];
      if (f.type && f.type.startsWith('image/')) {
        imageFile = f;
        break;
      }
    }
  }
  if (imageFile) {
    e.preventDefault();
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    showToast(_t('toast.savingImage') || '正在保存剪贴板图片…', 1500);
    const reader = new FileReader();
    reader.onload = async () => {
      const b64 = String(reader.result).split(',')[1] || '';
      try {
        const resp = await apiFetch('/api/image/save', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            dir: state.dir || '',
            data: b64,
            format: 'png',
            name: 'img_' + Date.now()
          })
        });
        const d = await resp.json();
        if (d && d.ok) {
          const insertRel = d.rel || d.relPath || d.path;
          cmInsertImage(insertRel);
          showToast(_t('toast.imgInsertedRel', { rel: insertRel }) || ('图片已插入（' + insertRel + '）'));
        } else {
          showToast(apiMessage(d, 'toast.imgSaveFailed') || '图片保存失败');
        }
      } catch (err) {
        showToast((_t('toast.imgSaveFail') || '图片保存失败：') + err.message);
      }
    };
    reader.readAsDataURL(imageFile);
    return;
  }

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

let editAiSnapshot = null;

function switchEditAiToChatPanel() {
  const bar = $('edit-ai-bar');
  if (bar && !bar.classList.contains('hidden') && bar.offsetParent !== null && state.editing) {
    const input = $('edit-ai-input');
    const promptText = (input && input.value.trim()) || (editAiSelectionRange && editAiSelectionRange.text) || '';
    closeEditAiBar();
    const aiPanel = $('ai-panel');
    if (aiPanel && aiPanel.classList.contains('hidden')) {
      if (typeof toggleAiPanel === 'function') toggleAiPanel();
    }
    if (promptText && $('ai-prompt')) {
      $('ai-prompt').value = promptText;
      setTimeout(() => {
        if ($('ai-prompt')) $('ai-prompt').focus();
      }, 50);
    }
  }
}
window.switchEditAiToChatPanel = switchEditAiToChatPanel;

async function runEditAiAction(act, customPrompt = '') {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const preview = $('edit-ai-preview');
  const previewContent = $('edit-ai-preview-content');
  const statusEl = $('edit-ai-status');
  if (preview) preview.classList.remove('hidden');
  if (previewContent) previewContent.innerHTML = '';
  if (statusEl) statusEl.textContent = _t('editai.generating') || '';
  const submitBtn = $('edit-ai-submit');
  const chips = document.querySelectorAll('.edit-ai-act-chip');
  if (submitBtn) { submitBtn.disabled = true; submitBtn.classList.add('btn-loading'); }
  chips.forEach(c => { c.disabled = true; });

  const currentDocStr = cmView ? cmView.state.doc.toString() : '';
  const range = editAiSelectionRange || {
    from: cmView ? cmView.state.selection.main.from : 0,
    to: cmView ? cmView.state.selection.main.to : 0,
    text: cmView ? cmView.state.sliceDoc(cmView.state.selection.main.from, cmView.state.selection.main.to) : ''
  };
  editAiSnapshot = {
    docText: currentDocStr,
    range: { ...range },
    hadSelection: range.from !== range.to && Boolean(range.text)
  };
  const skillByAction = { complete: 'readmd-continue', polish: 'readmd-polish', fix: 'readmd-format-fix', translate: 'readmd-translate' };
  const skillId = skillByAction[act] || 'readmd-polish';
  let userMessage = '';
  const sourceText = range.text || currentDocStr;

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
  } finally {
    if (submitBtn) { submitBtn.disabled = false; submitBtn.classList.remove('btn-loading'); }
    chips.forEach(c => { c.disabled = false; });
  }
}

function applyEditAiResult() {
  if (!cmView || !editAiCurrentResult) {
    closeEditAiBar();
    return;
  }
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const currentDoc = cmView.state.doc.toString();
  let from = 0, to = 0;

  if (editAiSnapshot && editAiSnapshot.hadSelection) {
    const origText = editAiSnapshot.range.text;
    const snapFrom = editAiSnapshot.range.from;
    const snapTo = editAiSnapshot.range.to;
    if (cmView.state.sliceDoc(snapFrom, snapTo) === origText) {
      from = snapFrom;
      to = snapTo;
    } else {
      const firstIdx = currentDoc.indexOf(origText);
      const secondIdx = firstIdx >= 0 ? currentDoc.indexOf(origText, firstIdx + 1) : -1;
      if (firstIdx >= 0 && secondIdx < 0) {
        from = firstIdx;
        to = firstIdx + origText.length;
      } else {
        showToast(_t('toast.appliedSelectionFallback') || '原选区无法唯一定位，已安全创建副本');
        if (typeof renderVirtual === 'function' && typeof getNextAiCopyTabName === 'function') {
          const cleanName = getNextAiCopyTabName(state.sourceName || state.file);
          renderVirtual('ai', cleanName, state.dir || '', editAiCurrentResult, [], { originPath: state.file });
        }
        closeEditAiBar();
        return;
      }
    }
  } else {
    const curSel = cmView.state.selection.main;
    from = curSel ? curSel.from : 0;
    to = curSel ? curSel.to : 0;
  }

  cmView.dispatch({
    changes: { from, to, insert: editAiCurrentResult },
    selection: { anchor: from + editAiCurrentResult.length },
    scrollIntoView: true
  });
  closeEditAiBar();
  showToast(_t('toast.appliedSavedNotice') || '已应用到正文（可按 Ctrl+Z 撤回，Ctrl+S 保存）');
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
  if (cmSelAi) cmSelAi.addEventListener('click', () => cmRequestInlineAi('selection-toolbar'));
  bindCmSelectionToolbar();
  bindEditBarExtras();
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

/* ============================================================
   Smart editing: priority keymap, input rules, paste, tables, lists
   ============================================================ */

const CM_CODE_NODE_RE = /^(FencedCode|CodeBlock|InlineCode|CodeText|HTMLBlock|CommentBlock|Comment|ProcessingInstructionBlock)$/;

function cmInCode(state, pos) {
  const CM = window.ReadMDCodeMirror;
  if (!CM || !CM.syntaxTree) return false;
  for (let n = CM.syntaxTree(state).resolveInner(pos, -1); n; n = n.parent) {
    if (CM_CODE_NODE_RE.test(n.name)) return true;
  }
  return false;
}

function cmDispatchEdit(view, e, userEvent) {
  if (!e) return false;
  view.dispatch({ changes: e.changes, selection: e.selection, scrollIntoView: true, userEvent: userEvent || 'input' });
  return true;
}

function cmSingleCursorLine(view) {
  const ranges = view.state.selection.ranges;
  if (ranges.length !== 1) return null;
  const r = ranges[0];
  const line = view.state.doc.lineAt(r.head);
  if (r.from < line.from || r.to > line.to) return null;
  return { range: r, line };
}

/* Tab / Shift+Tab inside a GFM table: realign pipes and jump between cells. */
function cmTableTab(view, dir) {
  const T = window.ReadMDTransforms;
  const hit = cmSingleCursorLine(view);
  if (!T || !hit || cmInCode(view.state, hit.range.head)) return false;
  return cmDispatchEdit(view, T.tableNavEdit(view.state.doc.toString(), hit.range.head, dir), 'input.table');
}

/* Enter inside a table adds a row; Enter on an empty last row leaves it. */
function cmTableEnter(view) {
  const T = window.ReadMDTransforms;
  const hit = cmSingleCursorLine(view);
  if (!T || !hit || !hit.range.empty || cmInCode(view.state, hit.range.head)) return false;
  return cmDispatchEdit(view, T.tableEnterEdit(view.state.doc.toString(), hit.range.head), 'input.table');
}

/* Enter at the end of an unclosed opening fence (```js) closes the block
   and puts the cursor inside it. */
function cmFenceEnter(view) {
  const hit = cmSingleCursorLine(view);
  if (!hit || !hit.range.empty || hit.range.head !== hit.line.to) return false;
  const m = /^(\s{0,3})(`{3,}|~{3,})([^`\s]*)\s*$/.exec(hit.line.text);
  if (!m) return false;
  const doc = view.state.doc;
  const fenceRe = /^\s{0,3}(`{3,}|~{3,})/;
  let before = 0, total = 0;
  for (let i = 1; i <= doc.lines; i++) {
    if (fenceRe.test(doc.line(i).text)) { total++; if (i < hit.line.number) before++; }
  }
  if (before % 2 !== 0 || total % 2 === 0) return false;
  const next = hit.line.number < doc.lines ? doc.line(hit.line.number + 1).text : '';
  if (next.trim()) return false;
  const insert = '\n' + m[1] + '\n' + m[1] + m[2];
  view.dispatch({ changes: { from: hit.line.to, insert }, selection: { anchor: hit.line.to + 1 + m[1].length }, scrollIntoView: true, userEvent: 'input' });
  return true;
}

const CM_LIST_LINE_RE = /^([ \t]*)([-*+]|\d{1,9}[.)])([ \t]+)(\[[ xX]\][ \t]+)?/;

/* Enter on an empty top-level list item leaves the list with a blank line,
   so the next paragraph is not a lazy continuation of the last item.
   Nested empty items are handled by CodeMirror (they outdent one level). */
function cmListExit(view) {
  const hit = cmSingleCursorLine(view);
  if (!hit || !hit.range.empty || hit.range.head !== hit.line.to) return false;
  if (!/^([-*+]|\d{1,9}[.)])([ \t]+\[[ xX]\])?[ \t]*$/.test(hit.line.text)) return false;
  if (hit.line.number < 2 || cmInCode(view.state, hit.line.from)) return false;
  const prev = view.state.doc.line(hit.line.number - 1).text;
  if (!prev.trim()) return false;
  view.dispatch({
    changes: { from: hit.line.from, to: hit.line.to, insert: '\n' },
    selection: { anchor: hit.line.from + 1 },
    scrollIntoView: true,
    userEvent: 'input',
  });
  return true;
}

/* Tab / Shift+Tab on list items nests / un-nests them by the width of the
   parent marker, so "1. " children line up with the parent's text. */
function cmListIndent(view, dir) {
  const { state } = view;
  const doc = state.doc;
  const lines = [];
  for (const r of state.selection.ranges) {
    const a = doc.lineAt(r.from).number, b = doc.lineAt(r.to).number;
    for (let i = a; i <= b; i++) if (!lines.includes(i)) lines.push(i);
  }
  lines.sort((x, y) => x - y);
  if (!lines.length || !lines.every(n => CM_LIST_LINE_RE.test(doc.line(n).text))) return false;
  if (lines.some(n => cmInCode(state, doc.line(n).to))) return false;
  const changes = [];
  for (const n of lines) {
    const line = doc.line(n);
    const m = CM_LIST_LINE_RE.exec(line.text);
    const ind = m[1].length;
    if (dir > 0) {
      let width = 2;
      for (let k = n - 1; k >= 1 && k >= n - 400; k--) {
        const t = doc.line(k).text;
        if (!t.trim()) continue;
        const pm = CM_LIST_LINE_RE.exec(t);
        if (!pm) { if (/^\s/.test(t)) continue; break; }
        if (pm[1].length === ind) { width = pm[2].length + pm[3].length; break; }
        if (pm[1].length < ind) break;
      }
      changes.push({ from: line.from, insert: ' '.repeat(width) });
      // A nested numbered list restarts at 1.
      if (/^\d/.test(m[2]) && m[2].slice(0, -1) !== '1') {
        changes.push({ from: line.from + ind, to: line.from + ind + m[2].length - 1, insert: '1' });
      }
    } else if (ind) {
      let target = 0;
      for (let k = n - 1; k >= 1 && k >= n - 400; k--) {
        const t = doc.line(k).text;
        if (!t.trim()) continue;
        const pm = CM_LIST_LINE_RE.exec(t);
        if (pm && pm[1].length < ind) { target = pm[1].length; break; }
        if (!pm && !/^\s/.test(t)) break;
      }
      changes.push({ from: line.from, to: line.from + (ind - target) });
    }
  }
  if (changes.length) view.dispatch({ changes, scrollIntoView: true, userEvent: dir > 0 ? 'input.indent' : 'delete.dedent' });
  return true;
}

function cmPriorityKeymap() {
  const CM = window.ReadMDCodeMirror;
  // nonTightLists:false — Enter on an empty item always leaves the list.
  const continueList = CM.insertNewlineContinueMarkupCommand
    ? CM.insertNewlineContinueMarkupCommand({ nonTightLists: false })
    : null;
  return [
    { key: 'ArrowDown', run: v => slashKey(v, 'down') },
    { key: 'ArrowUp', run: v => slashKey(v, 'up') },
    { key: 'ArrowLeft', run: v => slashKey(v, 'left') },
    { key: 'ArrowRight', run: v => slashKey(v, 'right') },
    { key: 'PageDown', run: v => slashKey(v, 'pagedown') },
    { key: 'PageUp', run: v => slashKey(v, 'pageup') },
    { key: 'Escape', run: v => slashKey(v, 'escape'), stopPropagation: true },
    { key: 'Enter', run: v => slashKey(v, 'enter') || cmTableEnter(v) || cmFenceEnter(v) || cmListExit(v) || (continueList ? continueList(v) : false) },
    { key: 'Tab', run: v => slashKey(v, 'enter') || cmTableTab(v, 1) || cmListIndent(v, 1) },
    { key: 'Shift-Tab', run: v => cmTableTab(v, -1) || cmListIndent(v, -1) },
  ];
}

/* Typed-character rules: "/" opens the block menu, "**" / "~~" pair up,
   typing over a closing mark steps over it, a third backtick opens a fence. */
function cmSmartInput(view, from, to, text) {
  const { state } = view;
  if (view.composing || state.selection.ranges.length !== 1) return false;
  const sel = state.selection.main;
  if (sel.from !== from || sel.to !== to) return false;
  if (text === '/' && from === to) {
    const line = state.doc.lineAt(from);
    const before = state.doc.sliceString(line.from, from);
    if ((before === '' || /\s$/.test(before)) && !cmInCode(state, from)) {
      view.dispatch({ changes: { from, insert: '/' }, selection: { anchor: from + 1 }, userEvent: 'input.type' });
      openSlashMenu(view, from);
      return true;
    }
    return false;
  }
  if (text === '*' || text === '~') return cmEmphasisInput(view, from, to, text);
  if (text === '`' && from === to) return cmBacktickInput(view, from);
  return false;
}

function cmEmphasisInput(view, from, to, ch) {
  const CM = window.ReadMDCodeMirror;
  const { state } = view;
  if (cmInCode(state, from)) return false;
  if (from !== to) {
    // Wrap the selection; typing the mark again wraps again (* -> **).
    view.dispatch({
      changes: [{ from, insert: ch }, { from: to, insert: ch }],
      selection: { anchor: from + 1, head: to + 1 },
      userEvent: 'input.type',
    });
    return true;
  }
  const next = state.sliceDoc(from, from + 1);
  // Step over a closing emphasis mark instead of doubling it.
  if (next === ch && CM.syntaxTree) {
    const node = CM.syntaxTree(state).resolveInner(from, 1);
    const markName = ch === '*' ? 'EmphasisMark' : 'StrikethroughMark';
    if (node && node.name === markName && node.from <= from && from < node.to && node.parent && node.parent.lastChild &&
        node.parent.lastChild.from === node.from) {
      view.dispatch({ selection: { anchor: from + 1 }, userEvent: 'select' });
      return true;
    }
  }
  // Second mark typed after text: "word **" -> "word **|**".
  const line = state.doc.lineAt(from);
  const prev = state.sliceDoc(from - 1, from);
  const prev2 = from - 2 >= line.from ? state.sliceDoc(from - 2, from - 1) : '';
  const lead = state.sliceDoc(line.from, Math.max(line.from, from - 1));
  if (prev === ch && prev2 !== ch && lead.trim() && (prev2 === '' || /[\s([{"'“‘（【「《]/.test(prev2)) &&
      (next === '' || /[\s)\]}.,;:!?"'”’）】」》，。；：！？]/.test(next))) {
    view.dispatch({ changes: { from, insert: ch + ch + ch }, selection: { anchor: from + 1 }, userEvent: 'input.type' });
    return true;
  }
  return false;
}

function cmBacktickInput(view, from) {
  const { state } = view;
  const line = state.doc.lineAt(from);
  const before = state.sliceDoc(line.from, from);
  const after = state.sliceDoc(from, line.to);
  if (!/^\s{0,3}``$/.test(before) || !/^`*$/.test(after)) return false;
  view.dispatch({ changes: { from, to: line.to, insert: '`' }, selection: { anchor: from + 1 }, userEvent: 'input.type' });
  return true;
}

/* Paste: images are saved next to the document, tab-separated cells become
   a table (both via handleSmartExcelPaste); a URL pasted over a selection
   becomes a link (CodeMirror's pasteURLAsLink handles that next). */
function cmHandlePaste(event, view) {
  if (view !== cmView) return false;
  handleSmartExcelPaste(event);
  return event.defaultPrevented;
}

/* ============================================================
   Slash command menu ("/" at line start or after a space)
   ============================================================ */

const slash = {
  open: false, view: null, el: null, from: 0, prefix: '/', mode: 'blocks',
  query: '', items: [], index: 0, cols: 3, rows: 3, lastHitLen: 0, suppress: false,
};

const SLASH_SVG = {
  text: '<path d="M5 7V5h14v2M12 5v14M9 19h6"/>',
  bullet: '<path d="M10 6h10M10 12h10M10 18h10M4.5 6h.01M4.5 12h.01M4.5 18h.01"/>',
  numbered: '<path d="M11 6h9M11 12h9M11 18h9M4 5h1.5v4M4 9h3M7 19H4c0-1.3 3-2 3-3.3 0-.8-.7-1.2-1.5-1.2S4 15 4 15.5"/>',
  task: '<rect x="3" y="4" width="7" height="7" rx="1.5"/><path d="m3.5 17.5 2 2 4-4.5M14 7.5h7M14 17.5h7"/>',
  quote: '<path d="M4 6v12M9 8h11M9 12h11M9 16h7"/>',
  divider: '<path d="M3 12h18"/><path d="M7 7h10M7 17h10" opacity=".45"/>',
  callout: '<path d="M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v10a1.5 1.5 0 0 1-1.5 1.5H9l-5 3.5z"/><path d="M12 8v3.5M12 14h.01"/>',
  code: '<path d="m15.5 17 5-5-5-5M8.5 7l-5 5 5 5"/>',
  table: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9.5h18M3 15h18M9.5 4v16"/>',
  math: '<path d="M18 5H6.5l6 7-6 7H18"/>',
  mermaid: '<rect x="3" y="3" width="7" height="6" rx="1.5"/><rect x="14" y="15" width="7" height="6" rx="1.5"/><path d="M6.5 9v3.5a2.5 2.5 0 0 0 2.5 2.5h5"/>',
  image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="1.8"/><path d="m20.5 16.5-4.8-4.8L7 20"/>',
  toc: '<path d="M4 6h.01M4 12h.01M4 18h.01M8 6h12M11 12h9M11 18h9"/>',
  date: '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M16 3v4M8 3v4M3 10h18"/>',
  time: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>',
  footnote: '<path d="M4 8h10M4 13h10M4 18h6M17.5 4v5.5M16 5.2l1.5-1.2"/>',
  lang: '<path d="m15.5 17 5-5-5-5M8.5 7l-5 5 5 5"/>',
};

function slashT(k, p) { return window.i18n ? window.i18n.t(k, p) : k; }

function slashPad(n) { return String(n).padStart(2, '0'); }

function slashCatalog() {
  const T = window.ReadMDTransforms;
  const line = prefix => (doc, from, to) => T.linePrefixEdit(doc, from, to, prefix);
  const block = (text, a, b) => (doc, from, to) => T.blockInsertEdit(doc, from, to, text, a, b);
  const inline = (textFn, a, b) => (doc, from, to) => {
    const text = typeof textFn === 'function' ? textFn() : textFn;
    return T.inlineInsertEdit(doc, from, to, text, typeof a === 'function' ? a(text) : a, typeof b === 'function' ? b(text) : b);
  };
  const callout = type => {
    const head = '> [!' + type + ']\n> ';
    return block(head, head.length);
  };
  const mermaid = '```mermaid\nflowchart LR\n  A[Start] --> B{Ready?}\n  B -->|Yes| C[Ship it]\n  B -->|No| D[Iterate]\n```';
  const mermaidBody = [mermaid.indexOf('\n') + 1, mermaid.lastIndexOf('\n')];
  const desc = slashT('editor.descWord') || 'image';
  const items = [
    { id: 'text', sec: 'basic', icon: 'text', kw: ['paragraph', 'plain', 'body', 'p', '正文', '文本', '段落'], run: line('') },
    { id: 'h1', sec: 'basic', tile: 'H1', hint: '#', kw: ['h1', 'heading', 'title', '#', '标题', '一级标题'], run: line('# ') },
    { id: 'h2', sec: 'basic', tile: 'H2', hint: '##', kw: ['h2', 'heading', 'subtitle', '##', '标题', '二级标题'], run: line('## ') },
    { id: 'h3', sec: 'basic', tile: 'H3', hint: '###', kw: ['h3', 'heading', '###', '标题', '三级标题'], run: line('### ') },
    { id: 'bullet', sec: 'basic', icon: 'bullet', hint: '-', kw: ['ul', 'unordered', 'list', 'bullets', '-', '无序列表', '列表'], run: line('- ') },
    { id: 'numbered', sec: 'basic', icon: 'numbered', hint: '1.', kw: ['ol', 'ordered', 'list', 'numbers', '1.', '有序列表'], run: line('1. ') },
    { id: 'task', sec: 'basic', icon: 'task', hint: '[ ]', kw: ['todo', 'checkbox', 'check', 'checklist', '[]', '任务', '待办'], run: line('- [ ] ') },
    { id: 'quote', sec: 'basic', icon: 'quote', hint: '>', kw: ['blockquote', 'cite', '>', '引用'], run: line('> ') },
    { id: 'divider', sec: 'basic', icon: 'divider', hint: '---', kw: ['hr', 'rule', 'line', 'separator', '---', '分隔线', '分割线'], run: block('---\n', 4) },
    { id: 'code', sec: 'insert', icon: 'code', hint: '```', kw: ['codeblock', 'fence', 'snippet', 'pre', '```', '代码块', '代码'], next: 'lang' },
    { id: 'table', sec: 'insert', icon: 'table', hint: '|', kw: ['grid', 'rows', 'columns', '|', '表格'], next: 'table' },
    { id: 'image', sec: 'insert', icon: 'image', hint: '![]', kw: ['picture', 'photo', 'img', 'figure', '![]', '图片', '插图'],
      run: inline('![' + desc + '](url)', t => t.length - 4, t => t.length - 1) },
    { id: 'math', sec: 'insert', icon: 'math', hint: '$$', kw: ['latex', 'formula', 'equation', 'katex', 'tex', '$$', '公式', '数学'], run: block('$$\n\n$$', 3) },
    { id: 'mermaid', sec: 'insert', icon: 'mermaid', kw: ['diagram', 'flowchart', 'chart', 'graph', 'sequence', 'gantt', '图表', '流程图'], run: block(mermaid, mermaidBody[0], mermaidBody[1]) },
    { id: 'calloutNote', sec: 'callout', icon: 'callout', tone: 'note', hint: '[!NOTE]', kw: ['callout', 'admonition', 'alert', 'note', 'info', '提示', '说明', '注'], run: callout('NOTE') },
    { id: 'calloutTip', sec: 'callout', icon: 'callout', tone: 'tip', hint: '[!TIP]', kw: ['callout', 'admonition', 'tip', 'hint', 'success', '技巧', '建议'], run: callout('TIP') },
    { id: 'calloutImportant', sec: 'callout', icon: 'callout', tone: 'important', hint: '[!IMPORTANT]', kw: ['callout', 'admonition', 'important', 'key', '重要'], run: callout('IMPORTANT') },
    { id: 'calloutWarning', sec: 'callout', icon: 'callout', tone: 'warning', hint: '[!WARNING]', kw: ['callout', 'admonition', 'warning', 'warn', 'attention', '警告', '注意'], run: callout('WARNING') },
    { id: 'calloutCaution', sec: 'callout', icon: 'callout', tone: 'caution', hint: '[!CAUTION]', kw: ['callout', 'admonition', 'caution', 'danger', 'error', '危险', '小心'], run: callout('CAUTION') },
    { id: 'toc', sec: 'more', icon: 'toc', hint: '[TOC]', kw: ['contents', 'outline', 'index', 'toc', '目录', '大纲'], run: slashTocEdit },
    { id: 'footnote', sec: 'more', icon: 'footnote', hint: '[^1]', kw: ['note', 'reference', 'ref', 'cite', '[^]', '脚注', '注释'],
      run: (doc, from, to) => T.footnoteEdit(doc, from, to) },
    { id: 'date', sec: 'more', icon: 'date', kw: ['today', 'day', 'now', 'calendar', '日期', '今天'],
      run: inline(() => { const d = new Date(); return d.getFullYear() + '-' + slashPad(d.getMonth() + 1) + '-' + slashPad(d.getDate()); }) },
    { id: 'time', sec: 'more', icon: 'time', kw: ['now', 'clock', 'hour', 'timestamp', '时间', '现在'],
      run: inline(() => { const d = new Date(); return slashPad(d.getHours()) + ':' + slashPad(d.getMinutes()); }) },
  ];
  items.forEach(it => {
    it.label = slashT('slash.' + it.id);
    it.desc = slashT('slash.' + it.id + 'Desc');
  });
  return items;
}

function slashTocEdit(doc, from, to) {
  const T = window.ReadMDTransforms;
  const toc = T.tocMarkdown(doc.slice(0, from) + doc.slice(to));
  if (!toc) {
    if (typeof showToast === 'function') showToast(slashT('slash.tocEmpty'), 2400);
    return { changes: { from, to, insert: '' }, selection: { anchor: from, head: from } };
  }
  return T.blockInsertEdit(doc, from, to, toc, toc.length);
}

const SLASH_LANGS = [
  ['javascript', 'JavaScript', ['js', 'node', 'jsx']], ['typescript', 'TypeScript', ['ts', 'tsx']],
  ['python', 'Python', ['py']], ['bash', 'Bash', ['sh', 'shell', 'zsh', 'terminal']], ['json', 'JSON', []],
  ['html', 'HTML', ['xml']], ['css', 'CSS', ['scss', 'style']], ['markdown', 'Markdown', ['md']],
  ['rust', 'Rust', ['rs']], ['go', 'Go', ['golang']], ['java', 'Java', []], ['c', 'C', []],
  ['cpp', 'C++', ['c++', 'cxx']], ['csharp', 'C#', ['cs', 'dotnet']], ['sql', 'SQL', ['mysql', 'postgres']],
  ['yaml', 'YAML', ['yml']], ['toml', 'TOML', ['ini']], ['diff', 'Diff', ['patch']], ['php', 'PHP', []],
  ['ruby', 'Ruby', ['rb']], ['swift', 'Swift', []], ['kotlin', 'Kotlin', ['kt']], ['latex', 'LaTeX', ['tex']],
  ['powershell', 'PowerShell', ['ps1', 'pwsh']], ['dockerfile', 'Dockerfile', ['docker']],
];

function slashLanguageItems(q) {
  const T = window.ReadMDTransforms;
  const query = q.trim().toLowerCase();
  const make = (lang, label) => ({
    id: 'lang-' + (lang || 'plain'), icon: 'lang', label, desc: lang ? '```' + lang : '```', lang,
    run: (doc, from, to) => T.blockInsertEdit(doc, from, to, '```' + lang + '\n\n```', 4 + lang.length),
  });
  const plain = make('', slashT('slash.langPlain'));
  if (!query) return [plain, ...SLASH_LANGS.map(([id, name]) => make(id, name))];
  const scored = SLASH_LANGS.map(([id, name, alias]) => ({
    it: make(id, name),
    s: Math.max(T.fuzzyScore(query, id), T.fuzzyScore(query, name), ...alias.map(a => (a === query ? 2000 : T.fuzzyScore(query, a)))),
  })).filter(x => x.s > 0).sort((a, b) => b.s - a.s).map(x => x.it);
  const exact = SLASH_LANGS.some(([id, , alias]) => id === query || alias.includes(query));
  if (!exact && /^[\w+#.-]+$/.test(query)) scored.push(make(query, slashT('slash.langUse', { lang: query })));
  return scored;
}

function slashFilter() {
  const T = window.ReadMDTransforms;
  const q = slash.query.trim();
  let list;
  if (slash.mode === 'lang') {
    list = slashLanguageItems(q);
  } else {
    const all = slashCatalog();
    list = !q ? all : all
      .map(it => ({ it, s: Math.max(T.fuzzyScore(q, it.label), ...it.kw.map(k => T.fuzzyScore(q, k) * 0.94)) }))
      .filter(x => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .map(x => x.it);
  }
  slash.items = list;
  if (list.length) slash.lastHitLen = slash.query.length;
  slash.index = 0;
}

function slashEnsureEl() {
  if (slash.el && slash.el.isConnected) return slash.el;
  const el = document.createElement('div');
  el.id = 'cm-slash-menu';
  el.className = 'cm-slash-menu hidden';
  // Keep focus (and the typed query) in the editor while using the mouse.
  el.addEventListener('mousedown', e => e.preventDefault());
  el.addEventListener('click', e => {
    const cell = e.target.closest('[data-cols]');
    if (cell) { slash.cols = +cell.dataset.cols; slash.rows = +cell.dataset.rows; slashInsertTable(); return; }
    const opt = e.target.closest('[data-slash-index]');
    if (opt) { slash.index = +opt.dataset.slashIndex; slashChoose(slash.items[slash.index]); }
  });
  el.addEventListener('mousemove', e => {
    const cell = e.target.closest('[data-cols]');
    if (cell) {
      const c = +cell.dataset.cols, r = +cell.dataset.rows;
      if (c !== slash.cols || r !== slash.rows) { slash.cols = c; slash.rows = r; slashSyncTable(); }
      return;
    }
    const opt = e.target.closest('[data-slash-index]');
    if (opt && +opt.dataset.slashIndex !== slash.index) { slash.index = +opt.dataset.slashIndex; slashSyncActive(false); }
  });
  document.body.appendChild(el);
  slash.el = el;
  return el;
}

function slashIcon(it) {
  const ic = document.createElement('span');
  ic.className = 'cm-slash-ic' + (it.tone ? ' is-' + it.tone : '') + (it.tile ? ' is-tile' : '');
  ic.setAttribute('aria-hidden', 'true');
  if (it.tile) ic.textContent = it.tile;
  else ic.innerHTML = '<svg viewBox="0 0 24 24">' + (SLASH_SVG[it.icon] || SLASH_SVG.text) + '</svg>';
  return ic;
}

function slashRender() {
  const el = slashEnsureEl();
  el.innerHTML = '';
  el.classList.toggle('is-table', slash.mode === 'table');
  if (slash.mode === 'table') { slashRenderTable(el); return; }
  el.setAttribute('role', 'listbox');
  el.setAttribute('aria-label', slashT(slash.mode === 'lang' ? 'slash.langTitle' : 'slash.menuLabel'));
  const scroll = document.createElement('div');
  scroll.className = 'cm-slash-scroll';
  const showSections = slash.mode !== 'lang' && !slash.query.trim();
  if (slash.mode === 'lang' || (!showSections && slash.items.length)) {
    const h = document.createElement('div');
    h.className = 'cm-slash-section';
    h.setAttribute('role', 'presentation');
    h.textContent = slashT(slash.mode === 'lang' ? 'slash.langTitle' : 'slash.sectionResults');
    scroll.appendChild(h);
  }
  if (!slash.items.length) {
    const empty = document.createElement('div');
    empty.className = 'cm-slash-empty';
    empty.setAttribute('role', 'presentation');
    empty.textContent = slashT('slash.noResults');
    scroll.appendChild(empty);
  }
  let sec = null, group = scroll;
  slash.items.forEach((it, i) => {
    if (showSections && it.sec !== sec) {
      sec = it.sec;
      group = document.createElement('div');
      group.setAttribute('role', 'group');
      const h = document.createElement('div');
      h.className = 'cm-slash-section';
      h.id = 'cm-slash-sec-' + sec;
      h.setAttribute('role', 'presentation');
      h.textContent = slashT('slash.section' + sec[0].toUpperCase() + sec.slice(1));
      group.setAttribute('aria-labelledby', h.id);
      group.appendChild(h);
      scroll.appendChild(group);
    }
    const opt = document.createElement('div');
    opt.className = 'cm-slash-item';
    opt.id = 'cm-slash-opt-' + i;
    opt.dataset.slashIndex = String(i);
    opt.setAttribute('role', 'option');
    opt.setAttribute('aria-selected', i === slash.index ? 'true' : 'false');
    opt.appendChild(slashIcon(it));
    const text = document.createElement('span');
    text.className = 'cm-slash-text';
    const label = document.createElement('span');
    label.className = 'cm-slash-label';
    label.textContent = it.label;
    text.appendChild(label);
    if (it.desc && it.desc !== 'slash.' + it.id + 'Desc') {
      const d = document.createElement('span');
      d.className = 'cm-slash-desc';
      d.textContent = it.desc;
      text.appendChild(d);
    }
    opt.appendChild(text);
    if (it.hint) {
      const k = document.createElement('kbd');
      k.className = 'cm-slash-kbd';
      k.textContent = it.hint;
      opt.appendChild(k);
    }
    group.appendChild(opt);
  });
  el.appendChild(scroll);
  const foot = document.createElement('div');
  foot.className = 'cm-slash-foot';
  foot.setAttribute('aria-hidden', 'true');
  foot.textContent = slashT('slash.hint');
  el.appendChild(foot);
  slashSyncActive(true);
}

const SLASH_GRID = 8;

function slashRenderTable(el) {
  el.setAttribute('role', 'group');
  el.setAttribute('aria-label', slashT('slash.tableTitle'));
  const head = document.createElement('div');
  head.className = 'cm-slash-table-head';
  const title = document.createElement('span');
  title.className = 'cm-slash-section';
  title.textContent = slashT('slash.tableTitle');
  const size = document.createElement('span');
  size.className = 'cm-slash-size';
  size.setAttribute('role', 'status');
  size.setAttribute('aria-live', 'polite');
  head.append(title, size);
  const grid = document.createElement('div');
  grid.className = 'cm-slash-grid';
  grid.setAttribute('aria-hidden', 'true');
  for (let r = 1; r <= SLASH_GRID; r++) {
    for (let c = 1; c <= SLASH_GRID; c++) {
      const cell = document.createElement('span');
      cell.className = 'cm-slash-cell' + (r === 1 ? ' is-head' : '');
      cell.dataset.cols = String(c);
      cell.dataset.rows = String(Math.max(2, r));
      grid.appendChild(cell);
    }
  }
  const foot = document.createElement('div');
  foot.className = 'cm-slash-foot';
  foot.setAttribute('aria-hidden', 'true');
  foot.textContent = slashT('slash.tableHint');
  el.append(head, grid, foot);
  slashSyncTable();
}

function slashSyncTable() {
  const el = slash.el;
  if (!el) return;
  el.querySelectorAll('.cm-slash-cell').forEach((cell, i) => {
    const r = Math.floor(i / SLASH_GRID) + 1, c = (i % SLASH_GRID) + 1;
    cell.classList.toggle('is-on', c <= slash.cols && r <= slash.rows);
  });
  const size = el.querySelector('.cm-slash-size');
  if (size) size.textContent = slashT('slash.tableSize', { cols: slash.cols, rows: slash.rows });
}

function slashSyncActive(scroll) {
  const el = slash.el;
  if (!el) return;
  let active = null;
  el.querySelectorAll('[data-slash-index]').forEach(opt => {
    const on = +opt.dataset.slashIndex === slash.index;
    opt.setAttribute('aria-selected', on ? 'true' : 'false');
    if (on) active = opt;
  });
  const content = slash.view && slash.view.contentDOM;
  if (content) {
    if (active) content.setAttribute('aria-activedescendant', active.id);
    else content.removeAttribute('aria-activedescendant');
  }
  if (active && scroll) active.scrollIntoView({ block: 'nearest' });
}

function positionSlashMenu() {
  if (!slash.open || !slash.view || !slash.el) return;
  const el = slash.el, view = slash.view;
  const c = view.coordsAtPos(slash.from, 1);
  const scroller = view.scrollDOM.getBoundingClientRect();
  if (!c || c.bottom < scroller.top || c.top > scroller.bottom) { el.style.visibility = 'hidden'; return; }
  el.style.visibility = '';
  const vw = document.documentElement.clientWidth || window.innerWidth;
  const vh = window.innerHeight;
  const w = Math.min(slash.mode === 'table' ? 256 : 328, vw - 16);
  el.style.width = w + 'px';
  el.style.maxHeight = '';
  const natural = el.scrollHeight;
  const below = vh - c.bottom - 14, above = c.top - 14;
  const placeBelow = below >= Math.min(natural, 280) || below >= above;
  const maxH = Math.max(140, Math.min(400, placeBelow ? below : above));
  el.style.maxHeight = maxH + 'px';
  const h = Math.min(natural, maxH);
  el.style.left = Math.round(Math.max(8, Math.min(vw - w - 8, c.left - 14))) + 'px';
  el.style.top = Math.round(placeBelow ? c.bottom + 6 : c.top - 6 - h) + 'px';
  el.classList.toggle('is-above', !placeBelow);
}

function openSlashMenu(view, from) {
  if (!view) return;
  slash.open = true;
  slash.view = view;
  slash.from = from;
  slash.prefix = '/';
  slash.mode = 'blocks';
  slash.query = '';
  slash.lastHitLen = 0;
  hideCmSelectionToolbar();
  slashFilter();
  const el = slashEnsureEl();
  slashRender();
  el.classList.remove('hidden', 'is-settled');
  const content = view.contentDOM;
  content.setAttribute('aria-haspopup', 'listbox');
  content.setAttribute('aria-expanded', 'true');
  content.setAttribute('aria-controls', 'cm-slash-menu');
  requestAnimationFrame(() => {
    positionSlashMenu();
    requestAnimationFrame(() => { if (slash.open) el.classList.add('is-settled'); });
  });
}

/* Ctrl+/ or the edit-bar button: insert "/" at the cursor and open the menu. */
function openSlashAtCursor(view) {
  if (!view) return;
  const sel = view.state.selection.main;
  const line = view.state.doc.lineAt(sel.from);
  const before = view.state.sliceDoc(line.from, sel.from);
  const pad = before && !/\s$/.test(before) ? ' ' : '';
  view.focus();
  view.dispatch({ changes: { from: sel.from, to: sel.to, insert: pad + '/' }, selection: { anchor: sel.from + pad.length + 1 }, scrollIntoView: true, userEvent: 'input.type' });
  openSlashMenu(view, sel.from + pad.length);
}
window.openSlashAtCursor = () => openSlashAtCursor(cmView);

function closeSlashMenu() {
  if (!slash.open && !(slash.el && !slash.el.classList.contains('hidden'))) return;
  slash.open = false;
  slash.mode = 'blocks';
  if (slash.el) slash.el.classList.add('hidden');
  const content = slash.view && slash.view.contentDOM;
  if (content) {
    content.removeAttribute('aria-activedescendant');
    content.removeAttribute('aria-controls');
    content.setAttribute('aria-expanded', 'false');
  }
}

function slashChoose(item) {
  const view = slash.view;
  if (!view || !item) return;
  const from = slash.from;
  const to = view.state.selection.main.head;
  if (item.next === 'lang') {
    slash.mode = 'lang';
    slash.prefix = '```';
    slash.query = '';
    slash.suppress = true;
    view.dispatch({ changes: { from, to, insert: '```' }, selection: { anchor: from + 3 }, userEvent: 'input.slash' });
    slash.suppress = false;
    slash.from = from;
    slashFilter();
    slashRender();
    requestAnimationFrame(positionSlashMenu);
    return;
  }
  if (item.next === 'table') {
    slash.mode = 'table';
    slash.cols = 3;
    slash.rows = 3;
    slashRender();
    requestAnimationFrame(positionSlashMenu);
    return;
  }
  const e = item.run(view.state.doc.toString(), from, to);
  closeSlashMenu();
  if (e) view.dispatch({ changes: e.changes, selection: e.selection, scrollIntoView: true, userEvent: 'input.slash' });
  view.focus();
}

function slashInsertTable() {
  const view = slash.view;
  const T = window.ReadMDTransforms;
  if (!view || !T) return;
  const tb = T.tableBlock(slash.cols, Math.max(1, slash.rows - 1), slashT('editor.tableHeaderPrefix') || 'Column');
  const e = T.blockInsertEdit(view.state.doc.toString(), slash.from, view.state.selection.main.head, tb.text, tb.selFrom, tb.selTo);
  closeSlashMenu();
  view.dispatch({ changes: e.changes, selection: e.selection, scrollIntoView: true, userEvent: 'input.slash' });
  view.focus();
}

function slashKey(view, key) {
  if (!slash.open || view !== slash.view) return false;
  if (slash.mode === 'table') {
    if (key === 'left') slash.cols = Math.max(1, slash.cols - 1);
    else if (key === 'right') slash.cols = Math.min(SLASH_GRID, slash.cols + 1);
    else if (key === 'up') slash.rows = Math.max(2, slash.rows - 1);
    else if (key === 'down') slash.rows = Math.min(SLASH_GRID, slash.rows + 1);
    else if (key === 'enter') { slashInsertTable(); return true; }
    else if (key === 'escape') { closeSlashMenu(); return true; }
    slashSyncTable();
    return true;
  }
  const n = slash.items.length;
  if (key === 'escape') { closeSlashMenu(); return true; }
  if (key === 'left' || key === 'right') return false;
  if (!n) {
    if (key === 'enter') closeSlashMenu();
    return false;
  }
  if (key === 'enter') { slashChoose(slash.items[slash.index]); return true; }
  if (key === 'down') slash.index = (slash.index + 1) % n;
  else if (key === 'up') slash.index = (slash.index - 1 + n) % n;
  else if (key === 'pagedown') slash.index = Math.min(n - 1, slash.index + 6);
  else if (key === 'pageup') slash.index = Math.max(0, slash.index - 6);
  slashSyncActive(true);
  return true;
}

/* Keep the menu in step with the document: the query is the text typed after
   the "/" (or after "```" when picking a language). */
function slashOnUpdate(u) {
  if (!slash.open || u.view !== slash.view || slash.suppress) return;
  const st = u.state;
  if (u.docChanged) slash.from = u.changes.mapPos(slash.from, 1);
  if (!u.docChanged && u.selectionSet) { closeSlashMenu(); return; }
  if (!u.docChanged) return;
  const sel = st.selection.main;
  const start = slash.from + slash.prefix.length;
  const valid = st.selection.ranges.length === 1 && sel.empty && sel.head >= start &&
    st.sliceDoc(slash.from, start) === slash.prefix && st.doc.lineAt(slash.from).number === st.doc.lineAt(sel.head).number;
  if (!valid) { closeSlashMenu(); return; }
  const query = st.sliceDoc(start, sel.head);
  if (query.length > 32 || (slash.mode !== 'lang' && /^\s/.test(query)) || /\s\s/.test(query)) { closeSlashMenu(); return; }
  if (slash.mode === 'table') slash.mode = 'blocks';
  slash.query = query;
  slashFilter();
  if (!slash.items.length && query.length - slash.lastHitLen >= 3) { closeSlashMenu(); return; }
  slashRender();
  requestAnimationFrame(positionSlashMenu);
}

window.addEventListener('resize', () => { if (slash.open) positionSlashMenu(); });

/* ============================================================
   Edit bar: view menu (line numbers / focus / typewriter), slash button,
   keyboard navigation inside the edit-bar menus.
   ============================================================ */

function editMenuArrowNav(e) {
  const items = [...e.currentTarget.querySelectorAll('button')].filter(b => b.offsetParent !== null && !b.disabled);
  if (!items.length) return;
  const i = items.indexOf(document.activeElement);
  let next = null;
  if (e.key === 'ArrowDown') next = items[(i + 1) % items.length];
  else if (e.key === 'ArrowUp') next = items[(i - 1 + items.length) % items.length];
  else if (e.key === 'Home') next = items[0];
  else if (e.key === 'End') next = items[items.length - 1];
  if (next) { e.preventDefault(); next.focus(); }
}

function bindEditBarExtras() {
  const trig = $('edit-view-trigger');
  const menu = $('edit-view-menu');
  if (trig && menu && !trig.dataset.bound) {
    trig.dataset.bound = '1';
    trig.addEventListener('click', e => {
      e.stopPropagation();
      const show = menu.classList.contains('hidden');
      closeMdPopups();
      menu.classList.toggle('hidden', !show);
      trig.setAttribute('aria-expanded', show ? 'true' : 'false');
      if (show && e.detail === 0) { const first = menu.querySelector('button'); if (first) first.focus(); }
    });
    [['edit-view-lines', 'lineNumbers'], ['edit-view-focus', 'focus'], ['edit-view-typewriter', 'typewriter']].forEach(([id, key]) => {
      const b = $(id);
      if (b) b.addEventListener('click', e => { e.stopPropagation(); setEditorPref(key); });
    });
  }
  const slashBtn = $('edit-slash-btn');
  if (slashBtn && !slashBtn.dataset.bound) {
    slashBtn.dataset.bound = '1';
    slashBtn.addEventListener('mousedown', e => e.preventDefault());
    slashBtn.addEventListener('click', e => { e.stopPropagation(); closeMdPopups(); if (cmView) openSlashAtCursor(cmView); });
  }
  document.querySelectorAll('#edit-bar .md-menu').forEach(m => {
    if (m.dataset.navBound) return;
    m.dataset.navBound = '1';
    m.addEventListener('keydown', editMenuArrowNav);
  });
  document.querySelectorAll('#md-tool [data-menu]').forEach(b => {
    if (b.dataset.ariaBound) return;
    b.dataset.ariaBound = '1';
    b.addEventListener('click', e => requestAnimationFrame(() => {
      const m = $(b.dataset.menu);
      const open = !!(m && !m.classList.contains('hidden'));
      b.setAttribute('aria-expanded', open ? 'true' : 'false');
      if (open && e.detail === 0) { const first = m.querySelector('button'); if (first) first.focus(); }
    }));
  });
  applyEditorViewClasses();
}
