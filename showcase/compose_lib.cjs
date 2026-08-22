'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const DEFAULT_DESIGN_PATH = path.join(__dirname, 'design', 'tokens.json');

function slug(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'feature';
}

function loadDesignSystem(designPath = DEFAULT_DESIGN_PATH) {
  const design = JSON.parse(fs.readFileSync(designPath, 'utf8'));
  if (design.schema_version !== 1) throw new Error('Unsupported design token schema');
  if (!/^#[0-9a-f]{6}$/i.test(design.palette.accent)) throw new Error('Design accent must be a hex color');
  if (design.type.display.size < 72 || design.type.body.size < 24) throw new Error('Card type is too small for Xiaohongshu');
  if (!/proof/i.test(design.signature)) throw new Error('Evidence-paper signature missing');
  return design;
}

function coverHook(story) {
  const hook = story.cover_hook;
  if (!hook || typeof hook !== 'object') throw new Error('story.cover_hook is missing');
  if (!/^\#\d+$/.test(hook.formula_id || '')) throw new Error('Cover hook formula id is missing');
  const title = String(hook.title || '').trim();
  const caption = String(hook.caption || '').trim();
  if (title.length < 2 || title.length > 8) throw new Error(`Cover hook title must contain 2-8 characters: ${title}`);
  if (caption.length < 8 || caption.length > 32) throw new Error(`Cover hook caption must contain 8-32 characters: ${caption}`);
  if (title === '本地文档台') throw new Error('Cover hook falls back to the generic local-workbench label');
  return { ...hook, title, caption };
}

function planCards(story) {
  const hook = coverHook(story);
  const cards = [{
    index: 1,
    file: 'xhs-01-cover.jpg',
    role: 'cover',
    shotId: story.selected_shots[0],
    title: hook.title,
    caption: hook.caption,
    uiMinRatio: 0,
  }];
  for (const shot of story.shots) {
    const index = cards.length + 1;
    cards.push({
      index,
      file: `xhs-${String(index).padStart(2, '0')}-${slug(shot.id.replace('.', '-'))}.jpg`,
      role: shot.role || (shot.id === 'overview.reader' ? 'pure_ui_hero' : 'annotated_ui'),
      shotId: shot.id,
      title: shot.name,
      caption: shot.description,
      uiMinRatio: (shot.role || (shot.id === 'overview.reader' ? 'pure_ui_hero' : 'annotated_ui')) === 'pure_ui_hero' ? 0.7 : 0.55,
    });
  }
  cards.push({
    index: cards.length + 1,
    file: `xhs-${String(cards.length + 1).padStart(2, '0')}-summary.jpg`,
    role: 'summary',
    shotId: story.selected_shots[0],
    title: '本地 Markdown 工作台',
    caption: '阅读、编辑、转换、学术排版与共享在同一处完成。',
    uiMinRatio: 0.30,
  });
  if (cards.length < 4 || cards.length > 9) throw new Error(`Card count must be between 4 and 9, got ${cards.length}`);
  if (cards[1].role !== 'pure_ui_hero' || cards[1].shotId !== 'overview.reader') throw new Error('Card 2 must be the complete overview.reader hero');
  return cards;
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (char) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[char]);
}

function imageSrc(packageDir, capture, card) {
  if (!card.shotId) return '';
  const entry = capture.shots.find((shot) => shot.shot_id === card.shotId);
  if (!entry) throw new Error(`Missing captured shot: ${card.shotId}`);
  const source = path.resolve(packageDir, entry.file);
  const actual = crypto.createHash('sha256').update(fs.readFileSync(source)).digest('hex');
  if (actual !== entry.sha256) throw new Error(`SHA-256 mismatch for ${card.shotId}`);
  return `data:image/png;base64,${fs.readFileSync(source).toString('base64')}`;
}

function buildCardHtml(card, source, context = {}) {
  const design = context.design || loadDesignSystem();
  const release = escapeHtml(context.release || '');
  const title = escapeHtml(card.title);
  const caption = escapeHtml(card.caption);
  let body;
  if (card.role === 'cover') {
    body = `
      <main class="cover">
        <header class="proof-strip"><span class="mark"></span><span>READMD ${release} · 真实运行画面</span></header>
        <section class="cover-copy"><h1>${title}</h1><p>${caption}</p></section>
        <img class="proof-image" src="${source}" alt="ReadMD 真实主界面"/>
        <footer><strong>GitHub 搜索 Natsummerance/readMD</strong><span>本地优先 · 开源 · 不改原文件</span></footer>
      </main>`;
  } else if (card.role === 'summary') {
    body = `
      <main class="summary">
        <header class="feature-head"><h2>${title}</h2><p>${caption}</p></header>
        <img src="${source}" alt="ReadMD 真实界面"/>
        <ul><li>阅读与编辑</li><li>转换与学术排版</li><li>演示与移动共享</li></ul>
        <footer class="proof-foot"><span>ReadMD ${release}</span><strong>GitHub 搜 Natsummerance/readMD</strong></footer>
      </main>`;
  } else if (card.role === 'pure_ui_hero') {
    body = `
      <main class="hero">
        <img src="${source}" alt="${title}"/>
        <footer class="proof-foot hero-proof"><strong>${title}</strong><span>真实运行画面</span></footer>
      </main>`;
  } else {
    body = `
      <main class="feature">
        <header class="feature-head"><h2>${title}</h2><p>${caption}</p></header>
        <img src="${source}" alt="${title}"/>
        <footer class="proof-foot"><span>真实运行画面</span><span>ReadMD ${release}</span></footer>
      </main>`;
  }
  return `<!doctype html><meta charset="utf-8"><title>${title}</title>
<style>
  *{box-sizing:border-box;margin:0}
  html,body{width:${design.layout.canvas[0]}px;height:${design.layout.canvas[1]}px;overflow:hidden;background:${design.palette.background};color:${design.palette.ink};font-family:${design.type.body.family}}
  main{width:100%;height:100%;padding:${design.layout.padding[0]}px ${design.layout.padding[1]}px;display:flex;flex-direction:column;gap:${design.layout.gap}px;background:${design.palette.background}}
  .proof-strip{display:flex;align-items:center;gap:16px;padding:18px 22px;border:1px solid ${design.palette.line};border-radius:${design.layout.radius}px;background:${design.palette.surface};font-family:${design.type.utility.family};font-size:${design.type.utility.size}px;font-weight:${design.type.utility.weight};color:${design.palette.ink};letter-spacing:.04em}
  .proof-strip .mark{width:18px;height:18px;background:${design.palette.accent};border-radius:3px}
  h1,h2{font-family:${design.type.display.family};color:${design.palette.ink};letter-spacing:0}
  .cover{gap:30px}
  .cover-copy{min-height:280px;display:flex;flex-direction:column;justify-content:center}
  .cover h1{font-size:${design.type.display.size}px;line-height:${design.type.display.line_height};font-weight:${design.type.display.weight};max-width:900px}
  .cover p{margin-top:26px;font-size:31px;line-height:1.42;color:${design.palette.muted};max-width:900px}
  .proof-image{flex:1;min-height:420px;width:100%;object-fit:cover;object-position:top;border:1px solid ${design.palette.screenshot_frame};border-radius:${design.layout.radius}px;background:${design.palette.surface}}
  .cover footer{margin-top:auto;padding-top:26px;border-top:3px solid ${design.palette.accent};display:flex;justify-content:space-between;align-items:center;gap:24px}
  .cover footer strong,.proof-foot strong{font-size:33px;color:${design.palette.ink}}
  .cover footer span,.proof-foot span{font-size:24px;color:${design.palette.muted}}
  .feature-head{min-height:210px;display:flex;flex-direction:column;justify-content:center}
  h2{font-size:64px;line-height:1.2;font-weight:900}
  header p,.summary p{margin-top:18px;font-size:29px;line-height:1.42;color:${design.palette.muted};max-width:900px}
  img{flex:1;min-height:0;width:100%;object-fit:cover;border:1px solid ${design.palette.screenshot_frame};border-radius:${design.layout.radius}px;background:${design.palette.surface}}
  .feature,.summary{gap:30px}
  .proof-foot{display:flex;justify-content:space-between;align-items:center;gap:20px;padding-top:20px;border-top:1px solid ${design.palette.line}}
  .proof-foot span{font-size:24px}
  .hero{padding:24px;gap:20px}
  .hero img{height:auto;flex:1}
  .hero-proof{padding:0}
  .summary ul{list-style:none;display:flex;gap:18px}
  .summary li{flex:1;background:${design.palette.surface};border:1px solid ${design.palette.line};border-left:4px solid ${design.palette.accent};border-radius:${design.layout.radius}px;padding:20px 22px;font-size:25px;line-height:1.3;color:${design.palette.ink}}
</style>${body}`;
}

module.exports = { buildCardHtml, imageSrc, loadDesignSystem, planCards, slug };
