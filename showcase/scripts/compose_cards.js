#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const { chromium } = require('../../ui-tests/node_modules/@playwright/test');
const { buildCardHtml, imageSrc, planCards } = require('../compose_lib.cjs');

async function main() {
  const packageDir = path.resolve(process.argv[2] || 'output/package');
  const story = JSON.parse(fs.readFileSync(path.join(packageDir, 'story.json'), 'utf8'));
  const capture = JSON.parse(fs.readFileSync(path.join(packageDir, 'raw', 'capture.json'), 'utf8'));
  const cards = planCards(story);
  const outputDir = path.join(packageDir, 'images');
  fs.mkdirSync(outputDir, { recursive: true });
  for (const file of fs.readdirSync(outputDir)) if (file.endsWith('.jpg')) fs.unlinkSync(path.join(outputDir, file));

  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1080, height: 1440 }, deviceScaleFactor: 1 });
    const report = [];
    for (const card of cards) {
      const html = buildCardHtml(card, imageSrc(packageDir, capture, card), { release: story.release });
      const htmlPath = path.join(packageDir, `.compose-${card.index}.html`);
      fs.writeFileSync(htmlPath, html, 'utf8');
      await page.goto(`file://${htmlPath.replace(/\\/g, '/')}`);
      await page.waitForLoadState('networkidle');
      const overflowErrors = await page.evaluate(() => {
        const errors = [];
        if (document.documentElement.scrollWidth > window.innerWidth) errors.push('horizontal page overflow');
        if (document.documentElement.scrollHeight > window.innerHeight) errors.push('vertical page overflow');
        for (const element of document.querySelectorAll('*')) {
          if (!(element instanceof HTMLElement) || !element.innerText.trim()) continue;
          if (element.scrollWidth > element.clientWidth + 1) errors.push(`text overflow: ${element.innerText.slice(0, 24)}`);
        }
        return errors;
      });
      if (overflowErrors.length) throw new Error(`${card.file}: ${overflowErrors.join('; ')}`);
      const screenshotBox = await page.evaluate(() => {
        const image = document.querySelector('img');
        if (!image) return null;
        const box = image.getBoundingClientRect();
        return { x: Math.max(0, box.x), y: Math.max(0, box.y), width: box.width, height: box.height };
      });
      await page.screenshot({
        path: path.join(outputDir, card.file),
        type: 'jpeg',
        quality: 92,
        clip: { x: 0, y: 0, width: 1080, height: 1440 },
      });
      report.push({
        file: card.file,
        role: card.role,
        ui_min_ratio: card.uiMinRatio,
        ui_area_ratio: screenshotBox ? (screenshotBox.width * screenshotBox.height) / (1080 * 1440) : 0,
        screenshot_box: screenshotBox,
      });
      fs.unlinkSync(htmlPath);
    }
    fs.writeFileSync(
      path.join(packageDir, 'composition.json'),
      JSON.stringify({ schema_version: 1, overflow_errors: [], cards: report }, null, 2),
    );
    const metadataPath = path.join(packageDir, 'metadata.json');
    const metadata = JSON.parse(fs.readFileSync(metadataPath, 'utf8'));
    metadata.images = cards.map((card) => path.join(outputDir, card.file));
    fs.writeFileSync(metadataPath, JSON.stringify(metadata, null, 2), 'utf8');
    console.log(`Composed ${cards.length} cards`);
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
