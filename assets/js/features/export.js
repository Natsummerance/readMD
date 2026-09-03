'use strict';
/* ============================================================
   ReadMD Features - Document Export Console & High-Fidelity Preview
   ============================================================ */

/* ---------------- 导出面板（PDF / DOCX / HTML + 样式定制） ---------------- */

const EXPORT_FONTS = ['MicrosoftYaHei', 'SimHei', 'SimSun', 'KaiTi', 'DengXian', 'Arial'];
const EXPORT_MONO = ['Consolas', 'Courier New', 'SimHei'];
const EXPORT_ALIGNS = ['left', 'center', 'right', 'justify'];
const EXPORT_PAGES = ['A4', 'A5', 'B5', 'Letter', 'Legal'];

function getExportPresetNames() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  return {
    minimal: _t('export.presetMinimal') || '',
    classic: _t('export.presetClassic') || '',
    business: _t('export.presetBusiness') || ''
  };
}

function getExportSections() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  return [
    /* --- EPUB 电子书专属参数 --- */
    { title: _t('export.secEpubMeta') || '', fmts: ['epub'], fields: [
      { k: 'epub.title', label: _t('export.epubTitle') || '', type: 'text', full: true },
      { k: 'epub.author', label: _t('export.epubAuthor') || '', type: 'text' },
      { k: 'epub.publisher', label: _t('export.epubPublisher') || '', type: 'text' },
      { k: 'epub.isbn', label: _t('export.epubIsbn') || '', type: 'text' },
      { k: 'epub.language', label: _t('export.epubLanguage') || '', type: 'select', opts: [
        ['zh-CN', '简体中文 (zh-CN)'], ['en', 'English (en)'], ['ja', '日本語 (ja)'], ['zh-TW', '繁體中文 (zh-TW)'], ['fr', 'Français (fr)'], ['de', 'Deutsch (de)'], ['es', 'Español (es)']
      ]},
      { k: 'epub.splitLevel', label: _t('export.epubSplitLevel') || '', type: 'select', opts: [
        ['h1', _t('export.epubSplitH1') || ''],
        ['h2', _t('export.epubSplitH2') || ''],
        ['none', _t('export.epubSplitNone') || '']
      ], full: true },
    ]},
    { title: _t('export.secEpubStyle') || '', fmts: ['epub'], fields: [
      { k: 'epub.fontSize', label: _t('export.bodySize') || '', type: 'number', min: 8, max: 24 },
      { k: 'epub.lineHeight', label: _t('export.lineHeight') || '', type: 'number', min: 1.2, max: 2.5, step: 0.1 },
      { k: 'epub.marginV', label: _t('export.epubMarginV') || '', type: 'number', min: 0, max: 20 },
      { k: 'epub.marginH', label: _t('export.epubMarginH') || '', type: 'number', min: 0, max: 20 },
    ]},

    /* --- LaTeX 学术源码专属参数 --- */
    { title: _t('export.secLatexDoc') || '', fmts: ['tex'], fields: [
      { k: 'tex.docClass', label: _t('export.latexDocClass') || '', type: 'select', opts: [
        ['ctexart', 'ctexart'],
        ['article', 'article'],
        ['ctexrep', 'ctexrep'],
        ['report', 'report'],
        ['book', 'book'],
        ['beamer', 'beamer']
      ], full: true },
      { k: 'tex.fontSize', label: _t('export.latexFontSize') || '', type: 'select', opts: [
        ['10pt', '10pt'], ['11pt', '11pt'], ['12pt', '12pt']
      ]},
      { k: 'tex.paperSize', label: _t('export.pageSize') || '', type: 'select', opts: [
        ['a4paper', 'A4'], ['letterpaper', 'US Letter']
      ]},
      { k: 'tex.margin', label: _t('export.latexMargin') || '', type: 'select', opts: [
        ['2.5cm', '2.5 cm'], ['1in', '1 in'], ['2cm', '2.0 cm'], ['3cm', '3.0 cm']
      ]},
      { k: 'tex.bibEngine', label: _t('export.latexBibEngine') || '', type: 'select', opts: [
        ['biblatex', 'BibLaTeX'], ['natbib', 'Natbib'], ['bibtex', 'BibTeX']
      ]},
      { k: 'tex.useCtex', label: _t('export.latexUseCtex') || '', type: 'checkbox' },
    ]},

    /* --- PDF / DOCX 页面与版式 --- */
    { title: _t('export.secPage') || '', fmts: ['pdf', 'docx'], fields: [
      { k: 'page.size', label: _t('export.pageSize') || '', type: 'select', opts: EXPORT_PAGES },
      { k: 'page.orientation', label: _t('export.pageOrientation') || '', type: 'select', opts: [['portrait', _t('export.portrait') || ''], ['landscape', _t('export.landscape') || '']] },
      { k: 'page.marginTop', label: _t('export.marginTop') || '', type: 'number', min: 0, max: 60 },
      { k: 'page.marginRight', label: _t('export.marginRight') || '', type: 'number', min: 0, max: 60 },
      { k: 'page.marginBottom', label: _t('export.marginBottom') || '', type: 'number', min: 0, max: 60 },
      { k: 'page.marginLeft', label: _t('export.marginLeft') || '', type: 'number', min: 0, max: 60 },
    ]},
    { title: _t('export.secCoverToc') || '', fmts: ['pdf', 'docx'], fields: [
      { k: 'cover.enabled', label: _t('export.enableCover') || '', type: 'checkbox' },
      { k: 'cover.title', label: _t('export.coverTitle') || '', type: 'text', full: true },
      { k: 'cover.subtitle', label: _t('export.coverSubtitle') || '', type: 'text', full: true },
      { k: 'cover.date', label: _t('export.coverDate') || '', type: 'text' },
      { k: 'cover.align', label: _t('export.coverAlign') || '', type: 'select', opts: [['center', _t('export.alignCenter') || ''], ['left', _t('export.alignLeft') || ''], ['right', _t('export.alignRight') || '']] },
      { k: 'toc.enabled', label: _t('export.enablePdfToc') || '', type: 'checkbox', fmts: ['pdf'] },
    ]},
    { title: _t('export.secTypography') || '', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'typography.font', label: _t('export.bodyFont') || '', type: 'select', opts: EXPORT_FONTS.map(f => [f, f]) },
      { k: 'typography.size', label: _t('export.bodySize') || '', type: 'number', min: 8, max: 20 },
      { k: 'typography.lineHeight', label: _t('export.lineHeight') || '', type: 'number', min: 1, max: 2.5, step: 0.1 },
      { k: 'typography.spacing', label: _t('export.paragraphSpacing') || '', type: 'number', min: 0, max: 30 },
      { k: 'typography.color', label: _t('export.bodyColor') || '', type: 'color' },
      { k: 'typography.align', label: _t('export.align') || '', type: 'select', opts: [['left', _t('export.alignLeft') || ''], ['center', _t('export.alignCenter') || ''], ['right', _t('export.alignRight') || ''], ['justify', _t('export.alignJustify') || '']] },
    ]},
    { title: _t('export.secHeadings') || '', fmts: ['pdf', 'docx', 'html'], headingRows: true },
    { title: _t('export.secTable') || '', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'table.headerBg', label: _t('export.tableHeaderBg') || '', type: 'color' },
      { k: 'table.headerColor', label: _t('export.tableHeaderColor') || '', type: 'color' },
      { k: 'table.headerBold', label: _t('export.tableHeaderBold') || '', type: 'checkbox' },
      { k: 'table.borderColor', label: _t('export.tableBorderColor') || '', type: 'color' },
      { k: 'table.borderWidth', label: _t('export.tableBorderWidth') || '', type: 'number', min: 0, max: 3, step: 0.25 },
      { k: 'table.banded', label: _t('export.tableBanded') || '', type: 'checkbox' },
      { k: 'table.bandColor', label: _t('export.tableBandColor') || '', type: 'color' },
      { k: 'table.cellSize', label: _t('export.tableCellSize') || '', type: 'number', min: 7, max: 16 },
      { k: 'table.cellPadding', label: _t('export.tableCellPadding') || '', type: 'number', min: 0, max: 20 },
      { k: 'table.align', label: _t('export.align') || '', type: 'select', opts: [['left', _t('export.alignLeft') || ''], ['center', _t('export.alignCenter') || ''], ['right', _t('export.alignRight') || ''], ['justify', _t('export.alignJustify') || '']] },
      { k: 'table.widthPct', label: _t('export.tableWidthPct') || '', type: 'number', min: 50, max: 100 },
    ]},
    { title: _t('export.secCode') || '', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'code.bg', label: _t('export.codeBg') || '', type: 'color' },
      { k: 'code.color', label: _t('export.codeColor') || '', type: 'color' },
      { k: 'code.font', label: _t('export.codeFont') || '', type: 'select', opts: EXPORT_MONO.map(f => [f, f]) },
      { k: 'code.size', label: _t('export.codeSize') || '', type: 'number', min: 6, max: 16 },
      { k: 'code.borderColor', label: _t('export.codeBorderColor') || '', type: 'color' },
      { k: 'code.borderWidth', label: _t('export.codeBorderWidth') || '', type: 'number', min: 0, max: 3, step: 0.25 },
      { k: 'code.rounded', label: _t('export.codeRounded') || '', type: 'checkbox', fmts: ['html'] },
    ]},
    { title: _t('export.secQuoteLink') || '', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'quote.barColor', label: _t('export.quoteBarColor') || '', type: 'color' },
      { k: 'quote.bg', label: _t('export.quoteBg') || '', type: 'color' },
      { k: 'quote.color', label: _t('export.quoteColor') || '', type: 'color' },
      { k: 'link.color', label: _t('export.linkColor') || '', type: 'color' },
      { k: 'hr.color', label: _t('export.hrColor') || '', type: 'color' },
    ]},
    { title: _t('export.secFooterMeta') || '', fmts: ['pdf', 'docx'], fields: [
      { k: 'footer.pageNumbers', label: _t('export.showPageNumbers') || '', type: 'checkbox' },
      { k: 'footer.text', label: _t('export.footerTextLabel') || '', type: 'text', full: true },
      { k: 'meta.title', label: _t('export.docMetaTitle') || '', type: 'text', full: true },
      { k: 'meta.author', label: _t('export.metaAuthor') || '', type: 'text' },
      { k: 'meta.subject', label: _t('export.metaSubject') || '', type: 'text' },
    ]},
    { title: _t('export.secMath') || '', fmts: ['pdf', 'docx'], fields: [
      { k: 'math.dpi', label: _t('export.mathDpi') || '', type: 'number', min: 100, max: 500, step: 10 },
    ]},
    { title: _t('export.secHtmlTheme') || '', fmts: ['html'], fields: [
      { k: 'htmlTheme', label: _t('export.htmlThemeLabel') || '', type: 'select', opts: [['light', _t('export.themeLight') || ''], ['dark', _t('export.themeDark') || ''], ['sepia', _t('export.themeSepia') || '']] },
    ]},
  ];
}


