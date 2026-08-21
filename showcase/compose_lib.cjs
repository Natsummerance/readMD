'use strict';

const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

function slug(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'feature';
}

function planCards(story) {
  const cards = [{
    index: 1,
    file: 'xhs-01-cover.jpg',
    role: 'cover',
    shotId: null,
    title: story.angle,
    caption: '真实运行画面 · 本地优先',
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
  const release = escapeHtml(context.release || '');
  const title = escapeHtml(card.title);
  const caption = escapeHtml(card.caption);
  let body;
  if (card.role === 'cover') {
    body = `
      <main class="cover">
        <section><p class="eyebrow">READMD ${release}</p><h1>${title}</h1></section>
        <section class="grid">
          <div><h3>阅读编辑</h3><p>长文分页与实时预览</p></div>
          <div><h3>格式转换</h3><p>Office / PDF / 网页转 MD</p></div>
          <div><h3>学术排版</h3><p>LaTeX 与引用支持</p></div>
          <div><h3>代码执行</h3><p>文档内直接运行片段</p></div>
          <div><h3>演说模式</h3><p>Markdown 变幻灯片</p></div>
          <div><h3>移动共享</h3><p>局域网扫码继续阅读</p></div>
        </section>
        <footer><strong>GitHub 搜索 Natsummerance/readMD</strong><span>${caption}</span></footer>
      </main>`;
  } else if (card.role === 'summary') {
    body = `
      <main class="summary">
        <header><h2>${title}</h2><p>${caption}</p></header>
        <img src="${source}" alt="ReadMD 真实界面"/>
        <ul><li>阅读与编辑</li><li>转换与学术排版</li><li>演示与移动共享</li></ul>
        <footer><span>ReadMD ${release}</span><strong>GitHub 搜 Natsummerance/readMD</strong></footer>
      </main>`;
  } else if (card.role === 'pure_ui_hero') {
    body = `
      <main class="hero">
        <img src="${source}" alt="${title}"/>
        <div class="hero-label"><strong>READMD</strong><span>${release}</span></div>
      </main>`;
  } else {
    body = `
      <main class="feature">
        <header><h2>${title}</h2><p>${caption}</p></header>
        <img src="${source}" alt="${title}"/>
        <footer><span>真实运行画面</span><span>ReadMD ${release}</span></footer>
      </main>`;
  }
  return `<!doctype html><meta charset="utf-8"><title>${title}</title>
<style>
  *{box-sizing:border-box;margin:0}html,body{width:1080px;height:1440px;overflow:hidden;background:#0e1630;color:#edf2f7;font-family:"Microsoft YaHei","Noto Sans CJK SC",sans-serif}
  main{width:100%;height:100%;padding:72px 76px;display:flex;flex-direction:column;gap:40px}
  .cover{gap:34px}.eyebrow{font-size:28px;letter-spacing:.08em;color:#e8ff00;font-weight:700}
  .cover h1{font-size:82px;line-height:1.22;margin-top:28px;max-width:920px}
  .grid{flex:1;display:grid;grid-template-columns:repeat(2,1fr);grid-template-rows:repeat(3,1fr);gap:22px}
  .grid div{background:#24365a;border:1px solid #46536a;border-radius:14px;padding:24px;display:flex;flex-direction:column;justify-content:center}
  .grid h3{font-size:35px;color:#e8ff00}.grid p{margin-top:10px;font-size:24px;line-height:1.35}
  .feature footer span,.summary li{background:#182338;border:1px solid #334155;border-radius:14px;padding:18px 24px;font-size:27px}
  .cover footer{border-top:4px solid #e8ff00;padding-top:34px;display:flex;flex-direction:column;gap:16px}
  .cover footer strong{font-size:38px}.cover footer span{font-size:25px;color:#a8b3c4}
  header h2,.summary h2{font-size:58px;line-height:1.25}header p,.summary p{margin-top:18px;font-size:29px;line-height:1.45;color:#c7d0dc;max-width:900px}
  img{flex:1;min-height:0;width:100%;object-fit:cover;border-radius:24px;border:1px solid #46536a;background:#111826}
  .feature{gap:34px}.feature footer{display:flex;justify-content:space-between;font-size:24px;color:#a8b3c4}
  .hero{position:relative;padding:24px}.hero img{height:100%;border-radius:12px}
  .hero-label{position:absolute;left:48px;bottom:48px;display:flex;gap:18px;align-items:center;background:rgba(14,22,48,.88);padding:18px 24px;border-radius:14px;border:1px solid #e8ff00}
  .hero-label strong{color:#e8ff00;font-size:31px}.hero-label span{font-size:25px}
  .summary{gap:34px}.summary img{flex:1}.summary ul{list-style:none;display:flex;gap:20px}.summary footer{display:flex;justify-content:space-between;align-items:center;font-size:27px;color:#a8b3c4}
</style>${body}`;
}

module.exports = { buildCardHtml, imageSrc, planCards, slug };
