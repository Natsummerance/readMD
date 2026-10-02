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
function input(sequence, keys = [], buttons = 0, keyboard = 0, mouse = 0, last = keys.at(-1)) {
  return { sequence, pressed_keys: keys, keyboard_down: !!keys.length, left_down: !!keys.length,
    right_down: false, mouse_down: !!buttons, mouse_buttons: buttons, keyboard_taps: keyboard,
    left_taps: keyboard, right_taps: 0, mouse_taps: mouse, last_key: last, pointer_x: 0, pointer_y: 0 };
}
async function dispatch(page, payload) { await page.evaluate(p => window.__readmdRustDispatch.bongoInput(p), payload); }
async function held(page) {
  return page.evaluate(() => { const s = window.__bongoPet.state;
    return { left: s.leftDown, right: s.rightDown, keys: [...s.pressedKeys], buttons: s.mouseButtons, taps: s.tapCounts }; });
}
async function screenshot(page, testInfo, name) {
  await page.screenshot({ path: process.env.READMD_PET_SCREENSHOTS ? path.join(process.env.READMD_PET_SCREENSHOTS, name) : testInfo.outputPath(name), omitBackground: true });
}
test('BongoCat modifier chords, independent mouse buttons and releases', async ({ page }) => {
  await mount(page); await page.evaluate(() => window.__bongoPet.useClassic());
  await dispatch(page, input(1, [0xe0], 0, 1));
  await dispatch(page, input(2, [0xe0, 4], 0, 2));
  await dispatch(page, input(3, [0xe0, 4], 3, 2, 2));
  await dispatch(page, input(4, [0xe0], 2, 2, 2, 4));
  expect(await held(page)).toEqual({ left: true, right: true, keys: [0xe0], buttons: 2, taps: { left: 2, right: 1 } });
  await dispatch(page, input(5, [0xe0], 0, 2, 2, 4));
  expect((await held(page)).left).toBe(true); expect((await held(page)).right).toBe(false);
  await dispatch(page, input(6, [], 0, 2, 2, 4)); expect((await held(page)).left).toBe(false);
});
test('BongoCat retains 100 fast taps and rejects stale or duplicate local key events', async ({ page }) => {
  await mount(page); await page.evaluate(() => window.__bongoPet.useClassic());
  await page.evaluate(events => {
    for (const event of events) window.__readmdRustDispatch.bongoInput(event);
    window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyA' }));
    window.dispatchEvent(new KeyboardEvent('keyup', { code: 'KeyA' }));
  }, Array.from({ length: 200 }, (_, i) => input(i + 1, i % 2 ? [] : [4], 0, Math.floor(i / 2) + 1, 0, 4)));
  expect(await held(page)).toEqual({ left: false, right: false, keys: [], buttons: 0, taps: { left: 100, right: 0 } });
  await dispatch(page, input(198, [4], 0, 99)); expect((await held(page)).left).toBe(false);
});
test('native drag crosses its threshold once, never writes JS bounds or pets on release', async ({ page }) => {
  await mount(page);
  await page.mouse.move(160, 230); await page.mouse.down(); await page.mouse.move(163, 230);
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'drag-start').length)).toBe(0);
  await page.mouse.move(180, 240); await page.mouse.move(230, 250); await page.mouse.up();
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'drag-start').length)).toBe(1);
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'bounds').length)).toBe(0);
  expect(await page.evaluate(() => window.__bongoPet.state.pettingLevel)).toBe(0);
  await page.evaluate(() => window.__readmdRustDispatch.control({ type: 'pet' }));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.row)).toBe(1);
});
test('cancelled pointer capture clears the drag without a pet reaction', async ({ page }) => {
  await mount(page); await page.mouse.move(160, 230); await page.mouse.down();
  await page.evaluate(() => window.dispatchEvent(new PointerEvent('pointercancel', { pointerId: 1 })));
  await page.mouse.move(200, 250); await page.mouse.up();
  expect(await page.evaluate(() => window.__petCommands.filter(c => c.type === 'drag-start').length)).toBe(0);
  expect(await page.evaluate(() => window.__bongoPet.state.pettingLevel)).toBe(0);
});
test('renderer ready waits for actual sprite art and hit regions when assets are delayed', async ({ page }) => {
  await mount(page,'mochi','hermes-sprite',500);
  expect(await page.evaluate(() => window.__firstReadyHasSprite)).toBe(true);
  expect(await page.evaluate(() => {
    const commands=window.__petCommands, ready=commands.findIndex(c=>c.payload?.type==='renderer-ready');
    return commands.slice(0,ready).some(c=>c.payload?.type==='interaction-regions' && c.payload.rects.length>0);
  })).toBe(true);
  expect(await page.evaluate(() => window.__petCommands.some(c=>c.payload?.type==='state-request'))).toBe(true);
  expect(await page.evaluate(() => window.__petCommands.some(c=>c.payload?.type==='ready'))).toBe(false);
});
test('rest uses the cats original closed-eye frame and deliberate petting wakes its own wave', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => window.readmdLive2dLife.setMood('sleeping'));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.frameIndex)).toBe(2);
  expect(await page.evaluate(() => window.__bongoPet.spritePose.row)).toBe(0);
  await page.evaluate(() => window.__readmdRustDispatch.control({type:'pet'}));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.row)).toBe(1);
  expect(await page.evaluate(() => window.__bongoPet.state.mood)).toBe('normal');
});
test('native hit regions account for WebView zoom and track a resized window', async ({ page }) => {
  await mount(page);
  for (const [width,height] of [[256,337],[512,337]]) {
    await page.setViewportSize({width,height});
    await page.evaluate(width => window.__readmdRustDispatch.state({bounds:{x:200,y:100,width,height:420}}),width*1.25);
    await page.waitForTimeout(100);
    const mapping = await page.evaluate(() => {
      const command=window.__petCommands.filter(c=>c.payload?.type==='interaction-regions').at(-1).payload;
      return {css:window.__bongoPet.interactionRegions.head,native:command.head};
    });
    expect(mapping.native.x).toBeCloseTo(mapping.css.x*1.25,0);
    expect(mapping.native.width).toBeCloseTo(mapping.css.width*1.25,1);
    expect(mapping.native.y).toBeCloseTo(mapping.css.y*420/height,0);
  }
});
for (const character of ['mochi','hermes','amber','moss','cache-capy','niu-lai']) {
  test(`${character} preserves complete original art, reacts with authored frames and has no keyboard`, async ({ page }, testInfo) => {
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await mount(page, character);
    await dispatch(page, input(1, [4, 0xe0], 3, 2, 2));
    expect((await held(page)).taps).toEqual({ left: 0, right: 0 });
    await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('idle');
    expect(await page.locator('#pet-instruments-canvas').count()).toBe(0);
    expect(await page.evaluate(() => !!window.__bongoClassic)).toBe(false);
    for (const [width, height] of [[320,420],[480,560]]) {
      await page.setViewportSize({ width, height }); await page.waitForTimeout(120);
      // Compare the actual complete rendered sprite with its source frame.
      // This catches cropped feet/props, copied limbs and injected instruments.
      expect(await page.evaluate(() => {
        const actual = document.querySelector('#bongocat-canvas'), p = window.__bongoPet.spritePose;
        const expected = document.createElement('canvas'); expected.width = actual.width; expected.height = actual.height;
        const c = expected.getContext('2d'), s = Math.min(innerWidth / 320, innerHeight / 380), dpr = actual.width / innerWidth;
        c.setTransform(dpr*s,0,0,dpr*s,dpr*(innerWidth-320*s)/2,dpr*(innerHeight-380*s)); c.imageSmoothingEnabled = false;
        c.drawImage(p.img,p.sx,p.sy,p.fw,p.fh,p.dx,p.dy,p.dw,p.dh);
        const a = actual.getContext('2d').getImageData(0,0,actual.width,actual.height).data;
        const b = c.getImageData(0,0,expected.width,expected.height).data;
        return a.every((byte,i) => byte === b[i]);
      })).toBe(true);
      const regions = await page.evaluate(() => window.__bongoPet.interactionRegions);
      expect(regions.rects).toHaveLength(1); const r = regions.rects[0];
      expect(r.x).toBeGreaterThan(0); expect(r.y).toBeGreaterThan(0);
      expect(r.x+r.width).toBeLessThan(width); expect(r.y+r.height).toBeLessThan(height);
      await screenshot(page,testInfo,`pet-mechanics-${character}-${width}x${height}.png`);
    }
    await page.evaluate(() => window.__readmdRustDispatch.control({ type: 'pet' }));
    await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.row)).toBe(character === 'niu-lai' || character === 'cache-capy' ? 3 : 1);
    await screenshot(page,testInfo,`pet-mechanics-${character}-response.png`);
    await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('idle');
    expect(errors).toEqual([]);
  });
}
test('BongoCat original Cubism keyboard, mouse and pressed-key artwork still respond', async ({ page }, testInfo) => {
  await mount(page); await page.evaluate(() => window.__bongoPet.useClassic());
  await dispatch(page,{ ...input(1,[4,0xe0],3,2,2,4),pointer_x:.7,pointer_y:-.5 });
  await expect.poll(() => page.evaluate(() => ({ ...window.__bongoClassic.parameters,
    a: window.__bongoClassic.overlays.get(4)?.visible, ctrl: window.__bongoClassic.overlays.get(0xe0)?.visible })))
    .toMatchObject({ CatParamLeftHandDown:1,ParamMouseLeftDown:1,ParamMouseRightDown:1,a:true,ctrl:true });
  await screenshot(page,testInfo,'pet-mechanics-bongocat.png');
  await dispatch(page,input(2,[0xe0],2,2,2,4));
  await expect.poll(() => page.evaluate(() => window.__bongoClassic.parameters))
    .toMatchObject({ CatParamLeftHandDown:1,ParamMouseLeftDown:0,ParamMouseRightDown:1 });
  await dispatch(page,input(3,[],0,2,2,4));
  await expect.poll(() => page.evaluate(() => window.__bongoClassic.parameters))
    .toMatchObject({ CatParamLeftHandDown:0,ParamMouseLeftDown:0,ParamMouseRightDown:0 });
  await dispatch(page,input(4,[0xe7],0,3,2,0xe7));
  await expect.poll(() => page.evaluate(() => window.__bongoClassic.overlays.get(0xe3)?.visible)).toBe(true);
  await page.evaluate(() => window.__bongoPet.applyCharacter('mochi'));
  expect(await page.evaluate(() => window.__bongoClassic.app.ticker.started)).toBe(false);
  expect(await page.evaluate(() => window.__bongoPet.presentation)).toBe('mochi');
});
for (const kind of ['hermes-sprite','live2d']) {
  test(`${kind} Live2D keeps its whole portrait, gaze, blush and original gesture`, async ({ page }, testInfo) => {
    await mount(page,'arch-chan',kind);
    await dispatch(page,{ ...input(1,[4,0xe0],3,2,2),pointer_x:.9,pointer_y:-.7 });
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.ParamEyeBallX)).toBeGreaterThan(.6);
    expect(await page.evaluate(() => window.__readmdLive2d.parameters.MouseToggle)).toBe(0);
    expect(await page.evaluate(() => window.__readmdLive2d.parameters.keyboard)).toBeUndefined();
    for (const [width,height] of [[320,420],[480,560]]) {
      await page.setViewportSize({ width,height }); await page.waitForTimeout(150);
      const layout = await page.evaluate(() => {
        const b = window.__readmdLive2d; return { height:b.model.height,y:b.model.y,bottom:b.model.y+b.model.height,
          head:b.interactionRegions().head,regions:window.__bongoPet.interactionRegions };
      });
      expect(layout.height).toBeGreaterThan(50); expect(layout.height).toBeLessThan(height*.5); expect(layout.y).toBeGreaterThanOrEqual(45);
      expect(layout.bottom).toBeLessThan(height); expect(layout.regions.rects).toHaveLength(1);
      expect(layout.regions.head).toEqual(layout.head);
      expect(await page.locator('#live2d-stage canvas').count()).toBe(1);
      await screenshot(page,testInfo,`pet-mechanics-arch-${kind}-${width}x${height}.png`);
    }
    await page.evaluate(() => window.__readmdRustDispatch.control({ type:'pet' }));
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.MouseToggle)).toBe(1);
    await screenshot(page,testInfo,`pet-mechanics-arch-${kind}-response.png`);
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.MouseToggle)).toBe(0);
    await page.evaluate(() => window.__bongoPet.applyCharacter('mochi'));
    await expect.poll(() => page.evaluate(() => window.__readmdLive2d.app.ticker.started)).toBe(false);
  });
}
test('task state selects authored thought/failure poses and completion settles after one response', async ({ page }) => {
  await mount(page,'niu-lai');
  await page.evaluate(() => window.__readmdRustDispatch.state({ activity:{busy:true} }));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('review');
  await page.evaluate(() => window.__readmdRustDispatch.state({ activity:{error:true} }));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('failed');
  await page.evaluate(() => window.__readmdRustDispatch.state({ activity:{justCompleted:true} }));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('jumping');
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('idle');
  await page.evaluate(() => { for(let i=0;i<5;i++)window.__readmdRustDispatch.state({activity:{justCompleted:true}}); });
  expect(await page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('idle');
});
test('water capybara and cow use original look frames without reacting to keys', async ({ page }) => {
  await mount(page,'cache-capy');
  await dispatch(page,{ ...input(1),pointer_x:1 });
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('look');
  const a = await page.evaluate(() => window.__bongoPet.spritePose.frameIndex);
  await dispatch(page,{ ...input(2),pointer_y:-1 });
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.frameIndex)).not.toBe(a);
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('idle');
});
test('imported sheets retain their own named rows, frame counts and complete art', async ({ page }) => {
  await mount(page);
  await page.evaluate(() => {
    const sheet = document.createElement('canvas'); sheet.width=120;sheet.height=90;
    const c = sheet.getContext('2d');
    for(let row=0;row<3;row++)for(let col=0;col<4;col++){ c.fillStyle=['#ffaa22','#2277ff','#22cc66'][row];c.fillRect(col*30+3,row*30+3,24,24); }
    window.__readmdRustDispatch.state({ info:{ character:'own-sheet',frameW:30,frameH:30,stateRows:['idle','waving','review'],
      framesByRow:{review:2},loopMs:200,spritesheetBase64:sheet.toDataURL().split(',')[1],mime:'image/png' } });
  });
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose?.fw)).toBe(30);
  await dispatch(page,input(1,[4],0,1));
  expect(await page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('idle');
  await page.evaluate(() => window.__readmdRustDispatch.state({activity:{busy:true}}));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.action)).toBe('review');
  const p = await page.evaluate(() => { const p=window.__bongoPet.spritePose;return {row:p.row,fh:p.fh,frame:p.frameIndex}; });
  expect(p).toMatchObject({row:2,fh:30}); expect(p.frame).toBeLessThan(2);
  await page.evaluate(() => window.__bongoPet.petPet());
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose.row)).toBe(1);
});
test('host-delivered sprite is decoded once and switching characters does not reuse another sheet metadata', async ({ page }) => {
  await mount(page,'niu-lai');
  const info = { character:'niu-lai',frameW:192,frameH:208,mime:'image/webp',
    spritesheetBase64:fs.readFileSync(path.join(repo,'assets/pet/niu-lai/spritesheet.webp')).toString('base64') };
  await page.evaluate(info => window.__readmdRustDispatch.state({info}),info);
  await expect.poll(() => page.evaluate(() => !!window.__bongoPet.spritePose.img.readmdImported)).toBe(true);
  await page.evaluate(() => window.__originalSheetImage=window.__bongoPet.spritePose.img);
  await page.evaluate(info => { for(let i=0;i<5;i++)window.__readmdRustDispatch.state({info}); },info);
  expect(await page.evaluate(() => window.__originalSheetImage===window.__bongoPet.spritePose.img)).toBe(true);
  await page.evaluate(() => window.__bongoPet.applyCharacter('mochi'));
  await expect.poll(() => page.evaluate(() => window.__bongoPet.spritePose?.fw)).toBe(384);
});
test('quiet Live2D paints its full WebGL portrait and responds once without restarting idle animation', async ({ page }) => {
  await mount(page,'arch-chan','live2d');
  await page.evaluate(() => window.__readmdRustDispatch.state({info:{character:'arch-chan',animation:{enabled:false,fpsCap:0}}}));
  expect(await page.evaluate(() => window.__readmdLive2d.app.ticker.started)).toBe(false);
  await page.setViewportSize({width:480,height:560});
  await dispatch(page,{ ...input(1),pointer_x:.8 });
  await expect.poll(() => page.evaluate(() => window.__readmdLive2d.parameters.ParamEyeBallX)).toBeGreaterThan(.6);
  expect(await page.evaluate(() => {
    const b=window.__readmdLive2d;b.app.renderer.render(b.app.stage); const pixels=b.app.renderer.extract.pixels();
    let visible=0;for(let i=3;i<pixels.length;i+=4)if(pixels[i]>127)visible++;return visible;
  })).toBeGreaterThan(2000);
  await page.evaluate(() => window.__readmdRustDispatch.control({type:'pet'}));
  expect(await page.evaluate(() => window.__readmdLive2d.parameters.MouseToggle)).toBe(1);
  expect(await page.evaluate(() => window.__readmdLive2d.app.ticker.started)).toBe(false);
});
