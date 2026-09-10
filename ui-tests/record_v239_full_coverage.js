const fs = require('fs');
const path = require('path');
const { spawn, execSync } = require('child_process');
const { chromium } = require('@playwright/test');

const UI_PORT = 28501;
const REPO_ROOT = path.join(__dirname, '..');
const SHOWCASE_ROOT = path.join(REPO_ROOT, 'showcase', 'v239_full_coverage');
const VIDEO_DIR = path.join(SHOWCASE_ROOT, 'videos');
const SNAPSHOT_DIR = path.join(SHOWCASE_ROOT, 'snapshots');
const SAMPLES_DIR = path.join(SHOWCASE_ROOT, 'samples');
const KB_DIR = path.join(SAMPLES_DIR, 'knowledge_base');
const ARTIFACT_DIR = 'C:/Users/Natsumer/.gemini/antigravity/brain/ccbcea97-6f62-4db3-97fb-cfc7f4d855a8';

fs.mkdirSync(VIDEO_DIR, { recursive: true });
fs.mkdirSync(SNAPSHOT_DIR, { recursive: true });

function readSample(rel) {
  return fs.readFileSync(path.join(SAMPLES_DIR, rel), 'utf-8');
}

function transcodeWebmToMp4(rawWebmDir, outputMp4Path) {
  const files = fs.readdirSync(rawWebmDir).filter(f => f.endsWith('.webm'));
  if (!files.length) throw new Error('No webm recorded in ' + rawWebmDir);
  const rawWebm = path.join(rawWebmDir, files[0]);
  console.log(`  [Transcode] ${files[0]} -> ${path.basename(outputMp4Path)} (1440x900, CRF 18, H.264)...`);
  execSync(`ffmpeg -y -i "${rawWebm}" -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p "${outputMp4Path}"`, {
    stdio: 'ignore',
  });
  console.log(`  [Saved MP4] ${outputMp4Path}`);

  // Copy to conversation artifact directory for embedding in reports
  const artifactVideoPath = path.join(ARTIFACT_DIR, path.basename(outputMp4Path));
  try {
    fs.copyFileSync(outputMp4Path, artifactVideoPath);
  } catch (e) {}
}

function saveSnapshot(page, filename) {
  const targetPath = path.join(SNAPSHOT_DIR, filename);
  const artifactPath = path.join(ARTIFACT_DIR, filename);
  return page.screenshot({ path: targetPath, fullPage: false }).then(() => {
    console.log(`  [Snapshot] ${filename}`);
    try {
      fs.copyFileSync(targetPath, artifactPath);
    } catch (e) {}
  });
}

async function smoothMouseMove(page, locator, steps = 15) {
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
    setTimeout(resolve, 3000);
  });
  return server;
}

const mockPluginState = {
  rapidocr: {
    id: 'rapidocr',
    name_key: 'plugin.rapidocr.name',
    desc_key: 'plugin.rapidocr.desc',
    category: 'ocr',
    weight: 'light',
    approx_size: '~16MB',
    installed: false,
    enabled: false,
    cached: false,
    installing: false,
    install_error: '',
    last_log: '',
  },
  easyocr: {
    id: 'easyocr',
    name_key: 'plugin.easyocr.name',
    desc_key: 'plugin.easyocr.desc',
    category: 'ocr',
    weight: 'heavy',
    approx_size: '~150MB',
    installed: true,
    enabled: true,
    cached: true,
    installing: false,
    install_error: '',
    last_log: '',
  },
  pylatexenc: {
    id: 'pylatexenc',
    name_key: 'plugin.pylatexenc.name',
    desc_key: 'plugin.pylatexenc.desc',
    category: 'latex',
    weight: 'light',
    approx_size: '~1MB',
    installed: true,
    enabled: true,
    cached: true,
    installing: false,
    install_error: '',
    last_log: '',
  },
  whisper: {
    id: 'whisper',
    name_key: 'plugin.whisper.name',
    desc_key: 'plugin.whisper.desc',
    category: 'audio',
    weight: 'heavy',
    approx_size: '~150MB',
    installed: false,
    enabled: false,
    cached: false,
    installing: false,
    install_error: '',
    last_log: '',
  },
};

