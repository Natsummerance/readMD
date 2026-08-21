const { test, expect } = require('../ui-tests/node_modules/@playwright/test');
const fs = require('fs');
const path = require('path');

const RAW_DIR = path.resolve(__dirname, 'raw');
if (!fs.existsSync(RAW_DIR)) {
  fs.mkdirSync(RAW_DIR, { recursive: true });
}

const DEMO_MD = fs.readFileSync(path.resolve(__dirname, 'fixtures/readmd-showcase.md'), 'utf-8');

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    try {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd_theme', 'dark');
    } catch (e) {}
  });
  await page.goto('/');
  await page.waitForFunction(() => typeof renderVirtual === 'function' || typeof renderMarkdown === 'function' || typeof state !== 'undefined');
});

async function openDemoInReader(page) {
  await page.evaluate((content) => {
    if (typeof renderVirtual === 'function') {
      renderVirtual(content, 'ReadMD_Research_v2.3.4.md');
    } else if (typeof renderMarkdown === 'function') {
      state.original = content;
      state.fixed = content;
      state.mode = 'view';
      renderMarkdown(content);
    }
  }, DEMO_MD);
  await page.waitForTimeout(600);
}

test('01 Shot: overview.reader (Hero Shot)', async ({ page }) => {
  await openDemoInReader(page);
  // 确保大纲目录展开
  await page.evaluate(() => {
    const outline = document.getElementById('outline-panel');
    if (outline && outline.classList.contains('hidden') && typeof toggleOutline === 'function') {
      toggleOutline();
    }
  });
  await page.waitForTimeout(400);
  const outPath = path.join(RAW_DIR, 'overview-reader.png');
  await page.screenshot({ path: outPath });
  expect(fs.existsSync(outPath)).toBeTruthy();
});

test('02 Shot: overview.editor', async ({ page }) => {
  await openDemoInReader(page);
  await page.evaluate(async () => {
    if (typeof toggleEdit === 'function') await toggleEdit();
    if (typeof setPvLayout === 'function') setPvLayout('split');
  });
  await page.waitForTimeout(500);
  const outPath = path.join(RAW_DIR, 'overview-editor.png');
  await page.screenshot({ path: outPath });
  expect(fs.existsSync(outPath)).toBeTruthy();
});

test('03 Shot: editor.diagram-picker', async ({ page }) => {
  await openDemoInReader(page);
  await page.evaluate(async () => {
    if (typeof toggleEdit === 'function') await toggleEdit();
  });
  await page.waitForTimeout(300);
  const diagramBtn = page.locator('#diagram-open, button[title*="图表"], button[data-action="diagram"], button:has-text("图表")');
  if (await diagramBtn.count() > 0 && await diagramBtn.first().isVisible()) {
    await diagramBtn.first().click();
    await page.waitForTimeout(300);
  }
  const outPath = path.join(RAW_DIR, 'diagram-picker.png');
  await page.screenshot({ path: outPath });
  expect(fs.existsSync(outPath)).toBeTruthy();
});

test('04 Shot: academic.latex-bib', async ({ page }) => {
  await openDemoInReader(page);
  await page.evaluate(() => {
    const mathEl = document.querySelector('.katex-display, .katex, .theorem, blockquote');
    if (mathEl) mathEl.scrollIntoView({ behavior: 'instant', block: 'center' });
  });
  await page.waitForTimeout(400);
  const outPath = path.join(RAW_DIR, 'academic-latex.png');
  await page.screenshot({ path: outPath });
  expect(fs.existsSync(outPath)).toBeTruthy();
});

test('05 Shot: editor.code-chunk', async ({ page }) => {
  await openDemoInReader(page);
  await page.evaluate(() => {
    const codeBlock = document.querySelector('pre, .code-chunk-wrap, code');
    if (codeBlock) codeBlock.scrollIntoView({ behavior: 'instant', block: 'center' });
  });
  await page.waitForTimeout(400);
  const outPath = path.join(RAW_DIR, 'code-chunk.png');
  await page.screenshot({ path: outPath });
  expect(fs.existsSync(outPath)).toBeTruthy();
});

test('06 Shot: modal.dirty-tab', async ({ page }) => {
  await page.evaluate(() => {
    state.isDirty = true;
    if (typeof promptDirtyClose === 'function') {
      promptDirtyClose(null, () => {}, () => {});
    } else {
      const modal = document.getElementById('dirty-modal');
      if (modal) modal.classList.remove('hidden');
    }
  });
  await page.waitForTimeout(400);
  const outPath = path.join(RAW_DIR, 'modal-dirty-tab.png');
  await page.screenshot({ path: outPath });
  expect(fs.existsSync(outPath)).toBeTruthy();
});
