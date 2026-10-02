const { test, expect } = require('@playwright/test');
const path = require('node:path');

test.beforeEach(async ({ page, request }) => {
  await request.post('/api/pets/configure',{data:{enabled:false,in_app:true,renderer:'hermes-sprite',character:'',scale:.22,always_on_top:true,lock_position:false,bubbles:true,quiet:false}});
  await page.addInitScript(()=>localStorage.setItem('readmd_language','en'));
  await page.goto('/');
  await page.waitForFunction(()=>typeof window.openPetSettings==='function');
  await page.evaluate(()=>window.openPetSettings());
});

test('window options, tiny size and quiet mode persist through the real Rust API and reload',async({page})=>{
  await page.locator('[data-pet-section=settings]').click();
  await page.locator('label:has(#pet-topmost)').click();
  await page.locator('label:has(#pet-lock-position)').click();
  await page.locator('#pet-scale').evaluate(el=>{el.value='8';el.dispatchEvent(new Event('input',{bubbles:true}));});
  await page.locator('#pet-scale').dispatchEvent('change');
  await page.locator('[data-pet-section=companion]').click();
  await page.locator('label:has(#pet-bubble-toggle)').click();
  await page.locator('#pet-mode-quiet').click();
  await expect.poll(async()=>{
    const s=await page.evaluate(()=>window.fetchPetRuntimeStatus());
    return {scale:s.preferences.scale,top:s.preferences.always_on_top,lock:s.preferences.lock_position,quiet:s.preferences.quiet,bubbles:s.preferences.bubbles};
  },{timeout:25000}).toEqual({scale:.08,top:false,lock:true,quiet:true,bubbles:false});
  await page.reload();await page.waitForFunction(()=>typeof window.openPetSettings==='function');
  await page.evaluate(()=>window.openPetSettings());
  await page.locator('[data-pet-section=settings]').click();
  await expect(page.locator('#pet-topmost')).not.toBeChecked();
  await expect(page.locator('#pet-lock-position')).toBeChecked();
  await expect(page.locator('#pet-scale')).toHaveValue('8');
  await page.locator('[data-pet-section=companion]').click();
  await expect(page.locator('#pet-mode-quiet')).toHaveAttribute('aria-pressed','true');
  await expect(page.locator('#pet-bubble-toggle')).not.toBeChecked();
});

for(const [width,height] of [[1160,820],[1024,680]]) test(`pet settings and original companion preview fit ${width}×${height}`,async({page},testInfo)=>{
  await page.setViewportSize({width,height});
  await expect.poll(()=>page.locator('#pet-preview-character').evaluate(el=>getComputedStyle(el).backgroundImage.startsWith('url("data:image/png'))).toBe(true);
  const preview=await page.locator('#pet-preview-character').evaluate(async el=>{
    const img=new Image();img.src=getComputedStyle(el).backgroundImage.slice(5,-2);await img.decode();return {width:img.width,height:img.height,size:getComputedStyle(el).backgroundSize};
  });
  expect(preview.width).toBeLessThan(preview.height);expect(preview.size).toBe('contain');
  for(const tab of ['characters','companion','settings']) {
    await page.locator(`[data-pet-section=${tab}]`).click();
    const modal=await page.locator('#pet-settings-box').boundingBox();
    expect(modal.x).toBeGreaterThanOrEqual(0);expect(modal.y).toBeGreaterThanOrEqual(0);
    expect(modal.y+modal.height).toBeLessThanOrEqual(height);
    const controls=page.locator('#pet-settings-box select:visible, #pet-settings-box button:visible:not(.pet-character-card), #pet-settings-box input[type=range]:visible');
    for(let n=0;n<await controls.count();n++) {
      const control=controls.nth(n);await control.scrollIntoViewIfNeeded();
      const b=await control.boundingBox();expect(b.height).toBeGreaterThanOrEqual(44);
      expect(await control.evaluate(el=>{const r=el.getBoundingClientRect(),at=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return at===el || el.contains(at);})).toBe(true);
    }
    if(tab==='settings') {
      const values=page.locator('#pet-settings-box .apple-range-val');
      for(let n=0;n<await values.count();n++) {
        const b=await values.nth(n).boundingBox();expect(b.x+b.width).toBeLessThanOrEqual(modal.x+modal.width-24);
      }
    }
  }
  if(process.env.READMD_PET_SCREENSHOTS) await page.screenshot({path:path.join(process.env.READMD_PET_SCREENSHOTS,`pet-settings-${width}x${height}.png`)});
});
