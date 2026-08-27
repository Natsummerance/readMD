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
    minimal: _t('export.presetMinimal') || '简约',
    classic: _t('export.presetClassic') || '经典',
    business: _t('export.presetBusiness') || '商务'
  };
}

function getExportSections() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  return [
    /* --- EPUB 电子书专属参数 --- */
    { title: _t('export.secEpubMeta') || '电子书元数据', fmts: ['epub'], fields: [
      { k: 'epub.title', label: _t('export.epubTitle') || '书籍标题（留空自动使用文件名）', type: 'text', full: true },
      { k: 'epub.author', label: _t('export.epubAuthor') || '书籍作者 / 译者', type: 'text' },
      { k: 'epub.publisher', label: _t('export.epubPublisher') || '出版方 / 制作方', type: 'text' },
      { k: 'epub.isbn', label: _t('export.epubIsbn') || '标准 ISBN 书号', type: 'text' },
      { k: 'epub.language', label: _t('export.epubLanguage') || '主要语言', type: 'select', opts: [
        ['zh-CN', '简体中文 (zh-CN)'], ['en', 'English (en)'], ['ja', '日本語 (ja)'], ['zh-TW', '繁體中文 (zh-TW)'], ['fr', 'Français (fr)'], ['de', 'Deutsch (de)'], ['es', 'Español (es)']
      ]},
      { k: 'epub.splitLevel', label: _t('export.epubSplitLevel') || '章节拆分策略', type: 'select', opts: [
        ['h1', _t('export.epubSplitH1') || '按一级标题 (H1) 智能切分多章节'],
        ['h2', _t('export.epubSplitH2') || '按一/二级标题 (H1+H2) 切分章节'],
        ['none', _t('export.epubSplitNone') || '单章节长文档 (不切分)']
      ], full: true },
    ]},
    { title: _t('export.secEpubStyle') || '电子书阅读版式', fmts: ['epub'], fields: [
      { k: 'epub.fontSize', label: _t('export.bodySize') || '基准字号 pt', type: 'number', min: 8, max: 24 },
      { k: 'epub.lineHeight', label: _t('export.lineHeight') || '行高倍数', type: 'number', min: 1.2, max: 2.5, step: 0.1 },
      { k: 'epub.marginV', label: _t('export.epubMarginV') || '垂直页边距 %', type: 'number', min: 0, max: 20 },
      { k: 'epub.marginH', label: _t('export.epubMarginH') || '水平页边距 %', type: 'number', min: 0, max: 20 },
    ]},

    /* --- LaTeX 学术源码专属参数 --- */
    { title: _t('export.secLatexDoc') || 'LaTeX 学术编译与宏包', fmts: ['tex'], fields: [
      { k: 'tex.docClass', label: _t('export.latexDocClass') || 'LaTeX 文档类', type: 'select', opts: [
        ['ctexart', 'ctexart'],
        ['article', 'article'],
        ['ctexrep', 'ctexrep'],
        ['report', 'report'],
        ['book', 'book'],
        ['beamer', 'beamer']
      ], full: true },
      { k: 'tex.fontSize', label: _t('export.latexFontSize') || '排版字号', type: 'select', opts: [
        ['10pt', '10pt'], ['11pt', '11pt'], ['12pt', '12pt']
      ]},
      { k: 'tex.paperSize', label: _t('export.pageSize') || '纸张规格', type: 'select', opts: [
        ['a4paper', 'A4'], ['letterpaper', 'US Letter']
      ]},
      { k: 'tex.margin', label: _t('export.latexMargin') || '页面边距 (Geometry)', type: 'select', opts: [
        ['2.5cm', '2.5 cm'], ['1in', '1 in'], ['2cm', '2.0 cm'], ['3cm', '3.0 cm']
      ]},
      { k: 'tex.bibEngine', label: _t('export.latexBibEngine') || '参考文献引擎', type: 'select', opts: [
        ['biblatex', 'BibLaTeX'], ['natbib', 'Natbib'], ['bibtex', 'BibTeX']
      ]},
      { k: 'tex.useCtex', label: _t('export.latexUseCtex') || '启用 CJK 中文宏包 (UTF-8 原生支持)', type: 'checkbox' },
    ]},

    /* --- PDF / DOCX 页面与版式 --- */
    { title: _t('export.secPage') || '页面设置', fmts: ['pdf', 'docx'], fields: [
      { k: 'page.size', label: _t('export.pageSize') || '纸张', type: 'select', opts: EXPORT_PAGES },
      { k: 'page.orientation', label: _t('export.pageOrientation') || '方向', type: 'select', opts: [['portrait', _t('export.portrait') || '纵向'], ['landscape', _t('export.landscape') || '横向']] },
      { k: 'page.marginTop', label: _t('export.marginTop') || '上边距 mm', type: 'number', min: 0, max: 60 },
      { k: 'page.marginRight', label: _t('export.marginRight') || '右边距 mm', type: 'number', min: 0, max: 60 },
      { k: 'page.marginBottom', label: _t('export.marginBottom') || '下边距 mm', type: 'number', min: 0, max: 60 },
      { k: 'page.marginLeft', label: _t('export.marginLeft') || '左边距 mm', type: 'number', min: 0, max: 60 },
    ]},
    { title: _t('export.secCoverToc') || '封面与目录', fmts: ['pdf', 'docx'], fields: [
      { k: 'cover.enabled', label: _t('export.enableCover') || '启用封面页', type: 'checkbox' },
      { k: 'cover.title', label: _t('export.coverTitle') || '封面标题（留空用文件名）', type: 'text', full: true },
      { k: 'cover.subtitle', label: _t('export.coverSubtitle') || '封面副标题', type: 'text', full: true },
      { k: 'cover.date', label: _t('export.coverDate') || '封面日期', type: 'text' },
      { k: 'cover.align', label: _t('export.coverAlign') || '封面对齐', type: 'select', opts: [['center', _t('export.alignCenter') || '居中'], ['left', _t('export.alignLeft') || '左对齐'], ['right', _t('export.alignRight') || '右对齐']] },
      { k: 'toc.enabled', label: _t('export.enablePdfToc') || 'PDF 目录页', type: 'checkbox', fmts: ['pdf'] },
    ]},
    { title: _t('export.secTypography') || '正文排版', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'typography.font', label: _t('export.bodyFont') || '正文字体', type: 'select', opts: EXPORT_FONTS.map(f => [f, f]) },
      { k: 'typography.size', label: _t('export.bodySize') || '字号 pt', type: 'number', min: 8, max: 20 },
      { k: 'typography.lineHeight', label: _t('export.lineHeight') || '行距', type: 'number', min: 1, max: 2.5, step: 0.1 },
      { k: 'typography.spacing', label: _t('export.paragraphSpacing') || '段间距 pt', type: 'number', min: 0, max: 30 },
      { k: 'typography.color', label: _t('export.bodyColor') || '正文颜色', type: 'color' },
      { k: 'typography.align', label: _t('export.align') || '对齐', type: 'select', opts: [['left', _t('export.alignLeft') || '左对齐'], ['center', _t('export.alignCenter') || '居中'], ['right', _t('export.alignRight') || '右对齐'], ['justify', _t('export.alignJustify') || '两端对齐']] },
    ]},
    { title: _t('export.secHeadings') || '标题（各级颜色 / 字号 / 加粗 / 对齐）', fmts: ['pdf', 'docx', 'html'], headingRows: true },
    { title: _t('export.secTable') || '表格', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'table.headerBg', label: _t('export.tableHeaderBg') || '表头背景', type: 'color' },
      { k: 'table.headerColor', label: _t('export.tableHeaderColor') || '表头文字色', type: 'color' },
      { k: 'table.headerBold', label: _t('export.tableHeaderBold') || '表头加粗', type: 'checkbox' },
      { k: 'table.borderColor', label: _t('export.tableBorderColor') || '边框颜色', type: 'color' },
      { k: 'table.borderWidth', label: _t('export.tableBorderWidth') || '边框宽度 pt', type: 'number', min: 0, max: 3, step: 0.25 },
      { k: 'table.banded', label: _t('export.tableBanded') || '斑马纹', type: 'checkbox' },
      { k: 'table.bandColor', label: _t('export.tableBandColor') || '斑马纹颜色', type: 'color' },
      { k: 'table.cellSize', label: _t('export.tableCellSize') || '单元格字号 pt', type: 'number', min: 7, max: 16 },
      { k: 'table.cellPadding', label: _t('export.tableCellPadding') || '单元格内边距 pt', type: 'number', min: 0, max: 20 },
      { k: 'table.align', label: _t('export.align') || '对齐', type: 'select', opts: [['left', _t('export.alignLeft') || '左对齐'], ['center', _t('export.alignCenter') || '居中'], ['right', _t('export.alignRight') || '右对齐'], ['justify', _t('export.alignJustify') || '两端对齐']] },
      { k: 'table.widthPct', label: _t('export.tableWidthPct') || '表格宽度 %', type: 'number', min: 50, max: 100 },
    ]},
    { title: _t('export.secCode') || '代码块', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'code.bg', label: _t('export.codeBg') || '背景色', type: 'color' },
      { k: 'code.color', label: _t('export.codeColor') || '文字色', type: 'color' },
      { k: 'code.font', label: _t('export.codeFont') || '等宽字体', type: 'select', opts: EXPORT_MONO.map(f => [f, f]) },
      { k: 'code.size', label: _t('export.codeSize') || '字号 pt', type: 'number', min: 6, max: 16 },
      { k: 'code.borderColor', label: _t('export.codeBorderColor') || '边框颜色', type: 'color' },
      { k: 'code.borderWidth', label: _t('export.codeBorderWidth') || '边框宽度 pt', type: 'number', min: 0, max: 3, step: 0.25 },
      { k: 'code.rounded', label: _t('export.codeRounded') || '圆角（HTML）', type: 'checkbox', fmts: ['html'] },
    ]},
    { title: _t('export.secQuoteLink') || '引用与链接', fmts: ['pdf', 'docx', 'html'], fields: [
      { k: 'quote.barColor', label: _t('export.quoteBarColor') || '引用左边条色', type: 'color' },
      { k: 'quote.bg', label: _t('export.quoteBg') || '引用背景', type: 'color' },
      { k: 'quote.color', label: _t('export.quoteColor') || '引用文字色', type: 'color' },
      { k: 'link.color', label: _t('export.linkColor') || '链接颜色', type: 'color' },
      { k: 'hr.color', label: _t('export.hrColor') || '分割线颜色', type: 'color' },
    ]},
    { title: _t('export.secFooterMeta') || '页脚与元数据', fmts: ['pdf', 'docx'], fields: [
      { k: 'footer.pageNumbers', label: _t('export.showPageNumbers') || '显示页码', type: 'checkbox' },
      { k: 'footer.text', label: _t('export.footerTextLabel') || '页脚文字', type: 'text', full: true },
      { k: 'meta.title', label: _t('export.docMetaTitle') || '文档标题（PDF 元数据）', type: 'text', full: true },
      { k: 'meta.author', label: _t('export.metaAuthor') || '作者', type: 'text' },
      { k: 'meta.subject', label: _t('export.metaSubject') || '主题', type: 'text' },
    ]},
    { title: _t('export.secMath') || '数学公式', fmts: ['pdf', 'docx'], fields: [
      { k: 'math.dpi', label: _t('export.mathDpi') || '渲染分辨率 DPI', type: 'number', min: 100, max: 500, step: 10 },
    ]},
    { title: _t('export.secHtmlTheme') || 'HTML 主题', fmts: ['html'], fields: [
      { k: 'htmlTheme', label: _t('export.htmlThemeLabel') || '页面主题', type: 'select', opts: [['light', _t('export.themeLight') || '亮色'], ['dark', _t('export.themeDark') || '暗色'], ['sepia', _t('export.themeSepia') || '米色']] },
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
    showToast(_t('toast.openDocumentToUse') || '请先打开文档后再使用此操作');
    return;
  }
  if (!bindPy()) { showToast(_t('toast.exportBrowserNotice') || '浏览器模式请使用桌面版导出'); return; }
  if (!state.export.ready) {
    loadExportPresets().then(ok => {
      if (ok) { state.export.ready = true; renderExportModal(); }
      else showToast(_t('toast.exportModuleLoadFail') || '导出模块加载失败');
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
  else n = (state.sourceName || (_t('export.defaultExportName') || '导出')).split(/[\\/]/).pop();
  n = n.replace(/\.[^.]+$/, '');
  return n || (_t('export.defaultExportName') || '导出');
}


function renderExportModal() {
  $('export-modal').classList.remove('hidden');
  renderExportPresetSelect();
  renderExportSections();
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
      background: ${pageBg} !important;
      color: ${baseFg} !important;
      font-family: ${fontFamily} !important;
      font-size: ${ty.size || 11}pt !important;
      line-height: ${ty.lineHeight || 1.6} !important;
      text-align: ${ty.align || 'left'} !important;
      padding: ${page.marginTop || 20}mm ${page.marginRight || 18}mm ${page.marginBottom || 20}mm ${page.marginLeft || 18}mm !important;
      ${page.orientation === 'landscape' ? 'width: 270mm; min-height: 190mm;' : 'width: 190mm; min-height: 270mm;'}
    }
    #export-preview-full-page p, #export-preview-full-page li, #export-preview-full-page span, #export-preview-full-page div {
      color: ${baseFg} !important;
      font-size: ${ty.size || 11}pt !important;
      line-height: ${ty.lineHeight || 1.6} !important;
      text-align: ${ty.align || 'left'} !important;
    }
    #export-preview-full-page p {
      margin: ${ty.spacing || 6}pt 0 !important;
    }
    #export-preview-full-page h1 {
      color: ${h1.color || '#1a1a1a'} !important;
      font-size: ${h1.size || 20}pt !important;
      font-weight: ${h1.bold ? 'bold' : 'normal'} !important;
      text-align: ${h1.align || 'left'} !important;
      margin-top: ${h1.before || 18}pt !important;
      margin-bottom: ${h1.after || 10}pt !important;
      line-height: 1.35 !important;
      border-bottom: none !important;
    }
    #export-preview-full-page h2 {
      color: ${h2.color || '#1f2937'} !important;
      font-size: ${h2.size || 16}pt !important;
      font-weight: ${h2.bold ? 'bold' : 'normal'} !important;
      text-align: ${h2.align || 'left'} !important;
      margin-top: ${h2.before || 14}pt !important;
      margin-bottom: ${h2.after || 8}pt !important;
      line-height: 1.35 !important;
      border-bottom: none !important;
    }
    #export-preview-full-page h3 {
      color: ${h3.color || '#2d3748'} !important;
      font-size: ${h3.size || 14}pt !important;
      font-weight: ${h3.bold ? 'bold' : 'normal'} !important;
      text-align: ${h3.align || 'left'} !important;
      margin-top: ${h3.before || 12}pt !important;
      margin-bottom: ${h3.after || 6}pt !important;
      line-height: 1.35 !important;
    }
    #export-preview-full-page h4 {
      color: ${h4.color || '#374151'} !important;
      font-size: ${h4.size || 12}pt !important;
      font-weight: ${h4.bold ? 'bold' : 'normal'} !important;
      text-align: ${h4.align || 'left'} !important;
      margin-top: ${h4.before || 10}pt !important;
      margin-bottom: ${h4.after || 6}pt !important;
    }
    #export-preview-full-page h5 {
      color: ${h5.color || '#4a5568'} !important;
      font-size: ${h5.size || 11}pt !important;
      font-weight: ${h5.bold ? 'bold' : 'normal'} !important;
      text-align: ${h5.align || 'left'} !important;
      margin-top: ${h5.before || 8}pt !important;
      margin-bottom: ${h5.after || 4}pt !important;
    }
    #export-preview-full-page h6 {
      color: ${h6.color || '#4a5568'} !important;
      font-size: ${h6.size || 10.5}pt !important;
      font-weight: ${h6.bold ? 'bold' : 'normal'} !important;
      text-align: ${h6.align || 'left'} !important;
      margin-top: ${h6.before || 8}pt !important;
      margin-bottom: ${h6.after || 4}pt !important;
    }
    #export-preview-full-page table {
      border-collapse: collapse !important;
      width: ${tb.widthPct || 100}% !important;
      margin: 12pt auto !important;
      font-size: ${tb.cellSize || 10}pt !important;
    }
    #export-preview-full-page th, #export-preview-full-page td {
      border: ${tb.borderWidth || 0.75}px solid ${tb.borderColor || '#c8cdd4'} !important;
      padding: ${tb.cellPadding || 6}px !important;
      text-align: ${tb.align || 'left'} !important;
      color: ${baseFg} !important;
    }
    #export-preview-full-page th {
      background: ${tb.headerBg || '#3b6ef5'} !important;
      color: ${tb.headerColor || '#ffffff'} !important;
      font-weight: ${tb.headerBold ? 'bold' : 'normal'} !important;
    }
    #export-preview-full-page tbody tr:nth-child(even) td {
      background: ${tb.banded ? (tb.bandColor || '#f3f5f9') : 'transparent'} !important;
    }
    #export-preview-full-page pre {
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
    #export-preview-full-page code {
      font-family: ${code.font || 'Consolas'}, Consolas, monospace !important;
    }
    #export-preview-full-page :not(pre) > code {
      background: ${codeBg} !important;
      color: #c7254e !important;
      padding: 2px 5px !important;
      border-radius: 4px !important;
      font-size: 0.92em !important;
    }
    #export-preview-full-page blockquote {
      margin: 8pt 0 !important;
      padding: 8pt 14pt !important;
      background: ${quoteBg} !important;
      color: ${quote.color || '#4a5568'} !important;
      border-left: 4px solid ${quote.barColor || '#3b6ef5'} !important;
    }
    #export-preview-full-page blockquote p {
      color: ${quote.color || '#4a5568'} !important;
      margin: 4pt 0 !important;
    }
    #export-preview-full-page a {
      color: ${link.color || '#2b6cb0'} !important;
      text-decoration: underline !important;
    }
    #export-preview-full-page hr {
      border: none !important;
      border-top: 1px solid ${hr.color || '#d8dce2'} !important;
      margin: 14pt 0 !important;
    }
    #export-preview-full-page img {
      max-width: 100% !important;
      height: auto !important;
    }
  `;
}

function updateExportLivePreview() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const fmt = state.export.fmt;
  const opts = collectExportOptions();
  const badge = $('export-preview-badge');
  const sel = $('exp-preset');
  const presetName = (sel && sel.selectedIndex >= 0) ? sel.options[sel.selectedIndex].text : (_t('export.presetDefault') || '默认');
  if (badge) badge.textContent = fmt.toUpperCase() + ' · ' + presetName;

  const paperMeta = $('export-preview-paper-meta');
  if (paperMeta) {
    const page = opts.page || {};
    const sz = page.size || 'A4';
    const ori = (page.orientation === 'landscape') ? (_t('export.orientationLandscape') || '横向') : (_t('export.orientationPortrait') || '纵向');
    paperMeta.textContent = sz + ' · ' + ori + ' · ' + presetName;
  }


  // 注入或更新动态样式表
  let styleEl = $('export-preview-dynamic-style');
  if (!styleEl) {
    styleEl = document.createElement('style');
    styleEl.id = 'export-preview-dynamic-style';
    document.head.appendChild(styleEl);
  }
  styleEl.textContent = generateExportPreviewCss(opts, fmt);

  const content = currentExportContent();
  const miniHost = $('export-preview-mini-content');
  if (miniHost) {
    const previewChunk = (content || '').slice(0, 1500);
    const prot = protectMath(previewChunk);
    const html = marked.parse(prot.src, { gfm: true, breaks: false });
    miniHost.innerHTML = restoreMath(html, prot.saved);
    renderMath(miniHost);
  }

  const fullModal = $('export-preview-modal');
  if (fullModal && !fullModal.classList.contains('hidden')) {
    const fullHost = $('export-preview-full-page');
    if (fullHost) {
      const fullProt = protectMath(content || '');
      const fullHtml = marked.parse(fullProt.src, { gfm: true, breaks: false });
      fullHost.innerHTML = restoreMath(fullHtml, fullProt.saved);
      renderMath(fullHost);
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
          '<input type="number" data-k="headings.h' + i + '.size" min="8" max="40" title="' + (_t('export.bodySize') || '字号') + '">' +
          '<input type="color" data-k="headings.h' + i + '.color" title="' + (_t('export.bodyColor') || '颜色') + '">' +
          '<label class="exp-check">' + (_t('export.bold') || '加粗') + '<input type="checkbox" data-k="headings.h' + i + '.bold"></label>' +
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

function renderExportPresetSelect() {
  const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
  const sel = $('exp-preset');
  sel.textContent = '';
  const presetNames = getExportPresetNames();
  const names = Object.keys(state.export.presets || {}).concat(Object.keys(state.export.custom || {}));
  sel.appendChild(new Option(_t('export.presetCustom') || '自定义', '__custom__'));
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
      if (hasPy && py.export_epub) {
        r = await py.export_epub(content, '', options.meta || {});
      } else {
        const resp = await apiFetch('/api/export/epub', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ content: content, meta: options.meta || {} })
        });
        r = await resp.json();
      }
    } else if (fmt === 'presentation') {
      if (hasPy && py.export_presentation) {
        r = await py.export_presentation(content, options.theme || 'black', options.transition || 'slide');
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
    showToast((_t('toast.exportFailed') || '导出失败：') + e.message);
    busy(false);
    return;
  }
  busy(false);
  if (!r) { showToast(_t('toast.exportFailedSimple') || '导出失败'); return; }
  if (r.canceled) return;
  if (!r.ok) { showToast((_t('toast.exportFailed') || '导出失败：') + (r.error || (_t('toast.unknownError') || '未知错误'))); return; }
  const res = $('export-result');
  res.textContent = (_t('toast.exportedPrefix') || '已导出：') + (r.path || '导出完成');
  res.className = 'export-result ok';
  if (r.path) {
    $('export-open').classList.remove('hidden');
    $('export-reveal').classList.remove('hidden');
    $('export-open').onclick = () => py.open_path(r.path);
    $('export-reveal').onclick = () => py.reveal_path(r.path);
  }
  try { py.save_export_presets({ last: { fmt: fmt, options: options } }); } catch (e) { /* ignore */ }
  if (r.warns && r.warns.length) showToast(_t('toast.exportCompleteWarns', { count: r.warns.length }) || ('导出完成，' + r.warns.length + ' 条提示'));
  else showToast(_t('toast.exportSuccess') || '导出成功');
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
    if (!name) { showToast(_t('toast.enterPresetName') || '请输入预设名称'); return; }
    const presetNames = getExportPresetNames();
    if (presetNames[name] || (state.export.presets && state.export.presets[name])) {
      showToast(_t('toast.presetNameConflict') || '名称与内置预设冲突');
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

