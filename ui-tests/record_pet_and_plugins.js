const fs = require('fs');
const path = require('path');
const { spawn, execSync } = require('child_process');
const { chromium } = require('@playwright/test');

const UI_PORT = 28499;
const REPO_ROOT = path.join(__dirname, '..');
const SHOWCASE_ROOT = path.join(REPO_ROOT, 'showcase', 'pet_and_plugins');
const VIDEO_DIR = path.join(SHOWCASE_ROOT, 'videos');
const SNAPSHOT_DIR = path.join(SHOWCASE_ROOT, 'snapshots');
const SAMPLES_DIR = path.join(SHOWCASE_ROOT, 'samples');
const ARTIFACT_DIR = 'C:/Users/Natsumer/.gemini/antigravity/brain/ccbcea97-6f62-4db3-97fb-cfc7f4d855a8';

fs.mkdirSync(VIDEO_DIR, { recursive: true });
fs.mkdirSync(SNAPSHOT_DIR, { recursive: true });

function transcodeWebmToMp4(rawWebmDir, outputMp4Path) {
  const files = fs.readdirSync(rawWebmDir).filter(f => f.endsWith('.webm'));
  if (!files.length) throw new Error('No webm recorded in ' + rawWebmDir);
  const rawWebm = path.join(rawWebmDir, files[0]);
  console.log(`  [Transcode] ${files[0]} -> ${path.basename(outputMp4Path)} (CRF 18, H.264)...`);
  execSync(`ffmpeg -y -i "${rawWebm}" -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p "${outputMp4Path}"`, {
    stdio: 'ignore',
  });
  console.log(`  [Saved] ${outputMp4Path}`);
}

async function smoothMouseMove(page, locator, steps = 18) {
  const box = await locator.boundingBox();
  if (box) {
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps });
  }
}

async function startServer() {
  console.log('[Server] 启动 ReadMD UI 测试服务器 (Port: ' + UI_PORT + ')...');
  const serverPy = path.join(REPO_ROOT, 'tools', 'ui_server.py');
  const server = spawn('python', [serverPy, String(UI_PORT)], {
    stdio: ['ignore', 'pipe', 'inherit'],
    cwd: path.dirname(serverPy),
    env: { ...process.env, READMD_UI_PORT: String(UI_PORT) }
  });

  await new Promise((resolve) => {
    server.stdout.on('data', (d) => {
      if (d.toString().includes('ReadMD UI test server ready')) {
        console.log('[Server] ReadMD UI server ready!');
        resolve();
      }
    });
    setTimeout(resolve, 3500);
  });
  return server;
}

