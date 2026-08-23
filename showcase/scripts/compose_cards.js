#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const { chromium } = require('../../ui-tests/node_modules/@playwright/test');
const { buildCardHtml, coverFeedReadiness, drawnImageBox, imageSrc, planCards } = require('../compose_lib.cjs');

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
      const designAudit = await page.evaluate(() => {
        const luminance = (rgb) => {
          const [r, g, b] = rgb.map((value) => {
            const channel = value / 255;
            return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
          });
          return 0.2126 * r + 0.7152 * g + 0.0722 * b;
        };
        const parseColor = (value) => (value.match(/[\d.]+/g) || []).slice(0, 3).map(Number);
        const contrast = (front, back) => {
          const a = luminance(parseColor(front));
          const b = luminance(parseColor(back));
          return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
        };
        const backgroundOf = (element) => {
          let current = element;
          while (current instanceof Element) {
            const background = getComputedStyle(current).backgroundColor;
            if (!/rgba\(\s*0,\s*0,\s*0,\s*0\s*\)/.test(background)) return background;
            current = current.parentElement;
          }
          return 'rgb(242,244,246)';
        };
        const contrastErrors = [];
        const smallText = [];
        for (const element of document.querySelectorAll('h1,h2,p,li,strong,span')) {
          if (!element.innerText.trim() || element.children.length > 0) continue;
          const style = getComputedStyle(element);
          const fontSize = Number.parseFloat(style.fontSize);
          const ratio = contrast(style.color, backgroundOf(element));
          const largeText = fontSize >= 24 || (fontSize >= 19 && Number.parseInt(style.fontWeight, 10) >= 700);
          if (ratio < (largeText ? 3 : 4.5)) contrastErrors.push(`${element.innerText.slice(0, 20)}:${ratio.toFixed(2)}`);
          if (fontSize < 22) smallText.push(`${element.innerText.slice(0, 20)}:${fontSize}px`);
        }
        const imagesFailed = [...document.images].filter((image) => !image.complete || image.naturalWidth === 0).map((image) => image.alt);
        return { contrast_errors: contrastErrors, small_text: smallText, images_failed: imagesFailed };
      });
      const auditErrors = [...designAudit.contrast_errors, ...designAudit.small_text, ...designAudit.images_failed];
      if (auditErrors.length) throw new Error(`${card.file} design audit: ${auditErrors.join('; ')}`);
      const screenshotBox = await page.evaluate(() => {
        const image = document.querySelector('img');
        if (!image) return null;
        const bounds = image.getBoundingClientRect();
        const style = getComputedStyle(image);
        return {
          bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height },
          naturalWidth: image.naturalWidth,
          naturalHeight: image.naturalHeight,
          objectFit: style.objectFit,
          objectPosition: style.objectPosition,
        };
      });
      const measuredBox = screenshotBox ? drawnImageBox(screenshotBox) : null;
      const feedReadiness = card.role === 'cover' ? await page.evaluate(() => {
        const title = document.querySelector('.cover h1');
        const caption = document.querySelector('.cover p');
        if (!(title instanceof HTMLElement) || !(caption instanceof HTMLElement)) return {};
        const range = document.createRange();
        range.selectNodeContents(title);
        const rects = [...range.getClientRects()].filter((rect) => rect.width > 0);
        const titleBox = title.getBoundingClientRect();
        return {
          title_font_size: Number.parseFloat(getComputedStyle(title).fontSize),
          title_width_ratio: (rects.length ? Math.max(...rects.map((rect) => rect.width)) : titleBox.width) / 1080,
          title_height_ratio: titleBox.height / 1440,
          caption_font_size: Number.parseFloat(getComputedStyle(caption).fontSize),
        };
      }) : null;
      if (feedReadiness && !coverFeedReadiness(feedReadiness).ok) {
        throw new Error(`${card.file} feed readiness: ${coverFeedReadiness(feedReadiness).failures.join('; ')}`);
      }
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
        ui_area_ratio: measuredBox ? (measuredBox.width * measuredBox.height) / (1080 * 1440) : 0,
        screenshot_box: measuredBox,
        feed_readiness: feedReadiness,
      });
      fs.unlinkSync(htmlPath);
    }
    fs.writeFileSync(
      path.join(packageDir, 'composition.json'),
      JSON.stringify({ schema_version: 1, overflow_errors: [], design_audit: { contrast_errors: [], small_text: [], images_failed: [] }, cards: report }, null, 2),
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