function expGet(obj, path) {
  return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj);
}
function expSet(obj, path, val) {
  const ks = path.split('.');
  let o = obj;
  for (let i = 0; i < ks.length - 1; i++) {
    if (typeof o[ks[i]] !== 'object' || o[ks[i]] === null) o[ks[i]] = {};
    o = o[ks[i]];
  }
  o[ks[ks.length - 1]] = val;
}
function expDeepMerge(base, over) {
  const out = JSON.parse(JSON.stringify(base || {}));
  if (!over || typeof over !== 'object') return out;
  Object.keys(over).forEach(k => {
    const v = over[k];
    if (v && typeof v === 'object' && !Array.isArray(v) && out[k] && typeof out[k] === 'object') {
      out[k] = expDeepMerge(out[k], v);
    } else if (v !== undefined) out[k] = JSON.parse(JSON.stringify(v));
  });
  return out;
}

async function loadExportPresets() {
  if (state.export.defaults) return true;
  if (!bindPy()) return false;
  try {
    const d = await py.get_export_presets();
    if (!d || d.error) throw new Error((d && d.error) || 'no data');
    state.export.defaults = d.defaults || {};
    state.export.presets = d.presets || {};
    state.export.custom = d.custom || {};
    state.export.last = d.last || null;
    if (state.export.last && state.export.last.options) {
      state.export.options = expDeepMerge(state.export.defaults, state.export.last.options);
    } else {
      state.export.options = expDeepMerge(state.export.defaults, {});
    }
    return true;
  } catch (e) {
    console.error(e);
    return false;
  }
}

function openExportModal() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const editorContent = typeof getEditContent === 'function' ? getEditContent() : '';
  const exportContent = (state.editing ? editorContent : '') || state.original || state.fixed || '';
  if (state.mode === 'welcome' || !exportContent) {
    showToast(_t('toast.openDocumentToUse') || '');
    return;
  }
  if (!bindPy()) { showToast(_t('toast.exportBrowserNotice') || ''); return; }
  if (!state.export.ready) {
    loadExportPresets().then(ok => {
      if (ok) { state.export.ready = true; renderExportModal(); }
      else showToast(_t('toast.exportModuleLoadFail') || '');
    });
    return;
  }
  renderExportModal();
}

function closeExportModal() { $('export-modal').classList.add('hidden'); }

