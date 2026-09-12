const { test } = require('@playwright/test');

test('measure tpl-box width and layout', async ({ page }) => {
  await page.goto('http://127.0.0.1:26891/');
  await page.waitForFunction(() => typeof openTplModal === 'function');
  await page.evaluate(() => openTplModal());
  await page.waitForTimeout(300);
  const m = await page.evaluate(() => {
    const box = document.querySelector('#tpl-box');
    const cs = getComputedStyle(box);
    const toolbar = document.querySelector('.tpl-toolbar-actions');
    const layout = document.querySelector('.tpl-layout');
    return {
      boxW: Math.round(box.getBoundingClientRect().width),
      boxStyleWidth: cs.width,
      boxMaxW: cs.maxWidth,
      boxMinW: cs.minWidth,
      boxSizing: cs.boxSizing,
      boxPadding: cs.padding,
      toolbarW: toolbar ? Math.round(toolbar.getBoundingClientRect().width) : null,
      toolbarScrollW: toolbar ? toolbar.scrollWidth : null,
      layoutW: layout ? Math.round(layout.getBoundingClientRect().width) : null,
      layoutMinW: layout ? getComputedStyle(layout).minWidth : null,
      layoutGrid: layout ? getComputedStyle(layout).gridTemplateColumns : null,
      winW: window.innerWidth,
    };
  });
  console.log(JSON.stringify(m, null, 1));
});
