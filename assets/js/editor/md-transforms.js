'use strict';
/* ============================================================
   ReadMD Editor - Markdown syntax transforms
   Pure functions (no DOM, no CodeMirror): the toolbar asks
   computeSyntaxEdit(doc, from, to, kind, ph) for ONE change and the
   resulting selection, so every toolbar action is a single undo step.
   ============================================================ */

(function (root) {
  const LIST_RE = /^(?:[-*+][ \t]+\[[ xX]\](?:[ \t]+|$)|[-*+](?:[ \t]+|$)|\d{1,9}[.)](?:[ \t]+|$))/;
  const HEADING_RE = /^#{1,6}(?:[ \t]+|$)/;
  const WORD_RE = /[A-Za-z0-9_À-ɏͰ-ϿЀ-ӿ]/;

  const INLINE = {
    bold:   { ch: '*', test: r => r >= 2, rm: () => 2, open: () => '**' },
    italic: { ch: '*', test: r => r % 2 === 1, rm: () => 1, open: () => '*' },
    strike: { ch: '~', test: r => r >= 2, rm: () => 2, open: () => '~~' },
    code:   { ch: '`', test: r => r >= 1, rm: r => r, open: s => '`'.repeat(maxRun(s, '`') + 1) },
    math:   { ch: '$', test: r => r === 1, rm: () => 1, open: () => '$' },
  };

  function lineAt(doc, pos) {
    const from = doc.lastIndexOf('\n', pos - 1) + 1;
    let to = doc.indexOf('\n', pos);
    if (to < 0) to = doc.length;
    return { from, to, text: doc.slice(from, to) };
  }

  function isBlank(s) { return !s || s.trim() === ''; }

  function maxRun(s, ch) {
    let best = 0, cur = 0;
    for (const c of s) { cur = c === ch ? cur + 1 : 0; if (cur > best) best = cur; }
    return best;
  }

  function runLeft(doc, pos, ch) { let n = 0; while (pos - n > 0 && doc[pos - n - 1] === ch) n++; return n; }
  function runRight(doc, pos, ch) { let n = 0; while (pos + n < doc.length && doc[pos + n] === ch) n++; return n; }

  /* Lines touched by [from, to]; a selection ending at column 0 does not include that line. */
  function blockRange(doc, from, to) {
    const first = lineAt(doc, from);
    let end = to;
    if (to > from && doc[to - 1] === '\n') end = to - 1;
    const last = lineAt(doc, Math.max(end, first.from));
    return { from: first.from, to: last.to };
  }

  function edit(from, to, insert, anchor, head) {
    return { changes: { from, to, insert }, selection: { anchor, head: head === undefined ? anchor : head } };
  }

  /* ---------------- 块级：标题 / 引用 / 列表 ---------------- */

  function listKind(body) {
    const m = LIST_RE.exec(body);
    if (!m) return null;
    if (/^[-*+][ \t]+\[[ xX]\]/.test(m[0])) return 'task';
    return /^\d/.test(m[0]) ? 'ordered' : 'list';
  }

  function lineEdit(doc, from, to, kind, ph) {
    const range = blockRange(doc, from, to);
    const lines = doc.slice(range.from, range.to).split('\n');
    const parts = lines.map(t => { const ind = /^[ \t]*/.exec(t)[0]; return { ind, body: t.slice(ind.length) }; });
    const content = parts.filter(p => !isBlank(p.body));
    const numbers = new Map();
    const prefix = (ind) => {
      if (kind === 'h2') return '## ';
      if (kind === 'quote') return '> ';
      if (kind === 'list') return '- ';
      if (kind === 'task') return '- [ ] ';
      const n = (numbers.get(ind) || 0) + 1;
      numbers.set(ind, n);
      return n + '. ';
    };

    if (!content.length) {
      const p = parts[0];
      const line = lineAt(doc, from);
      const pre = p.ind + prefix(p.ind);
      const at = line.from + pre.length;
      return edit(line.from, line.to, pre + ph, at, at + ph.length);
    }

    const has = p => {
      if (kind === 'h2') return /^##(?:[ \t]+|$)/.test(p.body);
      if (kind === 'quote') return p.body.startsWith('>');
      return listKind(p.body) === kind;
    };
    const strip = body => {
      if (kind === 'h2') return body.replace(HEADING_RE, '');
      if (kind === 'quote') return body.replace(/^>[ \t]?/, '');
      return body.replace(LIST_RE, '');
    };
    const remove = content.every(has);

    const out = parts.map(p => {
      if (isBlank(p.body)) {
        if (kind === 'quote' && !remove && lines.length > 1) return { text: p.ind + '>', pre: p.ind.length + 1 };
        return { text: p.ind + p.body, pre: p.ind.length + p.body.length };
      }
      if (remove) return { text: p.ind + strip(p.body), pre: p.ind.length };
      const pre = p.ind + prefix(p.ind);
      const body = kind === 'quote' ? p.body : strip(p.body);
      return { text: pre + body, pre: pre.length };
    });
    const insert = out.map(o => o.text).join('\n');

    if (from === to && lines.length === 1) {
      // 保持光标相对行尾的位置，但不落进新前缀里
      const fromEnd = range.to - from;
      const col = Math.max(out[0].pre, out[0].text.length - fromEnd);
      return edit(range.from, range.to, insert, range.from + col);
    }
    return edit(range.from, range.to, insert, range.from, range.from + insert.length);
  }

  /* ---------------- 独占行的块：分隔线 / 代码块 / 公式块 / 表格 ---------------- */

  /* Place `block` so it sits on its own lines with a blank line on both sides
     (except at the very start / end of the document). */
  function placeBlock(doc, rFrom, rTo, block, selFrom, selTo) {
    let lead = '', trail = '';
    if (rFrom > 0 && doc[rFrom - 1] !== '\n') {
      lead = '\n\n';
    } else if (rFrom > 0 && !isBlank(lineAt(doc, rFrom - 1).text)) {
      lead = '\n';
    }
    if (rTo < doc.length && !isBlank(lineAt(doc, rTo + 1).text)) trail = '\n';
    const base = rFrom + lead.length;
    return edit(rFrom, rTo, lead + block + trail, base + selFrom, base + selTo);
  }

  function targetRange(doc, from, to) {
    if (from !== to) return blockRange(doc, from, to);
    const line = lineAt(doc, from);
    return isBlank(line.text) ? { from: line.from, to: line.to } : { from: line.to, to: line.to };
  }

  function hrEdit(doc, from, to) {
    const line = lineAt(doc, to > from && doc[to - 1] === '\n' ? to - 1 : to);
    const r = isBlank(line.text) ? { from: line.from, to: line.to } : { from: line.to, to: line.to };
    const res = placeBlock(doc, r.from, r.to, '---', 3, 3);
    const after = res.changes.from + res.changes.insert.length;
    // 光标落到分隔线之后的空行（如果有），方便继续输入
    const cur = res.changes.insert.endsWith('\n') ? after : res.selection.anchor;
    res.selection = { anchor: cur, head: cur };
    return res;
  }

  function fencedEdit(doc, from, to, kind, ph) {
    const math = kind === 'mathblock';
    const r = targetRange(doc, from, to);
    const text = from === to ? '' : doc.slice(r.from, r.to);
    const lines = text.split('\n');
    if (text && lines.length >= 2) {
      const a = lines[0].trim(), z = lines[lines.length - 1].trim();
      const fenced = math
        ? a === '$$' && z === '$$'
        : /^(`{3,}|~{3,})/.test(a) && z.length >= 3 && z[0] === a[0] && /^(`+|~+)$/.test(z);
      if (fenced) {
        const inner = lines.slice(1, -1).join('\n');
        return edit(r.from, r.to, inner, r.from, r.from + inner.length);
      }
    }
    const body = text || (math ? 'x^2' : ph);
    const fence = math ? '$$' : '`'.repeat(Math.max(3, maxRun(body, '`') + 1));
    const block = fence + '\n' + body + '\n' + fence;
    return placeBlock(doc, r.from, r.to, block, fence.length + 1, fence.length + 1 + body.length);
  }

  function tableEdit(doc, from, to, ph) {
    const r = targetRange(doc, from, to);
    const text = from === to ? '' : doc.slice(r.from, r.to);
    const rows = text ? text.split('\n').filter(l => !isBlank(l)) : [];
    const esc = c => c.trim().replace(/\|/g, '\\|');
    if (rows.length && rows.every(l => l.includes('\t'))) {
      const cells = rows.map(l => l.split('\t').map(esc));
      const cols = Math.max(...cells.map(c => c.length));
      const fmt = c => '| ' + Array.from({ length: cols }, (_, i) => c[i] || '').join(' | ') + ' |';
      const block = [fmt(cells[0]), '| ' + Array(cols).fill('---').join(' | ') + ' |', ...cells.slice(1).map(fmt)].join('\n');
      return placeBlock(doc, r.from, r.to, block, 0, block.length);
    }
    const head = '| Col 1 | Col 2 |\n| --- | --- |\n';
    if (rows.length > 1) {
      const block = head + rows.map(l => '| ' + esc(l) + ' |  |').join('\n');
      return placeBlock(doc, r.from, r.to, block, 0, block.length);
    }
    const cell = rows.length ? esc(rows[0]) : ph;
    const block = head + '| ' + cell + ' |  |';
    return placeBlock(doc, r.from, r.to, block, head.length + 2, head.length + 2 + cell.length);
  }

  /* ---------------- 行内：加粗 / 斜体 / 删除线 / 行内代码 / 公式 ---------------- */

  function innerWrapped(s, spec) {
    let l = 0; while (l < s.length && s[l] === spec.ch) l++;
    let r = 0; while (r < s.length && s[s.length - 1 - r] === spec.ch) r++;
    if (l + r > s.length || l + r === s.length && l < 2 * spec.rm(l)) return 0;
    if (!spec.test(l) || !spec.test(r)) return 0;
    if (spec.ch === '`' && l !== r) return 0;
    const n = spec.rm(l);
    return s.length >= 2 * n ? n : 0;
  }

  function wrapText(s, spec) {
    const open = spec.open(s);
    const pad = spec.ch === '`' && (s.startsWith('`') || s.endsWith('`')) ? ' ' : '';
    return { text: open + pad + s + pad + open, lead: open.length + pad.length };
  }

  function inlineEdit(doc, from, to, kind, ph) {
    const spec = INLINE[kind];
    if (from === to) {
      // 空包裹 **|** → 取消
      const l = runLeft(doc, from, spec.ch), r = runRight(doc, from, spec.ch);
      if (l && r && spec.test(l) && spec.test(r) && (spec.ch !== '`' || l === r)) {
        const n = spec.rm(l);
        return edit(from - n, from + n, '', from - n);
      }
      // 光标在单词内部时作用于整个单词
      let a = from, b = from;
      while (a > 0 && WORD_RE.test(doc[a - 1])) a--;
      while (b < doc.length && WORD_RE.test(doc[b])) b++;
      if (a < b && a < from && from < b) return inlineEdit(doc, a, b, kind, ph);
      const w = wrapText(ph, spec);
      return edit(from, to, w.text, from + w.lead, from + w.lead + ph.length);
    }

    const s = doc.slice(from, to);
    if (s.includes('\n')) {
      const segs = s.split('\n');
      const unwrap = segs.filter(x => !isBlank(x)).every(x => innerWrapped(x.trim(), spec));
      const out = segs.map(x => {
        if (isBlank(x)) return x;
        const lw = /^\s*/.exec(x)[0], tw = /\s*$/.exec(x)[0];
        const core = x.slice(lw.length, x.length - tw.length);
        if (unwrap) { const n = innerWrapped(core, spec); return lw + core.slice(n, core.length - n) + tw; }
        return lw + (innerWrapped(core, spec) ? core : wrapText(core, spec).text) + tw;
      }).join('\n');
      return edit(from, to, out, from, from + out.length);
    }

    // 只包裹去掉首尾空白后的部分（`** x**` 不是加粗）
    const a = from + /^\s*/.exec(s)[0].length;
    const b = to - /\s*$/.exec(s)[0].length;
    if (a >= b) return inlineEdit(doc, from, from, kind, ph);
    const core = doc.slice(a, b);

    const inner = innerWrapped(core, spec);
    if (inner) {
      const t = core.slice(inner, core.length - inner);
      return edit(a, b, t, a, a + t.length);
    }
    const l = runLeft(doc, a, spec.ch), r = runRight(doc, b, spec.ch);
    if (l && r && spec.test(l) && spec.test(r) && (spec.ch !== '`' || l === r)) {
      const n = spec.rm(l);
      return edit(a - n, b + n, core, a - n, b - n);
    }
    const w = wrapText(core, spec);
    return edit(a, b, w.text, a + w.lead, a + w.lead + core.length);
  }

  /* ---------------- 链接 / 图片 ---------------- */

  function linkEdit(doc, from, to, kind, ph) {
    const s = doc.slice(from, to).trim();
    const bang = kind === 'image' ? '!' : '';
    const label = ph.desc && kind === 'image' ? ph.desc : ph.text;
    if (/^(https?:\/\/|www\.|\.{0,2}\/)\S*$/i.test(s)) {
      const text = bang + '[' + label + '](' + s + ')';
      const at = from + bang.length + 1;
      return edit(from, to, text, at, at + label.length);
    }
    const shown = s.replace(/\n+/g, ' ') || label;
    const text = bang + '[' + shown + '](url)';
    const at = from + bang.length + shown.length + 3;
    return edit(from, to, text, at, at + 3);
  }

  const DEFAULT_PH = { text: 'text', code: 'code', heading: 'Heading', quote: 'Quote', item: 'Item', task: 'Task', desc: 'image' };

  /**
   * @param {string} doc   whole document ("\n" line breaks)
   * @param {number} from  selection start
   * @param {number} to    selection end
   * @param {string} kind  toolbar command
   * @param {object} [ph]  localised placeholders
   * @returns {{changes:{from:number,to:number,insert:string}, selection:{anchor:number,head:number}}|null}
   */
  function computeSyntaxEdit(doc, from, to, kind, ph) {
    doc = String(doc == null ? '' : doc);
    from = Math.max(0, Math.min(from | 0, doc.length));
    to = Math.max(0, Math.min(to == null ? from : to | 0, doc.length));
    if (to < from) { const t = from; from = to; to = t; }
    const p = Object.assign({}, DEFAULT_PH, ph || {});
    switch (kind) {
      case 'h2': return lineEdit(doc, from, to, kind, p.heading);
      case 'quote': return lineEdit(doc, from, to, kind, p.quote);
      case 'list': case 'ordered': return lineEdit(doc, from, to, kind, p.item);
      case 'task': return lineEdit(doc, from, to, kind, p.task);
      case 'bold': case 'italic': case 'strike': return inlineEdit(doc, from, to, kind, p.text);
      case 'code': return inlineEdit(doc, from, to, kind, p.code);
      case 'math': return inlineEdit(doc, from, to, kind, 'x^2');
      case 'hr': return hrEdit(doc, from, to);
      case 'codeblock': case 'mathblock': return fencedEdit(doc, from, to, kind, p.code);
      case 'table': return tableEdit(doc, from, to, p.text);
      case 'link': case 'image': return linkEdit(doc, from, to, kind, p);
      default: return null;
    }
  }

  /* Apply an edit to a string (for tests and non-CodeMirror fallbacks). */
  function applySyntaxEdit(doc, e) {
    if (!e) return doc;
    return doc.slice(0, e.changes.from) + e.changes.insert + doc.slice(e.changes.to);
  }

  const api = { computeSyntaxEdit, applySyntaxEdit };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  if (root) root.ReadMDTransforms = api;
})(typeof window !== 'undefined' ? window : null);