function currentExportContent() {
  if (state.editing) {
    if (typeof getEditContent === 'function') {
      const txt = getEditContent();
      if (txt) return txt;
    }
    return ($('edit-area') && $('edit-area').value) || state.original || state.fixed || '';
  }
  if (state.mode === 'file') return state.original || state.fixed || '';
  return state.fixed || state.original || '';
}
function currentExportName() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  let n = '';
  if (state.mode === 'file' && state.file) n = state.file.split(/[\\/]/).pop();
  else n = (state.sourceName || (_t('export.defaultExportName') || '')).split(/[\\/]/).pop();
  n = n.replace(/\.[^.]+$/, '');
  return n || (_t('export.defaultExportName') || '');
}


function renderExportModal() {
  $('export-modal').classList.remove('hidden');
  renderExportPresetSelect();
  renderExportSections();
  initExportAiDesigner();
  updateExportLivePreview();
  const r = $('export-result');
  r.textContent = ''; r.className = 'export-result';
  $('export-open').classList.add('hidden');
  $('export-reveal').classList.add('hidden');
}

function generateExportPreviewCss(opts, fmt) {
  opts = opts || {};
  const ty = opts.typography || {};
  const hd = opts.headings || {};
  const tb = opts.table || {};
  const code = opts.code || {};
  const quote = opts.quote || {};
  const link = opts.link || {};
  const hr = opts.hr || {};
  const page = opts.page || {};
  const htmlTheme = opts.htmlTheme || 'light';

  const fontMap = {
    'MicrosoftYaHei': '"Microsoft YaHei", "Microsoft YaHei UI", "PingFang SC", "微软雅黑", sans-serif',
    'SimHei': '"SimHei", "黑体", sans-serif',
    'SimSun': '"SimSun", "宋体", serif',
    'KaiTi': '"KaiTi", "楷体", serif',
    'DengXian': '"DengXian", "等线", sans-serif',
    'Arial': 'Arial, sans-serif',
  };
  const fontFamily = fontMap[ty.font] || fontMap['MicrosoftYaHei'];

  let pageBg = '#ffffff';
  let baseFg = ty.color || '#262626';
  let codeBg = code.bg || '#f5f6f8';
  let quoteBg = quote.bg || '#f3f6ff';

  if (fmt === 'html') {
    if (htmlTheme === 'dark') {
      pageBg = '#14161a';
      baseFg = '#d6d9de';
      codeBg = '#1e2228';
      quoteBg = '#1c2230';
    } else if (htmlTheme === 'sepia') {
      pageBg = '#faf4e7';
      baseFg = '#3b2f1d';
      codeBg = '#f2ecdd';
      quoteBg = '#f2ead6';
    }
  }

  const h1 = hd.h1 || { size: 20, color: '#1a1a1a', bold: true, align: 'left', before: 18, after: 10 };
  const h2 = hd.h2 || { size: 16, color: '#1f2937', bold: true, align: 'left', before: 14, after: 8 };
  const h3 = hd.h3 || { size: 14, color: '#2d3748', bold: true, align: 'left', before: 12, after: 6 };
  const h4 = hd.h4 || { size: 12, color: '#374151', bold: true, align: 'left', before: 10, after: 6 };
  const h5 = hd.h5 || { size: 11, color: '#4a5568', bold: true, align: 'left', before: 8, after: 4 };
  const h6 = hd.h6 || { size: 10.5, color: '#4a5568', bold: true, align: 'left', before: 8, after: 4 };

  return `
    /* Mini Preview Dynamic Styling */
    #export-preview-mini-page {
      background: ${pageBg} !important;
      color: ${baseFg} !important;
      font-family: ${fontFamily} !important;
      text-align: ${ty.align || 'left'} !important;
    }
    #export-preview-mini-content {
      color: ${baseFg} !important;
      font-family: ${fontFamily} !important;
      font-size: 3.5px !important;
      line-height: ${ty.lineHeight || 1.6} !important;
    }
    #export-preview-mini-content p, #export-preview-mini-content li, #export-preview-mini-content span, #export-preview-mini-content div {
      color: ${baseFg} !important;
      font-size: ${(ty.size || 11) * 0.32}px !important;
      line-height: ${ty.lineHeight || 1.6} !important;
      text-align: ${ty.align || 'left'} !important;
    }
    #export-preview-mini-content p {
      margin: ${(ty.spacing || 6) * 0.25}px 0 !important;
    }
    #export-preview-mini-content h1 {
      color: ${h1.color || '#1a1a1a'} !important;
      font-size: ${(h1.size || 20) * 0.35}px !important;
      font-weight: ${h1.bold ? 'bold' : 'normal'} !important;
      text-align: ${h1.align || 'left'} !important;
      margin-top: ${(h1.before || 18) * 0.2}px !important;
      margin-bottom: ${(h1.after || 10) * 0.2}px !important;
      border-bottom: none !important;
    }
    #export-preview-mini-content h2 {
      color: ${h2.color || '#1f2937'} !important;
      font-size: ${(h2.size || 16) * 0.35}px !important;
      font-weight: ${h2.bold ? 'bold' : 'normal'} !important;
      text-align: ${h2.align || 'left'} !important;
      margin-top: ${(h2.before || 14) * 0.2}px !important;
      margin-bottom: ${(h2.after || 8) * 0.2}px !important;
      border-bottom: none !important;
    }
    #export-preview-mini-content h3 {
      color: ${h3.color || '#2d3748'} !important;
      font-size: ${(h3.size || 14) * 0.35}px !important;
      font-weight: ${h3.bold ? 'bold' : 'normal'} !important;
      text-align: ${h3.align || 'left'} !important;
      margin-top: ${(h3.before || 12) * 0.2}px !important;
      margin-bottom: ${(h3.after || 6) * 0.2}px !important;
    }
    #export-preview-mini-content h4, #export-preview-mini-content h5, #export-preview-mini-content h6 {
      color: ${h4.color || '#374151'} !important;
      font-size: ${(h4.size || 12) * 0.35}px !important;
      font-weight: ${h4.bold ? 'bold' : 'normal'} !important;
      text-align: ${h4.align || 'left'} !important;
    }
    #export-preview-mini-content table {
      border-collapse: collapse !important;
      width: ${tb.widthPct || 100}% !important;
      margin: 3px auto !important;
      font-size: ${(tb.cellSize || 10) * 0.32}px !important;
    }
    #export-preview-mini-content th, #export-preview-mini-content td {
      border: 0.5px solid ${tb.borderColor || '#c8cdd4'} !important;
      padding: 1px 2px !important;
      text-align: ${tb.align || 'left'} !important;
      color: ${baseFg} !important;
    }
    #export-preview-mini-content th {
      background: ${tb.headerBg || '#3b6ef5'} !important;
      color: ${tb.headerColor || '#ffffff'} !important;
      font-weight: ${tb.headerBold ? 'bold' : 'normal'} !important;
    }
    #export-preview-mini-content tbody tr:nth-child(even) td {
      background: ${tb.banded ? (tb.bandColor || '#f3f5f9') : 'transparent'} !important;
    }
    #export-preview-mini-content pre {
      background: ${codeBg} !important;
      color: ${code.color || '#2f3b4a'} !important;
      border: 0.5px solid ${code.borderColor || '#dfe3e8'} !important;
      border-radius: ${code.rounded ? '2px' : '0'} !important;
      padding: 2px 3px !important;
      font-size: ${(code.size || 9.5) * 0.32}px !important;
      margin: 2px 0 !important;
    }
    #export-preview-mini-content code {
      font-family: ${code.font || 'Consolas'}, Consolas, monospace !important;
    }
    #export-preview-mini-content :not(pre) > code {
      background: ${codeBg} !important;
      color: #c7254e !important;
      padding: 0 1px !important;
    }
    #export-preview-mini-content blockquote {
      margin: 2px 0 !important;
      padding: 1px 4px !important;
      background: ${quoteBg} !important;
      color: ${quote.color || '#4a5568'} !important;
      border-left: 2px solid ${quote.barColor || '#3b6ef5'} !important;
    }
    #export-preview-mini-content blockquote p {
      color: ${quote.color || '#4a5568'} !important;
    }
    #export-preview-mini-content a {
      color: ${link.color || '#2b6cb0'} !important;
    }
    #export-preview-mini-content hr {
      border: none !important;
      border-top: 0.5px solid ${hr.color || '#d8dce2'} !important;
      margin: 3px 0 !important;
    }

    /* Full Modal Preview Dynamic Styling */
    #export-preview-full-page {
      width: 100% !important;
      height: auto !important;
      min-height: 100% !important;
      overflow: visible !important;
      display: flex !important;
      flex-direction: column !important;
      align-items: center !important;
      gap: 24px !important;
      background: transparent !important;
      box-shadow: none !important;
      padding: 0 !important;
    }
    .export-preview-page-sheet {
      background: ${pageBg} !important;
      color: ${baseFg} !important;
      font-family: ${fontFamily} !important;
      font-size: ${ty.size || 11}pt !important;
      line-height: ${ty.lineHeight || 1.6} !important;
      text-align: ${ty.align || 'left'} !important;
      padding: ${page.marginTop || 20}mm ${page.marginRight || 18}mm ${page.marginBottom || 20}mm ${page.marginLeft || 18}mm !important;
      ${page.orientation === 'landscape' ? 'width: 297mm; height: 210mm;' : 'width: 210mm; height: 297mm;'}
      box-sizing: border-box !important;
      overflow: hidden !important;
      display: flex !important;
      flex-direction: column !important;
      justify-content: space-between !important;
      box-shadow: 0 4px 24px rgba(0, 0, 0, 0.35) !important;
      border-radius: 2px !important;
      margin-bottom: 24px !important;
      flex-shrink: 0 !important;
    }
    .export-page-body {
      flex: 1 1 auto !important;
      min-height: 0 !important;
      overflow: hidden !important;
      word-break: break-word !important;
    }
    #export-preview-full-page p, .export-preview-page-sheet p, .export-page-body p, #export-preview-full-page li, .export-page-body li, #export-preview-full-page span, .export-page-body span, #export-preview-full-page div, .export-page-body div {
      color: ${baseFg} !important;
      font-size: ${ty.size || 11}pt !important;
      line-height: ${ty.lineHeight || 1.6} !important;
      text-align: ${ty.align || 'left'} !important;
    }
    #export-preview-full-page p, .export-page-body p {
      margin: ${ty.spacing || 6}pt 0 !important;
    }
    #export-preview-full-page h1, .export-page-body h1 {
      color: ${h1.color || '#1a1a1a'} !important;
      font-size: ${h1.size || 20}pt !important;
      font-weight: ${h1.bold ? 'bold' : 'normal'} !important;
      text-align: ${h1.align || 'left'} !important;
      margin-top: ${h1.before || 18}pt !important;
      margin-bottom: ${h1.after || 10}pt !important;
      line-height: 1.35 !important;
      border-bottom: none !important;
    }
    #export-preview-full-page h2, .export-page-body h2 {
      color: ${h2.color || '#1f2937'} !important;
      font-size: ${h2.size || 16}pt !important;
      font-weight: ${h2.bold ? 'bold' : 'normal'} !important;
      text-align: ${h2.align || 'left'} !important;
      margin-top: ${h2.before || 14}pt !important;
      margin-bottom: ${h2.after || 8}pt !important;
      line-height: 1.35 !important;
      border-bottom: none !important;
    }
    #export-preview-full-page h3, .export-page-body h3 {
      color: ${h3.color || '#2d3748'} !important;
      font-size: ${h3.size || 14}pt !important;
      font-weight: ${h3.bold ? 'bold' : 'normal'} !important;
      text-align: ${h3.align || 'left'} !important;
      margin-top: ${h3.before || 12}pt !important;
      margin-bottom: ${h3.after || 6}pt !important;
      line-height: 1.35 !important;
    }
    #export-preview-full-page h4, .export-page-body h4 {
      color: ${h4.color || '#374151'} !important;
      font-size: ${h4.size || 12}pt !important;
      font-weight: ${h4.bold ? 'bold' : 'normal'} !important;
      text-align: ${h4.align || 'left'} !important;
      margin-top: ${h4.before || 10}pt !important;
      margin-bottom: ${h4.after || 6}pt !important;
    }
    #export-preview-full-page h5, .export-page-body h5 {
      color: ${h5.color || '#4a5568'} !important;
      font-size: ${h5.size || 11}pt !important;
      font-weight: ${h5.bold ? 'bold' : 'normal'} !important;
      text-align: ${h5.align || 'left'} !important;
      margin-top: ${h5.before || 8}pt !important;
      margin-bottom: ${h5.after || 4}pt !important;
    }
    #export-preview-full-page h6, .export-page-body h6 {
      color: ${h6.color || '#4a5568'} !important;
      font-size: ${h6.size || 10.5}pt !important;
      font-weight: ${h6.bold ? 'bold' : 'normal'} !important;
      text-align: ${h6.align || 'left'} !important;
      margin-top: ${h6.before || 8}pt !important;
      margin-bottom: ${h6.after || 4}pt !important;
    }
    #export-preview-full-page table, .export-page-body table {
      border-collapse: collapse !important;
      width: ${tb.widthPct || 100}% !important;
      margin: 12pt auto !important;
      font-size: ${tb.cellSize || 10}pt !important;
    }
    #export-preview-full-page th, .export-page-body th, #export-preview-full-page td, .export-page-body td {
      border: ${tb.borderWidth || 0.75}px solid ${tb.borderColor || '#c8cdd4'} !important;
      padding: ${tb.cellPadding || 6}px !important;
      text-align: ${tb.align || 'left'} !important;
      color: ${baseFg} !important;
    }
    #export-preview-full-page th, .export-page-body th {
      background: ${tb.headerBg || '#3b6ef5'} !important;
      color: ${tb.headerColor || '#ffffff'} !important;
      font-weight: ${tb.headerBold ? 'bold' : 'normal'} !important;
    }
    #export-preview-full-page tbody tr:nth-child(even) td, .export-page-body tbody tr:nth-child(even) td {
      background: ${tb.banded ? (tb.bandColor || '#f3f5f9') : 'transparent'} !important;
    }
    #export-preview-full-page pre, .export-page-body pre {
      background: ${codeBg} !important;
      color: ${code.color || '#2f3b4a'} !important;
      border: ${code.borderWidth || 0.5}px solid ${code.borderColor || '#dfe3e8'} !important;
      border-radius: ${code.rounded ? '6px' : '0'} !important;
      padding: 10pt 12pt !important;
      overflow: auto !important;
      font-family: ${code.font || 'Consolas'}, Consolas, monospace !important;
      font-size: ${code.size || 9.5}pt !important;
      line-height: 1.5 !important;
      margin: 8pt 0 !important;
    }
    #export-preview-full-page code, .export-page-body code {
      font-family: ${code.font || 'Consolas'}, Consolas, monospace !important;
    }
    #export-preview-full-page :not(pre) > code, .export-page-body :not(pre) > code {
      background: ${codeBg} !important;
      color: #c7254e !important;
      padding: 2px 5px !important;
      border-radius: 4px !important;
      font-size: 0.92em !important;
    }
    #export-preview-full-page blockquote, .export-page-body blockquote {
      margin: 8pt 0 !important;
      padding: 8pt 14pt !important;
      background: ${quoteBg} !important;
      color: ${quote.color || '#4a5568'} !important;
      border-left: 4px solid ${quote.barColor || '#3b6ef5'} !important;
    }
    #export-preview-full-page blockquote p, .export-page-body blockquote p {
      color: ${quote.color || '#4a5568'} !important;
      margin: 4pt 0 !important;
    }
    #export-preview-full-page a, .export-page-body a {
      color: ${link.color || '#2b6cb0'} !important;
      text-decoration: underline !important;
    }
    #export-preview-full-page hr, .export-page-body hr {
      border: none !important;
      border-top: 1px solid ${hr.color || '#d8dce2'} !important;
      margin: 14pt 0 !important;
    }
    #export-preview-full-page img, .export-page-body img {
      max-width: 100% !important;
      height: auto !important;
    }
    .export-page-header, .export-page-footer {
      color: ${baseFg} !important;
      opacity: 0.65;
      border-color: ${baseFg}33 !important;
    }
  `;
}

