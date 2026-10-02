const { test, expect } = require('@playwright/test');
const fs = require('node:fs');
const path = require('node:path');

const repo = path.resolve(__dirname, '..');
const renderer = process.env.READMD_PET_RENDERER_BUILD;
const preload = fs.readFileSync(path.join(repo, 'packages/readmd-pet-rust/src/webview/mod.rs'), 'utf8')
  .match(/pub const PRELOAD_ABI: &str = r#"([\s\S]*?)"#;/)[1];
test.skip(!renderer, 'Set READMD_PET_RENDERER_BUILD to the offline renderer build');

async function mount(page, character = 'mochi', rendererKind = 'hermes-sprite') {
  await page.setViewportSize({ width: 320, height: 420 });
  await page.route('**/pet-fixture/**', async route => {
    const relative = decodeURIComponent(new URL(route.request().url()).pathname.replace('/pet-fixture/', ''));
    const base = path.dirname(path.resolve(renderer));
    const file = path.resolve(base, relative);
    if (!file.startsWith(base + path.sep) || !fs.existsSync(file)) return route.fulfill({ status: 404, body: 'missing fixture' });
    return route.fulfill({ path: file });
  });
  await page.addInitScript(() => {
    window.__petCommands = [];
    window.ipc = { postMessage: raw => window.__petCommands.push(JSON.parse(raw)) };
    localStorage.setItem('readmd-pet-character', 'mochi');
    localStorage.setItem('readmd-pet-mode', 'keyboard');
    localStorage.setItem('readmd-pet-sound', 'false');
  });
  await page.addInitScript(preload);
  await page.goto(`/pet-fixture/renderer/index.html?generation=1&session=fixture&renderer=${rendererKind}`);
  await page.waitForFunction(() => window.__bongoPet);
  await page.evaluate(character => window.__readmdRustDispatch.state({ info: { character } }), character);
  await expect.poll(() => page.evaluate(() => window.__petCommands.some(c => c.payload?.type === 'renderer-ready'))).toBe(true);
  await page.waitForTimeout(200);
}

function input(sequence, keys = [], buttons = 0, keyboard = 0, mouse = 0, last = keys.at(-1)) {
  return { sequence, pressed_keys: keys, keyboard_down: !!keys.length, left_down: !!keys.length,
    right_down: false, mouse_down: !!buttons, mouse_buttons: buttons, keyboard_taps: keyboard,
    left_taps: keyboard, right_taps: 0, mouse_taps: mouse, last_key: last, pointer_x: 0, pointer_y: 0 };
}
async function dispatch(page, payload) {
  await page.evaluate(p => window.__readmdRustDispatch.bongoInput(p), payload);
}
async function held(page) {
  return page.evaluate(() => {
    const s = window.__bongoPet.state;
    return { left: s.leftDown, right: s.rightDown, keys: [...s.pressedKeys], buttons: s.mouseButtons, taps: s.tapCounts };
  });
}

test('modifier chords and mouse buttons keep independent held states', async ({ page }) => {
  await mount(page);
  await dispatch(page, input(1, [0xe0], 0, 1));
  await dispatch(page, input(2, [0xe0, 4], 0, 2));
  await dispatch(page, input(3, [0xe0, 4], 3, 2, 2));
  await dispatch(page, input(4, [0xe0], 2, 2, 2, 4));
  expect(await held(page)).toEqual({ left: true, right: true, keys: [0xe0], buttons: 2, taps: { left: 2, right: 1 } });
  await dispatch(page, input(5, [0xe0], 0, 2, 2, 4));
  expect((await held(page)).left).toBe(true);
  expect((await held(page)).right).toBe(false);
  await dispatch(page, input(6, [], 0, 2, 2, 4));
  expect((await held(page)).left).toBe(false);
});

test('100 fast taps survive a single rendered frame and local key events do not duplicate native input', async ({ page }) => {
  await mount(page);
  await page.evaluate(events => {
    for (const event of events) window.__readmdRustDispatch.bongoInput(event);
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyA' }));
    window.dispatchEvent(new KeyboardEvent('keyup', { code: 'KeyA' }));
  }, Array.from({ length: 200 }, (_, i) => input(i + 1, i % 2 ? [] : [4], 0, Math.floor(i / 2) + 1, 0, 4)));
  expect(await held(page)).toEqual({ left: false, right: false, keys: [], buttons: 0, taps: { left: 100, right: 0 } });
  await dispatch(page, input(198, [4], 0, 99));
  expect((await held(page)).left).toBe(false);
});

test('thresholded drag requests stay singular and native head-petting controls show a reaction', async ({ page }) => {
  await mount(page);
  await page.mouse.move(160, 200);
  await page.mouse.down();
  expect(await page.evaluate(() => window.__bongoPet.state.pettingLevel)).toBe(0);
  await page.mouse.move(163, 200);
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'drag-start').length)).toBe(0);
  await page.mouse.move(180, 210);
  await page.mouse.move(230, 220);
  await page.mouse.up();
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'drag-start').length)).toBe(1);
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'bounds').length)).toBe(0);
  expect(await page.evaluate(() => window.__bongoPet.state.pettingLevel)).toBe(0);
  await page.evaluate(() => window.__readmdRustDispatch.control({ type: 'pet' }));
  expect(await page.evaluate(() => window.__bongoPet.state.pettingLevel)).toBeGreaterThan(0);
});