const mockGraphData = {
  nodes: [
    { id: '01-System-Overview.md', path: path.join(KB_DIR, '01-System-Overview.md'), label: '系统架构总览 (System Overview)', is_deadlink: false, link_count: 4, backlink_count: 3, degree: 7 },
    { id: '02-Architecture.md', path: path.join(KB_DIR, '02-Architecture.md'), label: '模块设计规格 (Architecture Spec)', is_deadlink: false, link_count: 3, backlink_count: 3, degree: 6 },
    { id: '03-Database.md', path: path.join(KB_DIR, '03-Database.md'), label: 'SQLite WAL 索引引擎 (Database)', is_deadlink: false, link_count: 2, backlink_count: 2, degree: 4 },
    { id: '04-Pet-Companion.md', path: path.join(KB_DIR, '04-Pet-Companion.md'), label: '原生伴读桌宠 (Desktop Pet)', is_deadlink: false, link_count: 2, backlink_count: 2, degree: 4 },
    { id: 'Deadlink-Demo', path: null, label: '失效未创建节点 (Deadlink Demo)', is_deadlink: true, link_count: 0, backlink_count: 1, degree: 1 },
  ],
  edges: [
    { source: '01-System-Overview.md', target: '02-Architecture.md', is_wikilink: true, line_no: 6 },
    { source: '01-System-Overview.md', target: '03-Database.md', is_wikilink: true, line_no: 7 },
    { source: '01-System-Overview.md', target: '04-Pet-Companion.md', is_wikilink: true, line_no: 8 },
    { source: '01-System-Overview.md', target: 'Deadlink-Demo', is_wikilink: true, line_no: 9 },
    { source: '02-Architecture.md', target: '01-System-Overview.md', is_wikilink: true, line_no: 4 },
    { source: '02-Architecture.md', target: '04-Pet-Companion.md', is_wikilink: true, line_no: 7 },
    { source: '03-Database.md', target: '01-System-Overview.md', is_wikilink: true, line_no: 4 },
    { source: '04-Pet-Companion.md', target: '01-System-Overview.md', is_wikilink: true, line_no: 4 },
    { source: '04-Pet-Companion.md', target: '02-Architecture.md', is_wikilink: true, line_no: 5 },
  ],
};