/**
 * 真实渲染 DOM 像素高度测量与智能切分引擎
 * 保证导出预览与真实 PDF / Word A4 导出页面 100% 完全一致
 */
function paginateHtmlIntoExportSheets(fullHtml, opts = {}) {
  const page = opts.page || {};
  const isLandscape = page.orientation === 'landscape';

  // A4 标准毫米规格与边距
  const PAGE_WIDTH_MM = isLandscape ? 297 : 210;
  const PAGE_HEIGHT_MM = isLandscape ? 210 : 297;
  const marginTop = Number(page.marginTop) || 20;
  const marginBottom = Number(page.marginBottom) || 20;
  const marginLeft = Number(page.marginLeft) || 18;
  const marginRight = Number(page.marginRight) || 18;

  // 转换为 px (1mm = 3.7795px at 96 DPI)
  const MM_TO_PX = 3.779527559;
  const contentWidthPx = Math.max(200, (PAGE_WIDTH_MM - marginLeft - marginRight) * MM_TO_PX);
  // 可用正文高度（扣除上下边距和页眉页脚预留）
  const usableBodyHeightPx = Math.max(200, (PAGE_HEIGHT_MM - marginTop - marginBottom - 18) * MM_TO_PX);

  // 1. 创建离屏度量容器
  const measureHost = document.createElement('div');
  measureHost.id = 'export-preview-measure-host';
  measureHost.style.cssText = `
    position: absolute !important;
    left: -9999px !important;
    top: 0 !important;
    visibility: hidden !important;
    width: ${contentWidthPx}px !important;
    box-sizing: border-box !important;
    pointer-events: none !important;
    word-break: break-word !important;
  `;
  measureHost.className = 'export-page-body';
  measureHost.innerHTML = fullHtml;
  document.body.appendChild(measureHost);
  renderMath(measureHost);

  const pages = [];
  let currentPageElements = [];
  let currentHeight = 0;

  function pushCurrentPage() {
    if (currentPageElements.length > 0) {
      const container = document.createElement('div');
      currentPageElements.forEach(el => container.appendChild(el));
      pages.push(container.innerHTML);
      currentPageElements = [];
      currentHeight = 0;
    }
  }

  const childNodes = Array.from(measureHost.children);

  if (childNodes.length === 0) {
    document.body.removeChild(measureHost);
    return [fullHtml || ''];
  }

  for (let i = 0; i < childNodes.length; i++) {
    const el = childNodes[i];
    const rect = el.getBoundingClientRect();
    const computed = window.getComputedStyle(el);
    const mTop = parseFloat(computed.marginTop) || 0;
    const mBottom = parseFloat(computed.marginBottom) || 0;
    const blockHeight = (rect.height || el.offsetHeight || 20) + mTop + mBottom;

    // 手动分页符
    if (el.tagName === 'DIV' && el.style.pageBreakAfter === 'always') {
      pushCurrentPage();
      continue;
    }

    // 判断当前页是否放得下
    if (currentHeight + blockHeight <= usableBodyHeightPx) {
      currentPageElements.push(el.cloneNode(true));
      currentHeight += blockHeight;
    } else {
      // 放不下了，如果当前页已有内容，先落页
      if (currentPageElements.length > 0) {
        pushCurrentPage();
      }

      // 如果单个元素高度本身就超过单页（例如超长段落、超长表格或超长代码块）
      if (blockHeight > usableBodyHeightPx) {
        if (el.tagName === 'TABLE') {
          // 表格按行拆分
          const rows = Array.from(el.querySelectorAll('tr'));
          const thead = el.querySelector('thead');
          let tablePart = document.createElement('table');
          tablePart.className = el.className;
          let tbody = document.createElement('tbody');
          tablePart.appendChild(tbody);
          if (thead) tablePart.appendChild(thead.cloneNode(true));

          let subHeight = thead ? 36 : 0;
          rows.forEach(tr => {
            if (tr.parentElement && tr.parentElement.tagName === 'THEAD') return;
            const rH = tr.offsetHeight || 28;
            if (subHeight + rH > usableBodyHeightPx && tbody.children.length > 0) {
              currentPageElements.push(tablePart);
              pushCurrentPage();
              tablePart = document.createElement('table');
              tablePart.className = el.className;
              if (thead) tablePart.appendChild(thead.cloneNode(true));
              tbody = document.createElement('tbody');
              tablePart.appendChild(tbody);
              subHeight = thead ? 36 : 0;
            }
            tbody.appendChild(tr.cloneNode(true));
            subHeight += rH;
          });
          if (tbody.children.length > 0) {
            currentPageElements.push(tablePart);
            currentHeight = subHeight;
          }
        } else if (el.tagName === 'P' || el.tagName === 'BLOCKQUOTE') {
          // 长段落拆分
          const text = el.innerText || el.textContent || '';
          const sentences = text.split(/(?<=[。！？\.\!\?\n])/);
          let pPart = document.createElement(el.tagName.toLowerCase());
          pPart.className = el.className;
          let subText = '';

          sentences.forEach(s => {
            pPart.textContent = subText + s;
            measureHost.appendChild(pPart);
            const pH = pPart.offsetHeight;
            measureHost.removeChild(pPart);

            if (currentHeight + pH > usableBodyHeightPx && subText.length > 0) {
              pPart.textContent = subText;
              currentPageElements.push(pPart.cloneNode(true));
              pushCurrentPage();
              pPart = document.createElement(el.tagName.toLowerCase());
              pPart.className = el.className;
              subText = s;
            } else {
              subText += s;
            }
          });
          if (subText.length > 0) {
            pPart.textContent = subText;
            currentPageElements.push(pPart);
            currentHeight = pPart.offsetHeight || 20;
          }
        } else {
          // 其他不可拆分块（如 code block, img），直接整块放入新页
          currentPageElements.push(el.cloneNode(true));
          currentHeight = blockHeight;
        }
      } else {
        currentPageElements.push(el.cloneNode(true));
        currentHeight = blockHeight;
      }
    }
  }

  pushCurrentPage();
  document.body.removeChild(measureHost);

  return pages.length > 0 ? pages : [fullHtml];
}