test('cancelled pointer capture clears drag without petting', async ({ page }) => {
  await mount(page);
  await page.mouse.move(160, 200);
  await page.mouse.down();
  await page.evaluate(() => window.dispatchEvent(new PointerEvent('pointercancel', { pointerId: 1 })));
  await page.mouse.move(200, 220);
  await page.mouse.up();
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'drag-start').length)).toBe(0);
  expect(await page.evaluate(() => window.__bongoPet.state.pettingLevel)).toBe(0);
});

test('pointer movement moves the mouse hand without manufacturing keyboard taps', async ({ page }) => {
  await mount(page);
  await dispatch(page, { ...input(1), pointer_x: 1, pointer_y: -1 });
  await page.waitForTimeout(150);
  const a = await page.evaluate(() => ({ x: window.__bongoPet.state.pawRightX, y: window.__bongoPet.state.pawRightY }));
  await dispatch(page, { ...input(1), pointer_x: -1, pointer_y: 1 });
  await page.waitForTimeout(150);
  const b = await page.evaluate(() => ({ x: window.__bongoPet.state.pawRightX, y: window.__bongoPet.state.pawRightY }));
  expect(Math.abs(a.x - b.x)).toBeGreaterThan(8);
  expect(Math.abs(a.y - b.y)).toBeGreaterThan(4);
  expect((await held(page)).taps).toEqual({ left: 0, right: 0 });
});

for (const character of ['mochi', 'hermes', 'amber', 'moss', 'cache-capy', 'niu-lai']) {
  test(`${character} keeps a transparent margin and shows keyboard and mouse at two sizes`, async ({ page }, testInfo) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await mount(page, character);
    await dispatch(page, input(1, [4], 1, 1, 1));
    for (const [width, height] of [[320, 420], [480, 560]]) {
      await page.setViewportSize({ width, height });
      await page.waitForTimeout(160);
      expect(await page.evaluate(() => document.querySelector('#bongocat-canvas').getContext('2d').getImageData(0, 0, 1, 1).data[3])).toBe(0);
      const snapshot = `pet-originals-${character}-${width}x${height}.png`;
      await page.screenshot({ path: process.env.READMD_PET_SCREENSHOTS ? path.join(process.env.READMD_PET_SCREENSHOTS, snapshot) : testInfo.outputPath(snapshot), omitBackground: true });
    }
    expect(errors).toEqual([]);
    expect(await page.evaluate(() => window.__bongoPet.armPose.map(p => p.kind))).toEqual(['source-cutout','source-cutout']);
    expect(await page.evaluate(() => Boolean(window.__bongoClassic))).toBe(false);
  });
}

