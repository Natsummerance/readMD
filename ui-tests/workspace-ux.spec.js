const { test, expect } = require('@playwright/test');
test.beforeEach(async ({ page }) => {
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.ReadMDGraph && typeof openSkillIdeaDialog === 'function');
});
test('Skill form validates inline, restores draft and submits chosen output', async ({ page }) => {
  await page.evaluate(() => { window.skillResult = undefined; openSkillIdeaDialog().then(v => window.skillResult = v); });
  await page.locator('#skill-create-go').click();
  await expect(page.locator('#skill-create-error')).not.toBeEmpty();
  await page.locator('#skill-create-example').click();
  await expect(page.locator('#skill-create-purpose')).not.toHaveValue('');
  await page.locator('#skill-create-purpose').fill('Read and summarize the supplied document.');
  await page.locator('#skill-create-format').selectOption('table');
  await page.locator('#skill-create-cancel').click();
  await page.evaluate(() => { openSkillIdeaDialog().then(v => window.skillResult = v); });
  await expect(page.locator('#skill-create-purpose')).toHaveValue('Read and summarize the supplied document.');
  await page.locator('#skill-create-purpose').press('Control+Enter');
  await expect(page.locator('#skill-create-modal')).toBeHidden();
  expect((await page.evaluate(() => window.skillResult)).purpose).toContain('Read and summarize');
});
test('AI starters fill composer without sending a request', async ({ page }) => {
  await page.evaluate(() => { document.getElementById('ai-panel').classList.remove('hidden'); state.ai.messages=[]; renderAiEmptyState(); updateAiContextStrip(); });
  await page.locator('[data-starter="ux.askQuestions"]').click();
  await expect(page.locator('#ai-prompt')).not.toHaveValue('');
  await expect(page.locator('#ai-prompt')).toBeFocused();
  await expect(page.locator('#ai-context-file')).not.toBeEmpty();
});
const graph={nodes:[{id:'a',path:'C:/notes/Alpha.md',label:'Alpha',degree:1},{id:'b',path:'C:/notes/Beta.md',label:'Beta',degree:1},{id:'c',path:'',label:'Missing',is_deadlink:true}],edges:[{source:'a',target:'b'}]};
test('3D graph rotates, switches dimension and searches accessible nodes', async ({ page }) => {
  await page.route('**/api/links/graph?**',r=>r.fulfill({json:{ok:true,graph}}));
  await page.evaluate(async()=>{state.mode='file';state.original='[[Beta]]';await ReadMDGraph.open('C:/notes');});
  const canvas=page.locator('#graph-canvas');
  await expect(canvas).toHaveAttribute('data-node-count','3');
  const before=await canvas.getAttribute('data-yaw');
  await canvas.focus();await canvas.press('ArrowRight');
  expect(await canvas.getAttribute('data-yaw')).not.toBe(before);
  await page.locator('#graph-btn-dimension').click();
  await expect(canvas).toHaveAttribute('data-dimensions','2');
  await page.locator('#graph-search').fill('Beta');
  await expect(page.locator('.graph-node-row')).toHaveCount(1);
  await page.locator('.graph-node-row').click();
  await expect(page.locator('#graph-detail')).toContainText('Beta');
  const overflow=await page.locator('.graph-dialog').evaluate(el=>el.scrollWidth>el.clientWidth+1);
  expect(overflow).toBe(false);
});
test('Backlinks filters, switches tabs and exposes unresolved targets',async({page})=>{
  await page.route('**/api/links/backlinks?**',r=>r.fulfill({json:{ok:true,backlinks:[{source_title:'Research',source_path:'C:/notes/Research.md',line_no:9,alias:'Useful context'}],forward_links:[{target_clean:'Missing',target_path:null}]}}));
  await page.evaluate(async()=>{state.mode='file';state.original='[[Missing]]';await ReadMDGraph.refreshBacklinks('C:/notes/A.md');ReadMDGraph.toggleDrawer();});
  await expect(page.locator('.backlink-item')).toContainText('Research');
  await page.locator('#backlinks-search').fill('absent');
  await expect(page.locator('.backlink-item')).toHaveCount(0);
  await page.locator('#backlinks-search').fill('');
  await page.locator('[data-tab="outgoing"]').click();
  await expect(page.locator('.backlink-item')).toBeDisabled();
});