function updateExportLivePreview() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const fmt = state.export.fmt || 'pdf';
  const opts = collectExportOptions();
  const badge = $('export-preview-badge');
  const sel = $('exp-preset');
  const presetName = (sel && sel.selectedIndex >= 0) ? sel.options[sel.selectedIndex].text : (_t('export.presetDefault') || '');
  if (badge) badge.textContent = fmt.toUpperCase() + ' · ' + presetName;

  const content = currentExportContent();
  const docTitle = currentExportName();

  // 注入或更新动态样式表
  let styleEl = $('export-preview-dynamic-style');
  if (!styleEl) {
    styleEl = document.createElement('style');
    styleEl.id = 'export-preview-dynamic-style';
    document.head.appendChild(styleEl);
  }
  styleEl.textContent = generateExportPreviewCss(opts, fmt);

  const isHtmlMode = fmt === 'html';
  const fullProt = protectMath(content || '');
  const fullParsedHtml = marked.parse(fullProt.src, { gfm: true, breaks: false });
  const restoredFullHtml = restoreMath(fullParsedHtml, fullProt.saved);

  const pageHtmlList = isHtmlMode ? [restoredFullHtml] : paginateHtmlIntoExportSheets(restoredFullHtml, opts);
  const totalPages = pageHtmlList.length;

  const paperMeta = $('export-preview-paper-meta');
  if (paperMeta) {
    const page = opts.page || {};
    const sz = page.size || 'A4';
    const ori = (page.orientation === 'landscape') ? (_t('export.orientationLandscape') || '') : (_t('export.orientationPortrait') || '');
    const pageText = isHtmlMode ? 'HTML Web' : (_t('reader.totalPage', { total: totalPages }) || `共 ${totalPages} 页`);
    paperMeta.textContent = `${sz} · ${ori} · ${pageText} · ${presetName}`;
  }

  // Mini Preview 侧边栏预览
  const miniHost = $('export-preview-mini-content');
  if (miniHost) {
    miniHost.innerHTML = pageHtmlList[0] || restoredFullHtml;
    renderMath(miniHost);
  }

  // Full Modal Preview 真实多页排版渲染
  const fullModal = $('export-preview-modal');
  if (fullModal && !fullModal.classList.contains('hidden')) {
    const wrapper = fullModal.querySelector('.export-preview-paper-wrapper');
    if (wrapper) {
      // Keep the stable full-page host in the DOM.  Besides preserving CSS
      // hooks, this makes the preview accessible to keyboard/screen-reader
      // clients and avoids tests or extensions losing their target after a
      // style refresh.
      wrapper.innerHTML = '<div id="export-preview-full-page" class="export-preview-full-page"></div>';
      const fullPageHost = wrapper.querySelector('#export-preview-full-page');

      const esc = s => String(s || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

      pageHtmlList.forEach((pageHtml, index) => {
        const sheet = document.createElement('div');
        sheet.className = isHtmlMode ? 'export-preview-page-sheet export-preview-html-sheet' : 'export-preview-page-sheet';
        sheet.dataset.page = (index + 1).toString();

        if (!isHtmlMode) {
          const headerEl = document.createElement('div');
          headerEl.className = 'export-page-header';
          headerEl.innerHTML = `<span>${esc(docTitle)}</span><span>${fmt.toUpperCase()} · ${esc(presetName)}</span>`;
          sheet.appendChild(headerEl);
        }

        const bodyEl = document.createElement('div');
        bodyEl.className = 'export-page-body';
        bodyEl.innerHTML = pageHtml;
        sheet.appendChild(bodyEl);

        if (!isHtmlMode) {
          const footerEl = document.createElement('div');
          footerEl.className = 'export-page-footer';
          footerEl.innerHTML = `<span>ReadMD</span><span>${index + 1} / ${totalPages}</span>`;
          sheet.appendChild(footerEl);
        }

        fullPageHost.appendChild(sheet);
        renderMath(bodyEl);
      });

      const pagesMeta = $('export-preview-pages-meta');
      if (pagesMeta) {
        pagesMeta.textContent = (window.i18n ? window.i18n.t('export.previewPagesMeta', { total: totalPages }) : '') || `共 ${totalPages} 页`;
      }
      const prevBtn = $('export-preview-prev-btn');
      const nextBtn = $('export-preview-next-btn');
      if (prevBtn && nextBtn) {
        if (totalPages > 1) {
          prevBtn.style.display = 'inline-flex';
          nextBtn.style.display = 'inline-flex';
          let curIdx = 0;
          prevBtn.onclick = () => {
            const sheets = fullPageHost.querySelectorAll('.export-preview-page-sheet');
            if (curIdx > 0) {
              curIdx--;
              sheets[curIdx]?.scrollIntoView({ behavior: 'smooth', block: 'start' });
            }
          };
          nextBtn.onclick = () => {
            const sheets = fullPageHost.querySelectorAll('.export-preview-page-sheet');
            if (curIdx < sheets.length - 1) {
              curIdx++;
              sheets[curIdx]?.scrollIntoView({ behavior: 'smooth', block: 'start' });
            }
          };
        } else {
          prevBtn.style.display = 'none';
          nextBtn.style.display = 'none';
        }
      }
    }
  }
}

