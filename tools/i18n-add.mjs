// Insert/update keys in every locale file without reordering existing keys:
// a new key goes after the last existing key of the same namespace. Locales
// without their own text fall back to en (zh-TW / zh-HK to zh-TW, then zh-CN).
// Usage: node tools/i18n-add.mjs keys.json   (keys.json: {key: {en, "zh-CN", "zh-TW"?, ...}})
import fs from 'node:fs';
import path from 'node:path';

const dir = path.resolve('assets/i18n');
const spec = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
for (const f of fs.readdirSync(dir).filter(n => n.endsWith('.json'))) {
  const loc = f.replace(/\.json$/, '');
  const p = path.join(dir, f);
  const raw = fs.readFileSync(p, 'utf8');
  const entries = Object.entries(JSON.parse(raw));
  let changed = false;
  for (const [k, texts] of Object.entries(spec)) {
    const zhTrad = loc === 'zh-TW' || loc === 'zh-HK' ? (texts['zh-TW'] || texts['zh-CN']) : undefined;
    const v = texts[loc] ?? zhTrad ?? texts.en;
    const at = entries.findIndex(([ek]) => ek === k);
    if (at >= 0) {
      if (entries[at][1] !== v) { entries[at][1] = v; changed = true; }
      continue;
    }
    const ns = k.split('.')[0] + '.';
    let pos = -1;
    entries.forEach(([ek], i) => { if (ek.startsWith(ns) && ek < k) pos = i; });
    if (pos < 0) entries.forEach(([ek], i) => { if (ek.startsWith(ns)) pos = pos < 0 ? i - 1 : pos; });
    entries.splice(pos + 1, 0, [k, v]);
    changed = true;
  }
  if (!changed) continue;
  const eol = raw.includes('\r\n') ? '\r\n' : '\n';
  const out = JSON.stringify(Object.fromEntries(entries), null, 2).replace(/\n/g, eol) + (/\r?\n$/.test(raw) ? eol : '');
  fs.writeFileSync(p, out);
}