test('the original BongoCat model reacts through real Cubism hand, mouse and pressed-key layers', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => window.__bongoPet.useClassic());
  await page.waitForFunction(() => window.__bongoClassic);
  expect(await page.evaluate(() => {
    const b = window.__bongoClassic;
    b.app.renderer.render(b.app.stage);
    const pixels = b.app.renderer.extract.pixels();
    let visible = 0;
    for (let i = 3; i < pixels.length; i += 4) if (pixels[i] > 127) visible++;
    return visible;
  })).toBeGreaterThan(8000);
  await dispatch(page, { ...input(1, [4, 0xe0], 3, 2, 2, 4), pointer_x: 0.7, pointer_y: -0.5 });
  await expect.poll(() => page.evaluate(() => ({
    ...window.__bongoClassic.parameters,
    a: window.__bongoClassic.overlays.get(4)?.visible,
    ctrl: window.__bongoClassic.overlays.get(0xe0)?.visible
  }))).toMatchObject({ CatParamLeftHandDown: 1, ParamMouseLeftDown: 1, ParamMouseRightDown: 1, a: true, ctrl: true });
  await dispatch(page, input(2, [0xe0], 2, 2, 2, 4));
  await expect.poll(() => page.evaluate(() => ({ ...window.__bongoClassic.parameters, ctrl: window.__bongoClassic.overlays.get(0xe0)?.visible })))
    .toMatchObject({ CatParamLeftHandDown: 1, ParamMouseLeftDown: 0, ParamMouseRightDown: 1, ctrl: true });
  await dispatch(page, input(3, [], 0, 2, 2, 4));
  await expect.poll(() => page.evaluate(() => window.__bongoClassic.parameters))
    .toMatchObject({ CatParamLeftHandDown: 0, ParamMouseLeftDown: 0, ParamMouseRightDown: 0 });
  await dispatch(page, input(4, [0xe7], 0, 3, 2, 0xe7));
  await expect.poll(() => page.evaluate(() => window.__bongoClassic.overlays.get(0xe3)?.visible)).toBe(true);
  await dispatch(page, input(5, [], 0, 3, 2, 0xe7));
  await expect.poll(() => page.evaluate(() => window.__bongoClassic.overlays.get(0xe3)?.visible)).toBe(false);
});

test('the rebuilt optional Live2D model mounts without replacing the shared drag canvas', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await mount(page, 'arch-chan');
  await expect.poll(() => page.evaluate(() => document.querySelector('#live2d-stage canvas') !== null), { timeout: 20000 }).toBe(true);
  await expect(page.locator('#bongocat-canvas')).toBeVisible();
  expect(errors).toEqual([]);
});

