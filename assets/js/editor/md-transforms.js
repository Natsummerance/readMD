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

  /* ---------------- 标题级别：H1–H6 / 正文 ---------------- */

  /* Set every touched line to heading `level` (0 = paragraph). Pressing the
     same level again turns the lines back into paragraphs. */
  function headingLevelEdit(doc, from, to, level, ph) {
    const range = blockRange(doc, from, to);
    const lines = doc.slice(range.from, range.to).split('\n');
    const want = level ? '#'.repeat(level) + ' ' : '';
    const levelOf = l => { const m = /^[ \t]{0,3}(#{1,6})(?:[ \t]+|$)/.exec(l); return m ? m[1].length : 0; };
    const content = lines.filter(l => !isBlank(l));
    if (!content.length) {
      if (!level) return null;
      const line = lineAt(doc, from);
      const at = line.from + want.length;
      return edit(line.from, line.to, want + ph, at, at + ph.length);
    }
    const remove = level && content.every(l => levelOf(l) === level);
    const out = lines.map(l => {
      if (isBlank(l)) return { text: l, pre: l.length };
      const body = l.replace(/^[ \t]*/, '').replace(HEADING_RE, '');
      const pre = remove ? '' : want;
      return { text: pre + body, pre: pre.length };
    });
    const insert = out.map(o => o.text).join('\n');
    if (insert === doc.slice(range.from, range.to)) return null;
    if (from === to && lines.length === 1) {
      const fromEnd = range.to - from;
      const col = Math.max(out[0].pre, out[0].text.length - fromEnd);
      return edit(range.from, range.to, insert, range.from + col);
    }
    return edit(range.from, range.to, insert, range.from, range.from + insert.length);
  }

  /* ---------------- 提示块（GitHub callout） ---------------- */

  const CALLOUT_TYPES = ['NOTE', 'TIP', 'IMPORTANT', 'WARNING', 'CAUTION'];

  function calloutEdit(doc, from, to, type, ph) {
    const t = CALLOUT_TYPES.includes(String(type).toUpperCase()) ? String(type).toUpperCase() : 'NOTE';
    const head = '> [!' + t + ']';
    if (from === to || isBlank(doc.slice(from, to))) {
      const r = targetRange(doc, from, to);
      const block = head + '\n> ' + ph;
      return placeBlock(doc, r.from, r.to, block, head.length + 3, head.length + 3 + ph.length);
    }
    const r = blockRange(doc, from, to);
    const body = doc.slice(r.from, r.to).split('\n')
      .map(l => (isBlank(l) ? '>' : '> ' + l.replace(/^>[ \t]?/, ''))).join('\n');
    const block = head + '\n' + body;
    return placeBlock(doc, r.from, r.to, block, 0, block.length);
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
      case 'h1': case 'h3': case 'h4': case 'h5': case 'h6':
        return headingLevelEdit(doc, from, to, +kind[1], p.heading);
      case 'para': return headingLevelEdit(doc, from, to, 0, p.heading);
      case 'callout': return calloutEdit(doc, from, to, p.callout || 'NOTE', p.text);
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

  /* ================================================================
     Slash-menu inserts: every helper returns ONE change (one undo step)
     that also removes the typed "/query" text in [from, to).
     ================================================================ */

  /* Replace "/query" with a block that sits on its own lines. When the
     slash was typed after text ("intro /table"), the text stays and the
     block goes below it. */
  function blockInsertEdit(doc, from, to, block, selFrom, selTo) {
    doc = String(doc);
    const line = lineAt(doc, from);
    const before = doc.slice(line.from, from);
    const after = doc.slice(to, line.to);
    if (isBlank(before) && isBlank(after)) {
      return placeBlock(doc, line.from, line.to, block, selFrom, selTo == null ? selFrom : selTo);
    }
    const kept = (before.replace(/[ \t]+$/, '') + (isBlank(after) ? '' : (before && !/\s$/.test(before) ? ' ' : '') + after.replace(/^[ \t]+/, '')));
    const trail = line.to < doc.length && !isBlank(lineAt(doc, line.to + 1).text) ? '\n' : '';
    const insert = kept + '\n\n' + block + trail;
    const base = line.from + kept.length + 2;
    return edit(line.from, line.to, insert, base + selFrom, base + (selTo == null ? selFrom : selTo));
  }

  /* Replace "/query" and give the line a block prefix ("## ", "- [ ] ", "> "),
     replacing any heading / list / quote prefix it already had. */
  function linePrefixEdit(doc, from, to, prefix) {
    doc = String(doc);
    const line = lineAt(doc, from);
    const tail = doc.slice(to, line.to);
    let head = doc.slice(line.from, from);
    if (isBlank(tail)) head = head.replace(/[ \t]+$/, '') || head.replace(/[^ \t]/g, '');
    const text = head + tail;
    const ind = /^[ \t]*/.exec(text)[0];
    let body = text.slice(ind.length);
    const cursorInBody = Math.max(0, (from - line.from) - ind.length);
    const stripped = body.replace(HEADING_RE, '').replace(LIST_RE, '').replace(/^>[ \t]?/, '');
    const removed = body.length - stripped.length;
    body = stripped;
    const insert = ind + prefix + body;
    const col = ind.length + prefix.length + Math.max(0, Math.min(body.length, cursorInBody - removed));
    return edit(line.from, line.to, insert, line.from + col);
  }

  /* Replace "/query" with inline text; the selection is relative to `text`. */
  function inlineInsertEdit(doc, from, to, text, selFrom, selTo) {
    const a = from + (selFrom == null ? text.length : selFrom);
    const b = from + (selTo == null ? (selFrom == null ? text.length : selFrom) : selTo);
    return edit(from, to, text, a, b);
  }

  /* Footnote: reference where the slash was, definition at the end of the
     document, cursor on the definition. */
  function footnoteEdit(doc, from, to) {
    doc = String(doc);
    let n = 0;
    for (const m of doc.matchAll(/\[\^(\d+)\]/g)) n = Math.max(n, +m[1]);
    const id = n + 1;
    const ref = '[^' + id + ']';
    const rest = doc.slice(0, from) + ref + doc.slice(to);
    const tail = rest.replace(/\s+$/, '');
    const lastLine = tail.slice(tail.lastIndexOf('\n') + 1);
    const sep = !tail ? '' : /^\[\^[^\]]+\]:/.test(lastLine) ? '\n' : '\n\n';
    const def = '[^' + id + ']: ';
    const next = tail + sep + def;
    // One change covering [from, end): the reference plus the rewritten tail.
    const insert = next.slice(from);
    return { changes: { from, to: doc.length, insert }, selection: { anchor: next.length, head: next.length }, id };
  }

  function headingSlug(text, seen) {
    let slug = text.trim().toLowerCase().replace(/[^\w一-鿿\s-]/g, '').replace(/\s+/g, '-');
    if (!slug) slug = 'section';
    if (seen[slug]) { seen[slug]++; slug = slug + '-' + seen[slug]; } else seen[slug] = 1;
    return slug;
  }

  /* A Markdown list linking every ATX heading (fenced code is skipped). */
  function tocMarkdown(doc) {
    const heads = [];
    let fence = null;
    for (const raw of String(doc).split('\n')) {
      const f = /^\s{0,3}(`{3,}|~{3,})/.exec(raw);
      if (f) {
        if (!fence) fence = f[1][0].repeat(f[1].length);
        else if (f[1][0] === fence[0] && f[1].length >= fence.length) fence = null;
        continue;
      }
      if (fence) continue;
      const m = /^\s{0,3}(#{1,6})[ \t]+(.+?)[ \t#]*$/.exec(raw);
      if (!m) continue;
      const text = m[2]
        .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
        .replace(/[*_`~]/g, '')
        .trim();
      if (text) heads.push({ level: m[1].length, text });
    }
    if (!heads.length) return '';
    const min = Math.min(...heads.map(h => h.level));
    const seen = {};
    return heads.map(h => '  '.repeat(h.level - min) + '- [' + h.text.replace(/([[\]])/g, '\\$1') + '](#' + headingSlug(h.text, seen) + ')').join('\n');
  }

  /* ---------------- 模糊匹配（斜杠菜单） ---------------- */

  /* 0 = no match; higher is better. Prefix and word-start hits win over
     scattered subsequence hits. */
  function fuzzyScore(query, text) {
    const q = String(query || '').toLowerCase().replace(/\s+/g, '');
    const t = String(text || '').toLowerCase();
    if (!q) return 1;
    if (t.startsWith(q)) return 1000 - t.length;
    const at = t.indexOf(q);
    if (at > 0) return (/[\s\-_/(]/.test(t[at - 1]) ? 800 : 600) - at;
    let score = 0, ti = 0, run = 0;
    for (const ch of q) {
      const idx = t.indexOf(ch, ti);
      if (idx < 0) return 0;
      run = idx === ti ? run + 1 : 1;
      score += 10 + run * 5 + (idx === 0 || /[\s\-_/(]/.test(t[idx - 1]) ? 15 : 0) - Math.min(9, idx - ti);
      ti = idx + 1;
    }
    return Math.max(1, Math.min(499, score));
  }

  /* ---------------- 字数统计 ---------------- */

  const CJK_RE = /[぀-ヿ㐀-䶿一-鿿가-힯豈-﫿]/g;
  function textStats(text) {
    const s = String(text || '');
    const cjk = (s.match(CJK_RE) || []).length;
    const words = s.replace(CJK_RE, ' ').split(/\s+/).filter(w => /[\p{L}\p{N}]/u.test(w)).length;
    const chars = s.replace(/\s/g, '').length;
    const total = cjk + words;
    const minutes = total ? Math.max(1, Math.round(cjk / 400 + words / 230)) : 0;
    return { words: total, chars, cjk, minutes };
  }

  /* ================================================================
     Tables: parse / align / navigate. Pure; used by Tab / Enter.
     ================================================================ */

  const DELIM_CELL_RE = /^\s*:?-{1,}:?\s*$/;

  function charWidth(cp) {
    return (cp >= 0x1100 && (cp <= 0x115f || cp === 0x2329 || cp === 0x232a ||
      (cp >= 0x2e80 && cp <= 0xa4cf && cp !== 0x303f) || (cp >= 0xac00 && cp <= 0xd7a3) ||
      (cp >= 0xf900 && cp <= 0xfaff) || (cp >= 0xfe30 && cp <= 0xfe4f) || (cp >= 0xff00 && cp <= 0xff60) ||
      (cp >= 0xffe0 && cp <= 0xffe6) || (cp >= 0x1f300 && cp <= 0x1faff) || (cp >= 0x20000 && cp <= 0x3fffd))) ? 2 : 1;
  }
  function strWidth(s) { let w = 0; for (const c of s) w += charWidth(c.codePointAt(0)); return w; }

  /* Split a row on unescaped pipes (pipes inside `code` also count, as in GFM). */
  function splitRow(line) {
    let s = line.trim();
    if (s.startsWith('|')) s = s.slice(1);
    if (s.endsWith('|') && !s.endsWith('\\|')) s = s.slice(0, -1);
    const cells = [];
    let cur = '';
    for (let i = 0; i < s.length; i++) {
      if (s[i] === '\\' && s[i + 1] === '|') { cur += '\\|'; i++; continue; }
      if (s[i] === '|') { cells.push(cur.trim()); cur = ''; continue; }
      cur += s[i];
    }
    cells.push(cur.trim());
    return cells;
  }

  function isTableLine(text) { return /\|/.test(text) && !isBlank(text); }
  function isDelimRow(text) {
    if (!/-/.test(text) || !isTableLine(text)) return false;
    return splitRow(text).every(c => DELIM_CELL_RE.test(c));
  }

  function formatTable(lines) {
    const indent = /^[ \t]*/.exec(lines[0])[0];
    const rows = lines.map(splitRow);
    const align = rows[1].map(c => {
      const l = c.startsWith(':'), r = c.endsWith(':');
      return l && r ? 'c' : r ? 'r' : l ? 'l' : '';
    });
    const cols = Math.max(...rows.map(r => r.length));
    const widths = Array.from({ length: cols }, (_, i) =>
      Math.max(3, ...rows.map((r, ri) => ri === 1 ? 0 : strWidth(r[i] || ''))));
    const pad = (s, w, a) => {
      const gap = w - strWidth(s);
      if (a === 'r') return ' '.repeat(gap) + s;
      if (a === 'c') { const l = Math.floor(gap / 2); return ' '.repeat(l) + s + ' '.repeat(gap - l); }
      return s + ' '.repeat(gap);
    };
    return rows.map((r, ri) => {
      const cells = widths.map((w, i) => {
        if (ri === 1) {
          const a = align[i] || '';
          const dashes = '-'.repeat(w - (a === 'c' ? 2 : a ? 1 : 0));
          return a === 'c' ? ':' + dashes + ':' : a === 'l' ? ':' + dashes : a === 'r' ? dashes + ':' : dashes;
        }
        return pad(r[i] || '', w, align[i]);
      });
      return indent + '| ' + cells.join(' | ') + ' |';
    });
  }

  /* The table around `pos`: contiguous pipe lines with a delimiter row second. */
  function findTable(doc, pos) {
    doc = String(doc);
    const cur = lineAt(doc, pos);
    if (!isTableLine(cur.text)) return null;
    let from = cur.from, to = cur.to;
    while (from > 0) { const p = lineAt(doc, from - 1); if (!isTableLine(p.text)) break; from = p.from; }
    while (to < doc.length) { const n = lineAt(doc, to + 1); if (!isTableLine(n.text)) break; to = n.to; }
    const lines = doc.slice(from, to).split('\n');
    if (lines.length < 2 || !isDelimRow(lines[1]) || isDelimRow(lines[0])) return null;
    const row = doc.slice(from, cur.from).split('\n').length - 1;
    return { from, to, lines, row, colText: doc.slice(cur.from, pos) };
  }

  /* Content ranges [{from, to}] of each cell in a formatted row (offsets in the line). */
  function cellRanges(line) {
    const out = [];
    let i = line.indexOf('|');
    if (i < 0) return out;
    for (;;) {
      let j = i + 1;
      while (j < line.length && !(line[j] === '|' && line[j - 1] !== '\\')) j++;
      if (j >= line.length) break;
      const seg = line.slice(i + 1, j);
      const lead = seg.length - seg.replace(/^\s+/, '').length;
      const core = seg.trim();
      if (core) {
        const a = i + 1 + lead;
        out.push({ from: a, to: a + core.length });
      } else {
        // Empty cell: select its padding so typing replaces it and the
        // closing pipe stays where it is.
        const a = Math.min(i + 2, j);
        out.push({ from: a, to: Math.max(a, j - 1) });
      }
      i = j;
    }
    return out;
  }

  function cellIndex(text, colText) {
    const t = colText.replace(/^[ \t]*\|?/, '');
    let n = 0;
    for (let i = 0; i < t.length; i++) if (t[i] === '|' && t[i - 1] !== '\\') n++;
    return n;
  }

  function tableResult(t, rows, targetRow, targetCol) {
    const formatted = formatTable(rows);
    const insert = formatted.join('\n');
    let off = 0;
    for (let i = 0; i < targetRow; i++) off += formatted[i].length + 1;
    const cells = cellRanges(formatted[targetRow]);
    const c = cells[Math.max(0, Math.min(cells.length - 1, targetCol))] || { from: 0, to: 0 };
    return edit(t.from, t.to, insert, t.from + off + c.from, t.from + off + c.to);
  }

  /* Tab / Shift+Tab: realign the table and move to the next / previous cell
     (a new row is added after the last cell). */
  function tableNavEdit(doc, pos, dir) {
    const t = findTable(doc, pos);
    if (!t) return null;
    const rows = t.lines.slice();
    const cols = Math.max(...rows.map(r => splitRow(r).length));
    let r = t.row, c = Math.min(cellIndex(rows[r], t.colText), cols - 1);
    if (r === 1) { r = dir > 0 ? 2 : 0; c = dir > 0 ? -1 : cols; }
    c += dir;
    if (c >= cols) { c = 0; r++; if (r === 1) r = 2; }
    if (c < 0) { c = cols - 1; r--; if (r === 1) r = 0; }
    if (r < 0) return tableResult(t, rows, 0, 0);
    if (r >= rows.length) rows.push('|' + ' |'.repeat(cols));
    return tableResult(t, rows, r, c);
  }

  /* Enter inside a table: new row below (below the delimiter from the
     header); Enter on an empty last row leaves the table. */
  function tableEnterEdit(doc, pos) {
    const t = findTable(doc, pos);
    if (!t) return null;
    const rows = t.lines.slice();
    const cols = Math.max(...rows.map(r => splitRow(r).length));
    const empty = splitRow(rows[t.row]).every(c => !c);
    if (t.row >= 2 && empty && t.row === rows.length - 1) {
      rows.pop();
      const insert = formatTable(rows).join('\n') + '\n';
      const at = t.from + insert.length;
      return edit(t.from, t.to, insert, at);
    }
    const at = t.row <= 1 ? 2 : t.row + 1;
    rows.splice(at, 0, '|' + ' |'.repeat(cols));
    return tableResult(t, rows, at, 0);
  }

  /* A fresh table (header + `rows` body rows) and the header-cell selection. */
  function tableBlock(cols, rows, headerWord) {
    const head = Array.from({ length: cols }, (_, i) => (headerWord || 'Column') + ' ' + (i + 1));
    const lines = ['| ' + head.join(' | ') + ' |', '|' + ' --- |'.repeat(cols)];
    for (let r = 0; r < rows; r++) lines.push('|' + ' |'.repeat(cols));
    const formatted = formatTable(lines);
    const first = cellRanges(formatted[0])[0];
    return { text: formatted.join('\n'), selFrom: first.from, selTo: first.to };
  }

  const api = {
    computeSyntaxEdit, applySyntaxEdit,
    headingLevelEdit, calloutEdit, CALLOUT_TYPES,
    blockInsertEdit, linePrefixEdit, inlineInsertEdit, footnoteEdit, tocMarkdown, headingSlug,
    fuzzyScore, textStats,
    strWidth, splitRow, isDelimRow, formatTable, findTable, cellRanges, tableNavEdit, tableEnterEdit, tableBlock,
  };
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  if (root) root.ReadMDTransforms = api;
})(typeof window !== 'undefined' ? window : null);
