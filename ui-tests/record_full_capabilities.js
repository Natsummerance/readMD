const fs = require('fs');
const path = require('path');
const { spawn, execSync } = require('child_process');
const { chromium } = require('@playwright/test');

const UI_PORT = 28498;
const REPO_ROOT = path.join(__dirname, '..');
const SHOWCASE_ROOT = path.join(REPO_ROOT, 'showcase', 'v238_capabilities');
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

const mockPluginState = {
  rapidocr: {
    id: 'rapidocr',
    name_key: 'plugin.rapidocr.name',
    desc_key: 'plugin.rapidocr.desc',
    category: 'ocr',
    weight: 'light',
    approx_size: '~17MB',
    installed: true,
    enabled: true,
    cached: true,
    installing: false,
    install_error: '',
    last_log: '',
  },
  rapid_table: {
    id: 'rapid_table',
    name_key: 'plugin.rapid_table.name',
    desc_key: 'plugin.rapid_table.desc',
    category: 'ocr',
    weight: 'light',
    approx_size: '~16MB',
    installed: true,
    enabled: true,
    cached: true,
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
    { id: 'index.md', path: path.join(KB_DIR, 'index.md'), label: '理论物理与现代数学研究图谱 (Research Hub)', is_deadlink: false, link_count: 4, backlink_count: 3, degree: 7 },
    { id: 'Quantum Physics.md', path: path.join(KB_DIR, 'Quantum Physics.md'), label: '量子力学核心导论 (Quantum Physics)', is_deadlink: false, link_count: 3, backlink_count: 3, degree: 6 },
    { id: 'Electrodynamics.md', path: path.join(KB_DIR, 'Electrodynamics.md'), label: '电动力学与规范场论 (Electrodynamics)', is_deadlink: false, link_count: 3, backlink_count: 3, degree: 6 },
    { id: 'Mathematical Methods.md', path: path.join(KB_DIR, 'Mathematical Methods.md'), label: '数学物理方法 (Mathematical Methods)', is_deadlink: false, link_count: 3, backlink_count: 3, degree: 6 },
    { id: 'Nonexistent Conjecture', path: null, label: '超弦超对称未解难题 (Nonexistent)', is_deadlink: true, link_count: 0, backlink_count: 1, degree: 1 },
  ],
  edges: [
    { source: 'index.md', target: 'Quantum Physics.md', is_wikilink: true, line_no: 6 },
    { source: 'index.md', target: 'Electrodynamics.md', is_wikilink: true, line_no: 7 },
    { source: 'index.md', target: 'Mathematical Methods.md', is_wikilink: true, line_no: 8 },
    { source: 'index.md', target: 'Nonexistent Conjecture', is_wikilink: true, line_no: 9 },
    { source: 'Quantum Physics.md', target: 'Electrodynamics.md', is_wikilink: true, line_no: 16 },
    { source: 'Quantum Physics.md', target: 'Mathematical Methods.md', is_wikilink: true, line_no: 17 },
    { source: 'Quantum Physics.md', target: 'index.md', is_wikilink: true, line_no: 18 },
    { source: 'Electrodynamics.md', target: 'Quantum Physics.md', is_wikilink: true, line_no: 16 },
    { source: 'Electrodynamics.md', target: 'Mathematical Methods.md', is_wikilink: true, line_no: 17 },
    { source: 'Electrodynamics.md', target: 'index.md', is_wikilink: true, line_no: 18 },
    { source: 'Mathematical Methods.md', target: 'Electrodynamics.md', is_wikilink: true, line_no: 25 },
    { source: 'Mathematical Methods.md', target: 'Quantum Physics.md', is_wikilink: true, line_no: 26 },
    { source: 'Mathematical Methods.md', target: 'index.md', is_wikilink: true, line_no: 27 },
  ],
};

