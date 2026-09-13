#!/usr/bin/env node

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { requireInside, sha256, updateListMarkdown, validateDeck } = require('./deck-lib.cjs');
const { chromium } = require('../../ui-tests/node_modules/@playwright/test');

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const showcaseRoot = path.resolve(__dirname, '..');
const manifestPath = path.join(__dirname, 'release-delta.json');

function parseArgs(argv) {
  const result = {
    locale: 'zh-CN',
    input: path.join(showcaseRoot, 'raw_v239'),
    output: path.join(showcaseRoot, 'output', 'v239-update'),
    verifyOnly: false
  };
  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    if (option === '--locale') result.locale = argv[++index];
    else if (option === '--input') result.input = path.resolve(argv[++index]);
    else if (option === '--output') result.output = path.resolve(argv[++index]);
    else if (option === '--verify-only') result.verifyOnly = true;
    else throw new Error(`Unknown option: ${option}`);
  }
  if (!['zh-CN', 'en'].includes(result.locale)) throw new Error('Only zh-CN and en poster copy are supported');
  requireInside(path.join(showcaseRoot, 'output'), result.output, 'Deck output');
  requireInside(showcaseRoot, result.input, 'Deck input');
  return result;
}

function dataUri(filePath) {
  return `data:image/png;base64,${fs.readFileSync(filePath).toString('base64')}`;
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]);
}