for (const rendererKind of ['hermes-sprite', 'live2d']) {
  test(`${rendererKind} Arch-chan uses authored hand parameters for global input, independent chords and releases`, async ({ page }, testInfo) => {
    await mount(page, 'arch-chan', rendererKind);
    await page.waitForFunction(() => window.__readmdLive2d);
    const neutral = await page.evaluate(() => Array.from(window.__readmdLive2d.model.internalModel.coreModel._model.drawables.vertexPositions[27]));
    await dispatch(page, { ...input(1, [0xe0,4], 3, 2, 2), pointer_x: 0.9, pointer_y: -0.7 });
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters)).toMatchObject({ MouseToggle: 1 });
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.keyboard)).toBeGreaterThan(0.9);
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.MouseX)).toBeLessThan(-0.75);
    expect(await page.evaluate(before => {
      const vertices = window.__readmdLive2d.model.internalModel.coreModel._model.drawables.vertexPositions[27];
      return vertices.some((value, i) => Math.abs(value - before[i]) > 0.01);
    }, neutral)).toBe(true);
    expect(await page.evaluate(() => window.__bongoPet.armPose.length)).toBe(0);
    await dispatch(page, input(2, [0xe0], 0, 2, 2, 4));
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.mouse)).toBeLessThan(0.01);
    expect(await page.evaluate(() => window.__readmdLive2d.parameters.keyboard)).toBeGreaterThan(0.9);
    await dispatch(page, input(3, [], 0, 2, 2, 4));
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.keyboard)).toBeLessThan(0.01);
    for (const [width,height] of [[320,420],[480,560]]) {
      await page.setViewportSize({width,height});
      await page.waitForTimeout(150);
      const layout = await page.evaluate(() => {
        const b=window.__readmdLive2d, r=b.model.getBounds();
        return {width:r.width,height:r.height,bottom:r.bottom,screen:b.app.screen.width,
          head:b.interactionRegions().head, commands:window.__petCommands.filter(c=>c.payload?.type==='interaction-regions').at(-1)?.payload};
      });
      expect(layout.height).toBeGreaterThan(height * 0.45);
      expect(layout.head.y).toBeGreaterThanOrEqual(0);
      expect(layout.head.y+layout.head.height).toBeLessThan(height);
      expect(layout.screen).toBe(width);
      expect(layout.commands.head).toEqual(layout.head);
      expect(layout.commands.rects.length).toBe(2);
      const file=`pet-originals-arch-${rendererKind}-${width}x${height}.png`;
      await page.screenshot({path:process.env.READMD_PET_SCREENSHOTS?path.join(process.env.READMD_PET_SCREENSHOTS,file):testInfo.outputPath(file),omitBackground:true});
      const contacts=await page.evaluate(() => {const rig=window.__readmdLive2d.deskRig;return {active:rig.active,left:rig.left.at(-1),right:rig.right.at(-1)};});
      expect(contacts.active).toBe(true);
      expect(contacts.left.x).toBeLessThan(width/2);
      expect(contacts.right.x).toBeGreaterThan(width/2);
    }
    await page.evaluate(() => window.__bongoPet.setInstrument('keyboard', false));
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.MouseToggle)).toBe(0);
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.deskRig.active)).toBe(false);
    await page.evaluate(() => window.__bongoPet.applyCharacter('mochi'));
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.app.ticker.started)).toBe(false);
  });
}

test('an imported sheet keeps its own named rows, frame sizes, counts and art', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    const sheet=document.createElement('canvas'); sheet.width=120;sheet.height=90;
    const c=sheet.getContext('2d');
    for(let row=0;row<3;row++)for(let col=0;col<4;col++){
      c.fillStyle=['#ffaa22','#2277ff','#22cc66'][row];c.fillRect(col*30+3,row*30+3,24,24);
    }
    window.__readmdRustDispatch.state({info:{character:'own-sheet',frameW:30,frameH:30,stateRows:['idle','waving','review'],
      framesByRow:{review:2},loopMs:200,spritesheetBase64:sheet.toDataURL().split(',')[1],mime:'image/png'}});
  });
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose?.fw)).toBe(30);
  await dispatch(page,input(1,[4],0,1));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('review');
  const pose=await page.evaluate(() => {const p=window.__bongoPet.spritePose;return {row:p.row,fh:p.fh,frame:p.frameIndex,arms:window.__bongoPet.armPose.length};});
  expect(pose).toMatchObject({row:2,fh:30,arms:0});
  expect(pose.frame).toBeLessThan(2);
  const head=await page.evaluate(()=>window.__petCommands.filter(c=>c.payload?.type==='interaction-regions').at(-1).payload.head);
  expect(head.y).toBeLessThan(80); // This short sheet lives at the top, not at Mochi's head position.
  expect(head.width).toBeLessThan(30);
  await dispatch(page,input(2,[],0,1));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('idle');
  await page.evaluate(() => window.__bongoPet.petPet());
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.row)).toBe(1);
});

