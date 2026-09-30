'use strict';
/* ============================================================
   ReadMD Shell - Action registry, command palette & shortcut sheet
   ------------------------------------------------------------
   Loaded on demand (first interaction or idle) by the small loader in
   command-palette.js, together with assets/css/shell-deferred.css, so
   none of it sits on the startup path.

   window.ReadMDActions   : one registry of every app action (title key,
                            group, keywords, shortcut, icon, availability).
                            The palette and the cheat-sheet are both
                            generated from it.  Actions call the existing
                            feature functions; nothing is re-implemented.
   window.ReadMDPalette   : a ReadMDModal layer (role=dialog) with a combobox
                            + grouped listbox (aria-activedescendant), fuzzy
                            ranking, recent-first ordering and an empty state.
   window.ReadMDShortcuts : keyboard cheat-sheet overlay.
   ============================================================ */
(function () {
  const _t = (k, p) => (window.i18n ? window.i18n.t(k, p) : k);
  const byId = id => document.getElementById(id);
  const IS_MAC = !!(window.ReadMDKeys && window.ReadMDKeys.isMac);
  const MRU_KEY = 'readmd.palette.mru';
  const MRU_MAX = 16;

  /* ---------- context predicates (globals come from the other boot sources) ---------- */
  const S = () => (typeof state !== 'undefined' ? state : {});
  const ctx = {
    hasDoc: () => { const s = S(); return (s.mode === 'file' || s.mode === 'virtual') && !!s.original; },
    isFile: () => { const s = S(); return s.mode === 'file' && !!s.file; },
    editing: () => !!S().editing,
    notWelcome: () => S().mode !== 'welcome',
    paged: () => !!(S().pagination && S().pagination.enabled),
  };
  const call = (name, ...args) => {
    const fn = window[name];
    return typeof fn === 'function' ? fn(...args) : undefined;
  };
  const enabled = id => { const el = byId(id); return !!el && !el.disabled; };

  /* ---------- icons (24px grid, stroked) ---------- */
  const ICONS = {
    file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
    folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    clock: '<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>',
    save: '<path d="M5 4h11l3 3v12a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1z"/><path d="M8 4v5h7V4M8 20v-6h8v6"/>',
    refresh: '<path d="M20 12a8 8 0 1 1-2.4-5.7"/><path d="M20 4v5h-5"/>',
    pen: '<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="M13.5 6.5l4 4"/>',
    clipboard: '<rect x="6" y="4" width="12" height="17" rx="2"/><path d="M9 4h6v3H9z"/>',
    home: '<path d="M4 11l8-7 8 7v8a1 1 0 0 1-1 1h-5v-6h-4v6H5a1 1 0 0 1-1-1z"/>',
    convert: '<path d="M7 7h11l-3-3M17 17H6l3 3"/>',
    globe: '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.5 2.7 2.5 14.3 0 17M12 3.5c-2.5 2.7-2.5 14.3 0 17"/>',
    scan: '<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3M8 12h8"/>',
    export: '<path d="M12 15V4M8 8l4-4 4 4"/><path d="M5 14v5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-5"/>',
    play: '<rect x="3.5" y="5" width="17" height="12" rx="2"/><path d="M10.5 8.5l4 2.5-4 2.5zM9 21h6"/>',
    share: '<circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M8.3 10.8l7.4-3.6M8.3 13.2l7.4 3.6"/>',
    sidebar: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M9 4.5v15"/>',
    search: '<circle cx="11" cy="11" r="6.5"/><path d="M20 20l-4.2-4.2"/>',
    zen: '<path d="M8 4H5a1 1 0 0 0-1 1v3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/>',
    zoomIn: '<path d="M4 18l4.5-12L13 18M5.7 14h5.6M17 8v6M14 11h6"/>',
    zoomOut: '<path d="M4 18l4.5-12L13 18M5.7 14h5.6M14 11h6"/>',
    zoomReset: '<path d="M4 18l4.5-12L13 18M5.7 14h5.6"/><circle cx="17.5" cy="11" r="2.5"/>',
    book: '<path d="M4 5.5A1.5 1.5 0 0 1 5.5 4H11v16H5.5A1.5 1.5 0 0 1 4 18.5zM20 5.5A1.5 1.5 0 0 0 18.5 4H13v16h5.5a1.5 1.5 0 0 0 1.5-1.5z"/>',
    graph: '<circle cx="6" cy="7" r="2.5"/><circle cx="18" cy="8" r="2.5"/><circle cx="12" cy="18" r="2.5"/><path d="M8.4 7.5l7.2.4M7.2 9.2l3.6 6.6M16.8 10.2l-3.6 5.8"/>',
    link: '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
    wrench: '<path d="M14.5 5.5a4 4 0 0 0 5 5L12 18a2.1 2.1 0 0 1-3-3z"/><path d="M4 20l5-5"/>',
    theme: '<circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor" stroke="none"/>',
    sun: '<circle cx="12" cy="12" r="3.5"/><path d="M12 3v2M12 19v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M3 12h2M19 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
    moon: '<path d="M19.5 14.5A7.5 7.5 0 0 1 9.5 4.5a7.5 7.5 0 1 0 10 10z"/>',
    paper: '<path d="M6 3.5h9l3 3v14H6z"/><path d="M9 10h6M9 13.5h6M9 17h4"/>',
    edit: '<path d="M4 20h4L19 9l-4-4L4 16z"/>',
    bold: '<path d="M7 5h6a3.5 3.5 0 0 1 0 7H7zM7 12h7a3.5 3.5 0 0 1 0 7H7z"/>',
    italic: '<path d="M10 5h8M6 19h8M14 5l-4 14"/>',
    heading: '<path d="M6 5v14M17 5v14M6 12h11"/>',
    code: '<path d="M9 8l-4 4 4 4M15 8l4 4-4 4"/>',
    table: '<rect x="4" y="5" width="16" height="14" rx="1.5"/><path d="M4 10h16M4 14.5h16M10 5v14"/>',
    sigma: '<path d="M17 5H7l6 7-6 7h10"/>',
    columns: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M12 4.5v15"/>',
    rows: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M3.5 12h17"/>',
    eyeOff: '<path d="M4 4l16 16M10.6 6.1A9 9 0 0 1 21 12a13 13 0 0 1-2.2 2.9M6.3 7.4A12.7 12.7 0 0 0 3 12s3.5 6 9 6a8.6 8.6 0 0 0 3.7-.8"/>',
    sparkle: '<path d="M12 3.5l1.7 4.6 4.8 1.4-4.8 1.5L12 15.5l-1.7-4.5-4.8-1.5 4.8-1.4z"/><path d="M18.5 14.5l.8 2 2.2.7-2.2.7-.8 2.1-.8-2.1-2.2-.7 2.2-.7z"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8"/>',
    history: '<path d="M4 12a8 8 0 1 0 2.4-5.7L4 8.5"/><path d="M4 4v4.5h4.5M12 8v4l3 2"/>',
    puzzle: '<path d="M10 4h4v2.5a1.5 1.5 0 0 0 3 0V4h3v6h-2.5a1.5 1.5 0 0 0 0 3H20v7h-6v-2.5a1.5 1.5 0 0 0-3 0V20H4v-6h2.5a1.5 1.5 0 0 0 0-3H4V4z"/>',
    language: '<path d="M4 6h9M8.5 4v2M6 6c.5 3 3 5.5 6 6.5M11 6c-.6 3-3 6-7 7.5M13 20l3.5-8 3.5 8M14.2 17.5h4.6"/>',
    paw: '<circle cx="7" cy="10" r="1.8"/><circle cx="11" cy="6.5" r="1.8"/><circle cx="15.5" cy="7" r="1.8"/><circle cx="18.5" cy="11" r="1.8"/><path d="M8.5 17.5c0-2.5 2-5 4-5s4.5 2.5 4.5 5-2 2.5-4.2 1.7c-1.4-.5-2.2-.5-3.4.2-1 .5-.9-.4-.9-1.9z"/>',
    pin: '<path d="M12 21s-6-5.5-6-11a6 6 0 0 1 12 0c0 5.5-6 11-6 11z"/><circle cx="12" cy="10" r="2"/>',
    power: '<path d="M12 3.5v8M7.2 6.5a7 7 0 1 0 9.6 0"/>',
    update: '<path d="M12 4v10M8 10l4 4 4-4M5 19h14"/>',
    brush: '<path d="M19 4l-8.5 8.5M14 4.5l5.5 5.5M9.5 13.5c-2 0-3.5 1.5-3.5 3.5 0 1.3-.7 2-2 2.5 3 1 7 .5 7.5-3z"/>',
    keyboard: '<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M11 10h.01M15 10h.01M17 14H7"/>',
    command: '<path d="M9 9V6.5A2.5 2.5 0 1 0 6.5 9H9zm0 0h6m-6 0v6m6-6V6.5A2.5 2.5 0 1 1 17.5 9H15zm0 0v6m0 0h2.5a2.5 2.5 0 1 1-2.5 2.5V15zm0 0H9m0 0v2.5A2.5 2.5 0 1 1 6.5 15H9z"/>',
    tab: '<rect x="3.5" y="6" width="17" height="13" rx="2"/><path d="M3.5 10h7V6"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    arrow: '<path d="M5 12h14M13 6l6 6-6 6"/>',
  };
  const svg = name => `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">${ICONS[name] || ICONS.arrow}</svg>`;

  /* ---------- shortcut formatting (shared with the bundled loader) ---------- */
  const { formatShortcut, kbdHtml } = window.ReadMDKeys;

  /* ---------- theme helper (the shell adds the cross-fade on top) ---------- */
  function setThemeChoice(choice) {
    if (typeof state === 'undefined') return;
    const apply = () => {
      state.theme = choice;
      call('applySettings');
      call('applyCmTheme');
    };
    if (window.ReadMDShell && typeof window.ReadMDShell.withThemeTransition === 'function') window.ReadMDShell.withThemeTransition(apply);
    else apply();
    call('saveSettings');
    call('showToast', _t('theme.current', { name: _t('theme.' + choice) }), 1400);
  }

  function openExportAs(fmt) {
    call('openExportModal');
    const started = performance.now();
    const pick = () => {
      const modal = byId('export-modal');
      const tab = byId('export-tab-' + fmt);
      if (modal && tab && !modal.classList.contains('hidden')) { tab.click(); tab.focus({ preventScroll: true }); return; }
      if (performance.now() - started < 4000) requestAnimationFrame(pick);
    };
    requestAnimationFrame(pick);
  }

  const openKnowledgeGraph = () => {
    const s = S();
    const f = s.file || '';
    const dir = f ? f.substring(0, Math.max(f.lastIndexOf('/'), f.lastIndexOf('\\'))) : '';
    if (window.ReadMDGraph) window.ReadMDGraph.open(dir);
  };

  /* ---------- the registry ---------- */
  const GROUP_KEY = { file: 'menu.file', settings: 'menu.settings' };
  const groupTitle = g => _t(GROUP_KEY[g] || 'shell.group.' + g);
  const GROUP_ORDER = ['file', 'import', 'export', 'view', 'theme', 'edit', 'ai', 'settings', 'help'];
  const actions = [];
  const index = new Map();
  function register(a) {
    if (!a || !a.id || index.has(a.id)) return;
    a.group = a.group || 'help';
    actions.push(a);
    index.set(a.id, a);
  }
  const isAvailable = a => { try { return !a.when || !!a.when(); } catch (e) { return false; } };

  const EXPORT_FORMATS = [['pdf', 'PDF'], ['docx', 'Word (DOCX)'], ['epub', 'EPUB'], ['html', 'HTML'], ['tex', 'LaTeX'], ['presentation', 'Reveal.js']];

  [
    // File
    { id: 'file.open', group: 'file', title: 'menu.open', icon: 'file', shortcut: 'Mod+O', keywords: 'open file markdown load', run: () => call('loadFileDialog') },
    { id: 'file.openFolder', group: 'file', title: 'menu.openFolder', icon: 'folder', keywords: 'open folder directory workspace', when: () => enabled('btn-folder'), run: () => call('openFolder') },
    { id: 'file.recent', group: 'file', title: 'cmd.recent', icon: 'clock', keywords: 'recent history files', run: () => call('openHistoryModal') },
    { id: 'file.save', group: 'file', title: 'menu.save', icon: 'save', shortcut: 'Mod+S', keywords: 'save write', when: ctx.editing, run: () => call('saveEdit') },
    { id: 'file.saveAs', group: 'file', title: 'cmd.saveAs', icon: 'save', keywords: 'save as copy md', when: () => enabled('btn-saveas'), run: () => call('saveAs') },
    { id: 'file.reload', group: 'file', title: 'cmd.reload', icon: 'refresh', shortcut: 'Mod+R', keywords: 'reload refresh disk', when: ctx.isFile, run: () => call('loadFile', S().file, { force: true }) },
    { id: 'file.rename', group: 'file', title: 'cmd.rename', icon: 'pen', shortcut: 'F2', keywords: 'rename file name', when: () => ctx.hasDoc() && !ctx.editing(), run: () => call('openFileRename') },
    { id: 'file.clipboard', group: 'file', title: 'menu.clipboardSub', icon: 'clipboard', shortcut: 'Mod+V', keywords: 'clipboard paste new document', run: () => call('createFromClipboard') },
    { id: 'file.home', group: 'file', title: 'cmd.home', icon: 'home', keywords: 'home welcome start', when: ctx.notWelcome, run: () => call('goHome') },
    // Import & convert
    { id: 'import.convert', group: 'import', title: 'cmd.convert', icon: 'convert', keywords: 'convert docx pdf xlsx pptx word excel import', run: () => call('openConvertModal') },
    { id: 'import.web', group: 'import', title: 'cmd.web', icon: 'globe', shortcut: 'Mod+U', keywords: 'web url page import html fetch', run: () => call('openWebDialog') },
    { id: 'import.ocr', group: 'import', title: 'cmd.ocr', icon: 'scan', keywords: 'ocr scan image pdf recognize', run: () => call('chooseFile', 'ocr') },
    // Export
    { id: 'export.open', group: 'export', title: 'cmd.export', icon: 'export', shortcut: 'Mod+P', keywords: 'export print pdf docx html', when: ctx.hasDoc, run: () => call('openExportModal') },
    ...EXPORT_FORMATS.map(([fmt, name]) => ({
      id: 'export.' + fmt, group: 'export', title: 'cmd.exportAs', titleParams: { fmt: name }, icon: 'export',
      keywords: 'export ' + fmt + ' ' + name, when: ctx.hasDoc, run: () => openExportAs(fmt),
    })),
    { id: 'export.presentation', group: 'export', title: 'cmd.presentation', icon: 'play', shortcut: 'F5', keywords: 'presentation slides reveal present', when: ctx.hasDoc, run: () => call('launchPresentationMode') },
    { id: 'export.share', group: 'export', title: 'cmd.share', icon: 'share', keywords: 'share qr phone lan mobile', when: () => enabled('btn-share'), run: () => call('openShareModal') },
    // View
    { id: 'view.toc', group: 'view', title: 'cmd.toggleToc', icon: 'sidebar', shortcut: 'Mod+Shift+F', keywords: 'toc outline sidebar headings', run: () => call('toggleSide', 'toc') },
    { id: 'view.files', group: 'view', title: 'cmd.toggleFiles', icon: 'folder', keywords: 'files sidebar tree explorer', run: () => call('toggleSide', 'files') },
    { id: 'view.search', group: 'view', title: 'cmd.search', icon: 'search', shortcut: 'Mod+F', keywords: 'find search in document', when: ctx.hasDoc, run: () => call('toggleSearch') },
    { id: 'view.zen', group: 'view', title: 'cmd.zen', icon: 'zen', shortcut: 'F11', keywords: 'zen focus fullscreen distraction free', run: () => call('toggleZenMode') },
    { id: 'view.reading', group: 'view', title: 'cmd.readingMode', icon: 'book', keywords: 'reading mode paged continuous pagination', when: ctx.paged, run: () => call('togglePaginationMode') },
    { id: 'view.zoomIn', group: 'view', title: 'cmd.zoomIn', icon: 'zoomIn', shortcut: 'Mod+=', keywords: 'zoom in bigger font size larger', when: ctx.notWelcome, run: () => call('zoom', 10) },
    { id: 'view.zoomOut', group: 'view', title: 'cmd.zoomOut', icon: 'zoomOut', shortcut: 'Mod+-', keywords: 'zoom out smaller font size', when: ctx.notWelcome, run: () => call('zoom', -10) },
    { id: 'view.zoomReset', group: 'view', title: 'cmd.zoomReset', icon: 'zoomReset', shortcut: 'Mod+0', keywords: 'zoom reset actual size font', when: ctx.notWelcome, run: () => { if (typeof state === 'undefined') return; state.fontSize = 100; call('applySettings'); call('saveSettings'); } },
    { id: 'view.graph', group: 'view', title: 'cmd.graph', icon: 'graph', shortcut: 'Mod+G', keywords: 'graph knowledge links network', when: () => ctx.hasDoc() && !!window.ReadMDGraph, run: openKnowledgeGraph },
    { id: 'view.backlinks', group: 'view', title: 'cmd.backlinks', icon: 'link', keywords: 'backlinks references links', when: () => enabled('btn-backlinks-menu'), run: () => byId('btn-backlinks-menu').click() },
    { id: 'view.fix', group: 'view', title: 'cmd.fix', icon: 'wrench', keywords: 'fix report corrections repair', when: () => enabled('btn-fix'), run: () => call('showFixModal') },
    // Theme
    { id: 'theme.cycle', group: 'theme', title: 'cmd.themeCycle', icon: 'theme', shortcut: 'Mod+D', keywords: 'theme toggle dark light cycle appearance', run: () => call('toggleTheme') },
    ...[['auto', 'theme'], ['light', 'sun'], ['dark', 'moon'], ['sepia', 'paper']].map(([choice, icon]) => ({
      id: 'theme.' + choice, group: 'theme', title: 'theme.current', titleParams: () => ({ name: _t('theme.' + choice) }), icon,
      keywords: 'theme appearance ' + choice + (choice === 'sepia' ? ' paper warm eye' : ''),
      checked: () => S().theme === choice, run: () => setThemeChoice(choice),
    })),
    // Editor
    { id: 'edit.toggle', group: 'edit', title: 'cmd.edit', icon: 'edit', shortcut: 'Mod+E', keywords: 'edit editor write source', when: () => ctx.editing() || enabled('btn-edit'), run: () => call('toggleEdit') },
    { id: 'edit.bold', group: 'edit', title: 'editor.bold', icon: 'bold', shortcut: 'Mod+B', keywords: 'bold strong', when: ctx.editing, run: () => call('cmInsertSyntax', 'bold') },
    { id: 'edit.italic', group: 'edit', title: 'editor.italic', icon: 'italic', shortcut: 'Mod+I', keywords: 'italic emphasis', when: ctx.editing, run: () => call('cmInsertSyntax', 'italic') },
    { id: 'edit.link', group: 'edit', title: 'editor.link', icon: 'link', keywords: 'link url', when: ctx.editing, run: () => call('cmInsertSyntax', 'link') },
    { id: 'edit.heading', group: 'edit', title: 'editor.h2', icon: 'heading', keywords: 'heading title h2', when: ctx.editing, run: () => call('cmInsertSyntax', 'h2') },
    { id: 'edit.codeblock', group: 'edit', title: 'editor.codeBlock', icon: 'code', keywords: 'code block fence', when: ctx.editing, run: () => call('cmInsertSyntax', 'codeblock') },
    { id: 'edit.table', group: 'edit', title: 'cmd.insertTable', icon: 'table', keywords: 'table grid insert', when: ctx.editing, run: () => call('openTableModal') },
    { id: 'edit.formula', group: 'edit', title: 'cmd.insertFormula', icon: 'sigma', keywords: 'formula math latex equation', when: ctx.editing, run: () => call('openFormulaModal', 'inline') },
    { id: 'edit.previewRight', group: 'edit', title: 'editor.previewRight', icon: 'columns', keywords: 'preview split right side', when: ctx.editing, run: () => call('setPvLayout', 'right') },
    { id: 'edit.previewBottom', group: 'edit', title: 'editor.previewBottom', icon: 'rows', keywords: 'preview split bottom', when: ctx.editing, run: () => call('setPvLayout', 'bottom') },
    { id: 'edit.previewNone', group: 'edit', title: 'editor.previewNone', icon: 'eyeOff', keywords: 'preview hide close', when: ctx.editing, run: () => call('setPvLayout', 'none') },
    // AI
    { id: 'ai.toggle', group: 'ai', title: 'cmd.ai', icon: 'sparkle', shortcut: 'Mod+Shift+A', keywords: 'ai assistant chat panel', run: () => (typeof window.handleTopAiButtonClick === 'function' ? window.handleTopAiButtonClick() : call('toggleAiPanel')) },
    { id: 'ai.summary', group: 'ai', title: 'cmd.aiSummary', icon: 'sparkle', keywords: 'ai summarize summary key points', when: ctx.hasDoc, run: () => call('openAiPanelWithPrompt', 'ask', _t('ux.askSummary')) },
    { id: 'ai.questions', group: 'ai', title: 'cmd.aiQuestions', icon: 'sparkle', keywords: 'ai questions ask read', when: ctx.hasDoc, run: () => call('openAiPanelWithPrompt', 'ask', _t('ux.askQuestions')) },
    { id: 'ai.inline', group: 'ai', title: 'cmd.aiInline', icon: 'sparkle', shortcut: 'Alt+K', keywords: 'ai inline edit polish rewrite', when: ctx.editing, run: () => call('openEditAiBar') },
    { id: 'ai.settings', group: 'ai', title: 'cmd.aiSettings', icon: 'gear', keywords: 'ai settings provider model api key connection', run: () => call('openAiModal', 'ai-settings-modal', byId('btn-ai')) },
    { id: 'ai.history', group: 'ai', title: 'cmd.aiHistory', icon: 'history', keywords: 'ai history sessions conversations', run: () => { call('openAiModal', 'ai-history-modal', byId('btn-ai')); call('loadAiSessions'); } },
    { id: 'ai.skills', group: 'ai', title: 'cmd.skills', icon: 'puzzle', keywords: 'skills templates prompts workbench', run: () => call('openTplModal') },
    // Settings
    { id: 'settings.style', group: 'settings', title: 'cmd.customStyle', icon: 'brush', keywords: 'custom css style head inject', run: () => call('openStyleModal') },
    { id: 'settings.language', group: 'settings', title: 'cmd.language', icon: 'language', keywords: 'language locale translate i18n', run: () => window.i18n && window.i18n.openModal() },
    { id: 'settings.pet', group: 'settings', title: 'cmd.pet', icon: 'paw', keywords: 'pet desktop companion live2d', when: () => typeof window.openPetSettings === 'function', run: () => call('openPetSettings') },
    { id: 'settings.plugins', group: 'settings', title: 'cmd.plugins', icon: 'puzzle', keywords: 'plugins extensions', run: () => call('openPluginModal') },
    { id: 'settings.assoc', group: 'settings', title: 'cmd.assoc', icon: 'pin', keywords: 'default app association md open with', run: () => call('installAssoc') },
    { id: 'settings.autostart', group: 'settings', title: 'cmd.autostart', icon: 'power', keywords: 'autostart startup login boot', run: () => call('toggleAutostart') },
    { id: 'settings.update', group: 'settings', title: 'menu.checkUpdate', icon: 'update', keywords: 'update upgrade version check', run: () => call('checkUpdate', false) },
    // Help
    { id: 'help.shortcuts', group: 'help', title: 'cmd.shortcuts', icon: 'keyboard', shortcut: 'Mod+/', keywords: 'keyboard shortcuts keys cheat sheet help', run: () => window.ReadMDShortcuts && window.ReadMDShortcuts.open() },
    { id: 'help.palette', group: 'help', title: 'editor.command', icon: 'command', shortcut: 'Mod+K', keywords: 'command palette', hidden: true, run: () => {} },
  ].forEach(register);

  function titleOf(a) {
    const params = typeof a.titleParams === 'function' ? a.titleParams() : a.titleParams;
    return _t(a.title, params || {});
  }

  /* ---------- MRU (recent-first ranking) ---------- */
  function readMru() {
    try { const v = JSON.parse(localStorage.getItem(MRU_KEY) || '[]'); return Array.isArray(v) ? v.filter(x => typeof x === 'string') : []; } catch (e) { return []; }
  }
  function pushMru(key) {
    const list = readMru().filter(k => k !== key);
    list.unshift(key);
    try { localStorage.setItem(MRU_KEY, JSON.stringify(list.slice(0, MRU_MAX))); } catch (e) { /* storage full / disabled */ }
  }

  /* ---------- fuzzy matching ---------- */
  const isBoundary = (t, i) => i === 0 || /[\s\-_./\\:()[\]·，。、]/.test(t[i - 1]) || (/[a-z]/.test(t[i - 1]) && /[A-Z]/.test(t[i]));
  function fuzzy(q, text) {
    if (!text) return null;
    const t = text.toLowerCase();
    const at = t.indexOf(q);
    if (at >= 0) {
      const s = 120 + (at === 0 ? 60 : 0) + (isBoundary(text, at) ? 30 : 0) - at * 0.6 - (t.length - q.length) * 0.15;
      return { s, ranges: [[at, at + q.length]] };
    }
    let ti = 0, s = 0, last = -2, streak = 0;
    const ranges = [];
    for (const ch of q) {
      const f = t.indexOf(ch, ti);
      if (f < 0) return null;
      if (f === last + 1 && ranges.length) { streak += 1; s += 6 + streak * 3; ranges[ranges.length - 1][1] = f + 1; }
      else { streak = 0; s += isBoundary(text, f) ? 10 : 1; ranges.push([f, f + 1]); }
      s -= (f - ti) * 0.25;
      last = f;
      ti = f + 1;
    }
    return { s, ranges };
  }
  function matchItem(item, q) {
    const tokens = q.toLowerCase().split(/\s+/).filter(Boolean);
    let total = 0;
    const ranges = [];
    for (const tok of tokens) {
      const a = fuzzy(tok, item.label);
      // Keywords only match as substrings: scattered letters across a long
      // keyword list produce noise ("dark" -> "upgrade version check").
      const b = item.haystack.toLowerCase().includes(tok) ? fuzzy(tok, item.haystack) : null;
      if (!a && !b) return null;
      if (a && (!b || a.s >= b.s * 0.7)) { total += a.s; ranges.push(...a.ranges); }
      else total += b.s * 0.7;
    }
    ranges.sort((x, y) => x[0] - y[0]);
    const merged = [];
    for (const r of ranges) {
      const prev = merged[merged.length - 1];
      if (prev && r[0] <= prev[1]) prev[1] = Math.max(prev[1], r[1]);
      else merged.push([r[0], r[1]]);
    }
    return { s: total, ranges: merged };
  }

  /* ---------- palette state & DOM ---------- */
  let root = null, input = null, listEl = null, live = null, titleEl = null;
  let flat = [];            // rendered options in order
  let active = 0;
  let recentFiles = [];
  let recentToken = 0;

  function actionItem(a) {
    const label = titleOf(a);
    return {
      key: a.id, kind: 'action', group: a.group, label, icon: a.icon, shortcut: a.shortcut,
      checked: typeof a.checked === 'function' ? !!a.checked() : false,
      haystack: [a.keywords || '', a.id.replace(/[._]/g, ' '), a.group].join(' '),
      run: a.run,
    };
  }
  function fileItems() {
    const cur = (S().file || '').replace(/\\/g, '/').toLowerCase();
    return recentFiles
      .filter(p => String(p).replace(/\\/g, '/').toLowerCase() !== cur)
      .map(p => {
        const path = String(p);
        const name = path.split(/[\\/]/).pop() || path;
        const dir = path.slice(0, path.length - name.length).replace(/[\\/]+$/, '');
        return { key: 'file:' + path, kind: 'file', group: 'recentFiles', label: name, sub: dir, icon: 'file', haystack: path + ' recent file', run: () => call('loadFile', path) };
      });
  }
  function tabItems() {
    const s = S();
    const tabs = Array.isArray(s.tabs) ? s.tabs : [];
    if (tabs.length < 2) return [];
    return tabs.filter(tab => tab.id !== s.activeTabId).map(tab => ({
      key: 'tab:' + tab.id, kind: 'tab', group: 'tabs', label: tab.title || tab.name || _t('tabs.untitled'),
      sub: tab.path || '', icon: 'tab', haystack: (tab.path || '') + ' tab', run: () => call('switchTab', tab.id), noMru: true,
    }));
  }

  function collect(q) {
    const acts = actions.filter(a => !a.hidden && isAvailable(a)).map(actionItem);
    const files = fileItems();
    const tabs = tabItems();
    const mru = readMru();
    const mruRank = key => { const i = mru.indexOf(key); return i < 0 ? -1 : i; };
    if (!q) {
      const sections = [];
      const byKey = new Map([...acts, ...files].map(it => [it.key, it]));
      const used = mru.map(k => byKey.get(k)).filter(it => it && it.kind === 'action').slice(0, 4);
      if (used.length) sections.push({ id: 'recentUsed', items: used.map(it => Object.assign({}, it, { group: 'recentUsed' })) });
      if (files.length) sections.push({ id: 'recentFiles', items: files.slice(0, 5) });
      if (tabs.length) sections.push({ id: 'tabs', items: tabs.slice(0, 8) });
      for (const g of GROUP_ORDER) {
        const items = acts.filter(a => a.group === g);
        if (items.length) sections.push({ id: g, items });
      }
      return sections;
    }
    const scored = [];
    for (const it of [...acts, ...files, ...tabs]) {
      const m = matchItem(it, q);
      if (!m) continue;
      const r = it.noMru ? -1 : mruRank(it.key);
      const boost = r < 0 ? 0 : (MRU_MAX - r) * 3;
      scored.push(Object.assign({}, it, { score: m.s + boost + (it.kind === 'file' ? -4 : 0), ranges: m.ranges }));
    }
    scored.sort((a, b) => b.score - a.score);
    const top = scored.slice(0, 60);
    // Group by section; sections ordered by their best hit.
    const sections = [];
    const map = new Map();
    for (const it of top) {
      if (!map.has(it.group)) { const sec = { id: it.group, items: [] }; map.set(it.group, sec); sections.push(sec); }
      map.get(it.group).items.push(it);
    }
    return sections;
  }

  function highlight(label, ranges) {
    const frag = document.createDocumentFragment();
    let pos = 0;
    for (const [a, b] of ranges || []) {
      if (a > pos) frag.appendChild(document.createTextNode(label.slice(pos, a)));
      const m = document.createElement('mark');
      m.textContent = label.slice(a, b);
      frag.appendChild(m);
      pos = b;
    }
    if (pos < label.length) frag.appendChild(document.createTextNode(label.slice(pos)));
    return frag;
  }

  function render() {
    if (!root) return;
    const q = input.value.trim();
    const sections = collect(q);
    listEl.textContent = '';
    flat = [];
    let n = 0;
    sections.forEach((sec, si) => {
      const group = document.createElement('div');
      group.className = 'rm-palette-group';
      group.setAttribute('role', 'group');
      const labelId = 'cmdp-g-' + si;
      group.setAttribute('aria-labelledby', labelId);
      const lab = document.createElement('div');
      lab.className = 'rm-palette-group-label';
      lab.id = labelId;
      lab.textContent = groupTitle(sec.id);
      group.appendChild(lab);
      for (const it of sec.items) {
        const opt = document.createElement('div');
        opt.className = 'rm-palette-item';
        opt.id = 'cmdp-o-' + n;
        opt.setAttribute('role', 'option');
        opt.setAttribute('aria-selected', 'false');
        opt.dataset.index = String(n);
        const ic = document.createElement('span');
        ic.className = 'rm-palette-ic';
        ic.innerHTML = svg(it.icon);
        const text = document.createElement('span');
        text.className = 'rm-palette-text';
        const title = document.createElement('span');
        title.className = 'rm-palette-title';
        title.appendChild(highlight(it.label, it.ranges));
        text.appendChild(title);
        if (it.sub) {
          const sub = document.createElement('span');
          sub.className = 'rm-palette-sub';
          sub.textContent = it.sub;
          text.appendChild(sub);
        }
        opt.append(ic, text);
        if (it.checked) {
          const chk = document.createElement('span');
          chk.className = 'rm-palette-check';
          chk.innerHTML = svg('check');
          opt.appendChild(chk);
          opt.setAttribute('aria-current', 'true');
        }
        if (it.shortcut) {
          const keys = document.createElement('span');
          keys.className = 'rm-palette-keys';
          keys.setAttribute('aria-hidden', 'true');
          keys.innerHTML = kbdHtml(it.shortcut);
          opt.appendChild(keys);
          opt.setAttribute('aria-keyshortcuts', it.shortcut.replace(/Mod/g, IS_MAC ? 'Meta' : 'Control'));
        }
        group.appendChild(opt);
        flat.push(it);
        n += 1;
      }
      listEl.appendChild(group);
    });
    if (!flat.length) {
      const empty = document.createElement('div');
      empty.className = 'rm-palette-empty';
      empty.innerHTML = svg('search');
      const t1 = document.createElement('strong');
      t1.textContent = _t('shell.palette.empty');
      const t2 = document.createElement('span');
      t2.textContent = _t('shell.palette.emptyHint');
      empty.append(t1, t2);
      listEl.appendChild(empty);
    }
    live.textContent = _t('shell.palette.results', { count: flat.length });
    setActive(0, false);
  }

  function setActive(i, scroll = true) {
    if (!flat.length) { active = -1; input.removeAttribute('aria-activedescendant'); return; }
    active = (i + flat.length) % flat.length;
    listEl.querySelectorAll('.rm-palette-item[aria-selected="true"]').forEach(el => el.setAttribute('aria-selected', 'false'));
    const el = byId('cmdp-o-' + active);
    if (!el) return;
    el.setAttribute('aria-selected', 'true');
    input.setAttribute('aria-activedescendant', el.id);
    if (scroll) el.scrollIntoView({ block: 'nearest' });
    else if (active === 0) listEl.scrollTop = 0;
  }

  function runActive() {
    const it = flat[active];
    if (!it) return;
    if (!it.noMru) pushMru(it.key);
    close();
    // Let the modal layer hand focus back first, so a dialog opened by the
    // action records the original opener and returns focus there later.
    requestAnimationFrame(() => {
      try { it.run(); } catch (e) { console.error('[palette]', it.key, e); }
    });
  }

  function onKeyDown(e) {
    // Keys typed into the palette never reach the app-wide shortcut handlers.
    e.stopPropagation();
    if (e.isComposing || e.keyCode === 229) return;
    const k = e.key;
    const mod = e.metaKey || e.ctrlKey;
    const lower = k.length === 1 ? k.toLowerCase() : k;
    if (mod && (lower === 'k' || (e.shiftKey && lower === 'p'))) { e.preventDefault(); close(); } // same chord closes
    else if (k === 'ArrowDown' || (e.ctrlKey && !e.shiftKey && (lower === 'n' || lower === 'j'))) { e.preventDefault(); setActive(active + 1); }
    else if (k === 'ArrowUp' || (e.ctrlKey && !e.shiftKey && lower === 'p')) { e.preventDefault(); setActive(active - 1); }
    else if (k === 'PageDown') { e.preventDefault(); setActive(Math.min(flat.length - 1, active + 6)); }
    else if (k === 'PageUp') { e.preventDefault(); setActive(Math.max(0, active - 6)); }
    else if (k === 'Home' && mod) { e.preventDefault(); setActive(0); }
    else if (k === 'End' && mod) { e.preventDefault(); setActive(flat.length - 1); }
    else if (k === 'Enter') { e.preventDefault(); runActive(); }
  }

  function build() {
    root = document.createElement('div');
    root.id = 'command-palette-modal';
    root.className = 'rm-palette-overlay hidden';
    root.setAttribute('role', 'dialog');
    root.setAttribute('aria-modal', 'true');
    root.setAttribute('aria-labelledby', 'cmdp-title');
    root.innerHTML = `
      <div class="rm-palette">
        <h2 id="cmdp-title" class="visually-hidden"></h2>
        <div class="rm-palette-search">
          <span class="rm-palette-search-ic">${svg('search')}</span>
          <input id="cmdp-input" type="text" role="combobox" aria-expanded="true" aria-controls="cmdp-list" aria-autocomplete="list" autocomplete="off" autocapitalize="off" spellcheck="false">
          <kbd class="rm-kbd rm-palette-esc" aria-hidden="true">Esc</kbd>
        </div>
        <div id="cmdp-list" class="rm-palette-list" role="listbox"></div>
        <div class="rm-palette-foot" aria-hidden="true">
          <span class="rm-palette-hint"><kbd class="rm-kbd">↑</kbd><kbd class="rm-kbd">↓</kbd><span data-hint="navigate"></span></span>
          <span class="rm-palette-hint"><kbd class="rm-kbd">↵</kbd><span data-hint="run"></span></span>
          <span class="rm-palette-hint"><kbd class="rm-kbd">Esc</kbd><span data-hint="close"></span></span>
        </div>
        <div id="cmdp-live" class="visually-hidden" role="status" aria-live="polite"></div>
      </div>`;
    document.body.appendChild(root);
    input = byId('cmdp-input');
    listEl = byId('cmdp-list');
    live = byId('cmdp-live');
    titleEl = byId('cmdp-title');
    input.addEventListener('input', render);
    input.addEventListener('keydown', onKeyDown);
    // Backdrop click (and ReadMDModal's Esc path, which clicks the layer) closes.
    root.addEventListener('click', e => { if (e.target === root) close(); });
    listEl.addEventListener('mousemove', e => {
      const opt = e.target.closest('.rm-palette-item');
      if (opt && Number(opt.dataset.index) !== active) setActive(Number(opt.dataset.index), false);
    });
    listEl.addEventListener('mousedown', e => e.preventDefault()); // keep focus in the input
    listEl.addEventListener('click', e => {
      const opt = e.target.closest('.rm-palette-item');
      if (!opt) return;
      setActive(Number(opt.dataset.index), false);
      runActive();
    });
    window.addEventListener('readmd:language-changed', () => { if (isOpen()) { localize(); render(); } });
  }

  function localize() {
    titleEl.textContent = _t('editor.command');
    input.placeholder = _t('shell.palette.placeholder');
    input.setAttribute('aria-label', _t('editor.command'));
    listEl.setAttribute('aria-label', _t('editor.command'));
    root.querySelectorAll('[data-hint]').forEach(el => { el.textContent = _t(el.dataset.hint === 'close' ? 'toolbar.close' : 'shell.palette.' + el.dataset.hint); });
  }

  function isOpen() { return !!root && !root.classList.contains('hidden'); }

  function open(initial = '') {
    if (!root) build();
    if (isOpen()) { input.focus(); input.select(); return; }
    localize();
    input.value = initial;
    recentFiles = [];
    render();
    if (window.ReadMDModal) window.ReadMDModal.open(root);
    else root.classList.remove('hidden');
    requestAnimationFrame(() => { if (document.activeElement !== input) input.focus({ preventScroll: true }); });
    // Recent files arrive asynchronously; re-render without losing the query.
    const token = ++recentToken;
    if (typeof window.getRecentEntries === 'function') {
      Promise.resolve(window.getRecentEntries()).then(list => {
        if (token !== recentToken || !isOpen()) return;
        recentFiles = (Array.isArray(list) ? list : []).slice(0, 24);
        if (recentFiles.length) render();
      }).catch(() => {});
    }
  }

  function close() {
    if (!isOpen()) return;
    recentToken += 1;
    if (window.ReadMDModal) window.ReadMDModal.close(root);
    else root.classList.add('hidden');
  }

  function toggle() { if (isOpen()) close(); else open(); }

  const Actions = () => window.ReadMDActions;

  /* ---------- shortcut cheat-sheet ---------- */
  // Non-action keys worth knowing; everything else comes from the registry.
  const EXTRA_KEYS = [
    { group: 'general', title: 'editor.command', keys: ['Mod+K', 'Mod+Shift+P'] },
    { group: 'general', title: 'shell.key.closeLayer', keys: ['Escape'] },
    { group: 'general', title: 'shell.key.historyBack', keys: ['Mod+ArrowLeft'] },
    { group: 'general', title: 'shell.key.historyForward', keys: ['Mod+ArrowRight'] },
    { group: 'view', title: 'shell.key.findNext', keys: ['Enter', 'Shift+Enter'] },
    { group: 'view', title: 'shell.key.pagePrev', keys: ['Alt+ArrowLeft'] },
    { group: 'view', title: 'shell.key.pageNext', keys: ['Alt+ArrowRight'] },
    { group: 'file', title: 'shell.key.switchTab', keys: ['ArrowLeft', 'ArrowRight'] },
    { group: 'file', title: 'shell.key.moveTab', keys: ['Alt+ArrowLeft', 'Alt+ArrowRight'] },
    { group: 'file', title: 'shell.key.closeTab', keys: ['Delete'] },
  ];
  const SHEET_GROUPS = ['general', 'file', 'import', 'export', 'view', 'theme', 'edit', 'ai', 'settings', 'help'];
  let sheet = null, sheetFilter = null, sheetBody = null;

  function sheetRows() {
    const A = Actions();
    const rows = [];
    if (A) {
      for (const a of A.list()) {
        if (!a.shortcut || a.hidden || a.id === 'help.shortcuts') continue;
        rows.push({ group: a.group, label: A.title(a), keys: [a.shortcut] });
      }
      rows.push({ group: 'help', label: _t('cmd.shortcuts'), keys: ['Mod+/', '?'] });
    }
    for (const x of EXTRA_KEYS) rows.push({ group: x.group, label: _t(x.title), keys: x.keys });
    return rows;
  }

  function renderSheet() {
    const A = Actions();
    if (!A || !sheetBody) return;
    const q = (sheetFilter.value || '').trim().toLowerCase();
    const rows = sheetRows().filter(r => !q || r.label.toLowerCase().includes(q) || r.keys.join(' ').toLowerCase().includes(q));
    sheetBody.textContent = '';
    for (const g of SHEET_GROUPS) {
      const items = rows.filter(r => r.group === g);
      if (!items.length) continue;
      const sec = document.createElement('section');
      sec.className = 'rm-keys-group';
      const h = document.createElement('h3');
      h.textContent = groupTitle(g);
      const dl = document.createElement('dl');
      for (const r of items) {
        const row = document.createElement('div');
        row.className = 'rm-keys-row';
        const dt = document.createElement('dt');
        dt.textContent = r.label;
        const dd = document.createElement('dd');
        r.keys.forEach((spec, i) => {
          if (i) { const or = document.createElement('span'); or.className = 'rm-keys-or'; or.textContent = '/'; dd.appendChild(or); }
          const combo = document.createElement('span');
          combo.className = 'rm-keys-combo';
          combo.innerHTML = spec === '?' ? '<kbd class="rm-kbd">?</kbd>' : A.kbdHtml(spec);
          dd.appendChild(combo);
        });
        row.append(dt, dd);
        dl.appendChild(row);
      }
      sec.append(h, dl);
      sheetBody.appendChild(sec);
    }
    if (!sheetBody.children.length) {
      const empty = document.createElement('p');
      empty.className = 'rm-keys-empty';
      empty.textContent = _t('shell.shortcuts.empty');
      sheetBody.appendChild(empty);
    }
  }

  function buildSheet() {
    sheet = document.createElement('div');
    sheet.id = 'shortcuts-modal';
    sheet.className = 'rm-keys-overlay hidden';
    sheet.setAttribute('role', 'dialog');
    sheet.setAttribute('aria-modal', 'true');
    sheet.setAttribute('aria-labelledby', 'shortcuts-title');
    sheet.innerHTML = `
      <div class="rm-keys">
        <header class="rm-keys-head">
          <h2 id="shortcuts-title"></h2>
          <input id="shortcuts-filter" type="search" autocomplete="off" spellcheck="false">
          <button id="shortcuts-close" type="button" class="rm-icon-btn">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg>
          </button>
        </header>
        <div id="shortcuts-body" class="rm-keys-body"></div>
        <footer class="rm-keys-foot" id="shortcuts-hint"></footer>
      </div>`;
    document.body.appendChild(sheet);
    sheetFilter = byId('shortcuts-filter');
    sheetBody = byId('shortcuts-body');
    sheetFilter.addEventListener('input', renderSheet);
    sheetFilter.addEventListener('keydown', e => e.stopPropagation());
    byId('shortcuts-close').addEventListener('click', closeSheet);
    sheet.addEventListener('click', e => { if (e.target === sheet) closeSheet(); });
  }

  function localizeSheet() {
    byId('shortcuts-title').textContent = _t('shell.shortcuts.title');
    sheetFilter.placeholder = _t('shell.shortcuts.filter');
    sheetFilter.setAttribute('aria-label', _t('shell.shortcuts.filter'));
    const close = byId('shortcuts-close');
    close.title = _t('toolbar.close');
    close.setAttribute('aria-label', _t('toolbar.close'));
    byId('shortcuts-hint').textContent = _t('shell.shortcuts.hint');
  }

  const sheetOpen = () => !!sheet && !sheet.classList.contains('hidden');
  function openSheet() {
    if (!sheet) buildSheet();
    if (window.ReadMDPalette && window.ReadMDPalette.isOpen()) window.ReadMDPalette.close();
    localizeSheet();
    sheetFilter.value = '';
    renderSheet();
    if (window.ReadMDModal) window.ReadMDModal.open(sheet); else sheet.classList.remove('hidden');
    requestAnimationFrame(() => sheetFilter.focus({ preventScroll: true }));
  }
  function closeSheet() {
    if (!sheetOpen()) return;
    if (window.ReadMDModal) window.ReadMDModal.close(sheet); else sheet.classList.add('hidden');
  }
  window.addEventListener('readmd:language-changed', () => { if (sheetOpen()) { localizeSheet(); renderSheet(); } });

  window.ReadMDActions = {
    __real: true,
    register,
    list: () => actions.slice(),
    get: id => index.get(id) || null,
    run: id => { const a = index.get(id); if (a && isAvailable(a)) { pushMru(a.id); return a.run(); } return undefined; },
    title: titleOf,
    isAvailable,
    groups: GROUP_ORDER.slice(),
    formatShortcut,
    kbdHtml,
    icon: svg,
    isMac: IS_MAC,
  };
  // The bundled loader's stubs (window.ReadMDPalette / ReadMDShortcuts) forward here.
  window.__rmPalette = { open, close, toggle, isOpen };
  window.__rmShortcuts = { open: openSheet, close: closeSheet, toggle: () => (sheetOpen() ? closeSheet() : openSheet()), isOpen: sheetOpen };

  /* ============================================================
     Chrome enhancements (progressive: the chrome works without them)
     ============================================================ */
  /* ---------- tab strip: sliding active indicator ---------- */
  function bindIndicator(bar) {
    if (!bar || bar.__rmIndicator) return;
    bar.__rmIndicator = true;
    bar.classList.add('rm-has-indicator');
    const ind = document.createElement('span');
    ind.className = 'rm-tab-indicator';
    ind.setAttribute('aria-hidden', 'true');
    let last = null;          // last geometry, replayed after renderTabsBar() rebuilds the strip
    let reinserted = true;
    const apply = g => {
      ind.style.width = g.w + 'px';
      ind.style.height = g.h + 'px';
      ind.style.transform = `translate(${g.x}px, ${g.y}px)`;
    };
    const place = () => {
      const active = bar.querySelector('.tab-item.active');
      if (!active || !active.offsetParent) {
        // No active tab: keep the strip truly empty (so :empty rules still apply).
        bar.classList.remove('rm-ind-ready');
        if (ind.isConnected) ind.remove();
        last = null;
        return;
      }
      if (!ind.isConnected) { bar.insertBefore(ind, bar.firstChild); reinserted = true; }
      const g = { x: active.offsetLeft, y: active.offsetTop, w: active.offsetWidth, h: active.offsetHeight };
      if (reinserted) {
        // A freshly inserted node has no "before" style, so replay the old
        // geometry without animation, then slide to the new one.
        ind.classList.add('rm-no-anim');
        apply(last || g);
        void ind.offsetWidth;
        ind.classList.remove('rm-no-anim');
        reinserted = false;
      }
      apply(g);
      last = g;
      bar.classList.add('rm-ind-ready');
    };
    let queued = false;
    const schedule = () => { if (queued) return; queued = true; requestAnimationFrame(() => { queued = false; place(); }); };
    new MutationObserver(records => {
      // Ignore our own re-insertion.
      // Ignore our own mutations (indicator re-insertion, its classes, the bar's ready flag).
      const relevant = records.some(r => (r.type === 'childList'
        ? [...r.addedNodes, ...r.removedNodes].some(n => n !== ind)
        : r.target !== ind && r.target !== bar));
      if (!relevant) return;
      place(); // synchronous: no frame with a stale pill after a re-render
    }).observe(bar, { childList: true, subtree: true, attributes: true, attributeFilter: ['class'] });
    if (window.ResizeObserver) new ResizeObserver(schedule).observe(bar);
    schedule();
  }

  /* ---------- tab strip: edge fades when scrolled ---------- */
  function bindEdgeFade(bar) {
    if (!bar || bar.__rmFade) return;
    bar.__rmFade = true;
    const sync = () => {
      const max = bar.scrollWidth - bar.clientWidth;
      bar.classList.toggle('rm-fade-start', bar.scrollLeft > 2);
      bar.classList.toggle('rm-fade-end', max - bar.scrollLeft > 2);
    };
    bar.addEventListener('scroll', sync, { passive: true });
    // Vertical wheel scrolls the strip horizontally, like every native tab bar.
    bar.addEventListener('wheel', e => {
      if (Math.abs(e.deltaY) <= Math.abs(e.deltaX) || bar.scrollWidth <= bar.clientWidth) return;
      bar.scrollLeft += e.deltaY;
      e.preventDefault();
    }, { passive: false });
    new MutationObserver(() => requestAnimationFrame(sync)).observe(bar, { childList: true });
    if (window.ResizeObserver) new ResizeObserver(sync).observe(bar);
    sync();
  }

  /* ---------- tooltips (label + shortcut chips) ---------- */
  let tip = null, tipTimer = 0, tipFor = null, tipWarm = 0;
  const TIP_SCOPE = '#toolbar [title], #statusbar [title], #side-tabs [title]';
  function ensureTip() {
    if (tip) return tip;
    tip = document.createElement('div');
    tip.className = 'rm-tooltip';
    tip.setAttribute('role', 'tooltip');
    tip.id = 'rm-tooltip';
    document.body.appendChild(tip);
    return tip;
  }
  function splitTitle(title) {
    // "Open file (Ctrl+O)" -> ["Open file", "Ctrl+O"]
    const m = String(title).match(/^(.*?)\s*[（(]\s*((?:Ctrl|Cmd|Alt|Shift|F\d+|Esc)[^)）]*)[)）]\s*$/);
    return m ? [m[1], m[2]] : [String(title), ''];
  }
  function showTip(el) {
    const title = el.getAttribute('title') || el.dataset.rmTitle || '';
    if (!title || el.disabled && !el.getAttribute('aria-description')) { hideTip(); return; }
    // Park the native title while ours is visible so they never double up.
    if (el.hasAttribute('title')) { el.dataset.rmTitle = title; el.removeAttribute('title'); }
    const t = ensureTip();
    const [label, keys] = splitTitle(el.getAttribute('aria-description') && el.disabled ? el.getAttribute('aria-description') : title);
    t.textContent = '';
    const span = document.createElement('span');
    span.textContent = label.split('\n')[0];
    t.appendChild(span);
    if (keys) {
      const k = document.createElement('span');
      k.className = 'rm-tooltip-keys';
      keys.split(/\s*\/\s*/).forEach((combo, i) => {
        if (i) k.appendChild(document.createTextNode(' / '));
        combo.split('+').filter(Boolean).forEach(part => {
          const kbd = document.createElement('kbd');
          kbd.className = 'rm-kbd';
          kbd.textContent = part.trim();
          k.appendChild(kbd);
        });
      });
      t.appendChild(k);
    }
    t.classList.add('is-visible');
    const r = el.getBoundingClientRect();
    const tr = t.getBoundingClientRect();
    const below = r.bottom + 8 + tr.height < window.innerHeight;
    let x = r.left + r.width / 2 - tr.width / 2;
    x = Math.max(6, Math.min(window.innerWidth - tr.width - 6, x));
    const y = below ? r.bottom + 8 : r.top - tr.height - 8;
    t.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)`;
    t.dataset.side = below ? 'bottom' : 'top';
    tipFor = el;
    el.setAttribute('aria-describedby', 'rm-tooltip');
  }
  function restoreTitle(el) {
    if (!el) return;
    if (el.dataset.rmTitle && !el.hasAttribute('title')) el.setAttribute('title', el.dataset.rmTitle);
    delete el.dataset.rmTitle;
    if (el.getAttribute('aria-describedby') === 'rm-tooltip') el.removeAttribute('aria-describedby');
  }
  function hideTip() {
    clearTimeout(tipTimer);
    if (tip && tip.classList.contains('is-visible')) { tip.classList.remove('is-visible'); tipWarm = Date.now(); }
    restoreTitle(tipFor);
    tipFor = null;
  }
  function bindTooltips() {
    if (!window.matchMedia || !window.matchMedia('(hover: hover) and (pointer: fine)').matches) return;
    document.addEventListener('pointerover', e => {
      const el = e.target.closest && e.target.closest(TIP_SCOPE);
      if (!el || el === tipFor) return;
      hideTip();
      const warm = Date.now() - tipWarm < 400;
      tipTimer = setTimeout(() => { if (el.isConnected && el.matches(':hover')) showTip(el); }, warm ? 60 : 480);
    });
    document.addEventListener('pointerout', e => {
      const el = e.target.closest && e.target.closest(TIP_SCOPE);
      if (!el) { if (tipFor && !tipFor.contains(e.relatedTarget)) hideTip(); return; }
      if (e.relatedTarget && el.contains(e.relatedTarget)) return;
      if (el === tipFor || !tipFor) hideTip();
      else clearTimeout(tipTimer);
    });
    ['pointerdown', 'keydown', 'scroll', 'blur'].forEach(ev => window.addEventListener(ev, hideTip, true));
  }

  bindIndicator(byId('doc-tabs-bar'));
  bindIndicator(byId('doc-tabs-secondary-bar'));
  bindEdgeFade(byId('doc-tabs-bar'));
  bindTooltips();

  // Ctrl/Cmd+0 resets the reading font size (documents only).
  window.addEventListener('keydown', e => {
    if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey || e.defaultPrevented || e.isComposing) return;
    if (e.key !== '0' && e.code !== 'Digit0') return;
    if ((window.ReadMDModal && window.ReadMDModal.depth()) || typeof state === 'undefined' || state.mode === 'welcome') return;
    e.preventDefault();
    state.fontSize = 100;
    if (typeof applySettings === 'function') applySettings();
    if (typeof saveSettings === 'function') saveSettings();
  }, true);
  // Toolbar gets a hairline shadow once the document scrolls under it.
  const content = byId('content');
  if (content) content.addEventListener('scroll', () => {
    document.body.classList.toggle('rm-scrolled', content.scrollTop > 4);
  }, { passive: true });
})();