function posterHtml(page, index, total, locale, source) {
  const copy = page.copy[locale];
  const dark = page.layout === 'blueprint';
  const title = escapeHtml(copy.title);
  const body = escapeHtml(copy.body);
  const note = escapeHtml(copy.note);
  const eyebrow = escapeHtml(copy.eyebrow);
  const isChinese = locale === 'zh-CN';
  const sourceLabel = isChinese ? '真实运行截图 / 自动化采录' : 'Authentic runtime UI / automated capture';
  const evidenceStatus = isChinese ? '已验证界面' : 'AUTHENTIC UI';
  const verificationStatus = isChinese ? 'SHA-256 已验证' : 'SHA-256 VERIFIED';
  const annotationLabel = isChinese ? '功能说明' : 'Feature annotation';
  const footerScope = isChinese ? 'V2.3.8 → V2.3.9 · 桌面伴读、PDF矢量编辑与极速单机' : 'V2.3.8 → V2.3.9 · DESKTOP PET, VECTOR PDF & LOCAL FIRST';
  const pageNo = String(index + 1).padStart(2, '0');

  return `<!doctype html>
<html lang="${locale}"><meta charset="utf-8"><title>${title}</title>
<style>
  :root{
    --paper:${dark ? '#263F50' : '#F2F0EA'};
    --surface:${dark ? '#EDE9DE' : '#FBFAF5'};
    --ink:${dark ? '#F5F1E8' : '#20231F'};
    --muted:${dark ? '#D2D4CB' : '#5E6258'};
    --line:${dark ? '#8C9D8A' : '#C9C6B9'};
    --accent:#657555;
    --shadow:${dark ? 'rgba(8,16,21,.38)' : 'rgba(32,35,31,.15)'};
  }
  *{box-sizing:border-box}
  html,body{width:1080px;height:1440px;margin:0;overflow:hidden;background:var(--paper);color:var(--ink)}
  .poster{
    width:1080px;height:1440px;padding:48px 56px 40px;
    display:grid;grid-template-rows:auto 1fr auto;gap:22px;
    background:var(--paper);font-family:"Microsoft YaHei","Noto Sans CJK SC",sans-serif
  }
  .registration{display:grid;grid-template-columns:1fr auto;align-items:end;gap:20px;padding-bottom:15px;border-bottom:1px solid var(--line)}
  .registration p{margin:0;color:var(--muted);font:700 20px/1.2 "Cascadia Mono","SFMono-Regular",monospace;letter-spacing:.055em;text-transform:uppercase}
  .registration strong{font:700 22px/1.2 "Cascadia Mono","SFMono-Regular",monospace;color:var(--accent)}

  .page-body{min-height:0;display:grid;gap:20px}
  .specimen .page-body,.blueprint .page-body,.archive .page-body{
    position:relative;grid-template-columns:1fr;grid-template-rows:auto 1fr;gap:18px
  }
  .ledger .page-body{
    position:relative;grid-template-columns:1fr;grid-template-rows:1fr auto;gap:18px
  }

  .copy{min-width:0;max-width:640px}
  .copy h1{
    margin:0;font-family:"Bodoni MT","Iowan Old Style","Baskerville","Noto Serif CJK SC",serif;
    font-weight:500;font-size:74px;line-height:1.02;letter-spacing:-.04em
  }
  .blueprint .copy h1{font-size:70px}
  .copy .body{margin:14px 0 0;font-size:24px;line-height:1.5;color:var(--muted);letter-spacing:.005em}

  .callout{position:absolute;z-index:2;right:0;top:4px;width:260px;padding-top:14px;display:grid;gap:14px}
  .ledger .callout{top:auto;bottom:4px}
  .callout .rule{width:100%;height:1px;background:var(--accent);transform-origin:left}
  .callout .dot{width:12px;height:12px;border-radius:50%;background:var(--accent);margin-top:-20px}
  .callout p{margin:8px 0 0;color:var(--ink);font:700 18px/1.36 "Microsoft YaHei","Noto Sans CJK SC",sans-serif}
  .callout small{display:block;margin-top:6px;color:var(--muted);font:600 15px/1.4 "Cascadia Mono","SFMono-Regular",monospace}

  .evidence{min-height:0;display:flex;flex-direction:column;gap:10px;align-items:center;justify-content:center}
  .evidence-label{width:100%;display:flex;justify-content:space-between;gap:12px;color:var(--muted);font:700 17px/1.25 "Cascadia Mono","SFMono-Regular",monospace;letter-spacing:.035em}
  .evidence-label::after{content:"${verificationStatus}";color:var(--accent);white-space:nowrap}

  /* 精密自适应相框：紧致贴合图像原始长宽比，消除空隙，提升沉浸感与质感 */
  .frame{
    display:inline-flex;align-items:center;justify-content:center;
    padding:8px;background:var(--surface);border:1px solid var(--line);border-radius:8px;
    box-shadow:8px 10px 0 var(--shadow);
    width:fit-content;height:fit-content;max-width:100%;max-height:100%;
    margin:auto 0;
  }
  .blueprint .frame{border:6px solid var(--surface);box-shadow:none}
  .archive .frame{box-shadow:8px 10px 0 var(--accent)}
  .frame img{
    display:block;max-width:952px;max-height:780px;width:auto;height:auto;
    border-radius:4px;object-fit:contain
  }

  .footer{display:grid;grid-template-columns:auto 1fr auto;align-items:center;gap:16px;padding-top:14px;border-top:1px solid var(--line);color:var(--muted);font:700 17px/1.25 "Cascadia Mono","SFMono-Regular",monospace;letter-spacing:.035em}
  .footer .stamp{width:16px;height:16px;background:var(--accent)}
  .footer span:last-child{text-align:right;color:var(--ink)}

  .blueprint .frame{background:#F4F1E8}
  .blueprint .registration strong,.blueprint .footer span:last-child{color:#F5F1E8}
  .blueprint .callout p{color:#F5F1E8}
  .blueprint .callout .rule{background:#B8C2AB}
  .blueprint .callout .dot{background:#B8C2AB}
</style>
<article class="poster ${page.layout}">
  <header class="registration"><p>${eyebrow}</p><strong>${pageNo} / ${String(total).padStart(2, '0')}</strong></header>
  <section class="page-body">
    <section class="copy"><h1>${title}</h1><p class="body">${body}</p></section>
    <aside class="callout" aria-label="${annotationLabel}"><span class="rule"></span><span class="dot"></span><p>${note}</p><small>${sourceLabel}</small></aside>
    <figure class="evidence">
      <figcaption class="evidence-label">READMD 2.3.9 <span>${evidenceStatus}</span></figcaption>
      <div class="frame"><img src="${source}" alt="${title}"></div>
    </figure>
  </section>
  <footer class="footer"><span class="stamp" aria-hidden="true"></span><span>${footerScope}</span><span>${page.source.sha256.slice(0, 12)}</span></footer>
</article>`;
}

function galleryHtml(records, locale, title) {
  const cards = records.map((record) => `<a class="deck-card" href="${record.file}"><img src="${record.file}" alt="${escapeHtml(record.title)}"><span>${record.index}. ${escapeHtml(record.title)}</span></a>`).join('');
  return `<!doctype html><meta charset="utf-8"><title>${escapeHtml(title)}</title><style>body{margin:0;background:#F2F0EA;color:#20231F;font-family:"Microsoft YaHei",sans-serif}.wrap{max-width:1400px;margin:auto;padding:48px 32px}h1{font-family:"Bodoni MT","Iowan Old Style",serif;font-weight:500;font-size:52px}.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(240px,1fr));gap:22px}.deck-card{color:inherit;text-decoration:none}.deck-card img{display:block;width:100%;box-shadow:8px 10px 0 rgba(32,35,31,.14)}.deck-card span{display:block;padding:12px 0;font-weight:700}</style><main class="wrap"><h1>${escapeHtml(title)}</h1><section class="grid">${cards}</section></main>`;
}