async function main() {
  const server = await startServer();
  const browser = await chromium.launch({ headless: true });

  try {
    // Scenario 1: Plugin Center & OCR / Table / Formula
    console.log('\n[Scenario 1/5] 录制: 插件中心与 OCR / 表格识别 / 数学公式...');
    const raw1 = path.join(SHOWCASE_ROOT, 'temp_raw_1');
    fs.mkdirSync(raw1, { recursive: true });

    const ctx1 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw1, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page1 = await ctx1.newPage();

    await page1.route('**/api/plugins/list', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        ffmpeg: true,
        sandbox_dir: 'C:/Users/Natsumer/AppData/Roaming/ReadMD/plugins',
        plugins: mockPluginState,
      }),
    }));

    await page1.route('**/api/plugins/toggle', async r => {
      const data = JSON.parse(r.request().postData() || '{}');
      if (mockPluginState[data.id]) mockPluginState[data.id].enabled = !mockPluginState[data.id].enabled;
      await r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, enabled: mockPluginState[data.id]?.enabled }) });
    });

    await page1.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page1.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page1.waitForFunction(() => typeof renderContent === 'function');
    await page1.waitForTimeout(1000);

    // 打开更多菜单 -> 万物转MD
    await smoothMouseMove(page1, page1.locator('#btn-more'));
    await page1.waitForTimeout(400);
    await page1.locator('#btn-more').click();
    await page1.waitForTimeout(600);

    await smoothMouseMove(page1, page1.locator('#btn-convert'));
    await page1.waitForTimeout(400);
    await page1.locator('#btn-convert').click();
    await page1.waitForSelector('#convert-modal:not(.hidden)');
    await page1.waitForTimeout(800);

    // 打开插件中心
    await smoothMouseMove(page1, page1.locator('#btn-open-plugins'));
    await page1.waitForTimeout(500);
    await page1.locator('#btn-open-plugins').click();
    await page1.waitForSelector('#plugin-modal:not(.hidden)');
    await page1.waitForTimeout(1200);

    // 截图 01: 插件中心全景
    await page1.screenshot({ path: path.join(SNAPSHOT_DIR, '01-plugin-center-overview.png') });

    // 悬停 RapidOCR 卡片
    const rapidCard = page1.locator('.plugin-card[data-plugin-id="rapidocr"]');
    if (await rapidCard.count()) {
      await smoothMouseMove(page1, rapidCard);
      await page1.waitForTimeout(1000);
    }

    // 悬停 RapidTable 卡片
    const tableCard = page1.locator('.plugin-card[data-plugin-id="rapid_table"]');
    if (await tableCard.count()) {
      await smoothMouseMove(page1, tableCard);
      await page1.waitForTimeout(800);
    }

    // 切换 PyLaTeXEnc 开关
    const pylatexSwitch = page1.locator('.plugin-card[data-plugin-id="pylatexenc"] .plugin-switch');
    if (await pylatexSwitch.count()) {
      await smoothMouseMove(page1, pylatexSwitch);
      await page1.waitForTimeout(400);
      await pylatexSwitch.click();
      await page1.waitForTimeout(1000);
    }

    // 截图 02: 插件中心交互状态
    await page1.screenshot({ path: path.join(SNAPSHOT_DIR, '02-plugin-center-interactive.png') });

    // 关闭插件中心
    await page1.keyboard.press('Escape');
    await page1.waitForTimeout(500);
    await page1.keyboard.press('Escape');
    await page1.waitForTimeout(600);

    // 演示 OCR 识别渲染
    const ocrSampleMd = `# 扫描版复杂多栏学术文档 OCR 提取成果\n\n> 识别内核：**RapidOCR (ONNX) + RapidTable** 深度协同 · XY-Cut 递归自然阅读序分析\n\n---\n\n### 一、 实验数据对比矩阵\n\n| 算法模型 | 显存占用 | 推理延时 (单页) | 字符准确率 (Acc) | 表格网格保真度 |\n| :--- | :---: | :---: | :---: | :---: |\n| **RapidOCR (内置移动端)** | **零 GPU (纯 CPU)** | **38 ms** | **99.2%** | **完美还原** |\n| PaddleOCR v4 | 1.2 GB | 142 ms | 98.7% | 良好 |\n| EasyOCR PyTorch | 850 MB | 210 ms | 97.4% | 基础管道 |\n\n---\n\n### 二、 拓扑几何物理公式识别\n\n在双栏排版与公式上下标基线跳变检测下，正确还原微分方程算子：\n\n$$\ni\\hbar \\frac{\\partial}{\\partial t} \\Psi(\\mathbf{r}, t) = \\left[ -\\frac{\\hbar^2}{2m}\\nabla^2 + V(\\mathbf{r}, t) \\right] \\Psi(\\mathbf{r}, t)\n$$\n\n同时支持行内公式运算，如质能关系 $E_0 = m c^2$ 及动量修正 $p = \\gamma m v$。\n`;
    await page1.evaluate(async (doc) => {
      await renderContent(doc, 'ocr-result-demo.md');
    }, ocrSampleMd);
    await page1.waitForTimeout(1500);

    // 截图 03: OCR 与多栏表格渲染
    await page1.screenshot({ path: path.join(SNAPSHOT_DIR, '03-ocr-table-formula-result.png') });

    await page1.close();
    await ctx1.close();
    transcodeWebmToMp4(raw1, path.join(VIDEO_DIR, '01-plugin-center-and-ocr.mp4'));

    // Scenario 2: Bi-directional Links & 2D Force-Directed Knowledge Graph
    console.log('\n[Scenario 2/5] 录制: 双向链接与 2D 力导向知识图谱...');
    const raw2 = path.join(SHOWCASE_ROOT, 'temp_raw_2');
    fs.mkdirSync(raw2, { recursive: true });

    const ctx2 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw2, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page2 = await ctx2.newPage();

    await page2.route('**/api/links/graph*', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ ok: true, graph: mockGraphData }),
    }));

    await page2.route('**/api/links/backlinks*', r => r.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        backlinks: [
          { source_path: path.join(KB_DIR, 'Electrodynamics.md'), source_title: '电动力学与规范场论', line_no: 16, alias: null },
          { source_path: path.join(KB_DIR, 'Mathematical Methods.md'), source_title: '数学物理方法', line_no: 26, alias: null },
          { source_path: path.join(KB_DIR, 'index.md'), source_title: '理论物理与现代数学研究图谱', line_no: 6, alias: '量子力学核心导论' },
        ],
        forward_links: [
          { target_raw: 'Electrodynamics', target_path: path.join(KB_DIR, 'Electrodynamics.md'), target_title: '电动力学与规范场论', line_no: 16, is_deadlink: false },
          { target_raw: 'Mathematical Methods', target_path: path.join(KB_DIR, 'Mathematical Methods.md'), target_title: '数学物理方法', line_no: 17, is_deadlink: false },
          { target_raw: 'Nonexistent Conjecture', target_path: null, target_title: null, line_no: 19, is_deadlink: true },
        ],
      }),
    }));

    await page2.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page2.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page2.waitForFunction(() => typeof renderContent === 'function');

    const indexMd = readSample('knowledge_base/index.md');
    await page2.evaluate(async ({ doc, filePath }) => {
      if (typeof state !== 'undefined') {
        state.file = filePath;
        state.dir = filePath.substring(0, filePath.lastIndexOf('\\'));
      }
      await renderContent(doc, 'index.md');
    }, { doc: indexMd, filePath: path.join(KB_DIR, 'index.md') });
    await page2.waitForTimeout(1000);

    // 截图 04: 双链笔记界面
    await page2.screenshot({ path: path.join(SNAPSHOT_DIR, '04-wikilink-reader.png') });

    // 打开反向链接抽屉
    await smoothMouseMove(page2, page2.locator('#btn-more'));
    await page2.waitForTimeout(400);
    await page2.locator('#btn-more').click();
    await page2.waitForTimeout(500);

    await smoothMouseMove(page2, page2.locator('#btn-backlinks-menu'));
    await page2.waitForTimeout(400);
    await page2.locator('#btn-backlinks-menu').click();
    await page2.waitForTimeout(1200);

    // 截图 05: 反向链接抽屉展示
    await page2.screenshot({ path: path.join(SNAPSHOT_DIR, '05-backlinks-drawer.png') });

    await page2.keyboard.press('Escape');
    await page2.waitForTimeout(500);

    // 打开 2D Canvas 关系知识图谱
    await smoothMouseMove(page2, page2.locator('#btn-graph'));
    await page2.waitForTimeout(400);
    await page2.locator('#btn-graph').click();
    await page2.waitForSelector('#graph-modal:not(.hidden)');
    await page2.waitForTimeout(2000);

    // 截图 06: 知识图谱全网力导向展示
    await page2.screenshot({ path: path.join(SNAPSHOT_DIR, '06-knowledge-graph-modal.png') });

    const canvas = page2.locator('#graph-canvas');
    const cbox = await canvas.boundingBox();
    if (cbox) {
      const cx = cbox.x + cbox.width / 2;
      const cy = cbox.y + cbox.height / 2;

      await page2.mouse.move(cx, cy, { steps: 10 });
      await page2.mouse.wheel(0, -120);
      await page2.waitForTimeout(800);
      await page2.mouse.wheel(0, 80);
      await page2.waitForTimeout(800);

      await page2.mouse.move(cx, cy, { steps: 12 });
      await page2.mouse.down();
      await page2.mouse.move(cx + 120, cy - 80, { steps: 25 });
      await page2.waitForTimeout(600);
      await page2.mouse.up();
      await page2.waitForTimeout(1200);

      await page2.mouse.move(cx - 140, cy + 90, { steps: 20 });
      await page2.waitForTimeout(1200);
    }

    // 截图 07: 节点拖拽与连接高亮
    await page2.screenshot({ path: path.join(SNAPSHOT_DIR, '07-graph-interactive-node.png') });

    await page2.keyboard.press('Escape');
    await page2.waitForTimeout(800);

    await page2.close();
    await ctx2.close();
    transcodeWebmToMp4(raw2, path.join(VIDEO_DIR, '02-knowledge-graph-and-bidirectional-links.mp4'));

    // Scenario 3: Academic LaTeX to Markdown Engine
    console.log('\n[Scenario 3/5] 录制: 学术 LaTeX 转换与 KaTeX 公式渲染...');
    const raw3 = path.join(SHOWCASE_ROOT, 'temp_raw_3');
    fs.mkdirSync(raw3, { recursive: true });

    const ctx3 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw3, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page3 = await ctx3.newPage();

    await page3.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page3.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page3.waitForFunction(() => typeof renderContent === 'function');

    const convertedLatexMd = `---
title: "Quantum Electrodynamics & Non-Equilibrium Field Dynamics"
author: "Prof. Richard P. Feynman \\and Dr. Julian Schwinger"
date: "September 2026"
---

# Quantum Electrodynamics & Non-Equilibrium Field Dynamics

> **摘要 (Abstract)**: This paper presents a self-contained formulation of quantum electrodynamics (QED) in non-equilibrium thermodynamic regimes. We derive the exact path-integral propagator and demonstrate full gauge invariance under local $U(1)$ phase transformations.

---

## 1. Introduction

Quantum field theory describes fundamental interactions by quantizing classical fields over Minkowski spacetime. The interaction Lagrangian density for electrodynamics is given by:

$$
\\mathcal{L} = \\bar{\\psi} (i \\gamma^\\mu D_\\mu - m) \\psi - \\frac{1}{4} F_{\\mu\\nu} F^{\\mu\\nu}
$$

where $D_\\mu = \\partial_\\mu + i e A_\\mu$ represents the gauge-covariant derivative, and $F_{\\mu\\nu} = \\partial_\\mu A_\\nu - \\partial_\\nu A_\\mu$ is the electromagnetic field strength tensor.

---

## 2. Field Equations & Conservation Laws

Applying the Euler-Lagrange variational principle to the action $S = \\int d^4x \\, \\mathcal{L}$, we arrive at the coupled equations of motion:

$$
\\begin{aligned}
(i \\gamma^\\mu \\partial_\\mu - m)\\psi &= e \\gamma^\\mu A_\\mu \\psi \\\\
\\partial_\\nu F^{\\nu\\mu} &= e \\bar{\\psi} \\gamma^\\mu \\psi = J^\\mu
\\end{aligned}
$$

### 2.1 Commutation Relations

The canonical equal-time anti-commutation relations for the Dirac spinor fields satisfy:

$$
\\{\\psi_a(\\mathbf{x}, t), \\psi_b^\\dagger(\\mathbf{y}, t)\\} = \\delta_{ab} \\, \\delta^{(3)}(\\mathbf{x} - \\mathbf{y})
$$

---

## 3. Physical Parameters and Constants

The experimental measurements of fundamental interaction constants are summarized in the table below:

| Constant | Symbol | Value |
| :--- | :---: | ---: |
| Fine-structure constant | $\\alpha$ | $1/137.035999$ |
| Electron rest mass | $m_e$ | $0.510998950 \\text{ MeV}$ |
| Planck constant (reduced) | $\\hbar$ | $1.0545718 \\times 10^{-34} \\text{ J}\\cdot\\text{s}$ |
| Speed of light | $c$ | $299792458 \\text{ m/s}$ |

---

## 4. Theorems on Gauge Invariance

> **定理 (Theorem): Local Gauge Invariance**
>
> Every local gauge transformation $\\psi(x) \\to e^{i\\theta(x)}\\psi(x)$ preserves the physical observables of the electromagnetic stress-energy tensor.

> **证明 (Proof)**
>
> Direct substitution of the phase factor yields an identical scalar curvature and conserved Noether current $J^\\mu$.

---

## 5. 参考文献 (References)

- **[@feynman1949]** R. P. Feynman, *Space-Time Approach to Quantum Electrodynamics*, Phys. Rev. 76, 769 (1949).
- **[@schwinger1948]** J. Schwinger, *Quantum Electrodynamics. I. A Covariant Formulation*, Phys. Rev. 74, 1439 (1948).
`;

    await page3.evaluate(async (doc) => {
      await renderContent(doc, 'sample_paper.md');
    }, convertedLatexMd);
    await page3.waitForTimeout(1500);

    // 截图 08: 暗色学术论文 LaTeX 解析与公式展示
    await page3.screenshot({ path: path.join(SNAPSHOT_DIR, '08-latex-dark-math.png') });

    await page3.mouse.wheel(0, 480);
    await page3.waitForTimeout(1500);

    // 截图 09: 表格与定理引理
    await page3.screenshot({ path: path.join(SNAPSHOT_DIR, '09-latex-table-and-theorems.png') });

    // 切换至明亮模式
    await page3.evaluate(() => {
      if (typeof state !== 'undefined') {
        state.theme = 'light';
        if (typeof applySettings === 'function') applySettings();
      }
      document.body.dataset.theme = 'light';
    });
    await page3.waitForTimeout(1200);

    // 截图 10: 明亮模式学术排版
    await page3.screenshot({ path: path.join(SNAPSHOT_DIR, '10-latex-light-theme.png') });

    await page3.mouse.wheel(0, -480);
    await page3.waitForTimeout(800);
    await page3.evaluate(() => {
      if (typeof state !== 'undefined') {
        state.theme = 'dark';
        if (typeof applySettings === 'function') applySettings();
      }
      document.body.dataset.theme = 'dark';
    });
    await page3.waitForTimeout(600);

    await page3.close();
    await ctx3.close();
    transcodeWebmToMp4(raw3, path.join(VIDEO_DIR, '03-academic-latex-to-markdown.mp4'));

    // Scenario 4: Batch Convert Workbench & AV Transcription
    console.log('\n[Scenario 4/5] 录制: 万物转 MD 批量工作台与音视频转写...');
    const raw4 = path.join(SHOWCASE_ROOT, 'temp_raw_4');
    fs.mkdirSync(raw4, { recursive: true });

    const ctx4 = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 2,
      recordVideo: { dir: raw4, size: { width: 1440, height: 900 } },
      locale: 'zh-CN',
    });
    const page4 = await ctx4.newPage();

    await page4.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page4.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page4.waitForFunction(() => typeof renderContent === 'function');

    await smoothMouseMove(page4, page4.locator('#btn-more'));
    await page4.waitForTimeout(400);
    await page4.locator('#btn-more').click();
    await page4.waitForTimeout(500);

    await smoothMouseMove(page4, page4.locator('#btn-convert'));
    await page4.waitForTimeout(400);
    await page4.locator('#btn-convert').click();
    await page4.waitForSelector('#convert-modal:not(.hidden)');
    await page4.waitForTimeout(800);

    await page4.evaluate(() => {
      const list = $('convert-list');
      if (!list) return;
      list.innerHTML = `
        <div class="convert-item">
          <span class="convert-item-name">📄 sample_paper.tex (LaTeX 论文源码)</span>
          <span class="convert-item-status ready" style="color:#00e5a3;">✓ 转换成功 (texmd)</span>
        </div>
        <div class="convert-item">
          <span class="convert-item-name">📘 sample_doc.docx (技术设计规范)</span>
          <span class="convert-item-status ready" style="color:#00e5a3;">✓ 转换成功 (docx2md)</span>
        </div>
        <div class="convert-item">
          <span class="convert-item-name">📊 dataset.csv (科学测量数据表)</span>
          <span class="convert-item-status ready" style="color:#00e5a3;">✓ 转换成功 (csv2md)</span>
        </div>
        <div class="convert-item">
          <span class="convert-item-name">🎙️ speech_demo.wav (学术研讨会录音)</span>
          <span class="convert-item-status ready" style="color:#00e5a3;">✓ 分段转写完成 (whisper)</span>
        </div>
      `;
      const statusEl = $('convert-status');
      if (statusEl) statusEl.textContent = '全部 4 个文件转换完成，结果已存入同目录 .md';
    });
    await page4.waitForTimeout(1500);

    // 截图 11: 批量转换完成清单
    await page4.screenshot({ path: path.join(SNAPSHOT_DIR, '11-batch-convert-complete.png') });

    await page4.keyboard.press('Escape');
    await page4.waitForTimeout(600);

    const avTranscribedMd = `---
title: "speech_demo.wav"
format: "wav"
duration: "00:03"
model: "whisper-base"
language: "zh"
---

# 音频/视频转写：speech_demo.wav

> 识别语言：\`zh\` · 模型内核：\`whisper-base\` · 时长：\`00:03\`

**[00:00]** 各位学者与同行大家好，欢迎参与本次前沿物理计算研讨会。

**[00:01]** 我们基于 ReadMD 全新架构成功实现了纯本地、零依赖的学术论文解析引擎。

**[00:02]** 所有数学公式、双向知识图谱与语音分段全部在端侧高效完成，数据安全且秒级可用。
`;

    await page4.evaluate(async (doc) => {
      await renderContent(doc, 'speech_demo.md');
    }, avTranscribedMd);
    await page4.waitForTimeout(1500);

    // 截图 12: 音视频转写分段成果
    await page4.screenshot({ path: path.join(SNAPSHOT_DIR, '12-av-transcribe-result.png') });

    const fallbackNoticeMd = `---
title: "lecture_recording.mp4"
format: "mp4"
status: "unprocessed"
---

# 音频/视频转写：lecture_recording.mp4

> ⚠️ **未检测到语音转写模型或 FFmpeg 工具**
>
> **快速安装指引**：
> 1. **方式一（推荐）**：在 ReadMD 右上角打开「插件中心」，启用或一键安装 \`whisper\` 插件。
> 2. **方式二（手动 CLI 命令）**：
>    \`\`\`bash
>    pip install openai-whisper
>    \`\`\`
>    若系统缺少 FFmpeg，请运行对应命令安装并加入环境变量 PATH：
>    - **Windows**: \`winget install Gyan.FFmpeg\` 或从官网解压
>    - **macOS**: \`brew install ffmpeg\`
>    - **Linux**: \`sudo apt install ffmpeg\`
`;
    await page4.evaluate(async (doc) => {
      await renderContent(doc, 'transcribe_fallback_guide.md');
    }, fallbackNoticeMd);
    await page4.waitForTimeout(1800);

    // 截图 13: 友好降级指引
    await page4.screenshot({ path: path.join(SNAPSHOT_DIR, '13-transcribe-fallback-guide.png') });

    await page4.close();
    await ctx4.close();
    transcodeWebmToMp4(raw4, path.join(VIDEO_DIR, '04-batch-convert-and-av-transcribe.mp4'));

    // Scenario 5: Full Suite End-to-End Walkthrough
    console.log('\n[Scenario 5/5] 录制: 全功能贯通综合演示大片...');
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
      status: 200, contentType: 'application/json',
      body: JSON.stringify({ ok: true, ffmpeg: true, sandbox_dir: 'C:/Users/Natsumer/AppData/Roaming/ReadMD/plugins', plugins: mockPluginState }),
    }));
    await page5.route('**/api/links/graph*', r => r.fulfill({
      status: 200, contentType: 'application/json',
      body: JSON.stringify({ ok: true, graph: mockGraphData }),
    }));
    await page5.route('**/api/links/backlinks*', r => r.fulfill({
      status: 200, contentType: 'application/json',
      body: JSON.stringify({
        ok: true,
        backlinks: [{ source_path: path.join(KB_DIR, 'index.md'), source_title: '研究图谱', line_no: 6 }],
        forward_links: [{ target_raw: 'Electrodynamics', target_path: path.join(KB_DIR, 'Electrodynamics.md'), line_no: 16, is_deadlink: false }],
      }),
    }));

    await page5.addInitScript(() => {
      localStorage.setItem('readmd_language', 'zh-CN');
      localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
    });

    await page5.goto('http://127.0.0.1:' + UI_PORT + '/');
    await page5.waitForFunction(() => typeof renderContent === 'function');

    // 1. 论文阅读与公式漫游
    await page5.evaluate(async (doc) => {
      await renderContent(doc, 'quantum-electrodynamics.md');
    }, convertedLatexMd);
    await page5.waitForTimeout(1200);
    await page5.mouse.wheel(0, 360);
    await page5.waitForTimeout(1000);

    // 2. 呼出知识图谱
    await page5.locator('#btn-graph').click();
    await page5.waitForSelector('#graph-modal:not(.hidden)');
    await page5.waitForTimeout(1800);
    const cb = await page5.locator('#graph-canvas').boundingBox();
    if (cb) {
      await page5.mouse.move(cb.x + cb.width / 2, cb.y + cb.height / 2, { steps: 15 });
      await page5.mouse.down();
      await page5.mouse.move(cb.x + cb.width / 2 + 100, cb.y + cb.height / 2 - 50, { steps: 20 });
      await page5.mouse.up();
      await page5.waitForTimeout(1000);
    }
    await page5.keyboard.press('Escape');
    await page5.waitForTimeout(600);

    // 3. 打开万物转 MD 与插件中心
    await page5.locator('#btn-more').click();
    await page5.waitForTimeout(400);
    await page5.locator('#btn-convert').click();
    await page5.waitForSelector('#convert-modal:not(.hidden)');
    await page5.waitForTimeout(600);
    await page5.locator('#btn-open-plugins').click();
    await page5.waitForSelector('#plugin-modal:not(.hidden)');
    await page5.waitForTimeout(1200);
    await page5.keyboard.press('Escape');
    await page5.waitForTimeout(400);
    await page5.keyboard.press('Escape');
    await page5.waitForTimeout(600);

    // 4. 音视频转写成果展示
    await page5.evaluate(async (doc) => {
      await renderContent(doc, 'speech_demo.md');
    }, avTranscribedMd);
    await page5.waitForTimeout(1800);

    await page5.close();
    await ctx5.close();
    transcodeWebmToMp4(raw5, path.join(VIDEO_DIR, '05-full-suite-walkthrough.mp4'));

    // Clean up temporary directories and replicate to conversation artifacts
    console.log('\n[Sync] 复制录制资产至对话 Artifacts 目录...');
    const allVideos = fs.readdirSync(VIDEO_DIR);
    for (const v of allVideos) {
      const src = path.join(VIDEO_DIR, v);
      const dest = path.join(ARTIFACT_DIR, v);
      fs.copyFileSync(src, dest);
      console.log(`  ✓ 视频同步: ${v}`);
    }

    const allSnaps = fs.readdirSync(SNAPSHOT_DIR);
    for (const s of allSnaps) {
      const src = path.join(SNAPSHOT_DIR, s);
      const dest = path.join(ARTIFACT_DIR, s);
      fs.copyFileSync(src, dest);
      console.log(`  ✓ 截图同步: ${s}`);
    }

    // 清理临时 webm 目录
    for (const r of [raw1, raw2, raw3, raw4, raw5]) {
      try { fs.rmSync(r, { recursive: true, force: true }); } catch (_) {}
    }

    console.log('\n🎉 全部 5 部高清视频与 13 张高清 Retina 截图录制、转码与沉淀完成！');
  } finally {
    await browser.close();
    try { server.kill(); } catch (_) {}
  }
}

main().catch(err => {
  console.error('Recording process failed:', err);
  process.exit(1);
});