function expFieldApplicable(f, secFmts, fmt) {
  const fmts = f.fmts || secFmts || ['pdf', 'docx', 'html'];
  return fmts.indexOf(fmt) >= 0;
}

function renderExportSections() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const fmt = state.export.fmt || 'pdf';
  const host = $('export-opts');
  host.textContent = '';
  const sections = getExportSections();
  let renderedCount = 0;

  sections.forEach(sec => {
    const secFmts = sec.fmts || ['pdf', 'docx', 'html'];
    if (secFmts.indexOf(fmt) < 0) return;
    const fields = sec.fields || [];
    const applicable = sec.headingRows ? (secFmts.indexOf(fmt) >= 0)
      : fields.some(f => expFieldApplicable(f, secFmts, fmt));
    if (!applicable) return;

    const wrap = document.createElement('div');
    wrap.className = 'exp-sec'; // 默认折叠，点击标题展开
    if (renderedCount === 0) wrap.classList.add('open'); // 首个选项区默认展开
    renderedCount++;

    const head = document.createElement('button');
    head.type = 'button';
    head.className = 'exp-sec-head';
    head.innerHTML = '<span class="exp-arrow">&#9654;</span>' + sec.title;
    const body = document.createElement('div');
    body.className = 'exp-sec-body';
    if (sec.headingRows) {
      for (let i = 1; i <= 6; i++) {
        const row = document.createElement('div');
        row.className = 'exp-field full exp-h-row';
        row.innerHTML =
          '<label>H' + i + '</label>' +
          '<input type="number" data-k="headings.h' + i + '.size" min="8" max="40" title="' + (_t('export.bodySize') || '') + '">' +
          '<input type="color" data-k="headings.h' + i + '.color" title="' + (_t('export.bodyColor') || '') + '">' +
          '<label class="exp-check">' + (_t('export.bold') || '') + '<input type="checkbox" data-k="headings.h' + i + '.bold"></label>' +
          '<select data-k="headings.h' + i + '.align">' + EXPORT_ALIGNS.map(a => '<option value="' + a + '">' + a + '</option>').join('') + '</select>';
        body.appendChild(row);
      }
    } else {
      fields.forEach(f => {
        if (!expFieldApplicable(f, secFmts, fmt)) return;
        body.appendChild(expFieldEl(f));
      });
    }
    head.addEventListener('click', () => wrap.classList.toggle('open'));
    wrap.appendChild(head);
    wrap.appendChild(body);
    host.appendChild(wrap);
  });
  applyExportOptionsToDom();

  // 绑定配置项实时变动事件
  host.querySelectorAll('input, select').forEach(el => {
    const onValChange = () => {
      const sel = $('exp-preset');
      if (sel) sel.value = '__custom__';
      updateExportLivePreview();
    };
    el.addEventListener('input', onValChange);
    el.addEventListener('change', onValChange);
  });

  updateExportLivePreview();
}



