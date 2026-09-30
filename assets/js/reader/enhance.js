'use strict';
/* ============================================================
   ReadMD Reader - Reading enhancements
   Callouts, footnotes, definition lists, code headers + syntax colour,
   table wrappers, figures + lightbox, heading anchors, reading stats,
   progress and the reading-preferences popover.
   Everything here runs on DOM that sanitizeRenderedHtml already cleaned
   and only adds nodes built with DOM APIs (text via textContent), so it
   cannot reintroduce untrusted markup.
   ============================================================ */
(function () {
  const _t = (k, p) => (window.i18n ? window.i18n.t(k, p) : k);
  const byId = id => document.getElementById(id);
  const CHROME = 'data-rd-chrome';
  const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const fmtNum = n => { try { return Number(n).toLocaleString(window.i18n?.currentLang || undefined); } catch (_) { return String(n); } };

  /* ---------------- source pre-pass ---------------- */
  // "[^id]: word" is a valid CommonMark link reference definition, which
  // would silently swallow a footnote.  Escaping the bracket keeps the line
  // literal so the DOM pass can collect it.  Fenced code is left untouched.
  function prepareSource(src) {
    if (!src || src.indexOf('[^') < 0) return src;
    const fences = [];
    const guarded = String(src).replace(/(^|\n)(```|~~~)[^\n]*\n[\s\S]*?\n\2[^\n]*/g, m => {
      fences.push(m);
      return '\x00RDF' + (fences.length - 1) + '\x00';
    });
    return guarded
      .replace(/^( {0,3})\[\^([^\]\s]+)\]:/gm, '$1\\[^$2]:')
      .replace(/\x00RDF(\d+)\x00/g, (_, i) => fences[+i]);
  }

  /** Split an element's children at a regex that matches inside its direct text nodes. */
  function splitChildren(el, re) {
    const pieces = [{ key: null, frag: document.createDocumentFragment() }];
    Array.from(el.childNodes).forEach(node => {
      const cur = () => pieces[pieces.length - 1].frag;
      if (node.nodeType !== 3) { cur().appendChild(node); return; }
      const text = node.nodeValue;
      re.lastIndex = 0;
      let cursor = 0;
      let m;
      while ((m = re.exec(text))) {
        if (m.index > cursor) cur().appendChild(document.createTextNode(text.slice(cursor, m.index)));
        pieces.push({ key: m[1] == null ? '' : m[1], frag: document.createDocumentFragment() });
        cursor = re.lastIndex;
        if (!m[0]) re.lastIndex += 1;
      }
      if (cursor < text.length) cur().appendChild(document.createTextNode(text.slice(cursor)));
    });
    return pieces;
  }
  /* ---------------- icons (static, trusted markup) ---------------- */
  const ICONS = {
    note: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.5h.01"/>',
    tip: '<path d="M9 18h6M10 21h4"/><path d="M12 3a6 6 0 0 0-3.6 10.8c.7.5 1.1 1.3 1.1 2.1V16h5v-.1c0-.8.4-1.6 1.1-2.1A6 6 0 0 0 12 3z"/>',
    important: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M12 7v4M12 14h.01"/>',
    warning: '<path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/>',
    caution: '<path d="M7.9 2h8.2L22 7.9v8.2L16.1 22H7.9L2 16.1V7.9z"/><path d="M12 8v4M12 16h.01"/>',
    quote: '<path d="M6 17c2-1 3-3 3-6V7H5v4h4M15 17c2-1 3-3 3-6V7h-4v4h4"/>',
    chevron: '<path d="m9 6 6 6-6 6"/>',
    copy: '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V6a2 2 0 0 1 2-2h9"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    back: '<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
    close: '<path d="M18 6 6 18M6 6l12 12"/>',
    type: '<path d="M3.5 18 8 6h1l4.5 12M5 14h7"/><path d="M14.5 18l2.9-7.5h.6L21 18M15.6 15.8h4.3"/>',
    collapse: '<path d="m7 15 5-5 5 5"/>',
    expand: '<path d="m7 10 5 5 5-5"/>',
  };
  function icon(name, cls) {
    const t = document.createElement('template');
    t.innerHTML = `<svg class="${cls || 'rd-ic'}" viewBox="0 0 24 24" aria-hidden="true" focusable="false">${ICONS[name] || ''}</svg>`;
    return t.content.firstChild;
  }
  function chrome(el) { el.setAttribute(CHROME, ''); return el; }
  function make(tag, cls, text) {
    const el = document.createElement(tag);
    if (cls) el.className = cls;
    if (text != null) el.textContent = text;
    return el;
  }
  function carryLine(from, to) { if (from.dataset && from.dataset.sourceLine) to.dataset.sourceLine = from.dataset.sourceLine; }

  /* ---------------- callouts: GitHub alerts + Obsidian callouts ---------------- */
  const CALLOUT_VARIANT = {
    note: 'note', info: 'note', todo: 'note', abstract: 'note', summary: 'note', tldr: 'note',
    tip: 'tip', hint: 'tip', success: 'tip', check: 'tip', done: 'tip',
    important: 'important', question: 'important', help: 'important', faq: 'important', example: 'important',
    warning: 'warning', attention: 'warning',
    caution: 'caution', danger: 'caution', error: 'caution', failure: 'caution', fail: 'caution', missing: 'caution', bug: 'caution',
    quote: 'quote', cite: 'quote',
  };
  const CALLOUT_TITLE = {
    summary: 'abstract', tldr: 'abstract', hint: 'tip', check: 'success', done: 'success',
    help: 'question', faq: 'question', attention: 'warning', error: 'danger', fail: 'failure',
    missing: 'failure', cite: 'quote',
  };
  const CALLOUT_RE = /^\s*\[!([A-Za-z][\w-]*)\]([+-]?)[ \t]*/;
  function calloutTitleText(type, variant) {
    if (['note', 'tip', 'important', 'warning', 'caution'].includes(type)) return _t('reader.callout' + type[0].toUpperCase() + type.slice(1));
    const name = CALLOUT_TITLE[type] || type;
    return variant && name === variant ? _t('reader.callout' + name[0].toUpperCase() + name.slice(1)) : name[0].toUpperCase() + name.slice(1);
  }

  function upgradeCallout(bq) {
    const first = bq.firstElementChild;
    if (!first || first.tagName !== 'P') return;
    const lead = first.firstChild;
    if (!lead || lead.nodeType !== 3) return;
    const m = CALLOUT_RE.exec(lead.nodeValue);
    if (!m) return;
    const type = m[1].toLowerCase();
    const variant = CALLOUT_VARIANT[type] || 'note';
    const fold = m[2];
    lead.nodeValue = lead.nodeValue.slice(m[0].length);

    // Title = rest of the first line of the first paragraph.
    const titleFrag = document.createDocumentFragment();
    let node = first.firstChild;
    while (node) {
      const next = node.nextSibling;
      if (node.nodeType === 3) {
        const nl = node.nodeValue.indexOf('\n');
        if (nl >= 0) {
          titleFrag.appendChild(document.createTextNode(node.nodeValue.slice(0, nl)));
          node.nodeValue = node.nodeValue.slice(nl + 1);
          break;
        }
      } else if (node.nodeName === 'BR') { node.remove(); break; }
      titleFrag.appendChild(node);
      node = next;
    }
    const hasOwnTitle = (titleFrag.textContent || '').trim().length > 0 || titleFrag.querySelector?.('*');
    if (!first.textContent.trim() && !first.querySelector('img, mjx-container')) first.remove();

    const box = document.createElement(fold ? 'details' : 'div');
    box.className = `rd-callout rd-callout--${variant}`;
    box.dataset.callout = type;
    carryLine(bq, box);
    if (fold === '+') box.open = true;
    const head = make(fold ? 'summary' : 'div', 'rd-callout-title');
    head.appendChild(icon(variant === 'quote' ? 'quote' : variant, 'rd-callout-icon'));
    const label = make('span', 'rd-callout-label');
    if (hasOwnTitle) label.appendChild(titleFrag);
    else { label.textContent = calloutTitleText(type, variant); chrome(label); }
    head.appendChild(label);
    if (fold) head.appendChild(icon('chevron', 'rd-callout-chevron'));
    const content = make('div', 'rd-callout-body');
    while (bq.firstChild) content.appendChild(bq.firstChild);
    box.append(head);
    if (content.childNodes.length && content.textContent.trim() || content.querySelector('img, table, pre, mjx-container')) box.append(content);
    bq.replaceWith(box);
  }

  /* ---------------- footnotes ---------------- */
  function upgradeFootnotes(body) {
    const defs = new Map();
    const DEF_RE = /(?:^|\n)\[\^([^\]\s]+)\]:[ \t]*/g;
    body.querySelectorAll('p').forEach(p => {
      if (p.closest('pre, code, .rd-footnotes')) return;
      const lead = p.firstChild;
      if (!lead || lead.nodeType !== 3 || !/^\s*\[\^[^\]\s]+\]:/.test(lead.nodeValue)) return;
      lead.nodeValue = lead.nodeValue.replace(/^\s+/, '');
      const pieces = splitChildren(p, DEF_RE);
      if ((pieces[0].frag.textContent || '').trim()) return;
      pieces.slice(1).forEach(piece => { if (!defs.has(piece.key)) defs.set(piece.key, piece.frag); });
      p.remove();
    });
    if (!defs.size) return;

    const order = [];
    const REF_RE = /\[\^([^\]\s]+)\]/g;
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT, {
      acceptNode: n => (n.nodeValue.indexOf('[^') >= 0 && !n.parentElement.closest('pre, code, a, [' + CHROME + ']'))
        ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT,
    });
    const hits = [];
    while (walker.nextNode()) hits.push(walker.currentNode);
    const refCount = new Map();
    hits.forEach(textNode => {
      const text = textNode.nodeValue;
      const frag = document.createDocumentFragment();
      let cursor = 0;
      let m;
      REF_RE.lastIndex = 0;
      let changed = false;
      while ((m = REF_RE.exec(text))) {
        const key = m[1];
        if (!defs.has(key)) continue;
        if (!order.includes(key)) order.push(key);
        const n = order.indexOf(key) + 1;
        const k = (refCount.get(key) || 0) + 1;
        refCount.set(key, k);
        if (m.index > cursor) frag.appendChild(document.createTextNode(text.slice(cursor, m.index)));
        const sup = make('sup', 'rd-fnref');
        const a = make('a', null, String(n));
        a.href = '#fn-' + slugKey(key);
        a.id = 'fnref-' + slugKey(key) + (k > 1 ? '-' + k : '');
        a.setAttribute('aria-describedby', 'rd-footnotes-label');
        sup.appendChild(a);
        frag.appendChild(sup);
        cursor = REF_RE.lastIndex;
        changed = true;
      }
      if (!changed) return;
      if (cursor < text.length) frag.appendChild(document.createTextNode(text.slice(cursor)));
      textNode.replaceWith(frag);
    });
    defs.forEach((_, key) => { if (!order.includes(key)) order.push(key); });

    const section = make('section', 'rd-footnotes');
    const label = chrome(make('div', 'rd-footnotes-label', _t('reader.footnotes')));
    label.id = 'rd-footnotes-label';
    const ol = make('ol');
    order.forEach(key => {
      const li = make('li');
      li.id = 'fn-' + slugKey(key);
      const p = make('p');
      p.appendChild(defs.get(key));
      const refs = refCount.get(key) || 0;
      for (let k = 1; k <= refs; k += 1) {
        const back = chrome(make('a', 'rd-fnback'));
        back.href = '#fnref-' + slugKey(key) + (k > 1 ? '-' + k : '');
        back.setAttribute('aria-label', _t('reader.footnoteBack'));
        back.title = _t('reader.footnoteBack');
        back.appendChild(icon('back', 'rd-ic'));
        p.append(' ', back);
      }
      li.appendChild(p);
      ol.appendChild(li);
    });
    section.append(label, ol);
    body.appendChild(section);
  }
  function slugKey(key) { return String(key).replace(/[^A-Za-z0-9_-]/g, c => '_' + c.charCodeAt(0).toString(16)); }

  /* ---------------- definition lists (PHP-Markdown style) ---------------- */
  function upgradeDefinitionLists(body) {
    const DD_RE = /\n:[ \t]+/g;
    body.querySelectorAll('p').forEach(p => {
      if (p.closest('pre, code, li, .rd-footnotes')) return;
      const lead = p.firstChild;
      if (!lead || lead.nodeType !== 3 || /^\s*:/.test(lead.nodeValue)) return;
      let found = false;
      p.childNodes.forEach(n => { if (n.nodeType === 3 && /\n:[ \t]+/.test(n.nodeValue)) found = true; });
      if (!found) return;
      const pieces = splitChildren(p, DD_RE);
      if (!(pieces[0].frag.textContent || '').trim()) return;
      let dl = p.previousElementSibling;
      if (!dl || dl.tagName !== 'DL' || !dl.classList.contains('rd-dl')) {
        dl = make('dl', 'rd-dl');
        carryLine(p, dl);
        p.before(dl);
      }
      const dt = make('dt');
      dt.appendChild(pieces[0].frag);
      dl.appendChild(dt);
      pieces.slice(1).forEach(piece => { const dd = make('dd'); dd.appendChild(piece.frag); dl.appendChild(dd); });
      p.remove();
    });
  }
  /* ---------------- syntax highlighting (tiny, offline, escaped) ---------------- */
  const W = s => new Set(s.split(/\s+/).filter(Boolean));
  const C_LIKE_KW = 'if else for while do switch case default break continue return goto try catch finally throw new delete this super class struct enum union interface extends implements public private protected static final const volatile virtual override abstract import package namespace using typedef sizeof template typename operator inline extern register auto void';
  const LANGS = {
    js: { kw: W('break case catch class const continue debugger default delete do else export extends finally for from function if import in instanceof let new of return static super switch this throw try typeof var void while with yield async await get set as satisfies interface type enum implements declare readonly keyof namespace private protected public abstract'), lit: W('true false null undefined NaN Infinity'), line: '//', block: true, str: '"\'`', regex: true },
    py: { kw: W('and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield match case self cls print'), lit: W('True False None'), line: '#', triple: true, str: '"\'', deco: true },
    rust: { kw: W('as async await break const continue crate dyn else enum extern fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait type unsafe use where while macro_rules'), lit: W('true false None Some Ok Err'), types: W('i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize f32 f64 bool char str String Vec Option Result Box'), line: '//', block: true, str: '"', attr: true, macro: true },
    go: { kw: W('break case chan const continue default defer else fallthrough for func go goto if import interface map package range return select struct switch type var'), lit: W('true false nil iota'), types: W('int int8 int16 int32 int64 uint uint8 uint16 uint32 uint64 float32 float64 string bool byte rune error any'), line: '//', block: true, str: '"`\'' },
    c: { kw: W(C_LIKE_KW + ' fn func fun val var let when is in object companion data sealed open internal lateinit suspend def elif then end do begin module require nil self yield unless until defined echo function foreach endif endforeach'), lit: W('true false null nullptr NULL nil'), types: W('int long short char float double bool boolean byte string String unsigned signed size_t uint8_t uint32_t int32_t int64_t'), line: '//', block: true, str: '"\'', deco: true, pre: true },
    css: { css: true, block: true, str: '"\'' },
    json: { lit: W('true false null'), str: '"', json: true },
    sh: { kw: W('if then else elif fi for while until do done case esac in function return local export readonly declare set unset shift source alias echo exit cd sudo'), lit: W('true false'), line: '#', str: '"\'', vars: true },
    sql: { kw: W('select from where and or not insert into values update set delete create table index view drop alter add column primary key foreign references join left right inner outer full on group by order having limit offset as distinct union all exists in is null like between case when then else end with returning default unique'), lit: W('true false null'), line: '--', block: true, str: '\'"', ci: true },
    yaml: { yaml: true, lit: W('true false null yes no on off ~'), line: '#', str: '"\'' },
    toml: { yaml: true, lit: W('true false'), line: '#', str: '"\'' },
    html: { html: true },
    diff: { diff: true },
    lua: { kw: W('and break do else elseif end for function goto if in local not or repeat return then until while'), lit: W('true false nil'), line: '--', str: '"\'' },
  };
  const LANG_ALIAS = {
    javascript: 'js', jsx: 'js', mjs: 'js', cjs: 'js', ts: 'js', typescript: 'js', tsx: 'js', node: 'js',
    python: 'py', python3: 'py', py3: 'py', rs: 'rust', golang: 'go',
    java: 'c', kotlin: 'c', kt: 'c', scala: 'c', swift: 'c', cpp: 'c', 'c++': 'c', cc: 'c', h: 'c', hpp: 'c', cs: 'c', csharp: 'c', 'c#': 'c', dart: 'c', php: 'c', ruby: 'c', rb: 'c', objc: 'c', groovy: 'c', zig: 'c',
    scss: 'css', less: 'css', sass: 'css', jsonc: 'json', json5: 'json',
    bash: 'sh', shell: 'sh', zsh: 'sh', console: 'sh', shellscript: 'sh', powershell: 'sh', ps1: 'sh', ps: 'sh', bat: 'sh', cmd: 'sh', fish: 'sh', dockerfile: 'sh', docker: 'sh', makefile: 'sh', make: 'sh',
    yml: 'yaml', ini: 'toml', cfg: 'toml', conf: 'toml', properties: 'toml', env: 'toml',
    xml: 'html', svg: 'html', vue: 'html', svelte: 'html', xhtml: 'html', htm: 'html',
    patch: 'diff', mysql: 'sql', postgres: 'sql', postgresql: 'sql', sqlite: 'sql', plsql: 'sql',
  };
  const LANG_LABEL = {
    js: 'JavaScript', javascript: 'JavaScript', jsx: 'JSX', ts: 'TypeScript', typescript: 'TypeScript', tsx: 'TSX',
    py: 'Python', python: 'Python', rust: 'Rust', rs: 'Rust', go: 'Go', golang: 'Go', java: 'Java', kotlin: 'Kotlin', kt: 'Kotlin',
    swift: 'Swift', c: 'C', cpp: 'C++', 'c++': 'C++', cs: 'C#', csharp: 'C#', php: 'PHP', ruby: 'Ruby', rb: 'Ruby',
    css: 'CSS', scss: 'SCSS', less: 'Less', json: 'JSON', jsonc: 'JSON', sh: 'Shell', bash: 'Bash', shell: 'Shell', zsh: 'Zsh',
    powershell: 'PowerShell', ps1: 'PowerShell', sql: 'SQL', yaml: 'YAML', yml: 'YAML', toml: 'TOML', ini: 'INI',
    html: 'HTML', xml: 'XML', svg: 'SVG', vue: 'Vue', diff: 'Diff', patch: 'Diff', lua: 'Lua', md: 'Markdown', markdown: 'Markdown',
    text: 'Text', txt: 'Text', plaintext: 'Text', dockerfile: 'Dockerfile', makefile: 'Makefile', dart: 'Dart', scala: 'Scala',
  };
  const reEsc = s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const compiled = new Map();

  function tokenizerFor(key) {
    if (compiled.has(key)) return compiled.get(key);
    const L = LANGS[key];
    const parts = [];
    const add = (cls, src) => parts.push([cls, src]);
    if (L.html) {
      add('com', '<!--[\\s\\S]*?-->');
      add('tag', '<\\/?[A-Za-z][\\w:.-]*|\\/?>');
      add('attr', '[A-Za-z_:][\\w:.-]*(?=\\s*=)');
      add('str', '"[^"]*"|\'[^\']*\'');
      add('ent', '&[#\\w]+;');
    } else if (L.diff) {
      add('ins', '^\\+.*$');
      add('del', '^-.*$');
      add('meta', '^@@.*$|^(?:diff|index|---|\\+\\+\\+) .*$');
    } else {
      if (L.triple) add('str', '"""[\\s\\S]*?"""|\'\'\'[\\s\\S]*?\'\'\'');
      if (L.block) add('com', '\\/\\*[\\s\\S]*?\\*\\/');
      if (L.line) add('com', reEsc(L.line) + '.*');
      if (L.pre) add('meta', '^[ \\t]*#[ \\t]*[a-z]+\\b');
      if (L.attr) add('meta', '#!?\\[[^\\]\\n]*\\]');
      if (L.deco) add('meta', '@[A-Za-z_][\\w.]*');
      if (L.css) {
        add('meta', '@[\\w-]+');
        add('num', '#[\\da-fA-F]{3,8}\\b');
        add('attr', '[\\w-]+(?=\\s*:(?!:))');
      }
      if (L.yaml) {
        add('attr', '^[ \\t-]*[\\w.$@"\'-]+(?=\\s*[:=])');
        add('meta', '^\\s*\\[[^\\]\\n]*\\]');
      }
      if (L.json) add('attr', '"(?:\\\\.|[^"\\\\\\n])*"(?=\\s*:)');
      (L.str || '').split('').forEach(q => {
        const e = reEsc(q);
        add('str', q === '`' ? '`(?:\\\\[\\s\\S]|[^`\\\\])*`' : `${e}(?:\\\\.|[^${e}\\\\\\n])*${e}`);
      });
      if (L.vars) add('var', '\\$\\{[^}\\n]*\\}|\\$[\\w@#?*!-]+');
      if (L.regex) add('str', '(?<=[=(,:;!&|?{}\\[]\\s*)\\/(?![*/])(?:\\\\.|\\[(?:\\\\.|[^\\]\\\\\\n])*\\]|[^/\\\\\\n])+\\/[dgimsuy]*');
      add('num', '\\b(?:0[xX][\\da-fA-F_]+|0[bB][01_]+|0[oO][0-7_]+|\\d[\\d_]*(?:\\.\\d[\\d_]*)?(?:[eE][+-]?\\d+)?)(?:[a-zA-Z]{0,4}|%)?\\b');
      if (L.macro) add('fn', '[A-Za-z_]\\w*!(?=\\s*[([{])');
      add('id', L.css ? '-?[A-Za-z_][\\w-]*' : '[A-Za-z_$][\\w$]*');
      add('punct', '=>|->|::|[{}()[\\];,.:]');
    }
    const flags = 'g' + (L.html || L.diff || L.yaml || L.pre ? 'm' : '') + (L.ci ? 'i' : '');
    let re;
    try { re = new RegExp(parts.map(p => '(' + p[1] + ')').join('|'), flags); }
    catch (_) { re = new RegExp(parts.filter(p => !p[1].startsWith('(?<=')).map(p => '(' + p[1] + ')').join('|'), flags); }
    const classes = parts.map(p => p[0]);
    const tk = { re, classes, L };
    compiled.set(key, tk);
    return tk;
  }

  function highlightToHtml(text, key) {
    const { re, classes, L } = tokenizerFor(key);
    let out = '';
    let cursor = 0;
    let m;
    re.lastIndex = 0;
    while ((m = re.exec(text))) {
      if (!m[0]) { re.lastIndex += 1; continue; }
      if (m.index > cursor) out += esc(text.slice(cursor, m.index));
      let gi = 1;
      while (gi < m.length && m[gi] === undefined) gi += 1;
      let cls = classes[gi - 1];
      const word = m[0];
      if (cls === 'id') {
        const w = L.ci ? word.toLowerCase() : word;
        if (L.kw && L.kw.has(w)) cls = 'kw';
        else if (L.lit && L.lit.has(w)) cls = 'lit';
        else if (L.types && L.types.has(w)) cls = 'type';
        else {
          let j = re.lastIndex;
          while (text[j] === ' ') j += 1;
          if (text[j] === '(' && !L.css) cls = 'fn';
          else if (L.css && text[j] === '(') cls = 'fn';
          else if (/^[A-Z][a-z0-9]\w*$/.test(word) && !L.css && !L.sql) cls = 'type';
          else cls = '';
        }
      }
      out += cls ? `<span class="tk-${cls}">${esc(word)}</span>` : esc(word);
      cursor = re.lastIndex;
    }
    if (cursor < text.length) out += esc(text.slice(cursor));
    return out;
  }

  function langKey(lang) {
    const l = String(lang || '').toLowerCase();
    if (LANGS[l]) return l;
    return LANG_ALIAS[l] || null;
  }
  function langLabel(lang) {
    const l = String(lang || '').toLowerCase();
    if (!l) return _t('reader.codeLabel');
    return LANG_LABEL[l] || (l.length <= 4 ? l.toUpperCase() : l[0].toUpperCase() + l.slice(1));
  }

  const MAX_HIGHLIGHT_CHARS = 120000;
  function highlightBlock(code) {
    if (code.dataset.rdHl) return;
    code.dataset.rdHl = '1';
    if (code.querySelector('mark.hl')) { delete code.dataset.rdHl; pendingHighlight.add(code); return; }
    const key = langKey(code.dataset.rdLang);
    const text = code.textContent;
    if (!key || text.length > MAX_HIGHLIGHT_CHARS) return;
    // Built from escaped text only: every span wraps esc()-ed source.
    code.innerHTML = highlightToHtml(text, key);
  }

  const pendingHighlight = new Set();
  function flushPendingHighlight() {
    const list = Array.from(pendingHighlight);
    pendingHighlight.clear();
    list.forEach(code => { if (code.isConnected) highlightBlock(code); });
  }
  let hlObserver = null;
  function scheduleHighlight(code) {
    if (!('IntersectionObserver' in window)) { highlightBlock(code); return; }
    if (!hlObserver) {
      hlObserver = new IntersectionObserver(entries => {
        entries.forEach(entry => {
          if (!entry.isIntersecting) return;
          hlObserver.unobserve(entry.target);
          highlightBlock(entry.target);
        });
      }, { root: byId('content'), rootMargin: '600px 0px' });
    }
    hlObserver.observe(code);
  }
  /* ---------------- code blocks: header, copy, line numbers ---------------- */
  async function writeClipboard(text) {
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) { await navigator.clipboard.writeText(text); return true; }
    } catch (_) { /* fall back */ }
    try {
      const ta = document.createElement('textarea');
      ta.value = text;
      ta.setAttribute('readonly', '');
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand('copy');
      ta.remove();
      return ok;
    } catch (_) { return false; }
  }

  function setCopyState(btn, copied) {
    const label = btn.querySelector('.rd-code-copy-label');
    btn.replaceChild(icon(copied ? 'check' : 'copy', 'rd-ic'), btn.querySelector('svg'));
    label.textContent = copied ? _t('reader.codeCopied') : _t('reader.codeCopy');
    btn.classList.toggle('is-copied', copied);
  }

  function gutterFor(code) {
    const text = code.textContent.replace(/\n$/, '');
    const count = text ? text.split('\n').length : 1;
    const g = chrome(make('span', 'rd-code-gutter'));
    g.setAttribute('aria-hidden', 'true');
    let s = '';
    for (let i = 1; i <= count; i += 1) s += i + (i < count ? '\n' : '');
    g.textContent = s;
    return g;
  }

  function upgradeCode(body) {
    const isCodeDoc = !!body.querySelector(':scope > .code-doc-header');
    body.querySelectorAll('pre > code').forEach(code => {
      const pre = code.parentElement;
      const m = /(?:^|\s)language-([^\s]+)/.exec(code.className || '');
      const lang = m ? m[1] : '';
      code.dataset.rdLang = lang;
      if (pre.closest('.diagram-card')) return;
      if (lang && lang !== 'undefined') scheduleHighlight(code);
      if (pre.closest('.code-chunk-card') || pre.parentElement.classList.contains('rd-code-scroll')) return;

      const box = make('div', 'rd-code');
      carryLine(pre, box);
      pre.removeAttribute('data-source-line');
      const scroll = make('div', 'rd-code-scroll');
      const wantLines = isCodeDoc || pre.dataset.lineNumbers === 'true';
      if (wantLines) box.classList.add('rd-code--lines');
      if (isCodeDoc) {
        box.classList.add('rd-code--doc');
      } else {
        const head = chrome(make('div', 'rd-code-head'));
        head.appendChild(make('span', 'rd-code-lang', langLabel(lang === 'undefined' ? '' : lang)));
        const btn = make('button', 'rd-code-copy');
        btn.type = 'button';
        btn.title = _t('reader.codeCopyTitle');
        btn.setAttribute('aria-label', _t('reader.codeCopyTitle'));
        btn.append(icon('copy', 'rd-ic'), make('span', 'rd-code-copy-label', _t('reader.codeCopy')));
        btn.addEventListener('click', async () => {
          const ok = await writeClipboard(code.textContent);
          if (!ok) { if (typeof showToast === 'function') showToast(_t('toast.copyFailed')); return; }
          setCopyState(btn, true);
          clearTimeout(btn._rdTimer);
          btn._rdTimer = setTimeout(() => setCopyState(btn, false), 1600);
        });
        head.appendChild(btn);
        box.appendChild(head);
      }
      pre.replaceWith(box);
      if (wantLines) scroll.appendChild(gutterFor(code));
      scroll.appendChild(pre);
      box.appendChild(scroll);
    });
  }

  /* ---------------- tables ---------------- */
  const NUM_RE = /^[\s(]*[-+−]?[$€£¥￥]?\s?\d[\d,.\s]*(?:[%‰]|[kKmMbB])?\)?\s*(?:[A-Za-z%]{0,4})?$/;
  function upgradeTables(body) {
    body.querySelectorAll('table').forEach(table => {
      if (table.closest('.rd-table-wrap, .code-chunk-card, .diagram-card')) return;
      const wrap = make('div', 'rd-table-wrap');
      carryLine(table, wrap);
      const bodyRows = table.tBodies[0] ? Array.from(table.tBodies[0].rows) : [];
      if (bodyRows.length > 14) wrap.classList.add('rd-table-wrap--tall');
      const headRow = table.tHead && table.tHead.rows[0];
      const cols = headRow ? headRow.cells.length : (bodyRows[0] ? bodyRows[0].cells.length : 0);
      for (let c = 0; c < cols; c += 1) {
        const cells = bodyRows.map(r => r.cells[c]).filter(Boolean);
        const filled = cells.filter(cell => cell.textContent.trim());
        const aligned = (headRow && headRow.cells[c] && headRow.cells[c].getAttribute('align'));
        if (aligned || !filled.length || !filled.every(cell => NUM_RE.test(cell.textContent.trim()))) continue;
        cells.forEach(cell => cell.classList.add('rd-num'));
        if (headRow && headRow.cells[c]) headRow.cells[c].classList.add('rd-num');
      }
      table.replaceWith(wrap);
      wrap.appendChild(table);
    });
  }
  function refreshTableScroll(body) {
    body.querySelectorAll('.rd-table-wrap').forEach(wrap => {
      const scrolls = wrap.scrollWidth > wrap.clientWidth + 1;
      wrap.classList.toggle('is-scrollable', scrolls);
      if (scrolls) {
        wrap.tabIndex = 0;
        wrap.setAttribute('role', 'region');
        wrap.setAttribute('aria-label', _t('reader.tableRegion'));
      } else {
        wrap.removeAttribute('tabindex');
        wrap.removeAttribute('role');
        wrap.removeAttribute('aria-label');
      }
    });
  }

  /* ---------------- figures + tasks ---------------- */
  const FILE_ALT_RE = /^(?:image|img|screenshot|pic|photo)?[\w\s-]*\.(?:png|jpe?g|gif|webp|svg|bmp|avif)$/i;
  function upgradeFigures(body) {
    body.querySelectorAll('p').forEach(p => {
      if (p.closest('li, td, th, .rd-callout, blockquote')) return;
      const kids = Array.from(p.childNodes).filter(n => !(n.nodeType === 3 && !n.nodeValue.trim()));
      if (kids.length !== 1) return;
      let media = kids[0];
      const img = media.nodeName === 'IMG' ? media
        : (media.nodeName === 'A' && media.children.length === 1 && media.firstElementChild.nodeName === 'IMG' && !media.textContent.trim() ? media.firstElementChild : null);
      if (!img) return;
      const fig = make('figure', 'rd-figure');
      carryLine(p, fig);
      fig.appendChild(media);
      const caption = (img.getAttribute('title') || '').trim() || (img.getAttribute('alt') || '').trim();
      if (caption && !FILE_ALT_RE.test(caption)) fig.appendChild(make('figcaption', null, caption));
      p.replaceWith(fig);
    });
    body.querySelectorAll('img').forEach(img => {
      if (!img.hasAttribute('loading')) img.setAttribute('loading', 'lazy');
      img.decoding = 'async';
      if (!img.closest('a, .code-chunk-card, .diagram-card')) {
        img.classList.add('rd-zoomable');
        img.tabIndex = 0;
      }
    });
  }
  function upgradeTasks(body) {
    body.querySelectorAll('li > input[type="checkbox"]').forEach(box => {
      const li = box.parentElement;
      li.classList.add('rd-task');
      if (box.checked) li.classList.add('is-done');
      li.parentElement.classList.add('rd-task-list');
    });
  }

  /* ---------------- heading anchors ---------------- */
  function upgradeHeadings(body) {
    body.querySelectorAll('h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]').forEach(h => {
      if (h.querySelector(':scope > .rd-anchor') || h.closest('.rd-callout, .academic-callout')) return;
      const a = chrome(make('a', 'rd-anchor'));
      a.href = '#' + h.id;
      a.setAttribute('aria-label', _t('reader.headingAnchor', { title: h.textContent.trim() }));
      const t = document.createElement('template');
      t.innerHTML = '<svg class="rd-ic" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M9 15l6-6"/><path d="M11 6.5l1.2-1.2a4 4 0 0 1 5.6 5.6L16.5 12.1M13 17.5l-1.2 1.2a4 4 0 0 1-5.6-5.6l1.3-1.3"/></svg>';
      a.appendChild(t.content.firstChild);
      h.appendChild(a);
    });
  }
  /* ---------------- reading statistics ---------------- */
  const CJK_RE = /[぀-ヿ㐀-䶿一-鿿豈-﫿가-힯]/g;
  function countWords(text) {
    const s = String(text || '');
    const cjk = (s.match(CJK_RE) || []).length;
    const latin = (s.replace(CJK_RE, ' ').match(/[A-Za-z0-9À-ɏЀ-ӿ]+(?:['’-][A-Za-z0-9À-ɏ]+)*/g) || []).length;
    // Silent reading: ~238 wpm for Latin scripts, ~400 characters/min for CJK.
    const minutes = Math.max(1, Math.round(latin / 238 + cjk / 400));
    return { words: latin + cjk, cjk, latin, minutes };
  }
  function markdownPlainText(md) {
    return String(md || '')
      .replace(/```[\s\S]*?```|~~~[\s\S]*?~~~/g, ' ')
      .replace(/!\[[^\]]*\]\([^)]*\)/g, ' ')
      .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
      .replace(/<[^>]+>/g, ' ')
      .replace(/[#>*_`~|\\-]+/g, ' ');
  }
  let lastStats = null;
  function computeStats(body) {
    const p = window.state && state.pagination;
    if (p && p.enabled && p.rawContent) {
      if (!p.__rdStats || p.__rdStatsSrc !== p.rawContent) {
        p.__rdStats = countWords(markdownPlainText(p.rawContent));
        p.__rdStatsSrc = p.rawContent;
      }
      return p.__rdStats;
    }
    const clone = body.cloneNode(true);
    clone.querySelectorAll('pre, [' + CHROME + '], .code-doc-header, mjx-container, .diagram-card, .rd-footnotes').forEach(n => n.remove());
    return countWords(clone.textContent);
  }
  function statsText(st) {
    return _t('reader.readingTime', { minutes: fmtNum(st.minutes) }) + ' · ' + _t('reader.wordCount', { count: fmtNum(st.words) });
  }

  /* ---------------- progress ---------------- */
  let progressEl = null;
  let progressFrame = 0;
  function ensureProgress() {
    if (progressEl && progressEl.isConnected) return progressEl;
    const content = byId('content');
    if (!content || !content.parentElement) return null;
    progressEl = chrome(make('div', 'rd-progress'));
    progressEl.id = 'rd-progress';
    progressEl.setAttribute('aria-hidden', 'true');
    progressEl.appendChild(make('span', 'rd-progress-fill'));
    content.parentElement.insertBefore(progressEl, content);
    return progressEl;
  }
  function readingRatio() {
    const content = byId('content');
    if (!content) return 0;
    const max = content.scrollHeight - content.clientHeight;
    return max > 4 ? Math.min(1, Math.max(0, content.scrollTop / max)) : 1;
  }
  function updateProgress() {
    progressFrame = 0;
    const content = byId('content');
    const bar = ensureProgress();
    const article = content && content.querySelector(':scope > .markdown-body');
    const active = !!article && !(window.state && state.editing) && content.scrollHeight - content.clientHeight > 4;
    const ratio = readingRatio();
    if (bar) {
      bar.classList.toggle('is-active', active);
      bar.style.setProperty('--rd-progress', ratio.toFixed(4));
    }
    const pct = byId('rd-toc-progress');
    if (pct) {
      const pageLabel = pagedLabel();
      pct.textContent = pageLabel || _t('reader.progressPercent', { percent: Math.round(ratio * 100) });
      const fill = byId('rd-toc-progress-fill');
      if (fill) fill.style.setProperty('--rd-progress', ratio.toFixed(4));
    }
    updateZenPill(ratio);
  }
  function pagedLabel() {
    const p = window.state && state.pagination;
    if (!p || !p.enabled || p.mode !== 'paged' || !p.totalPages) return '';
    const within = readingRatio();
    const overall = Math.round(((p.currentPage + within) / p.totalPages) * 100);
    return _t('reader.progressPercent', { percent: Math.min(100, overall) });
  }
  function scheduleProgress() {
    if (!progressFrame) progressFrame = requestAnimationFrame(updateProgress);
  }

  /* ---------------- zen reading pill ---------------- */
  let zenPill = null;
  let zenPillTimer = 0;
  function updateZenPill(ratio) {
    if (!document.body.classList.contains('zen-mode') || !lastStats) { if (zenPill) zenPill.classList.remove('is-visible'); return; }
    if (!zenPill || !zenPill.isConnected) {
      zenPill = chrome(make('div', 'rd-zen-pill'));
      zenPill.setAttribute('aria-hidden', 'true');
      document.body.appendChild(zenPill);
    }
    const left = Math.max(0, Math.round(lastStats.minutes * (1 - ratio)));
    zenPill.textContent = left > 0 ? _t('reader.minutesLeft', { minutes: fmtNum(left) }) : _t('reader.finished');
    zenPill.classList.add('is-visible');
    clearTimeout(zenPillTimer);
    zenPillTimer = setTimeout(() => zenPill && zenPill.classList.remove('is-visible'), 1400);
  }
  /* ---------------- document meta line ---------------- */
  function insertDocMeta(body, st) {
    body.querySelectorAll(':scope > .rd-doc-meta').forEach(n => n.remove());
    if (!st || st.words < 60 || body.querySelector(':scope > .code-doc-header')) return;
    const p = window.state && state.pagination;
    if (p && p.enabled && p.mode === 'paged' && p.currentPage > 0) return;
    const meta = chrome(make('p', 'rd-doc-meta'));
    meta.appendChild(make('span', null, _t('reader.readingTime', { minutes: fmtNum(st.minutes) })));
    meta.appendChild(make('span', 'rd-doc-meta-dot', '·'));
    meta.appendChild(make('span', null, _t('reader.wordCount', { count: fmtNum(st.words) })));
    const first = body.firstElementChild;
    if (first && first.tagName === 'H1') first.after(meta);
    else body.prepend(meta);
  }

  /* ---------------- lightbox ---------------- */
  let lightbox = null;
  function ensureLightbox() {
    if (lightbox && lightbox.isConnected) return lightbox;
    lightbox = make('div', 'rd-lightbox hidden');
    lightbox.id = 'rd-lightbox-modal';
    lightbox.setAttribute('role', 'dialog');
    lightbox.setAttribute('aria-modal', 'true');
    lightbox.setAttribute('aria-label', _t('reader.imagePreview'));
    const close = make('button', 'rd-lightbox-close');
    close.id = 'rd-lightbox-close';
    close.type = 'button';
    close.setAttribute('aria-label', _t('reader.closePreview'));
    close.title = _t('reader.closePreview');
    close.appendChild(icon('close', 'rd-ic'));
    const figure = make('figure', 'rd-lightbox-figure');
    const img = make('img', 'rd-lightbox-img');
    img.alt = '';
    const cap = make('figcaption', 'rd-lightbox-caption');
    figure.append(img, cap);
    lightbox.append(close, figure);
    const hide = () => {
      lightbox.classList.remove('is-open');
      lightbox.classList.add('hidden');
    };
    close.addEventListener('click', event => { event.stopPropagation(); hide(); });
    lightbox.addEventListener('click', event => { if (event.target !== img) hide(); });
    document.body.appendChild(lightbox);
    return lightbox;
  }
  function openLightbox(source) {
    const box = ensureLightbox();
    const img = box.querySelector('.rd-lightbox-img');
    const cap = box.querySelector('.rd-lightbox-caption');
    img.src = source.currentSrc || source.src;
    img.alt = source.alt || '';
    const text = (source.getAttribute('title') || source.alt || '').trim();
    cap.textContent = FILE_ALT_RE.test(text) ? '' : text;
    cap.hidden = !cap.textContent;
    box.classList.remove('hidden');
    if (window.ReadMDModal) window.ReadMDModal.open(box);
    requestAnimationFrame(() => box.classList.add('is-open'));
    box.querySelector('.rd-lightbox-close').focus({ preventScroll: true });
  }

  /* ---------------- reading preferences ---------------- */
  const PREF_WIDTHS = ['narrow', 'normal', 'wide'];
  const PREF_LEADING = ['compact', 'normal', 'relaxed'];
  const PREF_FONTS = ['sans', 'serif'];
  function applyReadingPrefs() {
    if (!window.state) return;
    const b = document.body;
    b.dataset.readingFont = PREF_FONTS.includes(state.readingFont) ? state.readingFont : 'sans';
    b.dataset.readingWidth = PREF_WIDTHS.includes(state.readingWidth) ? state.readingWidth : 'normal';
    b.dataset.readingLeading = PREF_LEADING.includes(state.readingLeading) ? state.readingLeading : 'normal';
    syncPrefsPanel();
    invalidateSpy();
  }
  function setPref(key, value) {
    state[key] = value;
    applyReadingPrefs();
    if (typeof saveSettings === 'function') saveSettings();
    requestAnimationFrame(() => { const body = articleEl(); if (body) refreshTableScroll(body); scheduleProgress(); });
  }

  let toolsEl = null;
  let prefsEl = null;
  function segmented(name, key, options) {
    const group = make('div', 'rd-seg');
    group.setAttribute('role', 'radiogroup');
    group.setAttribute('aria-label', _t('reader.pref' + name));
    options.forEach(([value, labelKey]) => {
      const btn = make('button', 'rd-seg-btn', _t(labelKey));
      btn.type = 'button';
      btn.setAttribute('role', 'radio');
      btn.dataset.prefKey = key;
      btn.dataset.prefValue = value;
      btn.addEventListener('click', () => setPref(key, value));
      group.appendChild(btn);
    });
    group.addEventListener('keydown', event => {
      if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
      event.preventDefault();
      const buttons = Array.from(group.querySelectorAll('.rd-seg-btn'));
      const at = buttons.findIndex(b => b.getAttribute('aria-checked') === 'true');
      const dir = event.key === 'ArrowLeft' || event.key === 'ArrowUp' ? -1 : 1;
      const next = buttons[(at + dir + buttons.length) % buttons.length];
      next.click();
      next.focus();
    });
    return group;
  }
  function prefRow(labelKey, control) {
    const row = make('div', 'rd-pref-row');
    const label = make('span', 'rd-pref-label', _t(labelKey));
    row.append(label, control);
    return row;
  }
  function buildPrefsPanel() {
    const panel = make('div', 'rd-prefs hidden');
    panel.id = 'rd-prefs';
    panel.setAttribute('role', 'dialog');
    panel.setAttribute('aria-label', _t('reader.prefsTitle'));
    panel.appendChild(make('div', 'rd-prefs-title', _t('reader.prefsTitle')));

    const size = make('div', 'rd-size');
    const dec = make('button', 'rd-size-btn', 'A');
    dec.type = 'button';
    dec.classList.add('rd-size-btn--dec');
    dec.setAttribute('aria-label', _t('toolbar.zoomOut'));
    dec.title = _t('toolbar.zoomOut');
    const val = make('output', 'rd-size-value');
    val.id = 'rd-size-value';
    val.setAttribute('aria-live', 'polite');
    const inc = make('button', 'rd-size-btn', 'A');
    inc.type = 'button';
    inc.classList.add('rd-size-btn--inc');
    inc.setAttribute('aria-label', _t('toolbar.zoomIn'));
    inc.title = _t('toolbar.zoomIn');
    dec.addEventListener('click', () => { if (typeof zoom === 'function') zoom(-10); syncPrefsPanel(); invalidateSpy(); });
    inc.addEventListener('click', () => { if (typeof zoom === 'function') zoom(10); syncPrefsPanel(); invalidateSpy(); });
    size.append(dec, val, inc);
    panel.appendChild(prefRow('reader.prefSize', size));
    panel.appendChild(prefRow('reader.prefFont', segmented('Font', 'readingFont', [['sans', 'reader.fontSans'], ['serif', 'reader.fontSerif']])));
    panel.appendChild(prefRow('reader.prefLeading', segmented('Leading', 'readingLeading', [['compact', 'reader.leadingCompact'], ['normal', 'reader.leadingNormal'], ['relaxed', 'reader.leadingRelaxed']])));
    panel.appendChild(prefRow('reader.prefWidth', segmented('Width', 'readingWidth', [['narrow', 'reader.widthNarrow'], ['normal', 'reader.widthNormal'], ['wide', 'reader.widthWide']])));
    const foot = make('div', 'rd-prefs-foot');
    foot.id = 'rd-prefs-stats';
    panel.appendChild(foot);
    return panel;
  }
  function syncPrefsPanel() {
    if (!prefsEl || !window.state) return;
    prefsEl.querySelectorAll('.rd-seg-btn').forEach(btn => {
      const on = document.body.dataset[btn.dataset.prefKey] === btn.dataset.prefValue;
      btn.setAttribute('aria-checked', on ? 'true' : 'false');
      btn.tabIndex = on ? 0 : -1;
    });
    const val = prefsEl.querySelector('#rd-size-value');
    if (val) val.textContent = (state.fontSize || 100) + '%';
    const dec = prefsEl.querySelector('.rd-size-btn--dec');
    const inc = prefsEl.querySelector('.rd-size-btn--inc');
    if (dec) dec.disabled = state.fontSize <= 70;
    if (inc) inc.disabled = state.fontSize >= 180;
    const foot = prefsEl.querySelector('#rd-prefs-stats');
    if (foot) foot.textContent = lastStats ? statsText(lastStats) : '';
  }
  function ensureTools() {
    if (toolsEl && toolsEl.isConnected) return toolsEl;
    const content = byId('content');
    if (!content || !content.parentElement) return null;
    toolsEl = chrome(make('div', 'rd-tools'));
    toolsEl.id = 'rd-tools';
    const btn = make('button', 'rd-tools-btn');
    btn.id = 'rd-prefs-btn';
    btn.type = 'button';
    btn.setAttribute('aria-haspopup', 'dialog');
    btn.setAttribute('aria-expanded', 'false');
    btn.setAttribute('aria-controls', 'rd-prefs');
    btn.setAttribute('aria-label', _t('reader.prefsTitle'));
    btn.title = _t('reader.prefsTitle');
    btn.appendChild(icon('type', 'rd-ic'));
    btn.addEventListener('click', () => (prefsEl && !prefsEl.classList.contains('hidden') ? closePrefs(true) : openPrefs()));
    prefsEl = buildPrefsPanel();
    toolsEl.append(btn, prefsEl);
    toolsEl.addEventListener('keydown', event => {
      if (event.key === 'Escape' && prefsEl && !prefsEl.classList.contains('hidden')) {
        event.preventDefault();
        event.stopPropagation();
        closePrefs(true);
      }
    });
    content.parentElement.insertBefore(toolsEl, content);
    document.addEventListener('pointerdown', event => {
      if (prefsEl && !prefsEl.classList.contains('hidden') && !toolsEl.contains(event.target)) closePrefs(false);
    }, true);
    syncPrefsPanel();
    return toolsEl;
  }
  function openPrefs() {
    ensureTools();
    syncPrefsPanel();
    prefsEl.classList.remove('hidden');
    toolsEl.classList.add('is-open');
    byId('rd-prefs-btn').setAttribute('aria-expanded', 'true');
    const first = prefsEl.querySelector('button:not([disabled])');
    if (first) first.focus({ preventScroll: true });
  }
  function closePrefs(restoreFocus) {
    if (!prefsEl || prefsEl.classList.contains('hidden')) return;
    prefsEl.classList.add('hidden');
    toolsEl.classList.remove('is-open');
    const btn = byId('rd-prefs-btn');
    btn.setAttribute('aria-expanded', 'false');
    if (restoreFocus) btn.focus({ preventScroll: true });
  }
  function relabelTools() {
    if (!toolsEl) return;
    const open = prefsEl && !prefsEl.classList.contains('hidden');
    prefsEl.remove();
    prefsEl = buildPrefsPanel();
    toolsEl.appendChild(prefsEl);
    if (open) prefsEl.classList.remove('hidden');
    const btn = byId('rd-prefs-btn');
    btn.setAttribute('aria-label', _t('reader.prefsTitle'));
    btn.title = _t('reader.prefsTitle');
    syncPrefsPanel();
  }
  /* ---------------- floating chrome placement ---------------- */
  // The progress bar and the preferences button live outside #content (its
  // children are replaced on every render and some specs count buttons in
  // it), so they are fixed-positioned against the reading area's rect.
  let placeFrame = 0;
  function placeChrome() {
    placeFrame = 0;
    const content = byId('content');
    const tools = ensureTools();
    const bar = ensureProgress();
    if (!content) return;
    const r = content.getBoundingClientRect();
    const visible = r.width > 0 && r.height > 0 && !content.classList.contains('hidden') && !!content.querySelector(':scope > .markdown-body');
    const scrollbar = content.offsetWidth - content.clientWidth;
    const zenTop = document.body.classList.contains('zen-mode') ? 0 : r.top;
    [tools, bar].forEach(el => {
      if (!el) return;
      el.style.setProperty('--rd-stage-top', Math.round(zenTop) + 'px');
      el.style.setProperty('--rd-stage-left', Math.round(r.left) + 'px');
      el.style.setProperty('--rd-stage-width', Math.max(0, Math.round(r.width - scrollbar)) + 'px');
    });
    if (tools) {
      tools.classList.toggle('is-visible', visible && !(window.state && state.editing));
      if (!visible) closePrefs(false);
    }
    if (bar && !visible) bar.classList.remove('is-active');
  }
  let lastScrollTop = 0;
  function tuckTools() {
    const content = byId('content');
    if (!content || !toolsEl) return;
    const top = content.scrollTop;
    const delta = top - lastScrollTop;
    if (Math.abs(delta) < 6) return;
    const open = prefsEl && !prefsEl.classList.contains('hidden');
    toolsEl.classList.toggle('is-tucked', !open && delta > 0 && top > 160);
    lastScrollTop = top;
  }
  function schedulePlace() { if (!placeFrame) placeFrame = requestAnimationFrame(placeChrome); }

  /* ---------------- scroll-spy cache hook ---------------- */
  function invalidateSpy() {
    if (typeof window.invalidateTocSpy === 'function') window.invalidateTocSpy();
  }

  function articleEl() { return document.querySelector('#content > .markdown-body'); }

  /* ---------------- outline decoration (header, folding) ---------------- */
  function buildTocHead() {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    const head = document.createElement('div');
    head.className = 'rd-toc-head';
    const row = document.createElement('div');
    row.className = 'rd-toc-row';
    const stats = document.createElement('span');
    stats.className = 'rd-toc-stats';
    stats.id = 'rd-toc-stats';
    stats.textContent = statsForToc();
    const right = document.createElement('span');
    right.className = 'rd-toc-row';
    const pct = document.createElement('span');
    pct.className = 'rd-toc-pct';
    pct.id = 'rd-toc-progress';
    const fold = document.createElement('button');
    fold.type = 'button';
    fold.className = 'rd-toc-fold';
    fold.id = 'rd-toc-fold';
    fold.addEventListener('click', () => {
      const list = byId('toc-list');
      const collapse = fold.dataset.state !== 'collapsed';
      list.querySelectorAll('.rd-toc-item.has-kids').forEach(item => {
        setTocItemCollapsed(item, collapse && +item.dataset.level >= tocTopLevel(list));
      });
      applyTocFolding(list);
    });
    right.append(pct, fold);
    row.append(stats, right);
    const bar = document.createElement('div');
    bar.className = 'rd-toc-bar';
    bar.setAttribute('aria-hidden', 'true');
    const fill = document.createElement('span');
    fill.className = 'rd-toc-bar-fill';
    fill.id = 'rd-toc-progress-fill';
    bar.appendChild(fill);
    head.append(row, bar);
    return head;
  }

  function tocTopLevel(list) {
    let top = 6;
    list.querySelectorAll('.rd-toc-item').forEach(item => { top = Math.min(top, +item.dataset.level); });
    return top;
  }

  function setTocItemCollapsed(item, collapsed) {
    item.classList.toggle('is-collapsed', collapsed);
    const twisty = item.querySelector('.rd-toc-twisty');
    if (twisty) twisty.setAttribute('aria-expanded', collapsed ? 'false' : 'true');
  }

  function applyTocFolding(list) {
    let hideBelow = Infinity;
    list.querySelectorAll('.rd-toc-item').forEach(item => {
      const level = +item.dataset.level;
      if (level <= hideBelow) hideBelow = Infinity;
      item.classList.toggle('is-hidden', level > hideBelow);
      if (hideBelow === Infinity && item.classList.contains('is-collapsed')) hideBelow = level;
    });
    syncTocFoldAllButton(list);
    if (typeof window.updateActiveTocHeading === 'function') window.updateActiveTocHeading();
  }

  function syncTocFoldAllButton(list) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    const fold = list.querySelector('#rd-toc-fold');
    if (!fold) return;
    const parents = list.querySelectorAll('.rd-toc-item.has-kids');
    fold.hidden = parents.length === 0;
    const anyCollapsed = list.querySelector('.rd-toc-item.is-collapsed');
    fold.dataset.state = anyCollapsed ? 'collapsed' : 'expanded';
    const label = anyCollapsed ? _t('reader.tocExpandAll') : _t('reader.tocCollapseAll');
    fold.setAttribute('aria-label', label);
    fold.title = label;
    fold.innerHTML = anyCollapsed
      ? '<svg class="rd-ic" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="m7 9 5 5 5-5"/><path d="m7 4 5 5 5-5" opacity=".45"/></svg>'
      : '<svg class="rd-ic" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="m7 15 5-5 5 5"/><path d="m7 20 5-5 5 5" opacity=".45"/></svg>';
  }

  function decorateToc(list, items) {
    const _t = (k, p) => window.i18n ? window.i18n.t(k, p) : k;
    list.querySelectorAll(':scope > .rd-toc-head').forEach(node => node.remove());
    list.prepend(buildTocHead());
    // Items followed by a deeper heading get a twisty that folds their subtree.
    (items || []).forEach((item, index) => {
      const next = items[index + 1];
      if (!next || +next.dataset.level <= +item.dataset.level) return;
      item.classList.add('has-kids');
      const twisty = document.createElement('button');
      twisty.type = 'button';
      twisty.className = 'rd-toc-twisty';
      twisty.setAttribute('aria-expanded', 'true');
      twisty.setAttribute('aria-label', _t('reader.tocToggleSection', { title: item.firstChild.textContent }));
      twisty.innerHTML = '<svg class="rd-ic" viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="m9 6 6 6-6 6"/></svg>';
      twisty.addEventListener('click', event => {
        event.preventDefault();
        event.stopPropagation();
        setTocItemCollapsed(item, !item.classList.contains('is-collapsed'));
        applyTocFolding(list);
      });
      item.appendChild(twisty);
    });
    syncTocFoldAllButton(list);
  }


  /* ---------------- public entry points ---------------- */
  /** Structural upgrades; runs before link/image fix-ups so new anchors get handlers. */
  function enhance(body) {
    if (!body || body.__rdEnhanced) return;
    body.__rdEnhanced = true;
    if (hlObserver) hlObserver.disconnect();
    try {
      body.querySelectorAll('blockquote').forEach(upgradeCallout);
      upgradeFootnotes(body);
      upgradeDefinitionLists(body);
      upgradeCode(body);
      upgradeTables(body);
      upgradeFigures(body);
      upgradeTasks(body);
      upgradeHeadings(body);
    } catch (e) {
      console.debug('reader enhance failed:', e);
    }
  }

  /** Measurements that need final layout: stats, table scroll hints, progress. */
  function afterRender(body) {
    if (!body) return;
    const isReader = body === articleEl();
    if (isReader) {
      try {
        lastStats = computeStats(body);
        insertDocMeta(body, lastStats);
        const tocStats = byId('rd-toc-stats');
        if (tocStats) tocStats.textContent = statsText(lastStats);
      } catch (_) { lastStats = null; }
    }
    requestAnimationFrame(() => {
      refreshTableScroll(body);
      invalidateSpy();
      schedulePlace();
      scheduleProgress();
      syncPrefsPanel();
      if (typeof window.updateActiveTocHeading === 'function') window.updateActiveTocHeading();
    });
    body.querySelectorAll('img').forEach(img => {
      if (!img.complete) img.addEventListener('load', () => { invalidateSpy(); refreshTableScroll(body); }, { once: true });
    });
  }

  function statsForToc() { return lastStats ? statsText(lastStats) : ''; }

  function init() {
    const content = byId('content');
    if (!content || content.__rdBound) return;
    content.__rdBound = true;
    content.addEventListener('scroll', scheduleProgress, { passive: true });
    content.addEventListener('scroll', tuckTools, { passive: true });
    content.addEventListener('keydown', event => {
      if (event.key !== 'Enter' && event.key !== ' ') return;
      const img = event.target.closest && event.target.closest('img.rd-zoomable');
      if (!img) return;
      event.preventDefault();
      openLightbox(img);
    });
    content.addEventListener('click', event => {
      const img = event.target.closest && event.target.closest('img.rd-zoomable');
      if (!img || !content.contains(img) || event.defaultPrevented) return;
      event.preventDefault();
      openLightbox(img);
    });
    if ('ResizeObserver' in window) {
      new ResizeObserver(() => {
        schedulePlace();
        invalidateSpy();
        const body = articleEl();
        if (body) refreshTableScroll(body);
      }).observe(content);
    }
    window.addEventListener('resize', schedulePlace);
    new MutationObserver(schedulePlace).observe(document.body, { attributes: true, attributeFilter: ['class'] });
    new MutationObserver(schedulePlace).observe(content, { attributes: true, attributeFilter: ['class'] });
    window.addEventListener('readmd:language-changed', () => {
      relabelTools();
      if (lightbox) lightbox.setAttribute('aria-label', _t('reader.imagePreview'));
    });
    applyReadingPrefs();
    schedulePlace();
  }

  window.ReadMDReader = {
    enhance,
    prepare: prepareSource,
    afterRender,
    applyReadingPrefs,
    openPrefs,
    closePrefs,
    highlight: highlightToHtml,
    countWords,
    statsText: statsForToc,
    readingRatio,
    langLabel,
    flushPendingHighlight,
    decorateToc,
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
