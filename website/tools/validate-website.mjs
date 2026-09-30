#!/usr/bin/env node
// Validate the staged ReadMD website (Node port of
// showcase/scripts/validate_website.py; same checks, same error texts).
// Usage: node website/tools/validate-website.mjs [--release] [--root DIR]
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const argv = process.argv.slice(2);
const RELEASE = argv.includes('--release');
const rootArg = argv.indexOf('--root');
const ROOT = rootArg >= 0 ? path.resolve(argv[rootArg + 1]) : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const SITE = path.join(ROOT, 'website');
const PUBLIC = path.join(SITE, 'public');
const P = (...parts) => path.join(PUBLIC, ...parts);
// Python prints PosixPath/WindowsPath via str(): the OS-native absolute path.
const show = p => p;

const LANGUAGES = {
  en: { path: P('index.html'), canonical: 'https://rust.readmd.asia/', full: P('llms-full.txt') },
  'zh-CN': { path: P('zh-cn', 'index.html'), canonical: 'https://rust.readmd.asia/zh-cn/', full: P('zh-cn', 'llms-full.txt') },
  'zh-TW': { path: P('zh-tw', 'index.html'), canonical: 'https://rust.readmd.asia/zh-tw/', full: P('zh-tw', 'llms-full.txt') },
  ja: { path: P('ja', 'index.html'), canonical: 'https://rust.readmd.asia/ja/', full: P('ja', 'llms-full.txt') },
};
const LANG_PREFIX = { en: '', 'zh-CN': 'zh-cn', 'zh-TW': 'zh-tw', ja: 'ja' };
const pageSet = slug => Object.fromEntries(Object.entries(LANG_PREFIX).map(([lang, pre]) => [lang, {
  path: P(...(pre ? [pre] : []), slug, 'index.html'),
  canonical: `https://rust.readmd.asia/${pre ? pre + '/' : ''}${slug}/`,
}]));
const INTENT_PAGES = pageSet('workflows');
const DOWNLOAD_PAGES = pageSet('download');
const ANSWER_TOPICS = [
  ['large-files', 'large-markdown-files'], ['slides', 'markdown-to-slides'], ['conversion', 'convert-to-markdown'],
  ['pdf', 'pdf-to-markdown'], ['tables', 'markdown-tables'], ['release-notes', 'release-notes'],
  ['ocr', 'scan-to-markdown'], ['bibtex', 'bibtex-citations'],
];
const ANSWER_PAGES = {};
for (const [key, slug] of ANSWER_TOPICS) {
  for (const [lang, c] of Object.entries(pageSet(slug))) ANSWER_PAGES[`${lang}-${key}`] = c;
}

const VERSION = fs.readFileSync(path.join(ROOT, 'VERSION'), 'utf8').trim();
const RELEASE_ASSETS = new Set([
  `ReadMDSetup-v${VERSION}.exe`, `ReadMD-portable-v${VERSION}.exe`, `ReadMD-macos-arm64-v${VERSION}.zip`,
  `ReadMD-macos-x64-v${VERSION}.zip`, `ReadMD-linux-x86_64-v${VERSION}.AppImage`, `ReadMD-linux-aarch64-v${VERSION}.AppImage`,
  `readmd_${VERSION}_amd64.deb`, `readmd_${VERSION}_arm64.deb`, `readmd-vscode-${VERSION}.vsix`,
  `readmd-mcp-server-${VERSION}.zip`, 'SHA256SUMS.txt',
]);
const AI_CRAWLERS = ['GPTBot', 'OAI-SearchBot', 'ClaudeBot', 'PerplexityBot'];
const FAQ_QUESTION_COUNTS = {};
for (const c of Object.values(LANGUAGES)) FAQ_QUESTION_COUNTS[c.canonical] = 6;
for (const c of Object.values(INTENT_PAGES)) FAQ_QUESTION_COUNTS[c.canonical] = 5;
for (const c of Object.values(ANSWER_PAGES)) FAQ_QUESTION_COUNTS[c.canonical] = 4;