function expFieldEl(f) {
  const box = document.createElement('div');
  box.className = 'exp-field' + (f.full ? ' full' : '');
  const fieldId = 'exp-field-' + f.k.replace(/[^a-z0-9_-]+/ig, '-') + '-' + Math.random().toString(36).slice(2, 7);
  let inner = '<label for="' + fieldId + '">' + f.label + '</label>';
  if (f.type === 'select') {
    inner += '<select data-k="' + f.k + '">' + (f.opts || []).map(o =>
      '<option value="' + (Array.isArray(o) ? o[0] : o) + '">' + (Array.isArray(o) ? o[1] : o) + '</option>'
    ).join('') + '</select>';
  } else if (f.type === 'checkbox') {
    inner = '<label class="exp-check"><input id="' + fieldId + '" type="checkbox" data-k="' + f.k + '"> ' + f.label + '</label>';
  } else if (f.type === 'color') {
    inner += '<input id="' + fieldId + '" type="color" data-k="' + f.k + '">';
  } else if (f.type === 'number') {
    inner += '<input id="' + fieldId + '" type="number" data-k="' + f.k + '" min="' + (f.min != null ? f.min : '') + '" max="' + (f.max != null ? f.max : '') + '" step="' + (f.step != null ? f.step : '1') + '">';
  } else {
    inner += '<input id="' + fieldId + '" type="text" data-k="' + f.k + '">';
  }
  box.innerHTML = inner;
  return box;
}

function applyExportOptionsToDom() {
  const opts = state.export.options || {};
  document.querySelectorAll('#export-opts [data-k]').forEach(el => {
    const v = expGet(opts, el.dataset.k);
    if (v === undefined || v === null) return;
    if (el.type === 'checkbox') el.checked = !!v;
    else el.value = v;
  });
}

function collectExportOptions() {
  const opts = expDeepMerge(state.export.defaults, {});
  document.querySelectorAll('#export-opts [data-k]').forEach(el => {
    let v;
    if (el.type === 'checkbox') v = el.checked;
    else if (el.type === 'number') v = parseFloat(el.value);
    else v = el.value;
    expSet(opts, el.dataset.k, v);
  });
  return opts;
}

// AI providers sometimes return a flat dotted object while the exporter uses
// nested options. Normalize both forms and drop unknown keys before merging so
// a model cannot mutate unrelated export state.
function normalizeExportAiPayload(value) {
  const allowed = new Set(['typography', 'headings', 'table', 'page', 'epub']);
  const ALLOWED_EPUB_KEYS = new Set([
    'title', 'author', 'publisher', 'isbn', 'language', 'cover',
    'splitLevel', 'fontSize', 'lineHeight', 'marginV', 'marginH',
    'css', 'toc', 'generateToc'
  ]);
  const out = {};
  const visit = (obj, prefix = '') => {
    if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return;
    Object.entries(obj).forEach(([key, val]) => {
      const full = prefix ? prefix + '.' + key : key;
      if (full.includes('.')) {
        const parts = full.split('.');
        if (!allowed.has(parts[0]) || parts.length > 4) return;
        if (parts[0] === 'epub' && !ALLOWED_EPUB_KEYS.has(parts[1])) return;
        expSet(out, full, val);
      } else if (allowed.has(key) && val && typeof val === 'object' && !Array.isArray(val)) {
        visit(val, full);
      }
    });
  };
  visit(value);
  return out;
}