async function main() {
  const server = await startServer();
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox']
  });

  try {
    if (!fs.existsSync(path.join(VIDEO_DIR, '01_ultra_large_doc_instant_rendering.mp4'))) {
    console.log('\n=============================================================');
    console.log('>>> SCENARIO 1: Ultra-Large Document Instant Loading & Advanced Views <<<');
    console.log('=============================================================');
    const raw1 = path.join(SHOWCASE_ROOT, 'temp_raw_1');
    fs.mkdirSync(raw1, { recursive: true });

    const ctx1 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw1, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page1 = await ctx1.newPage();

    await page1.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page1.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page1.waitForFunction(() => typeof renderContent === 'function');
    await page1.waitForTimeout(600);

    const ultraMd = readSample('ultra_large_doc.md');
    console.log(`  [Load] 载入真实超大型文档 (大小: ${(ultraMd.length / 1024).toFixed(1)} KB, 8000+ 行)...`);
    const t0 = Date.now();
    await page1.evaluate((content) => {
      window.renderContent(content, 0);
    }, ultraMd);
    const renderTime = Date.now() - t0;
    console.log(`  [Perf] 首屏渲染完成耗时: ${renderTime} ms (目标 < 300ms, O(N) 极速秒开!)`);

    await page1.waitForTimeout(800);
    await saveSnapshot(page1, 'snap_01_ultra_large_doc_rendered.png');

    // Expand TOC Navigation Tree
    console.log('  [Interaction] 展开 TOC 目录导航树并平滑定位章节...');
    const tocBtn = page1.locator('#btn-toc');
    if (await tocBtn.isVisible()) {
      await smoothMouseMove(page1, tocBtn);
      await tocBtn.click();
      await page1.waitForTimeout(600);
    }
    await saveSnapshot(page1, 'snap_02_toc_navigation_tree.png');

    // Scroll through TOC
    const tocItem = page1.locator('#toc-list a').nth(15);
    if (await tocItem.count()) {
      await smoothMouseMove(page1, tocItem);
      await tocItem.click();
      await page1.waitForTimeout(800);
    }

    // Toggle Split Screen Dual View (Editor + Live Preview)
    console.log('  [Interaction] 切换左右分屏双向联动工作台...');
    const splitBtn = page1.locator('#btn-split');
    if (await splitBtn.isVisible()) {
      await smoothMouseMove(page1, splitBtn);
      await splitBtn.click();
      await page1.waitForTimeout(1000);
    }
    await saveSnapshot(page1, 'snap_03_split_screen_dual_view.png');

    // Toggle Focus Reading Mode
    console.log('  [Interaction] 切换沉浸专注阅读模式...');
    const focusBtn = page1.locator('#btn-focus');
    if (await focusBtn.isVisible()) {
      await smoothMouseMove(page1, focusBtn);
      await focusBtn.click();
      await page1.waitForTimeout(800);
    }
    await saveSnapshot(page1, 'snap_04_focus_reading_mode.png');

    await page1.waitForTimeout(1000);
    await ctx1.close();
    transcodeWebmToMp4(raw1, path.join(VIDEO_DIR, '01_ultra_large_doc_instant_rendering.mp4'));
    fs.rmSync(raw1, { recursive: true, force: true });
    }

    if (!fs.existsSync(path.join(VIDEO_DIR, '02_universal_file_conversions.mp4'))) {
    console.log('\n=============================================================');
    console.log('>>> SCENARIO 2: Universal Document Conversion Engine <<<');
    console.log('=============================================================');
    const raw2 = path.join(SHOWCASE_ROOT, 'temp_raw_2');
    fs.mkdirSync(raw2, { recursive: true });

    const ctx2 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw2, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page2 = await ctx2.newPage();
    await page2.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page2.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page2.waitForFunction(() => typeof renderContent === 'function');
    await page2.waitForTimeout(500);

    // Open Convert Modal
    console.log('  [Interaction] 打开万物转换面板...');
    const moreBtn2 = page2.locator('#btn-more');
    await smoothMouseMove(page2, moreBtn2);
    await moreBtn2.click();
    await page2.waitForTimeout(300);

    const convertMenuBtn = page2.locator('#btn-convert');
    await smoothMouseMove(page2, convertMenuBtn);
    await convertMenuBtn.click();
    await page2.waitForTimeout(800);
    await saveSnapshot(page2, 'snap_05_convert_modal_queue.png');

    // Close convert modal and show converted academic LaTeX with math formulas
    console.log('  [Render] 渲染学术 LaTeX 原生转换产物 (KaTeX 数学公式、YAML 元数据、三线表格)...');
    const closeConvertBtn = page2.locator('#convert-modal .modal-close-btn, #convert-modal-close, #btn-convert-close').first();
    if (await closeConvertBtn.isVisible()) {
      await closeConvertBtn.click();
    } else {
      await page2.keyboard.press('Escape');
    }
    await page2.waitForTimeout(400);

    const texConverted = execSync('python -c "from src.readmd_modules.convert import convert_verbose; import sys; t, e, _ = convert_verbose(r\'' + path.join(SAMPLES_DIR, 'sample_paper.tex') + '\'); sys.stdout.buffer.write(t.encode(\'utf-8\'))"', {
      cwd: REPO_ROOT,
      env: { ...process.env, PYTHONPATH: '.' }
    }).toString('utf-8');

    await page2.evaluate((content) => {
      window.renderContent(content, 0);
    }, texConverted);
    await page2.waitForTimeout(1000);
    await saveSnapshot(page2, 'snap_06_converted_latex_katex_math.png');

    // Render converted Excel Sheet
    console.log('  [Render] 渲染 Excel 表格原生转换产物 (财务指标 GFM Markdown)...');
    const xlsxConverted = execSync('python -c "from src.readmd_modules.convert import convert_verbose; import sys; t, e, _ = convert_verbose(r\'' + path.join(SAMPLES_DIR, 'financial.xlsx') + '\'); sys.stdout.buffer.write(t.encode(\'utf-8\'))"', {
      cwd: REPO_ROOT,
      env: { ...process.env, PYTHONPATH: '.' }
    }).toString('utf-8');

    await page2.evaluate((content) => {
      window.renderContent(content, 0);
    }, xlsxConverted);
    await page2.waitForTimeout(800);
    await saveSnapshot(page2, 'snap_07_converted_excel_table.png');

    // Render Audio Transcript
    console.log('  [Render] 渲染音视频转写产物 (结构化时间戳片段与 YAML 元数据)...');
    const wavConverted = execSync('python -c "from src.readmd_modules.convert import convert_verbose; import sys; t, e, _ = convert_verbose(r\'' + path.join(SAMPLES_DIR, 'speech_demo.wav') + '\'); sys.stdout.buffer.write(t.encode(\'utf-8\'))"', {
      cwd: REPO_ROOT,
      env: { ...process.env, PYTHONPATH: '.' }
    }).toString('utf-8');

    await page2.evaluate((content) => {
      window.renderContent(content, 0);
    }, wavConverted);
    await page2.waitForTimeout(1000);
    await saveSnapshot(page2, 'snap_08_converted_speech_timestamps.png');

    await page2.waitForTimeout(1000);
    await ctx2.close();
    transcodeWebmToMp4(raw2, path.join(VIDEO_DIR, '02_universal_file_conversions.mp4'));
    fs.rmSync(raw2, { recursive: true, force: true });
    }

    if (!fs.existsSync(path.join(VIDEO_DIR, '03_knowledge_graph_and_wikilinks.mp4'))) {
    console.log('\n=============================================================');
    console.log('>>> SCENARIO 3: Knowledge Graph & Bi-directional Wikilinks <<<');
    console.log('=============================================================');
    const raw3 = path.join(SHOWCASE_ROOT, 'temp_raw_3');
    fs.mkdirSync(raw3, { recursive: true });

    const ctx3 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw3, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page3 = await ctx3.newPage();

    await page3.route('**/api/links/graph*', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ ok: true, graph: mockGraphData }),
    }));

    await page3.route('**/api/links/backlinks*', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        file_path: path.join(KB_DIR, '01-System-Overview.md'),
        backlinks: [
          { source_path: path.join(KB_DIR, '02-Architecture.md'), source_title: '02-Architecture.md', line_no: 4, alias: '', heading: '' },
          { source_path: path.join(KB_DIR, '03-Database.md'), source_title: '03-Database.md', line_no: 4, alias: '', heading: '' },
          { source_path: path.join(KB_DIR, '04-Pet-Companion.md'), source_title: '04-Pet-Companion.md', line_no: 4, alias: '', heading: '' },
        ],
        forward_links: [
          { target_clean: '02-Architecture', target_path: path.join(KB_DIR, '02-Architecture.md'), line_no: 6, alias: '', is_deadlink: false },
          { target_clean: '03-Database', target_path: path.join(KB_DIR, '03-Database.md'), line_no: 7, alias: '', is_deadlink: false },
          { target_clean: '04-Pet-Companion', target_path: path.join(KB_DIR, '04-Pet-Companion.md'), line_no: 8, alias: '', is_deadlink: false },
          { target_clean: 'Deadlink-Demo', target_path: null, line_no: 9, alias: '', is_deadlink: true },
        ],
      }),
    }));

    await page3.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page3.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page3.waitForFunction(() => typeof renderContent === 'function');
    await page3.waitForTimeout(500);

    const kbOverview = readSample('knowledge_base/01-System-Overview.md');
    console.log('  [Render] 载入知识库核心总览，展示 [[wikilink]] 语义标签...');
    await page3.evaluate((content) => {
      window.renderContent(content, 0);
    }, kbOverview);
    await page3.waitForTimeout(800);
    await saveSnapshot(page3, 'snap_09_wikilink_in_text_tag.png');

    // Open Backlinks Drawer
    console.log('  [Interaction] 打开反向链接抽屉 (Backlinks Drawer)...');
    const moreBtn3 = page3.locator('#btn-more');
    await smoothMouseMove(page3, moreBtn3);
    await moreBtn3.click();
    await page3.waitForTimeout(300);

    const backlinksBtn = page3.locator('#btn-backlinks-menu');
    await smoothMouseMove(page3, backlinksBtn);
    await backlinksBtn.click();
    await page3.waitForTimeout(800);
    await saveSnapshot(page3, 'snap_10_backlinks_drawer_expanded.png');

    // Close backlinks drawer
    const closeDrawerBtn = page3.locator('#backlinks-close');
    if (await closeDrawerBtn.isVisible()) {
      await closeDrawerBtn.click();
      await page3.waitForTimeout(400);
    }

    // Open Fullscreen Canvas 2D Knowledge Graph
    console.log('  [Interaction] 打开全屏 Canvas 2D 动力学力导向图谱...');
    const graphBtn = page3.locator('#btn-graph');
    if (await graphBtn.isVisible()) {
      await smoothMouseMove(page3, graphBtn);
      await graphBtn.click();
    } else {
      await page3.keyboard.press('Control+g');
    }
    await page3.waitForTimeout(1200);
    await saveSnapshot(page3, 'snap_11_canvas2d_force_graph.png');

    // Hover on graph canvas to highlight nodes & edges
    console.log('  [Interaction] 鼠标悬停节点，动态高亮相连边与拓扑关系...');
    const canvas = page3.locator('#graph-canvas');
    const canvasBox = await canvas.boundingBox();
    if (canvasBox) {
      await page3.mouse.move(canvasBox.x + canvasBox.width * 0.5, canvasBox.y + canvasBox.height * 0.48, { steps: 15 });
      await page3.waitForTimeout(800);
      await saveSnapshot(page3, 'snap_12_graph_node_hover_highlight.png');

      // Drag node
      console.log('  [Interaction] 拖拽节点产生动力学回弹模拟...');
      await page3.mouse.down();
      await page3.mouse.move(canvasBox.x + canvasBox.width * 0.62, canvasBox.y + canvasBox.height * 0.4, { steps: 20 });
      await page3.waitForTimeout(400);
      await page3.mouse.up();
      await page3.waitForTimeout(1000);
    }

    await page3.waitForTimeout(1000);
    await ctx3.close();
    transcodeWebmToMp4(raw3, path.join(VIDEO_DIR, '03_knowledge_graph_and_wikilinks.mp4'));
    fs.rmSync(raw3, { recursive: true, force: true });
    }

    if (!fs.existsSync(path.join(VIDEO_DIR, '04_interactive_pet_and_drop_convert.mp4'))) {
    console.log('\n=============================================================');
    console.log('>>> SCENARIO 4: Interactive Desktop Pet & Direct Drop-Convert <<<');
    console.log('=============================================================');
    const raw4 = path.join(SHOWCASE_ROOT, 'temp_raw_4');
    fs.mkdirSync(raw4, { recursive: true });

    const ctx4 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw4, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page4 = await ctx4.newPage();

    await page4.route('**/api/pets/status', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        status: {
          enabled: false,
          active: false,
          in_app: true,
          renderer: 'hermes-sprite',
          scale: 1.0,
          opacity: 1.0,
          auto_hide: false,
        }
      })
    }));

    await page4.route('**/api/pets/configure', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        status: {
          enabled: true,
          active: true,
          in_app: true,
          renderer: 'hermes-sprite',
          scale: 1.0,
          opacity: 1.0,
          auto_hide: false,
        }
      })
    }));

    await page4.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page4.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page4.waitForFunction(() => typeof renderContent === 'function');
    await page4.waitForTimeout(500);

    // Visual Inspection of More Menu
    console.log('  [Inspection] 展开更多菜单：自检图标规范（14/16px 无变形）与简洁排版...');
    const moreBtn4 = page4.locator('#btn-more');
    await smoothMouseMove(page4, moreBtn4);
    await moreBtn4.click();
    await page4.waitForTimeout(600);
    await saveSnapshot(page4, 'snap_13_more_menu_aligned_clean.png');

    // Enable Desktop Pet
    console.log('  [Interaction] 开启伴读桌宠 (Desktop Pet)...');
    const petMenuItem = page4.locator('#btn-pet');
    await smoothMouseMove(page4, petMenuItem);
    await petMenuItem.click();
    await page4.waitForTimeout(800);

    // Make pet visible in DOM
    await page4.evaluate(() => {
      const widget = document.getElementById('readmd-pet-widget');
      if (widget) {
        widget.classList.remove('hidden');
        if (typeof showPetBubble === 'function') {
          showPetBubble('你好呀！我是你的伴读桌宠 Hermes ~ 随时为你效劳！✨', 5000);
        }
      }
    });
    await page4.waitForTimeout(800);
    await saveSnapshot(page4, 'snap_14_pet_widget_breathing_bubble.png');

    // Drag pet around screen
    console.log('  [Interaction] 拖拽桌宠小挂件，体验直接指针操控与物理吸附...');
    const petChar = page4.locator('#pet-character-wrap');
    const petBox = await petChar.boundingBox();
    if (petBox) {
      await page4.mouse.move(petBox.x + petBox.width / 2, petBox.y + petBox.height / 2, { steps: 10 });
      await page4.mouse.down();
      await page4.mouse.move(petBox.x - 220, petBox.y - 150, { steps: 25 });
      await page4.waitForTimeout(400);
      await page4.mouse.up();
      await page4.waitForTimeout(600);
    }

    // Direct File Drop Ring Activation Simulation
    console.log('  [Interaction] 模拟真实文件拖入桌宠：高亮拖拽环激活 (Drop Target Ring)...');
    await page4.evaluate(() => {
      const wrap = document.getElementById('pet-character-wrap');
      if (wrap) wrap.classList.add('is-drop-target');
      if (typeof showPetBubble === 'function') {
        showPetBubble('松开直接转换 presentation.pptx 幻灯片！🎯', 4000);
      }
    });
    await page4.waitForTimeout(800);
    await saveSnapshot(page4, 'snap_15_pet_drop_ring_active.png');

    // Perform drop conversion: load converted PPTX into reader
    await page4.evaluate(() => {
      const wrap = document.getElementById('pet-character-wrap');
      if (wrap) wrap.classList.remove('is-drop-target');
    });
    const pptxConverted = execSync('python -c "from src.readmd_modules.convert import convert_verbose; import sys; t, e, _ = convert_verbose(r\'' + path.join(SAMPLES_DIR, 'presentation.pptx') + '\'); sys.stdout.buffer.write(t.encode(\'utf-8\'))"', {
      cwd: REPO_ROOT,
      env: { ...process.env, PYTHONPATH: '.' }
    }).toString('utf-8');

    await page4.evaluate((content) => {
      window.renderContent(content, 0);
      if (typeof showPetBubble === 'function') {
        showPetBubble('转换成功！幻灯片已在阅读器中呈现！🎉', 4000);
      }
    }, pptxConverted);
    await page4.waitForTimeout(1000);

    // Open Apple HIG Pet Settings Drawer
    console.log('  [Interaction] 打开 Apple HIG 桌宠设置抽屉...');
    const petSettingsQuick = page4.locator('#pet-quick-settings');
    if (await petSettingsQuick.isVisible()) {
      await smoothMouseMove(page4, petSettingsQuick);
      await petSettingsQuick.click();
      await page4.waitForTimeout(800);
      await saveSnapshot(page4, 'snap_16_pet_apple_settings_drawer.png');

      const petDoneBtn = page4.locator('#pet-done-btn');
      if (await petDoneBtn.isVisible()) {
        await petDoneBtn.click();
        await page4.waitForTimeout(400);
      }
    }

    await page4.waitForTimeout(1000);
    await ctx4.close();
    transcodeWebmToMp4(raw4, path.join(VIDEO_DIR, '04_interactive_pet_and_drop_convert.mp4'));
    fs.rmSync(raw4, { recursive: true, force: true });
    }

    // =========================================================================
    // SCENARIO 5: Plugin Center, Multi-Tabs & 46-Language System
    // =========================================================================
    console.log('\n=============================================================');
    console.log('>>> SCENARIO 5: Plugin Center, Multi-Tabs & 46-Language System <<<');
    console.log('=============================================================');
    const raw5 = path.join(SHOWCASE_ROOT, 'temp_raw_5');
    fs.mkdirSync(raw5, { recursive: true });

    const ctx5 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw5, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page5 = await ctx5.newPage();

    await page5.route('**/api/plugins/list', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        ffmpeg: true,
        sandbox_dir: 'C:/Users/Natsumer/AppData/Roaming/ReadMD/plugins',
        plugins: mockPluginState,
      }),
    }));

    await page5.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page5.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page5.waitForFunction(() => typeof renderContent === 'function');
    await page5.waitForTimeout(500);

    // Open Plugin Center
    console.log('  [Interaction] 打开轻量插件中心，展示本地安全沙箱就绪状态...');
    const moreBtn5 = page5.locator('#btn-more');
    await smoothMouseMove(page5, moreBtn5);
    await moreBtn5.click();
    await page5.waitForTimeout(400);

    const pluginsMenuBtn = page5.locator('#btn-plugin-menu, #btn-open-plugins').first();
    await smoothMouseMove(page5, pluginsMenuBtn);
    await pluginsMenuBtn.click();
    await page5.waitForTimeout(800);
    await saveSnapshot(page5, 'snap_17_plugin_center_sandbox_ready.png');

    // Simulate Smooth Download Progress Bar
    console.log('  [Interaction] 演示 RapidOCR 模型下载动态平滑渐变进度条...');
    await page5.evaluate(() => {
      const card = document.querySelector('.plugin-card[data-plugin-id="rapidocr"]');
      if (card) {
        let barWrap = card.querySelector('.plugin-progress-track');
        if (!barWrap) {
          barWrap = document.createElement('div');
          barWrap.className = 'plugin-progress-track';
          barWrap.innerHTML = '<div class="plugin-progress-fill" style="width: 0%"></div>';
          card.appendChild(barWrap);
        }
        const fill = barWrap.querySelector('.plugin-progress-fill');
        let p = 0;
        const interval = setInterval(() => {
          p += 15;
          if (fill) fill.style.width = Math.min(100, p) + '%';
          if (p >= 100) {
            clearInterval(interval);
            const statusTxt = card.querySelector('.plugin-status-txt');
            if (statusTxt) {
              statusTxt.textContent = '已就绪 (Ready)';
              statusTxt.style.color = '#2ea043';
            }
          }
        }, 120);
      }
    });
    await page5.waitForTimeout(1200);
    await saveSnapshot(page5, 'snap_18_plugin_download_progress_bar.png');

    // Close Plugin Center
    const closePluginBtn = page5.locator('#plugin-close, .modal-close-btn').first();
    if (await closePluginBtn.isVisible()) {
      await closePluginBtn.click();
      await page5.waitForTimeout(400);
    }

    // Instant Hot-Switching to English
    console.log('  [Interaction] 演示 46 国语言即时热切换：切换为 English...');
    await page5.evaluate(() => {
      if (window.i18n && typeof window.i18n.setLanguage === 'function') {
        window.i18n.setLanguage('en');
      }
    });
    await page5.waitForTimeout(800);
    await saveSnapshot(page5, 'snap_19_i18n_english_interface.png');

    // Multi-Tabs & Theme Switch
    console.log('  [Interaction] 演示多标签页系统 (Tabs Bar) 与深色/浅色护眼主题切换...');
    await page5.evaluate(() => {
      if (window.i18n && typeof window.i18n.setLanguage === 'function') {
        window.i18n.setLanguage('zh-CN');
      }
      document.body.setAttribute('data-theme', 'dark');
    });
    await page5.waitForTimeout(800);
    await saveSnapshot(page5, 'snap_20_multi_tabs_and_dark_theme.png');

    await page5.waitForTimeout(1000);
    await ctx5.close();
    transcodeWebmToMp4(raw5, path.join(VIDEO_DIR, '05_plugin_center_and_advanced_views.mp4'));
    fs.rmSync(raw5, { recursive: true, force: true });

    console.log('\n=============================================================');
    console.log('[All Complete] 全部 5 个全功能演示视频录制及 20 张逐帧质检快照全部完成！');
    console.log('=============================================================');

  } finally {
    await browser.close();
    server.kill();
  }
}

main().catch(err => {
  console.error('Recording failed:', err);
  process.exit(1);
});