function outputName(index, id) {
  return `${String(index + 1).padStart(2, '0')}-${id}.png`;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const { manifest, pages } = validateDeck({ manifestPath, showcaseRoot, captureDir: options.input });
  if (options.verifyOnly) {
    console.log(`Verified ${pages.length} distinct authentic V2.3.9 screenshot sources.`);
    return;
  }
  fs.rmSync(options.output, { recursive: true, force: true });
  fs.mkdirSync(options.output, { recursive: true });
  const browser = await chromium.launch();
  const records = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1080, height: 1440 }, deviceScaleFactor: 1 });
    for (const [index, deckPage] of pages.entries()) {
      const file = outputName(index, deckPage.id);
      await page.setContent(posterHtml(deckPage, index, pages.length, options.locale, dataUri(deckPage.source.path)), { waitUntil: 'load' });
      const audit = await page.evaluate(() => {
        const clipped = [...document.querySelectorAll('h1,p,small,span,strong')].filter((element) => {
          const style = getComputedStyle(element);
          const clipsX = ['hidden', 'clip', 'auto', 'scroll'].includes(style.overflowX) && element.scrollWidth > element.clientWidth + 1;
          const clipsY = ['hidden', 'clip', 'auto', 'scroll'].includes(style.overflowY) && element.scrollHeight > element.clientHeight + 1;
          return clipsX || clipsY || style.visibility === 'hidden';
        }).map((element) => element.textContent.trim().slice(0, 50));
        const image = document.querySelector('.frame img');
        const frame = document.querySelector('.frame');
        const frameBounds = frame && frame.getBoundingClientRect();
        const evidenceBounds = document.querySelector('.evidence')?.getBoundingClientRect();
        const evidenceRatio = frameBounds ? (frameBounds.width * frameBounds.height) / (1080 * 1440) : 0;
        return {
          width: document.documentElement.scrollWidth,
          height: document.documentElement.scrollHeight,
          clipped,
          imageReady: Boolean(image && image.complete && image.naturalWidth > 0),
          evidenceRatio,
          frame: frameBounds && { width: frameBounds.width, height: frameBounds.height },
          evidence: evidenceBounds && { width: evidenceBounds.width, height: evidenceBounds.height }
        };
      });
      if (audit.width !== 1080 || audit.height !== 1440 || audit.clipped.length || !audit.imageReady || audit.evidenceRatio < 0.25) {
        throw new Error(`${deckPage.id} layout audit failed: ${JSON.stringify(audit)}`);
      }
      const destination = path.join(options.output, file);
      await page.screenshot({ path: destination, type: 'png' });
      records.push({
        index: index + 1,
        id: deckPage.id,
        file,
        title: deckPage.copy[options.locale].title,
        layout: deckPage.layout,
        ui_area_ratio: Number(audit.evidenceRatio.toFixed(4)),
        source: { id: deckPage.source.id, kind: deckPage.source.kind, sha256: deckPage.source.sha256 },
        sha256: sha256(destination),
        evidence: deckPage.evidence
      });
    }
  } finally {
    await browser.close();
  }
  const duplicateOutput = new Map();
  for (const record of records) {
    if (duplicateOutput.has(record.sha256)) throw new Error(`Poster output duplicates ${duplicateOutput.get(record.sha256)}: ${record.file}`);
    duplicateOutput.set(record.sha256, record.file);
  }
  fs.writeFileSync(path.join(options.output, 'deck-evidence.json'), JSON.stringify({
    schema_version: 1,
    release: manifest.release,
    range: manifest.range,
    locale: options.locale,
    generated_at: new Date().toISOString(),
    renderer_sha256: crypto.createHash('sha256').update(fs.readFileSync(fileURLToPath(import.meta.url))).digest('hex'),
    posters: records
  }, null, 2));
  fs.writeFileSync(path.join(options.output, `update-list.${options.locale}.md`), updateListMarkdown(manifest, options.locale), 'utf8');
  fs.writeFileSync(path.join(options.output, 'index.html'), galleryHtml(records, options.locale, manifest.title[options.locale]), 'utf8');
  console.log(`Rendered ${records.length} distinct ${options.locale} evidence posters to ${options.output}`);
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exitCode = 1;
});