test('the host-delivered original sprite keeps its matching rig and is not decoded on every snapshot', async ({ page }) => {
  await mount(page,'niu-lai');
  const spritesheetBase64=fs.readFileSync(path.join(repo,'assets/pet/niu-lai/spritesheet.webp')).toString('base64');
  const info={character:'niu-lai',frameW:192,frameH:208,spritesheetBase64,mime:'image/webp'};
  await page.evaluate(info=>window.__readmdRustDispatch.state({info}),info);
  await expect.poll(()=>page.evaluate(()=>!!window.__bongoPet.spritePose.img.readmdImported)).toBe(true);
  expect(await page.evaluate(()=>window.__bongoPet.spritePose.originalArms)).toBe(true);
  await page.evaluate(()=>window.__originalSheetImage=window.__bongoPet.spritePose.img);
  await page.evaluate(info=>{for(let i=0;i<5;i++)window.__readmdRustDispatch.state({info});},info);
  await page.waitForTimeout(100);
  expect(await page.evaluate(()=>window.__originalSheetImage===window.__bongoPet.spritePose.img)).toBe(true);
  expect(await page.evaluate(()=>window.__bongoPet.armPose.length)).toBe(2);
});

test('repeated Live2D rig frames and desk toggles restore the original mesh without accumulating deformation', async ({ page }) => {
  await mount(page,'arch-chan','live2d');
  await page.waitForFunction(()=>window.__readmdLive2d?.deskRig.active);
  const result=await page.evaluate(()=>{
    const b=window.__readmdLive2d,core=b.model.internalModel.coreModel;
    b.app.ticker.stop();core.update();core.update();
    const before=core._model.drawables.vertexPositions.map(p=>Array.from(p));
    for(let frame=0;frame<240;frame++)core.update();
    let drift=0;
    core._model.drawables.vertexPositions.forEach((p,i)=>p.forEach((value,j)=>drift=Math.max(drift,Math.abs(value-before[i][j]))));
    b.app.ticker.start();return drift;
  });
  expect(result).toBeLessThan(0.000001);
  for(let toggle=0;toggle<3;toggle++){
    await page.evaluate(()=>window.__bongoPet.setInstrument('keyboard',false));
    await expect.poll(()=>page.evaluate(()=>window.__readmdLive2d.deskRig.active)).toBe(false);
    await page.evaluate(()=>window.__bongoPet.setInstrument('keyboard',true));
    await expect.poll(()=>page.evaluate(()=>window.__readmdLive2d.deskRig.active)).toBe(true);
  }
});

test('switching instruments while controls are held updates both hands without creating extra taps', async ({ page })=>{
  await mount(page);
  await dispatch(page,{...input(1,[4],0,1),left_down:false,right_down:true});
  const originalTaps=(await held(page)).taps;
  await page.evaluate(()=>window.__bongoPet.setInstrument('bongos',true));
  expect((await held(page)).left).toBe(false);
  expect((await held(page)).right).toBe(true);
  await page.evaluate(()=>window.__bongoPet.setInstrument('keyboard',true));
  expect((await held(page)).left).toBe(true);
  expect((await held(page)).right).toBe(false);
  expect((await held(page)).taps).toEqual(originalTaps);
});

test('quiet Live2D keeps a visible static pose and reacts to deliberate input after resizing', async ({ page })=>{
  await mount(page,'arch-chan','live2d');
  await page.waitForFunction(()=>window.__readmdLive2d);
  await page.evaluate(()=>window.__readmdRustDispatch.state({info:{character:'arch-chan',animation:{enabled:false,fpsCap:0}}}));
  expect(await page.evaluate(()=>window.__readmdLive2d.app.ticker.started)).toBe(false);
  await page.setViewportSize({width:480,height:560});
  await dispatch(page,{...input(1,[4],1,1,1),pointer_x:0.8});
  await expect.poll(()=>page.evaluate(()=>window.__readmdLive2d.parameters.keyboard)).toBeGreaterThan(0);
  expect(await page.evaluate(()=>{
    const canvas=document.querySelector('#live2d-stage canvas'),pixels=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;
    let visible=0;for(let i=3;i<pixels.length;i+=4)if(pixels[i]>127)visible++;
    return visible;
  })).toBeGreaterThan(8000);
  expect(await page.evaluate(()=>window.__readmdLive2d.app.ticker.started)).toBe(false);
});