function renderExportPresetSelect() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = $('exp-preset');
  sel.textContent = '';
  const presetNames = getExportPresetNames();
  const names = Object.keys(state.export.presets || {}).concat(Object.keys(state.export.custom || {}));
  sel.appendChild(new Option(_t('export.presetCustom') || '', '__custom__'));
  names.forEach(n => {
    sel.appendChild(new Option(presetNames[n] || n, n));
  });
  sel.value = '__custom__';
  sel.onchange = () => {
    const v = sel.value;
    if (v === '__custom__') return;
    const preset = (state.export.presets[v] || state.export.custom[v] || {});
    state.export.options = expDeepMerge(state.export.defaults, preset);
    renderExportSections();
  };
}


async function runExport() {
  const fmt = state.export.fmt;
  const options = collectExportOptions();
  const content = currentExportContent();
  const baseDir = state.dir || '';
  const suggestedName = currentExportName();
  busy(true);
  let r = null;
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;

  try {
    if (fmt === 'epub') {
      const epubOpts = options.epub || options.meta || {};
      const epubPayload = {
        title: epubOpts.title || '',
        author: epubOpts.author || '',
        publisher: epubOpts.publisher || '',
        isbn: epubOpts.isbn || '',
        language: epubOpts.language || 'zh-CN',
        splitLevel: epubOpts.splitLevel || 'h1',
        fontSize: epubOpts.fontSize,
        lineHeight: epubOpts.lineHeight,
        marginV: epubOpts.marginV,
        marginH: epubOpts.marginH,
        ...epubOpts
      };
      const fullPayload = { epub: epubPayload, meta: epubPayload, ...options };
      if (hasPy && py.export_epub) {
        r = await py.export_epub(content, '', fullPayload, true);
      } else {
        const resp = await apiFetch('/api/export/epub', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ content: content, meta: epubPayload, epub: epubPayload, options: fullPayload, confirm: true })
        });
        r = await resp.json();
      }
    } else if (fmt === 'presentation') {
      if (hasPy && py.export_presentation) {
        r = await py.export_presentation(content, options.theme || 'black', options.transition || 'slide', true);
      } else {
        const resp = await apiFetch('/api/export/presentation', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ content: content, theme: options.theme || 'black', transition: options.transition || 'slide' })
        });
        r = await resp.json();
      }
    } else {
      const payload = {
        content: content,
        baseDir: baseDir,
        suggestedName: suggestedName,
        options: options,
      };
      r = await py.export_doc(fmt, payload);
    }
  } catch (e) {
    showToast((_t('toast.exportFailed') || '') + e.message);
    busy(false);
    return;
  }
  busy(false);
  if (!r) { showToast(_t('toast.exportFailedSimple') || ''); return; }
  if (r.canceled) return;
  if (!r.ok) { showToast((_t('toast.exportFailed') || '') + (r.error || (_t('toast.unknownError') || ''))); return; }
  const res = $('export-result');
  res.textContent = (_t('toast.exportedPrefix') || '') + (r.path || '导出完成');
  res.className = 'export-result ok';
  if (r.path && hasPy && py) {
    $('export-open').classList.remove('hidden');
    $('export-reveal').classList.remove('hidden');
    $('export-open').onclick = () => py.open_path(r.path);
    $('export-reveal').onclick = () => py.reveal_path(r.path);
  }
  try { if (hasPy && py.save_export_presets) py.save_export_presets({ last: { fmt: fmt, options: options } }); } catch (e) { /* ignore */ }
  if (r.warns && r.warns.length) showToast(_t('toast.exportCompleteWarns', { count: r.warns.length }) || ('导出完成，' + r.warns.length + ' 条提示'));
  else showToast(_t('toast.exportSuccess') || '');
}

async function expSavePreset() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const box = $('exp-save-name');
  box.classList.remove('hidden');
  const input = $('exp-save-input');
  input.value = '';
  input.focus();
  $('exp-save-ok').onclick = async () => {
    const name = input.value.trim();
    if (!name) { showToast(_t('toast.enterPresetName') || ''); return; }
    const presetNames = getExportPresetNames();
    if (presetNames[name] || (state.export.presets && state.export.presets[name])) {
      showToast(_t('toast.presetNameConflict') || '');
      return;
    }
    state.export.custom[name] = collectExportOptions();
    try { await py.save_export_presets({ custom: state.export.custom }); } catch (e) { /* ignore */ }
    renderExportPresetSelect();
    box.classList.add('hidden');
    showToast(_t('toast.presetSaved', { name }) || ('预设已保存：' + name));
  };
  $('exp-save-cancel').onclick = () => box.classList.add('hidden');
}

/* ---------------- AI 风格排版设计师 ---------------- */

function initExportAiDesigner() {
  const genBtn = $('exp-ai-gen-btn');
  const promptInput = $('exp-ai-prompt');
  if (genBtn && promptInput) {
    genBtn.onclick = () => generateExportStyleWithAi(promptInput.value.trim());
    promptInput.onkeydown = e => {
      if (e.key === 'Enter') {
        e.preventDefault();
        generateExportStyleWithAi(promptInput.value.trim());
      }
    };
  }
}

async function generateExportStyleWithAi(stylePrompt) {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  if (!stylePrompt) {
    showToast(_t('exportai.placeholder') || '');
    return;
  }
  const statusEl = $('exp-ai-status');
  if (statusEl) {
    statusEl.classList.remove('hidden');
    statusEl.textContent = _t('exportai.generating') || '';
  }

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
        skill_id: 'readmd-export-style',
        skill_variables: {
          request: stylePrompt,
          context: '',
          document: stylePrompt,
          language: (window.i18n && window.i18n.locale) || document.documentElement.lang || 'en',
          output_format: 'JSON'
        },
        messages: [{ role: 'user', content: stylePrompt }],
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

    let text = '';
    const contentType = res.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
      const data = await res.json();
      text = data.content || (data.choices && data.choices[0] && data.choices[0].message && data.choices[0].message.content) || '';
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
              if (d.type === 'delta' && d.delta) chunks.push(d.delta);
              else if (d.d) chunks.push(d.d);
              else if (d.content) chunks.push(d.content);
            } catch (e) {}
          }
        }
      }
      text = chunks.length ? chunks.join('') : rawText;
    }

    text = text.replace(/^```json\s*/i, '').replace(/^```\s*/, '').replace(/\s*```$/, '').trim();
    const parsed = normalizeExportAiPayload(JSON.parse(text));

    // Apply options to export state and DOM
    state.export.options = expDeepMerge(state.export.options || state.export.defaults, parsed);
    const presetSelect = $('exp-preset');
    if (presetSelect) presetSelect.value = '__custom__';
    applyExportOptionsToDom();
    updateExportLivePreview();

    if (statusEl) {
      statusEl.textContent = _t('exportai.applied') || '';
      setTimeout(() => statusEl.classList.add('hidden'), 3000);
    }
    showToast(_t('exportai.applied') || '');
  } catch (e) {
    if (statusEl) {
      statusEl.textContent = (_t('ai.reqFailMsg') || '') + e.message;
    }
    showToast((_t('toast.unknownError') || '') + e.message);
  }
}