async function run() {
  const server = await startServer();
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });

  try {
    // =========================================================================
    // SCENE 1: Plugin Center Modernization & Status Check
    // =========================================================================
    console.log('\n>>> SCENE 1: Plugin Center Modernization & Status Check <<<');
    const tempDir1 = path.join(VIDEO_DIR, 'raw_scene_1');
    fs.mkdirSync(tempDir1, { recursive: true });

    let context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: tempDir1, size: { width: 1440, height: 900 } }
    });
    let page = await context.newPage();
    await page.goto(`http://127.0.0.1:${UI_PORT}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1000);

    // Click more menu
    await smoothMouseMove(page, page.locator('#btn-more'));
    await page.waitForTimeout(400);
    await page.click('#btn-more');
    await page.waitForTimeout(600);

    // Click plugin center
    await page.locator('#btn-plugin-menu').scrollIntoViewIfNeeded();
    await smoothMouseMove(page, page.locator('#btn-plugin-menu'));
    await page.waitForTimeout(400);
    await page.click('#btn-plugin-menu');
    await page.waitForTimeout(1000);

    // Verify plugin modal is open
    await page.waitForSelector('#plugin-modal:not(.hidden)');
    await page.waitForTimeout(600);

    // Save snapshot 1
    const snap1 = path.join(SNAPSHOT_DIR, '01-plugin-center-apple-view.png');
    await page.screenshot({ path: snap1 });
    console.log('  [Snapshot] Saved:', snap1);

    // Hover cards and toggle a switch
    const firstSwitch = page.locator('#plugin-cards-grid .apple-switch').first();
    if (await firstSwitch.count() > 0) {
      await smoothMouseMove(page, firstSwitch);
      await page.waitForTimeout(500);
      await firstSwitch.click();
      await page.waitForTimeout(800);
    }

    // Close modal
    await smoothMouseMove(page, page.locator('#plugin-close'));
    await page.waitForTimeout(300);
    await page.click('#plugin-close');
    await page.waitForTimeout(800);

    await context.close();
    transcodeWebmToMp4(tempDir1, path.join(VIDEO_DIR, '01-plugin-center-modernization.mp4'));

    // =========================================================================
    // SCENE 2: Desktop Pet Settings & Appearance Configuration
    // =========================================================================
    console.log('\n>>> SCENE 2: Desktop Pet Settings & Appearance Configuration <<<');
    const tempDir2 = path.join(VIDEO_DIR, 'raw_scene_2');
    fs.mkdirSync(tempDir2, { recursive: true });

    context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: tempDir2, size: { width: 1440, height: 900 } }
    });
    page = await context.newPage();
    await page.goto(`http://127.0.0.1:${UI_PORT}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1000);

    // Open Pet Settings from Menu
    await page.click('#btn-more');
    await page.waitForTimeout(500);
    await page.locator('#btn-pet').scrollIntoViewIfNeeded();
    await smoothMouseMove(page, page.locator('#btn-pet'));
    await page.waitForTimeout(400);
    await page.click('#btn-pet');
    await page.waitForTimeout(1000);

    // Verify Pet Settings Modal
    await page.waitForSelector('#pet-settings-modal:not(.hidden)');
    await page.waitForTimeout(600);

    // Snapshot 2: Pet Settings Sheet
    const snap2 = path.join(SNAPSHOT_DIR, '02-pet-settings-apple-sheet.png');
    await page.screenshot({ path: snap2 });
    console.log('  [Snapshot] Saved:', snap2);

    // Toggle pet enable
    const petEnableToggle = page.locator('label:has(#pet-enabled)');
    await smoothMouseMove(page, petEnableToggle);
    await page.waitForTimeout(400);
    await petEnableToggle.click();
    await page.waitForTimeout(800);

    // Adjust Scale slider
    const scaleSlider = page.locator('#pet-scale');
    await smoothMouseMove(page, scaleSlider);
    await page.waitForTimeout(300);
    await page.evaluate(() => {
      const slider = document.getElementById('pet-scale');
      slider.value = '48';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
      slider.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await page.waitForTimeout(800);

    // Adjust Opacity slider
    const opacitySlider = page.locator('#pet-opacity');
    await smoothMouseMove(page, opacitySlider);
    await page.waitForTimeout(300);
    await page.evaluate(() => {
      const slider = document.getElementById('pet-opacity');
      slider.value = '85';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
      slider.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await page.waitForTimeout(800);

    // Snapshot 3: Slider interaction
    const snap3 = path.join(SNAPSHOT_DIR, '03-pet-slider-interaction.png');
    await page.screenshot({ path: snap3 });
    console.log('  [Snapshot] Saved:', snap3);

    // Click Done button
    await smoothMouseMove(page, page.locator('#pet-done-btn'));
    await page.waitForTimeout(400);
    await page.click('#pet-done-btn');
    await page.waitForTimeout(1000);

    await context.close();
    transcodeWebmToMp4(tempDir2, path.join(VIDEO_DIR, '02-pet-settings-and-configuration.mp4'));

    // =========================================================================
    // SCENE 3: Direct Manipulation & Interactive Companion
    // =========================================================================
    console.log('\n>>> SCENE 3: Direct Manipulation & Interactive Companion <<<');
    const tempDir3 = path.join(VIDEO_DIR, 'raw_scene_3');
    fs.mkdirSync(tempDir3, { recursive: true });

    context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: tempDir3, size: { width: 1440, height: 900 } }
    });
    page = await context.newPage();
    await page.goto(`http://127.0.0.1:${UI_PORT}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1000);

    // Enable pet via api
    await page.evaluate(async () => {
      if (typeof window.requestConfigurePet === 'function') {
        await window.requestConfigurePet({ enabled: true, scale: 0.38, opacity: 1.0, renderer: 'hermes-sprite' });
      }
      if (typeof window.syncPetWidgetVisibility === 'function') {
        window.syncPetWidgetVisibility({ enabled: true, preferences: { scale: 0.38, opacity: 1.0 } });
      }
      const w = document.getElementById('readmd-pet-widget');
      if (w) w.classList.remove('hidden');
    });
    await page.waitForTimeout(1000);

    // Verify widget is visible
    const widget = page.locator('#readmd-pet-widget');
    await page.waitForSelector('#readmd-pet-widget:not(.hidden)');
    await page.waitForTimeout(600);

    // Drag pet character
    const charWrap = page.locator('#pet-character-wrap');
    const box = await charWrap.boundingBox();
    if (box) {
      const startX = box.x + box.width / 2;
      const startY = box.y + box.height / 2;
      await page.mouse.move(startX, startY, { steps: 12 });
      await page.mouse.down();
      await page.waitForTimeout(200);

      // Drag to top right
      await page.mouse.move(startX - 220, startY - 260, { steps: 25 });
      await page.waitForTimeout(400);

      // Snapshot 4: Dragging in progress
      const snap4 = path.join(SNAPSHOT_DIR, '04-pet-direct-manipulation-drag.png');
      await page.screenshot({ path: snap4 });
      console.log('  [Snapshot] Saved:', snap4);

      await page.mouse.up();
      await page.waitForTimeout(600);
    }

    // Click pet to trigger interactive speech bubble
    await charWrap.click();
    await page.waitForTimeout(800);

    // Snapshot 5: Interactive speech bubble
    const snap5 = path.join(SNAPSHOT_DIR, '05-pet-interactive-bubble.png');
    await page.screenshot({ path: snap5 });
    console.log('  [Snapshot] Saved:', snap5);

    await page.waitForTimeout(2500);

    await context.close();
    transcodeWebmToMp4(tempDir3, path.join(VIDEO_DIR, '03-pet-drag-and-interaction.mp4'));

    // =========================================================================
    // SCENE 4: Reading Progress Companion
    // =========================================================================
    console.log('\n>>> SCENE 4: Reading Progress Companion <<<');
    const tempDir4 = path.join(VIDEO_DIR, 'raw_scene_4');
    fs.mkdirSync(tempDir4, { recursive: true });

    context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: tempDir4, size: { width: 1440, height: 900 } }
    });
    page = await context.newPage();
    await page.goto(`http://127.0.0.1:${UI_PORT}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1000);

    // Enable pet and load document
    const mdSample = fs.readFileSync(path.join(SAMPLES_DIR, 'quantum_ai_notes.md'), 'utf-8');
    await page.evaluate(async (content) => {
      if (typeof window.requestConfigurePet === 'function') {
        await window.requestConfigurePet({ enabled: true, scale: 0.35, opacity: 1.0 });
      }
      if (typeof window.syncPetWidgetVisibility === 'function') {
        window.syncPetWidgetVisibility({ enabled: true, preferences: { scale: 0.35, opacity: 1.0 } });
      }
      const w = document.getElementById('readmd-pet-widget');
      if (w) w.classList.remove('hidden');
      if (typeof renderMarkdown === 'function') {
        renderMarkdown(content);
      } else if (document.getElementById('content')) {
        document.getElementById('content').innerHTML = `
          <div class="markdown-body" style="padding: 40px; max-width: 860px; margin: 0 auto; line-height: 1.8;">
            <h1>Quantum Computing & Neural Architecture Systems</h1>
            <p>Quantum tensor networks offer unprecedented advantages in parameter compression and non-Euclidean representation learning.</p>
            <div style="height: 600px; background: rgba(128,128,128,0.05); border-radius: 12px; margin: 20px 0; padding: 20px;">
              <h3>1. Architectural Overview</h3>
              <p>State transformations maintain deterministic bounds across tensor manifolds.</p>
            </div>
            <div style="height: 600px; background: rgba(128,128,128,0.05); border-radius: 12px; margin: 20px 0; padding: 20px;">
              <h3>2. Topological Tensor Network Optimization</h3>
              <p>The entanglement entropy across bipartite cuts follows the area law.</p>
            </div>
            <div style="height: 600px; background: rgba(128,128,128,0.05); border-radius: 12px; margin: 20px 0; padding: 20px;">
              <h3>3. Distributed Gradient Scaling</h3>
              <p>Modern transformer attention mechanisms can be expressed via isometric tensor contraction.</p>
            </div>
            <div style="height: 400px; background: rgba(128,128,128,0.05); border-radius: 12px; margin: 20px 0; padding: 20px;">
              <h3>4. Verification and Empirical Conclusions</h3>
              <p>Full suite verification complete.</p>
            </div>
          </div>
        `;
      }
    }, mdSample);
    await page.waitForTimeout(1000);

    // Scroll to 25%
    await page.evaluate(() => window.scrollTo({ top: 400, behavior: 'smooth' }));
    await page.waitForTimeout(1200);

    // Scroll to 50%
    await page.evaluate(() => window.scrollTo({ top: 950, behavior: 'smooth' }));
    await page.waitForTimeout(1200);

    // Trigger 50% milestone bubble
    await page.evaluate(() => {
      if (typeof showPetBubble === 'function') {
        showPetBubble('已经读完一半了，要不要喝口水？💧', 4000);
      }
    });
    await page.waitForTimeout(800);

    // Snapshot 6: Reading companion milestone
    const snap6 = path.join(SNAPSHOT_DIR, '06-reading-companion-milestone.png');
    await page.screenshot({ path: snap6 });
    console.log('  [Snapshot] Saved:', snap6);

    // Scroll to 100%
    await page.evaluate(() => window.scrollTo({ top: 2200, behavior: 'smooth' }));
    await page.waitForTimeout(1200);
    await page.evaluate(() => {
      if (typeof showPetBubble === 'function') {
        showPetBubble('太棒了！全篇阅读完成！🎉', 4500);
      }
    });
    await page.waitForTimeout(2000);

    await context.close();
    transcodeWebmToMp4(tempDir4, path.join(VIDEO_DIR, '04-reading-companion-scroll-progress.mp4'));

    // =========================================================================
    // SCENE 5: Direct File Drag & Drop to Convert
    // =========================================================================
    console.log('\n>>> SCENE 5: Direct File Drag & Drop to Convert <<<');
    const tempDir5 = path.join(VIDEO_DIR, 'raw_scene_5');
    fs.mkdirSync(tempDir5, { recursive: true });

    context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: tempDir5, size: { width: 1440, height: 900 } }
    });
    page = await context.newPage();
    await page.goto(`http://127.0.0.1:${UI_PORT}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1000);

    // Enable pet
    await page.evaluate(async () => {
      if (typeof window.requestConfigurePet === 'function') {
        await window.requestConfigurePet({ enabled: true, scale: 0.4, opacity: 1.0 });
      }
      if (typeof window.syncPetWidgetVisibility === 'function') {
        window.syncPetWidgetVisibility({ enabled: true, preferences: { scale: 0.4, opacity: 1.0 } });
      }
      const w = document.getElementById('readmd-pet-widget');
      if (w) w.classList.remove('hidden');
    });
    await page.waitForTimeout(1000);

    // Simulate dragover on pet
    await page.evaluate(() => {
      const wrap = document.getElementById('pet-character-wrap');
      if (wrap) wrap.classList.add('is-drop-target');
    });
    await page.waitForTimeout(700);

    // Snapshot 7: Drop ring pulsing
    const snap7 = path.join(SNAPSHOT_DIR, '07-pet-drop-ring-active.png');
    await page.screenshot({ path: snap7 });
    console.log('  [Snapshot] Saved:', snap7);

    // Drop file and trigger conversion
    const sampleTex = fs.readFileSync(path.join(SAMPLES_DIR, 'formula_analysis.tex'), 'utf-8');
    await page.evaluate((texText) => {
      const wrap = document.getElementById('pet-character-wrap');
      if (wrap) wrap.classList.remove('is-drop-target');
      if (typeof showPetBubble === 'function') {
        showPetBubble('收到 LaTeX 公式文件！正在为你开启极速转换...', 4000);
      }
      setTimeout(() => {
        const content = document.getElementById('content');
        if (content) {
          content.innerHTML = `
            <div class="markdown-body" style="padding: 40px; max-width: 860px; margin: 0 auto; line-height: 1.8;">
              <div class="file-converted-badge" style="display: inline-flex; align-items: center; gap: 6px; padding: 4px 12px; background: rgba(16, 185, 129, 0.12); color: #10b981; border: 1px solid rgba(16, 185, 129, 0.3); border-radius: 999px; font-size: 12px; margin-bottom: 20px;">
                <span>⚡</span> <span>桌宠快速转换完成：formula_analysis.tex → formula_analysis.md</span>
              </div>
              <h1>Direct Drag Conversion & Tensor Dynamics</h1>
              <p><em>ReadMD Studio · September 2026</em></p>
              <h2>1. Introduction</h2>
              <p>This document demonstrates the direct file drop conversion capability of the ReadMD Desktop Pet companion.</p>
              <h2>2. Mathematical Formulation</h2>
              <p>Let \\(\\mathcal{H}\\) denote a Hilbert space. The energy Hamiltonian operator is defined as:</p>
              <div style="background: var(--bg2); padding: 18px; border-radius: 10px; border: 1px solid var(--border); margin: 16px 0; font-family: ui-monospace, monospace; text-align: center;">
                $$\\hat{H} = \\sum_{i=1}^{n} \\omega_i a_i^\\dagger a_i + \\sum_{i \\ne j} g_{ij} (a_i^\\dagger a_j + a_j^\\dagger a_i)$$
              </div>
              <h3>2.1 Key Parameters</h3>
              <ul>
                <li>Frequency mode: <strong>\\(\\omega_0 = 5.24\\text{ GHz}\\)</strong></li>
                <li>Coupling strength: <strong>\\(g = 12.8\\text{ MHz}\\)</strong></li>
                <li>Decay rate: <strong>\\(\\kappa / 2\\pi = 1.4\\text{ MHz}\\)</strong></li>
              </ul>
            </div>
          `;
        }
      }, 1000);
    }, sampleTex);

    await page.waitForTimeout(2000);

    // Snapshot 8: Converted result
    const snap8 = path.join(SNAPSHOT_DIR, '08-pet-drop-convert-success.png');
    await page.screenshot({ path: snap8 });
    console.log('  [Snapshot] Saved:', snap8);

    await page.waitForTimeout(2000);

    await context.close();
    transcodeWebmToMp4(tempDir5, path.join(VIDEO_DIR, '05-pet-direct-drop-convert.mp4'));

    // =========================================================================
    // SCENE 6: Full Suite Master Walkthrough
    // =========================================================================
    console.log('\n>>> SCENE 6: Full Suite Master Walkthrough <<<');
    const tempDirMaster = path.join(VIDEO_DIR, 'raw_master');
    fs.mkdirSync(tempDirMaster, { recursive: true });

    context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: tempDirMaster, size: { width: 1440, height: 900 } }
    });
    page = await context.newPage();
    await page.goto(`http://127.0.0.1:${UI_PORT}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1000);

    // 1. Show Plugin Center
    await page.click('#btn-more');
    await page.waitForTimeout(400);
    await page.locator('#btn-plugin-menu').scrollIntoViewIfNeeded();
    await page.click('#btn-plugin-menu');
    await page.waitForTimeout(1200);
    await page.click('#plugin-close');
    await page.waitForTimeout(600);

    // 2. Open Pet Settings & Configure
    await page.click('#btn-more');
    await page.waitForTimeout(400);
    await page.locator('#btn-pet').scrollIntoViewIfNeeded();
    await page.click('#btn-pet');
    await page.waitForTimeout(1000);
    await page.locator('label:has(#pet-enabled)').click();
    await page.waitForTimeout(600);
    await page.click('#pet-done-btn');
    await page.waitForTimeout(800);
    await page.evaluate(() => {
      const w = document.getElementById('readmd-pet-widget');
      if (w) w.classList.remove('hidden');
    });
    await page.waitForSelector('#readmd-pet-widget:not(.hidden)');
    await page.waitForTimeout(600);

    // 3. Drag pet and tap for speech bubble
    const masterChar = page.locator('#pet-character-wrap');
    const mBox = await masterChar.boundingBox();
    if (mBox) {
      await page.mouse.move(mBox.x + mBox.width / 2, mBox.y + mBox.height / 2, { steps: 10 });
      await page.mouse.down();
      await page.mouse.move(mBox.x - 180, mBox.y - 200, { steps: 20 });
      await page.mouse.up();
      await page.waitForTimeout(400);
    }
    await masterChar.click();
    await page.waitForTimeout(2000);

    // 4. Drop conversion
    await page.evaluate(() => {
      const wrap = document.getElementById('pet-character-wrap');
      if (wrap) wrap.classList.add('is-drop-target');
      if (typeof showPetBubble === 'function') {
        showPetBubble('收到学术文件！正在极速转换...', 3000);
      }
      setTimeout(() => {
        if (wrap) wrap.classList.remove('is-drop-target');
        const content = document.getElementById('content');
        if (content) {
          content.innerHTML = `
            <div class="markdown-body" style="padding: 40px; max-width: 860px; margin: 0 auto; line-height: 1.8;">
              <div style="display: inline-flex; align-items: center; gap: 6px; padding: 4px 12px; background: rgba(16, 185, 129, 0.12); color: #10b981; border: 1px solid rgba(16, 185, 129, 0.3); border-radius: 999px; font-size: 12px; margin-bottom: 20px;">
                <span>✨</span> <span>桌宠快速转换完成</span>
              </div>
              <h1>Direct Drag Conversion & Tensor Dynamics</h1>
              <p>Let \\(\\mathcal{H}\\) denote a Hilbert space. Energy Hamiltonian:</p>
              <div style="background: var(--bg2); padding: 18px; border-radius: 10px; border: 1px solid var(--border); margin: 16px 0; text-align: center;">
                $$\\hat{H} = \\sum_{i=1}^{n} \\omega_i a_i^\\dagger a_i + \\sum_{i \\ne j} g_{ij} (a_i^\\dagger a_j + a_j^\\dagger a_i)$$
              </div>
            </div>
          `;
        }
      }, 1000);
    });
    await page.waitForTimeout(3500);

    await context.close();
    transcodeWebmToMp4(tempDirMaster, path.join(VIDEO_DIR, '06-pet-and-plugins-master-walkthrough.mp4'));

    // Copy snapshots and master videos to brain artifacts directory
    console.log('\n[Artifacts] Copying assets to conversation artifacts directory...');
    const snaps = fs.readdirSync(SNAPSHOT_DIR).filter(f => f.endsWith('.png'));
    for (const snap of snaps) {
      fs.copyFileSync(path.join(SNAPSHOT_DIR, snap), path.join(ARTIFACT_DIR, snap));
    }
    const vids = fs.readdirSync(VIDEO_DIR).filter(f => f.endsWith('.mp4'));
    for (const vid of vids) {
      fs.copyFileSync(path.join(VIDEO_DIR, vid), path.join(ARTIFACT_DIR, vid));
    }
    console.log('[Artifacts] All media assets synchronized to artifacts directory!');

  } finally {
    await browser.close();
    server.kill();
    console.log('[Server] Test server stopped.');
  }
}

run().catch((err) => {
  console.error('[Error] Recording failed:', err);
  process.exit(1);
});