// Python's read_text() uses universal newlines: \r\n and \r become \n.
const read = p => fs.readFileSync(p, 'utf8').replace(/\r\n?/g, '\n');
const isFile = p => { try { return fs.statSync(p).isFile(); } catch { return false; } };
const isDir = p => { try { return fs.statSync(p).isDirectory(); } catch { return false; } };
const urlPath = u => { try { return new URL(u).pathname; } catch { return u.split(/[?#]/)[0]; } };
const sorted = xs => [...xs].sort();
const pyList = xs => '[' + xs.map(x => `'${x}'`).join(', ') + ']';
const pySet = xs => (xs.length ? '{' + xs.map(x => `'${x}'`).join(', ') + '}' : 'set()');
// json.dumps(..., ensure_ascii=False) separators: ', ' and ': '.
const pyJson = v => {
  if (Array.isArray(v)) return '[' + v.map(pyJson).join(', ') + ']';
  if (v && typeof v === 'object') return '{' + Object.entries(v).map(([k, x]) => JSON.stringify(k) + ': ' + pyJson(x)).join(', ') + '}';
  return JSON.stringify(v);
};
const setEq = (a, b) => a.size === b.size && [...a].every(x => b.has(x));
const minus = (a, b) => [...a].filter(x => !b.has(x));
const JSONLD_RE = /<script type="application\/ld\+json">([\s\S]*?)<\/script>/g;
const jsonBlocks = s => [...s.matchAll(JSONLD_RE)].map(m => m[1]);

// ---- minimal HTMLParser equivalent -----------------------------------------
const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' };
function unescape(s) {
  return s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);?/gi, (m, e) => {
    if (e[0] === '#') {
      const n = e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
      return Number.isFinite(n) ? String.fromCodePoint(n) : m;
    }
    return ENTITIES[e.toLowerCase()] ?? m;
  });
}
function parseAttrs(src) {
  const out = {};
  const re = /([^\s=\/>"']+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
  let m;
  while ((m = re.exec(src))) {
    const key = m[1].toLowerCase();
    const val = m[2] ?? m[3] ?? m[4] ?? '';
    out[key] = unescape(val);
  }
  return out;
}
class PageAudit {
  constructor(content) {
    this.title = ''; this.metas = []; this.links = []; this.headings = []; this.images = []; this.stylesheets = [];
    let inTitle = false, heading = '';
    const re = /<!--[\s\S]*?-->|<(script|style)\b[^>]*>([\s\S]*?)<\/\1\s*>|<\/([a-zA-Z][\w-]*)\s*>|<([a-zA-Z][\w-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>|([^<]+)/g;
    let m;
    const data = text => {
      if (inTitle) this.title += text.trim();
      if (heading) this.headings.push([heading, text.split(/\s+/).filter(Boolean).join(' ')]);
    };
    while ((m = re.exec(content))) {
      if (m[1]) { continue; }             // script / style bodies are CDATA; not titles or headings here
      if (m[3]) {
        const tag = m[3].toLowerCase();
        if (tag === 'title') inTitle = false;
        else if (tag === 'h1' || tag === 'h2' || tag === 'h3') heading = '';
        continue;
      }
      if (m[4]) {
        const tag = m[4].toLowerCase();
        const attrs = parseAttrs(m[5] || '');
        if (tag === 'title') inTitle = true;
        else if (tag === 'meta') this.metas.push(attrs);
        else if (tag === 'link') { this.links.push(attrs); if (attrs.rel === 'stylesheet') this.stylesheets.push(attrs.href || ''); }
        else if (tag === 'h1' || tag === 'h2' || tag === 'h3') heading = tag;
        else if (tag === 'img') this.images.push(attrs);
        continue;
      }
      if (m[6] !== undefined) data(unescape(m[6]));
    }
  }
}

function auditPage(p, canonical) {
  const errors = [];
  const content = read(p);
  const audit = new PageAudit(content);
  const s = show(p);
  if (audit.title.length < 15 || !audit.title.includes('ReadMD')) errors.push(`${s}: missing descriptive ReadMD title`);
  const descriptions = audit.metas.filter(i => i.name === 'description');
  if (descriptions.length !== 1 || (descriptions[0].content || '').length < 80) errors.push(`${s}: missing unique meta description of at least 80 characters`);
  const robots = audit.metas.filter(i => i.name === 'robots');
  const expectedRobots = 'index,follow,max-image-preview:large,max-snippet:-1,max-video-preview:-1';
  if (robots.length !== 1 || robots[0].content !== expectedRobots) errors.push(`${s}: missing canonical ${expectedRobots} directive`);
  const authors = audit.metas.filter(i => i.name === 'author');
  if (authors.length !== 1 || authors[0].content !== 'ReadMD') errors.push(`${s}: missing unique ReadMD author metadata`);
  const og = {};
  for (const i of audit.metas) if (String(i.property || '').startsWith('og:')) og[i.property] = i.content;
  for (const r of ['og:title', 'og:description', 'og:url', 'og:image', 'og:image:type', 'og:image:alt']) {
    if ((og[r] || '').length < 8) errors.push(`${s}: missing complete ${r}`);
  }
  if (og['og:image:width'] !== '1440' || og['og:image:height'] !== '900') errors.push(`${s}: Open Graph image must declare 1440x900`);
  const ogImage = og['og:image'] || '';
  if (!ogImage.endsWith('.png')) errors.push(`${s}: Open Graph image must use the compatible PNG fallback`);
  if (og['og:image:type'] !== 'image/png') errors.push(`${s}: Open Graph image type must be image/png`);
  else if (!isFile(path.join(PUBLIC, urlPath(ogImage).replace(/^\/+/, '')))) errors.push(`${s}: Open Graph image is missing from media assets`);
  if (og['og:url'] !== canonical) errors.push(`${s}: og:url differs from canonical`);
  const tw = {};
  for (const i of audit.metas) if (String(i.name || '').startsWith('twitter:')) tw[i.name] = i.content;
  for (const r of ['twitter:card', 'twitter:title', 'twitter:description', 'twitter:image', 'twitter:image:alt']) {
    if ((tw[r] || '').length < 8) errors.push(`${s}: missing complete ${r}`);
  }
  const ogLocales = new Set(audit.metas.filter(i => String(i.property || '').startsWith('og:locale')).map(i => i.content));
  if (!setEq(ogLocales, new Set(['en_US', 'zh_CN', 'zh_TW', 'ja_JP']))) errors.push(`${s}: incomplete Open Graph locale signal: ${pyList(sorted([...ogLocales].filter(Boolean)))}`);
  if ((tw['twitter:image'] || '') !== ogImage) errors.push(`${s}: Twitter image must match Open Graph image`);
  const canonicals = audit.links.filter(i => i.rel === 'canonical');
  if (canonicals.length !== 1 || canonicals[0].href !== canonical) errors.push(`${s}: canonical must be exactly ${canonical}`);
  const hreflang = new Set(audit.links.filter(i => i.rel === 'alternate' && i.hreflang).map(i => i.hreflang));
  if (!setEq(hreflang, new Set(['en', 'zh-CN', 'zh-TW', 'ja', 'x-default']))) errors.push(`${s}: incomplete hreflang set: ${pyList(sorted([...hreflang].filter(Boolean)))}`);
  if (audit.headings.filter(([t, x]) => t === 'h1' && x).length !== 1) errors.push(`${s}: page must contain exactly one non-empty h1`);
  for (const img of audit.images) {
    if ((img.alt || '').trim().length < 10) errors.push(`${s}: image lacks meaningful alt text: ${img.src || ''}`);
    if (img.loading === 'eager' && img.fetchpriority !== 'high') errors.push(`${s}: eager hero image must declare fetchpriority=high`);
  }
  if (!audit.stylesheets.includes('/assets/site.css')) errors.push(`${s}: production stylesheet link is missing`);
  const rels = new Set(audit.links.map(i => i.rel));
  for (const r of ['icon', 'apple-touch-icon', 'manifest', 'license']) if (!rels.has(r)) errors.push(`${s}: missing ${r} link`);
  if (!audit.links.some(i => i.type === 'application/atom+xml' && (i.href || '').endsWith('releases.atom'))) errors.push(`${s}: release Atom feed link is missing`);
  if (!audit.links.some(i => i.type === 'application/atom+xml' && i.href === '/feed.xml')) errors.push(`${s}: full-site Atom feed link is missing`);
  if (content.split('<picture>').length - 1 !== audit.images.length) errors.push(`${s}: every product image must have a WebP picture fallback`);
  if (audit.images.length && !content.includes('.webp')) errors.push(`${s}: optimized WebP source is missing`);
  if (!/https:\/\/github\.com\/Natsummerance\/(?:rust-)?readMD\/stargazers/i.test(content)) errors.push(`${s}: star call to action is missing`);
  const blocks = jsonBlocks(content);
  if (!blocks.length) {
    errors.push(`${s}: server-rendered JSON-LD is missing`);
  } else {
    try {
      const first = JSON.parse(blocks[0]);
      const graph = (first && first['@graph']) || [];
      const objs = graph.filter(i => i && typeof i === 'object' && !Array.isArray(i));
      const types = new Set(objs.map(i => i['@type']));
      if (!(types.has('WebPage') && types.has('SoftwareApplication'))) errors.push(`${s}: JSON-LD lacks WebPage and SoftwareApplication`);
      const webpage = objs.find(i => i['@type'] === 'WebPage') || {};
      const app = objs.find(i => i['@type'] === 'SoftwareApplication') || {};
      const language = canonical.includes('/zh-cn/') ? 'zh-CN' : canonical.includes('/zh-tw/') ? 'zh-TW' : canonical.includes('/ja/') ? 'ja' : 'en';
      if (webpage.inLanguage !== language) errors.push(`${s}: JSON-LD WebPage lacks correct inLanguage`);
      if ((webpage.mainEntityOfPage || {})['@id'] !== canonical) errors.push(`${s}: JSON-LD WebPage lacks mainEntityOfPage identity`);
      const offers = app.offers === undefined ? {} : app.offers;
      if (!offers || typeof offers !== 'object' || Array.isArray(offers) || offers.price !== '0' || offers.priceCurrency !== 'USD') errors.push(`${s}: SoftwareApplication lacks free-offer structured data`);
      if (app.releaseNotes !== `https://rust.readmd.asia${language === 'en' ? '' : '/' + language.toLowerCase()}/release-notes/`) errors.push(`${s}: SoftwareApplication lacks localized releaseNotes`);
      if (!app.screenshot || !app.featureList || (Array.isArray(app.featureList) && !app.featureList.length) || (Array.isArray(app.screenshot) && !app.screenshot.length)) errors.push(`${s}: SoftwareApplication lacks screenshot/features`);
      const homes = new Set(['https://rust.readmd.asia/', 'https://rust.readmd.asia/zh-cn/', 'https://rust.readmd.asia/zh-tw/', 'https://rust.readmd.asia/ja/']);
      if (!homes.has(canonical)) {
        const all = blocks.map(b => JSON.parse(b));
        if (!all.some(x => x && typeof x === 'object' && !Array.isArray(x) && x['@type'] === 'BreadcrumbList')) errors.push(`${s}: non-home page lacks BreadcrumbList`);
      }
      if (canonical.includes('/release-notes/') && !objs.some(i => i['@type'] === 'TechArticle')) errors.push(`${s}: release-notes page lacks TechArticle entity`);
      if (!objs.some(i => 'speakable' in i)) errors.push(`${s}: JSON-LD lacks speakable definition`);
      for (const u of objs.map(i => i.primaryImageOfPage).filter(Boolean)) {
        if (!String(u).endsWith('.png')) errors.push(`${s}: structured primary image must use PNG fallback`);
        else if (!isFile(path.join(PUBLIC, urlPath(u).replace(/^\/+/, '')))) errors.push(`${s}: structured primary image is missing from media assets`);
      }
    } catch (exc) {
      errors.push(`${s}: invalid JSON-LD: ${exc.message}`);
    }
  }
  const expectedQ = FAQ_QUESTION_COUNTS[canonical];
  if (expectedQ) {
    const faq = [];
    for (const b of blocks) {
      let payload;
      try { payload = JSON.parse(b); } catch (exc) { errors.push(`${s}: invalid JSON-LD: ${exc.message}`); continue; }
      if (payload && typeof payload === 'object' && !Array.isArray(payload) && payload['@type'] === 'FAQPage') faq.push(payload);
    }
    if (faq.length !== 1) errors.push(`${s}: expected exactly one visible FAQPage schema`);
    else if ((faq[0].mainEntity || []).length !== expectedQ) errors.push(`${s}: FAQPage must expose ${expectedQ} visible questions`);
    else if (faq[0]['@id'] !== `${canonical}#faq`) errors.push(`${s}: FAQPage identifier differs from canonical URL`);
  }
  return errors;
}

function validateLlms(p, minimum = 5) {
  const errors = [];
  const text = read(p);
  const lines = text.split(/\r\n|\n|\r/);
  if (lines.length && lines[lines.length - 1] === '' && /(\r\n|\n|\r)$/.test(text)) lines.pop();
  const s = show(p);
  if (!lines.length || !lines[0].startsWith('# ReadMD')) errors.push(`${s}: first line must be an H1 starting with ReadMD`);
  if (lines.length < 2 || !lines[1].startsWith('>')) errors.push(`${s}: second line must be a blockquote description`);
  if ((lines[1] || '').length > 220) errors.push(`${s}: description exceeds the compact llms.txt contract`);
  const links = text.match(/https:\/\/(?:rust\.)?readmd\.asia(?:\/[\w.-]+)*/g) || [];
  if (links.length < minimum) errors.push(`${s}: fewer than ${minimum} absolute canonical entries`);
  return errors;
}

function validateRobotsAndSitemap() {
  const errors = [];
  const robots = read(P('robots.txt'));
  for (const c of AI_CRAWLERS) if (!robots.includes(`User-agent: ${c}\nAllow: /`)) errors.push(`robots.txt does not explicitly allow ${c}`);
  if (!robots.includes('Disallow: /*?')) errors.push('robots.txt does not exclude parameter URLs from crawl budget');
  if (!robots.includes('Disallow: /*.json$')) errors.push('robots.txt does not exclude raw JSON endpoints from crawl budget');
  if (!robots.includes('Sitemap: https://rust.readmd.asia/sitemap.xml')) errors.push('robots.txt omits canonical sitemap');
  const sitemap = read(P('sitemap.xml'));
  if (!sitemap.includes('xmlns:xhtml="http://www.w3.org/1999/xhtml"')) errors.push('sitemap omits XHTML hreflang namespace');
  const expected = new Set([LANGUAGES, INTENT_PAGES, DOWNLOAD_PAGES, ANSWER_PAGES].flatMap(g => Object.values(g).map(c => c.canonical)));
  const actual = new Set([...sitemap.matchAll(/<loc>(.*?)<\/loc>/g)].map(m => m[1]));
  if (!setEq(actual, expected)) errors.push(`sitemap mismatch: missing=${pySet(minus(expected, actual))}, extra=${pySet(minus(actual, expected))}`);
  const entries = [...sitemap.matchAll(/<url>([\s\S]*?)<\/url>/g)].map(m => m[1]);
  if (entries.length !== expected.size) errors.push(`sitemap must contain ${expected.size} URLs`);
  const bases = { en: 'https://rust.readmd.asia', 'zh-CN': 'https://rust.readmd.asia/zh-cn', 'zh-TW': 'https://rust.readmd.asia/zh-tw', ja: 'https://rust.readmd.asia/ja' };
  bases['x-default'] = bases.en;
  for (const entry of entries) {
    const um = /<loc>(.*?)<\/loc>/.exec(entry);
    if (!um) { errors.push('sitemap entry lacks loc'); continue; }
    const url = um[1];
    const alternates = {};
    for (const m of entry.matchAll(/<xhtml:link[^>]+hreflang="([^"]+)"[^>]+href="([^"]+)"/g)) alternates[m[1]] = m[2];
    let section = urlPath(url);
    for (const pre of ['/zh-cn', '/zh-tw', '/ja']) if (section.startsWith(pre + '/')) { section = section.slice(pre.length); break; }
    for (const [lang, base] of Object.entries(bases)) {
      if (alternates[lang] !== base + section) errors.push(`sitemap ${url} has bad hreflang ${lang}: ${alternates[lang] ?? 'None'}`);
    }
    if (!/<lastmod>\d{4}-\d{2}-\d{2}<\/lastmod>/.test(entry)) errors.push(`sitemap ${url} lacks valid ISO lastmod`);
  }
  return errors;
}

function validateLanguageCrosslinks() {
  const errors = [];
  for (const [lang, c] of Object.entries(LANGUAGES)) {
    const html = read(c.path);
    const rel = path.relative(PUBLIC, c.full).split(path.sep).join('/');
    if (!html.includes(`href="/${rel}"`)) errors.push(`${lang} index does not link its own llms-full corpus`);
    const idx = path.join(path.dirname(c.path), 'llms.txt');
    if (!isFile(idx)) errors.push(`${lang} is missing llms.txt`);
    else {
      errors.push(...validateLlms(idx, 3));
      if (!read(idx).includes(c.canonical)) errors.push(`${lang} llms.txt omits its localized homepage`);
    }
    errors.push(...validateLlms(c.full));
  }
  return errors;
}

function validateApproval() {
  const errors = [];
  const a = JSON.parse(read(path.join(ROOT, 'showcase', 'reports', 'website_approval.json')));
  const minimum = parseFloat(a.minimum_score);
  const rounds = a.rounds || [];
  const pyFloat = x => (Number.isInteger(x) ? x.toFixed(1) : String(x));
  if (rounds.length < parseInt(a.required_rounds ?? 3, 10)) errors.push('approval has fewer than three review rounds');
  for (const r of rounds) {
    if (r.status !== 'approved' || parseFloat(r.score ?? 0) < minimum) errors.push(`approval round ${r.round ?? 'None'} is not approved at or above ${pyFloat(minimum)}`);
  }
  const m = a.final_decision_meeting || {};
  if (m.status !== 'approved_for_staged_publication' || parseFloat(m.score ?? 0) < minimum) errors.push('final decision meeting is not approved at or above threshold');
  const conds = m.conditions;
  const all = conds == null ? true : (Array.isArray(conds) ? conds.every(Boolean) : Object.keys(conds).every(Boolean));
  if (!all) errors.push('final decision conditions are incomplete');
  return errors;
}

function* walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) yield* walk(p); else yield p;
  }
}

function validateRights() {
  const errors = [];
  for (const p of walk(PUBLIC)) {
    if (!p.endsWith('.html')) continue;
    const t = read(p);
    for (const marker of ['apple.com', '1比1', '1:1 copy', '完全一致']) if (t.includes(marker)) errors.push(`${show(p)}: forbidden clone/reference marker found: ${marker}`);
  }
  return errors;
}

function validateMotionExperience() {
  const errors = [];
  const content = read(P('index.html'));
  for (const m of ['particle-field', 'data-journey', '/assets/site.js', 'journey-film', 'data-frame-count="57"']) if (!content.includes(m)) errors.push(`home page omits motion experience marker: ${m}`);
  const scriptPath = P('assets', 'site.js');
  if (!isFile(scriptPath)) errors.push('assets/site.js is missing');
  else {
    const sc = read(scriptPath);
    if (sc.includes('addEventListener("scroll"') || sc.includes("addEventListener('scroll'")) errors.push('site.js uses a main-thread scroll event listener');
  }
  const frameDir = P('media', 'journey-frames');
  const frames = isDir(frameDir) ? fs.readdirSync(frameDir).filter(f => /^frame-.*\.webp$/.test(f)).sort().map(f => path.join(frameDir, f)) : [];
  if (frames.length !== 57) errors.push(`scroll-driven film expects 57 frames, found ${frames.length}`);
  if (frames.length) {
    const hashes = frames.map(f => crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex'));
    const ratio = new Set(hashes).size / hashes.length;
    if (ratio < 0.75) errors.push(`scroll-driven film has too many duplicate frames (unique ratio ${(ratio * 100).toFixed(2)}%)`);
    let run = 1;
    for (let i = 1; i < hashes.length; i++) {
      if (hashes[i] === hashes[i - 1]) {
        if (++run > 8) { errors.push('scroll-driven film contains a repeated frame run longer than 8 frames'); break; }
      } else run = 1;
    }
  }
  return errors;
}

function validateCapabilityCinema() {
  const errors = [];
  const content = read(P('index.html'));
  for (const m of ['id="capability-cinema"', 'data-capability-cinema', 'capability-canvas', 'data-capability-track', '/large-markdown-files/', '/markdown-to-slides/', '/release-notes/']) {
    if (!content.includes(m)) errors.push(`capability cinema omits ${m}`);
  }
  const panels = content.split('class="capability-panel"').length - 1;
  if (panels !== 9) errors.push(`capability cinema expects 9 feature panels, found ${panels}`);
  const sp = P('assets', 'site.js');
  const script = isFile(sp) ? read(sp) : '';
  for (const m of ['startCapabilityCinema', 'drawBlendedFrame', 'drawSlices', 'drawParticles']) if (!script.includes(m)) errors.push(`capability cinema script omits ${m}`);
  return errors;
}

function validateSecurityHeaders() {
  const errors = [];
  const headers = read(P('_headers'));
  for (const h of ['Content-Security-Policy:', 'Strict-Transport-Security:', 'X-Content-Type-Options: nosniff', 'X-Frame-Options: DENY', 'Referrer-Policy:', 'Permissions-Policy:']) {
    if (!headers.includes(h)) errors.push(`_headers omits security header: ${h}`);
  }
  if (!headers.includes("script-src 'self'")) errors.push("_headers CSP does not constrain scripts to self");
  const blocks = jsonBlocks(read(P('index.html')));
  if (blocks.length) {
    const hash = 'sha256-' + crypto.createHash('sha256').update(blocks[0], 'utf8').digest('base64');
    if (!headers.includes(hash)) errors.push(`_headers CSP omits current JSON-LD hash: ${hash}`);
  }
  return errors;
}

function validateGrowthHomepages() {
  const errors = [];
  for (const [lang, c] of Object.entries(LANGUAGES)) {
    const content = read(c.path);
    if (!content.includes('rel="preload" as="image" href="/media/overview-reader.webp"')) errors.push(`${lang}: hero WebP preload is missing`);
    if (!content.includes('id="share"')) errors.push(`${lang}: share section is missing`);
    for (const sig of ['twitter.com/intent/tweet', 't.me/share/url', 'linkedin.com/sharing/share-offsite']) if (!content.includes(sig)) errors.push(`${lang}: share network missing: ${sig}`);
  }
  return errors;
}

function validateSpecialPageInternalLinks() {
  const errors = [];
  for (const lang of Object.keys(LANGUAGES)) {
    const workflow = INTENT_PAGES[lang], download = DOWNLOAD_PAGES[lang];
    for (const [source, contract] of [['workflow', workflow], ['download', download]]) {
      const content = read(contract.path);
      const target = urlPath(source === 'workflow' ? download.canonical : workflow.canonical);
      if (!content.includes(`href="${target}"`)) errors.push(`${lang} ${source} page omits its sibling internal link`);
      if (!/"@type"\s*:\s*"BreadcrumbList"/.test(content)) errors.push(`${lang} ${source} page omits BreadcrumbList structured data`);
    }
  }
  return errors;
}

function validateAnswerInternalLinks() {
  const errors = [];
  const slugs = ['large-markdown-files', 'markdown-to-slides', 'convert-to-markdown', 'scan-to-markdown', 'bibtex-citations', 'release-notes'];
  for (const lang of Object.keys(LANGUAGES)) {
    for (const slug of slugs) {
      const target = urlPath(`https://rust.readmd.asia${lang === 'en' ? '' : '/' + lang.toLowerCase()}/${slug}/`);
      for (const [surface, src] of [['home', LANGUAGES[lang].path], ['workflow', INTENT_PAGES[lang].path], ['download', DOWNLOAD_PAGES[lang].path]]) {
        if (!read(src).includes(`href="${target}"`)) errors.push(`${lang} ${surface} omits internal link to ${target}`);
      }
    }
  }
  return errors;
}

function validateReleaseAssetLinks() {
  const errors = [];
  const esc = VERSION.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const re = new RegExp(`href="https://github\\.com/Natsummerance/(?:rust-)?ReadMD/releases/(?:latest/download|download/v${esc})/([^"]+)"`, 'gi');
  for (const [lang, c] of Object.entries(DOWNLOAD_PAGES)) {
    const linked = new Set([...read(c.path).matchAll(re)].map(m => m[1]));
    if (!setEq(linked, RELEASE_ASSETS)) errors.push(`${lang} download assets mismatch: missing=${pyList(sorted(minus(RELEASE_ASSETS, linked)))}, extra=${pyList(sorted(minus(linked, RELEASE_ASSETS)))}`);
  }
  return errors;
}

function validateReleaseBuild() {
  const errors = [];
  const dist = path.join(SITE, 'dist');
  if (!isFile(path.join(dist, 'assets', 'site.css'))) errors.push('dist/assets/site.css is missing');
  if (!isFile(path.join(dist, 'index.html'))) errors.push('dist/index.html is missing');
  return errors;
}

function validateIndexnow() {
  const keys = fs.readdirSync(PUBLIC).filter(f => /^[0-9a-f]{32}\.txt$/.test(f) && isFile(P(f)));
  if (keys.length !== 1) return ['IndexNow requires exactly one 32-character hexadecimal key file'];
  const errors = [];
  const key = keys[0].slice(0, -4);
  if (read(P(keys[0])).trim() !== key) errors.push('IndexNow key filename and contents do not match');
  if (!isFile(path.join(SITE, 'tools', 'indexnow-submit.mjs'))) errors.push('IndexNow submission script is missing');
  if (!read(path.join(SITE, 'package.json')).includes('"indexnow"')) errors.push('package.json omits the IndexNow command');
  return errors;
}

/* Well-formedness in the way expat reports it: `mismatched tag: line L, column C`
   (column = 0-based offset just past the `</` of the offending end tag). */
function xmlProblem(xml) {
  const stack = [];
  const re = /<!--[\s\S]*?-->|<!\[CDATA\[[\s\S]*?\]\]>|<\?[\s\S]*?\?>|<!DOCTYPE[^>]*>|<\/([^\s>]+)\s*>|<([^\s/>!?]+)(?:[^>"']|"[^"]*"|'[^']*')*?(\/?)>/g;
  const where = i => { const before = xml.slice(0, i); const line = before.split('\n').length; return `line ${line}, column ${i - before.lastIndexOf('\n') - 1}`; };
  let m;
  while ((m = re.exec(xml))) {
    if (m[1]) {
      if (stack.pop() !== m[1]) return `mismatched tag: ${where(m.index + 2)}`;
    } else if (m[2] && !m[3]) stack.push(m[2]);
  }
  return stack.length ? `no element found: ${where(xml.length)}` : '';
}

function validateFeed() {
  const p = P('feed.xml');
  if (!isFile(p)) return ['full-site Atom feed is missing'];
  const xml = read(p);
  const bad = xmlProblem(xml);
  if (bad) return [`full-site Atom feed is invalid: ${bad}`];
  if (!/<feed\b[^>]*xmlns="http:\/\/www\.w3\.org\/2005\/Atom"/.test(xml)) return ['full-site Atom feed is invalid: not an Atom feed'];
  const errors = [];
  const text = (src, tag) => { const m = new RegExp(`<${tag}\\b[^>]*>([\\s\\S]*?)</${tag}>`).exec(src); return m ? unescape(m[1].replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, '$1')) : ''; };
  const entries = [...xml.matchAll(/<entry\b[^>]*>([\s\S]*?)<\/entry>/g)].map(m => m[1]);
  const head = xml.replace(/<entry\b[^>]*>[\s\S]*?<\/entry>/g, '');
  const feedLinks = [...head.matchAll(/<link\b([^>]*)\/?>/g)].map(m => parseAttrs(m[1]));
  const selfLinks = feedLinks.filter(l => l.rel === 'self').map(l => l.href);
  const alternates = feedLinks.filter(l => l.rel === 'alternate' && l.href !== 'https://rust.readmd.asia/feed.xml').map(l => l.href);
  const ids = new Set(entries.map(e => text(e, 'id')));
  const canonicalIds = new Set([...read(P('sitemap.xml')).matchAll(/<loc>(.*?)<\/loc>/g)].map(m => m[1]));
  if (entries.length !== 44 || !setEq(ids, canonicalIds)) errors.push(`Atom feed must contain exactly 44 canonical entries, found ${entries.length}`);
  if (JSON.stringify(selfLinks) !== JSON.stringify(['https://rust.readmd.asia/feed.xml'])) errors.push('Atom feed lacks its canonical self link');
  if (JSON.stringify(alternates) !== JSON.stringify(['https://rust.readmd.asia/'])) errors.push('Atom feed lacks the homepage alternate link');
  if (!text(head, 'updated')) errors.push('Atom feed lacks an updated timestamp');
  for (const e of entries) {
    const id = text(e, 'id');
    if (!text(e, 'title')) errors.push(`Atom entry lacks title: ${id}`);
    if (!text(e, 'summary')) errors.push(`Atom entry lacks summary: ${id}`);
  }
  return errors;
}

function validateSecurityTxt() {
  const p = P('.well-known', 'security.txt');
  if (!isFile(p)) return ['security.txt is missing'];
  const t = read(p);
  const req = ['Contact: https://github.com/Natsummerance/readMD/security/advisories/new', 'Expires: 2027-08-26T00:00:00Z', 'Preferred-Languages: en, zh-CN, zh-TW, ja', 'Canonical: https://rust.readmd.asia/.well-known/security.txt'];
  return req.every(r => t.includes(r)) ? [] : ['security.txt omits required trust fields'];
}

function validate404() {
  const p = P('404.html');
  if (!isFile(p)) return ['localized-entry 404 page is missing'];
  const c = read(p);
  const req = ['<meta name="robots" content="noindex,nofollow">', 'href="/download/"', 'href="/workflows/"', 'href="/release-notes/"', 'href="/zh-cn/"', 'href="/zh-tw/"', 'href="/ja/"'];
  return req.every(r => c.includes(r)) ? [] : ['404 page omits recovery entry points'];
}

function main() {
  const errors = [];
  const groups = [[LANGUAGES, 'index'], [INTENT_PAGES, 'workflow page'], [DOWNLOAD_PAGES, 'download page'], [ANSWER_PAGES, 'answer page']];
  for (const [group, label] of groups) {
    for (const [name, c] of Object.entries(group)) {
      if (!isFile(c.path)) { errors.push(`missing ${name} ${label}`); continue; }
      errors.push(...auditPage(c.path, c.canonical));
    }
  }
  if (!isFile(P('llms.txt'))) errors.push('missing public/llms.txt'); else errors.push(...validateLlms(P('llms.txt')));
  for (const f of [validateLanguageCrosslinks, validateRobotsAndSitemap, validateApproval, validateRights, validateMotionExperience,
    validateCapabilityCinema, validateSecurityHeaders, validateGrowthHomepages, validateSpecialPageInternalLinks,
    validateAnswerInternalLinks, validateIndexnow, validateReleaseAssetLinks, validateFeed, validateSecurityTxt, validate404]) {
    errors.push(...f());
  }
  if (RELEASE) errors.push(...validateReleaseBuild());
  if (errors.length) {
    console.log(pyJson({ ok: false, errors }));
    return 1;
  }
  console.log(pyJson({
    ok: true,
    languages: Object.keys(LANGUAGES),
    intent_pages: Object.keys(INTENT_PAGES),
    download_pages: Object.keys(DOWNLOAD_PAGES),
    answer_pages: Object.keys(ANSWER_PAGES),
    review_rounds: 3,
    broad_seo: {
      canonical_pages: [LANGUAGES, INTENT_PAGES, DOWNLOAD_PAGES, ANSWER_PAGES].reduce((n, g) => n + Object.keys(g).length, 0),
      atom_feed: true, entity_graph: true, security_txt: true, quality_404: true, capability_cinema: true,
    },
  }));
  return 0;
}

process.exitCode = main();
