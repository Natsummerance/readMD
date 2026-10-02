// Real offline renderer + Rust preload. No fake sprites or Cubism implementation.
const { test, expect } = require('@playwright/test');
const fs = require('node:fs');
const path = require('node:path');
const repo = path.resolve(__dirname, '..');
const renderer = process.env.READMD_PET_RENDERER_BUILD;
const preload = fs.readFileSync(path.join(repo, 'packages/readmd-pet-rust/src/webview/mod.rs'), 'utf8')
  .match(/pub const PRELOAD_ABI: &str = r#"([\s\S]*?)"#;/)[1];
test.skip(!renderer, 'Set READMD_PET_RENDERER_BUILD to the offline renderer build');
async function mount(page, character = 'mochi', kind = 'hermes-sprite', assetDelay = 0) {
  await page.setViewportSize({ width: 320, height: 420 });
  await page.route('**/pet-fixture/**', async route => {
    const relative = decodeURIComponent(new URL(route.request().url()).pathname.replace('/pet-fixture/', ''));
    const base = path.dirname(path.resolve(renderer)), file = path.resolve(base, relative);
    if (!file.startsWith(base + path.sep) || !fs.existsSync(file)) return route.fulfill({ status: 404, body: 'missing fixture' });
    if (assetDelay && relative.endsWith('mochi-sprite.png')) await new Promise(resolve => setTimeout(resolve, assetDelay));
    return route.fulfill({ path: file });
  });
  await page.addInitScript(() => {
    window.__petCommands = [];
    window.ipc = { postMessage: raw => {
      const command = JSON.parse(raw); window.__petCommands.push(command);
      if (command.payload?.type === 'renderer-ready') window.__firstReadyHasSprite = !!window.__bongoPet?.spritePose;
    } };
    localStorage.setItem('readmd-pet-character', 'mochi');
    localStorage.setItem('readmd-pet-showdesk', 'true'); // Old preferences must not give originals a keyboard.
    localStorage.setItem('readmd-pet-sound', 'false');
  });
  await page.addInitScript(preload);
  await page.goto(`/pet-fixture/renderer/index.html?generation=1&session=fixture&renderer=${kind}`);
  await page.waitForFunction(() => window.__bongoPet);
  await page.evaluate(character => window.__readmdRustDispatch.state({ info: { character } }), character);
  await expect.poll(() => page.evaluate(() => window.__petCommands.some(c => c.payload?.type === 'renderer-ready'))).toBe(true);
  if (character === 'arch-chan') await page.waitForFunction(() => window.__readmdLive2d);
  else await page.waitForFunction(() => window.__bongoPet.spritePose);
}

async function localized(page, extra = {}) {
  const lines = JSON.parse(fs.readFileSync(path.join(repo,'assets/i18n/en.json'),'utf8'));
  await page.evaluate(({lines,extra}) => window.__readmdRustDispatch.state({info:{...window.__bongoPet.state.petInfo,lines,...extra}}),{lines,extra});
}
async function regionPayload(page) { return page.evaluate(()=>window.__petCommands.filter(c=>c.payload?.type==='interaction-regions').at(-1).payload); }

test('opaque pixel runs leave most of the pet window free and include current authored paws', async ({page}) => {
  await mount(page,'hermes'); await localized(page);
  await page.waitForTimeout(200);
  const p=await regionPayload(page);
  expect(p.rects.length).toBeGreaterThan(3); expect(p.rects.length).toBeLessThanOrEqual(512);
  const area=p.petRects.reduce((sum,r)=>sum+r.width*r.height,0);
  expect(area).toBeLessThan(320*420*.12);
  expect(p.petRects.every(r=>r.y>250 && r.x>0 && r.x+r.width<320)).toBe(true);
  expect(p.petRects.some(r=>160>=r.x && 160<r.x+r.width && 5>=r.y && 5<r.y+r.height)).toBe(false);
});

test('smallest, default and largest sprite sizes are distinct and all compact', async ({page}) => {
  await mount(page,'mochi');
  const heights=[];
  for(const scale of [.08,.22,.48]) {
    await localized(page,{scale}); await page.waitForTimeout(180);
    heights.push(await page.evaluate(()=>window.__bongoPet.interactionRegions.rects[0].height));
  }
  expect(heights[0]).toBeLessThan(75); expect(heights[1]).toBeGreaterThan(heights[0]*1.4);
  expect(heights[2]).toBeGreaterThan(heights[1]*1.5); expect(heights[2]).toBeLessThan(195);
});

test('click opens localized dialogue and accessible actions that issue real companion commands', async ({page}) => {
  await mount(page); await localized(page);
  await page.evaluate(()=>window.__readmdRustDispatch.control({type:'pet'}));
  const bubble=page.locator('.readmd-pet-life__bubble[data-pet-interactive]');
  await expect(bubble).toBeVisible(); await expect(bubble.locator('[role=status]')).not.toBeEmpty();
  await expect(bubble.getByRole('button')).toHaveCount(6);
  await page.waitForTimeout(250);
  const p=await regionPayload(page), box=await bubble.boundingBox();
  expect(p.rects.some(r=>r.x<=box.x+3 && r.y<=box.y+3 && r.x+r.width>=box.x+box.width-3)).toBe(true);
  await bubble.getByRole('button',{name:/play/i}).click();
  expect(await page.evaluate(()=>window.__petCommands.some(c=>c.payload?.type==='interact' && c.payload.action==='play'))).toBe(true);
  await expect(bubble).toHaveCount(0);
});

test('right-click menu buttons receive native hit regions and character switches persist through the bridge', async ({page}) => {
  await mount(page);await localized(page);
  await page.locator('#bongocat-canvas').click({button:'right',position:{x:160,y:365}});
  const menu=page.getByRole('menu');await expect(menu).toBeVisible();
  await menu.getByRole('menuitem',{name:'Change character',exact:true}).click();
  await menu.getByRole('menuitemradio',{name:'Hermes',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__bongoPet.presentation)).toBe('hermes');
  expect(await page.evaluate(()=>window.__petCommands.some(c=>c.payload?.type==='character' && c.payload.slug==='hermes' && c.payload.renderer==='hermes-sprite'))).toBe(true);
  await page.locator('#bongocat-canvas').click({button:'right',position:{x:160,y:365}});
  await menu.getByRole('menuitem',{name:/settings/i}).click();
  expect(await page.evaluate(()=>window.__petCommands.some(c=>c.payload?.type==='open-app' && c.payload.target==='pet-settings'))).toBe(true);
});

test('locked position suppresses browser drag but keeps click dialogue', async ({page}) => {
  await mount(page);await localized(page,{lock_position:true});
  await page.mouse.move(160,365);await page.mouse.down();await page.mouse.move(200,370);await page.mouse.up();
  expect(await page.evaluate(()=>window.__petCommands.filter(c=>c.type==='drag-start').length)).toBe(0);
  await page.evaluate(()=>window.__readmdRustDispatch.control({type:'pet'}));
  await expect(page.locator('.readmd-pet-life__bubble[data-pet-interactive]')).toBeVisible();
});

for(const character of ['arch-chan','bongocat']) test(`${character} exposes real opaque model pixels at a compact size`,async({page})=>{
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await mount(page);await localized(page);
  await page.evaluate(character=>window.__bongoPet.applyCharacter(character),character);
  await page.waitForTimeout(350);
  const p=await regionPayload(page);expect(p.petRects.length).toBeGreaterThan(3);expect(p.petRects.length).toBeLessThanOrEqual(512);
  expect(p.petRects.reduce((s,r)=>s+r.width*r.height,0)).toBeLessThan(320*420*.16);
  expect(errors).toEqual([]);
});
