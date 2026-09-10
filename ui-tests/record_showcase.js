const fs = require('fs');
const path = require('path');
const { spawn, execSync } = require('child_process');
const { chromium } = require('@playwright/test');

const UI_PORT = 28492;
const ARTIFACT_DIR = 'C:/Users/Natsumer/.gemini/antigravity/brain/ccbcea97-6f62-4db3-97fb-cfc7f4d855a8';
const OUTPUT_DIR = path.join(__dirname, 'showcase_output');

if (!fs.existsSync(OUTPUT_DIR)) {
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });
}

const SAMPLE_DOC = `# 物理学深度论文与公式实录 (LaTeX 极速转换演示)

> 本文由 ReadMD 原生 LaTeX 解析内核于 12ms 内瞬间转写生成，零外部进程依赖。

---

### 一、 麦克斯韦电磁场方程组 (Maxwell's Equations)

在微分几何与矢量微积分表述下，自由空间中的麦克斯韦方程组呈现出极佳的对称性：

$$
\\begin{aligned}
\\nabla \\cdot \\mathbf{E} &= \\frac{\\rho}{\\varepsilon_0} \\\\
\\nabla \\cdot \\mathbf{B} &= 0 \\\\
\\nabla \\times \\mathbf{E} &= -\\frac{\\partial \\mathbf{B}}{\\partial t} \\\\
\\nabla \\times \\mathbf{B} &= \\mu_0 \\mathbf{J} + \\mu_0 \\varepsilon_0 \\frac{\\partial \\mathbf{E}}{\\partial t}
\\end{aligned}
$$

其中 $\\mathbf{E}$ 为电场强度，$\\mathbf{B}$ 为磁感应强度，$\\rho$ 与 $\\mathbf{J}$ 分别对应自由电荷与电流密度。

---

### 二、 纳维-斯托克斯流体力学动量方程

$$
\\rho \\left( \\frac{\\partial \\mathbf{u}}{\\partial t} + \\mathbf{u} \\cdot \\nabla \\mathbf{u} \\right) = -\\nabla p + \\mu \\nabla^2 \\mathbf{u} + \\mathbf{f}
$$

式中第一项为瞬态加速度，第二项为对流加速度，右侧依次为压力梯度、粘性耗散与体积力源项。

---

### 三、 扩展插件沙箱矩阵状态

| 核心组件 | 运行模式 | 预估体积 | 调度策略与技术规格 |
| :--- | :--- | :--- | :--- |
| **Docling** | 独立沙箱 | ~500 MB | IBM 深度学术排版解析模型，公式表格多栏智能重组 |
| **EasyOCR** | 独立沙箱 | ~150 MB | 深度卷积识别网络，手写体/倾斜纸质图片高精度兜底 |
| **PyLaTeXEnc** | 独立沙箱 | ~1 MB | 容错 AST 语法树增强解析器 |
| **Whisper** | 独立沙箱 | ~150 MB | OpenAI 语音转写引擎，高精度分段与时间戳自动对齐 |
`;

async function main() {
  console.log('[1/6] 启动独立 ReadMD UI 服务...');
  const serverPy = path.join(__dirname, '..', 'tools', 'ui_server.py');
  const server = spawn('python', [serverPy, String(UI_PORT)], {
    stdio: ['ignore', 'pipe', 'inherit'],
    cwd: path.dirname(serverPy),
    env: { ...process.env, READMD_UI_PORT: String(UI_PORT) }
  });

  await new Promise((resolve) => {
    server.stdout.on('data', (d) => {
      const msg = d.toString();
      if (msg.includes('ReadMD UI test server ready')) {
        console.log('UI Server ready on port ' + UI_PORT);
        resolve();
      }
    });
    setTimeout(resolve, 3000);
  });

  console.log('[2/6] 启动 Playwright 录制环境 (1440x900 Retina 2x)...');
  const browser = await chromium.launch({
    headless: true,
  });

  const videoDir = path.join(OUTPUT_DIR, 'raw_video');
  if (fs.existsSync(videoDir)) {
    fs.rmSync(videoDir, { recursive: true, force: true });
  }
  fs.mkdirSync(videoDir, { recursive: true });

  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    recordVideo: {
      dir: videoDir,
      size: { width: 1440, height: 900 },
    },
    locale: 'zh-CN',
  });

  const page = await context.newPage();

  let pluginState = {
    docling: {
      id: 'docling',
      name_key: 'plugin.docling.name',
      desc_key: 'plugin.docling.desc',
      category: 'document',
      weight: 'heavy',
      approx_size: '~500MB',
      installed: false,
      enabled: false,
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
      installing: false,
      install_error: '',
      last_log: '',
    },
  };

  await page.route('**/api/update/check', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ ok: false }),
  }));

  await page.route('**/api/plugins/list', route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({
      ok: true,
      ffmpeg: true,
      sandbox_dir: 'C:/Users/Natsumer/AppData/Roaming/ReadMD/plugins',
      plugins: pluginState,
    }),
  }));

  await page.route('**/api/plugins/toggle', async route => {
    const postData = JSON.parse(route.request().postData() || '{}');
    const id = postData.id;
    if (pluginState[id]) {
      pluginState[id].enabled = !pluginState[id].enabled;
    }
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ ok: true, enabled: pluginState[id] ? pluginState[id].enabled : false }),
    });
  });

  await page.addInitScript(() => {
    localStorage.setItem('readmd_language', 'zh-CN');
    localStorage.setItem('readmd-settings', JSON.stringify({ theme: 'dark', fontSize: 100, lineWidth: 860 }));
  });

  console.log('[3/6] 打开首页并进入暗黑模式排版...');
  await page.goto(`http://127.0.0.1:${UI_PORT}/`);
  await page.waitForFunction(() => typeof renderContent === 'function');

  // 确保应用 Dark 主题与渲染文档
  await page.evaluate(async (doc) => {
    if (typeof state !== 'undefined') {
      state.theme = 'dark';
      if (typeof applySettings === 'function') applySettings();
    }
    document.body.dataset.theme = 'dark';
    await renderContent(doc, 'physics-paper-preview.md');
  }, SAMPLE_DOC);

  await page.waitForTimeout(2000);

  // 截取暗黑模式阅读器 LaTeX 公式渲染图
  const shot4 = path.join(OUTPUT_DIR, '04-reader-latex-math-dark.png');
  await page.screenshot({ path: shot4, fullPage: false });
  console.log('✓ 已生成截图: 04-reader-latex-math-dark.png');

  console.log('[4/6] 演示交互：平滑移动鼠标 -> 打开更多菜单 -> 打开万物转换工作台...');
  const btnMore = page.locator('#btn-more');
  const moreBox = await btnMore.boundingBox();
  if (moreBox) {
    await page.mouse.move(moreBox.x + moreBox.width / 2, moreBox.y + moreBox.height / 2, { steps: 15 });
  }
  await page.waitForTimeout(500);
  await btnMore.click();
  await page.waitForTimeout(800);

  const btnConvert = page.locator('#btn-convert');
  const convBox = await btnConvert.boundingBox();
  if (convBox) {
    await page.mouse.move(convBox.x + convBox.width / 2, convBox.y + convBox.height / 2, { steps: 15 });
  }
  await page.waitForTimeout(500);
  await btnConvert.click();
  await page.waitForSelector('#convert-modal:not(.hidden)');
  await page.waitForTimeout(1000);

  // 截取暗黑模式万物转 MD 模态框
  const shot1 = path.join(OUTPUT_DIR, '01-convert-workbench-entry-dark.png');
  await page.screenshot({ path: shot1, fullPage: false });
  console.log('✓ 已生成截图: 01-convert-workbench-entry-dark.png');

  console.log('[5/6] 演示交互：点击扩展插件入口 -> 展开插件管理中心 (#plugin-modal)...');
  const btnOpenPlugins = page.locator('#btn-open-plugins');
  const plugBtnBox = await btnOpenPlugins.boundingBox();
  if (plugBtnBox) {
    await page.mouse.move(plugBtnBox.x + plugBtnBox.width / 2, plugBtnBox.y + plugBtnBox.height / 2, { steps: 15 });
  }
  await page.waitForTimeout(600);
  await btnOpenPlugins.click();
  await page.waitForSelector('#plugin-modal:not(.hidden)');
  await page.waitForTimeout(1500);

  // 截取暗黑模式插件中心全景
  const shot2 = path.join(OUTPUT_DIR, '02-plugin-center-modal-dark.png');
  await page.screenshot({ path: shot2, fullPage: false });
  console.log('✓ 已生成截图: 02-plugin-center-modal-dark.png');

  // 交互演示：移动鼠标悬停 EasyOCR 卡片
  const easyocrCard = page.locator('.plugin-card[data-plugin-id="easyocr"]');
  const easyBox = await easyocrCard.boundingBox();
  if (easyBox) {
    await page.mouse.move(easyBox.x + easyBox.width / 2, easyBox.y + easyBox.height / 2, { steps: 20 });
  }
  await page.waitForTimeout(1000);

  // 切换 PyLaTeXEnc 开关
  const pylatexencToggle = page.locator('.plugin-card[data-plugin-id="pylatexenc"] .plugin-switch');
  const toggleBox = await pylatexencToggle.boundingBox();
  if (toggleBox) {
    await page.mouse.move(toggleBox.x + toggleBox.width / 2, toggleBox.y + toggleBox.height / 2, { steps: 15 });
  }
  await page.waitForTimeout(600);
  await pylatexencToggle.click();
  await page.waitForTimeout(1000);

  // 移动到 Docling 卡片
  const doclingCard = page.locator('.plugin-card[data-plugin-id="docling"]');
  const docBox = await doclingCard.boundingBox();
  if (docBox) {
    await page.mouse.move(docBox.x + docBox.width / 2, docBox.y + docBox.height / 2, { steps: 15 });
  }
  await page.waitForTimeout(1000);

  // 截取交互细节
  const shot3 = path.join(OUTPUT_DIR, '03-plugin-center-interactive-dark.png');
  await page.screenshot({ path: shot3, fullPage: false });
  console.log('✓ 已生成截图: 03-plugin-center-interactive-dark.png');

  // 按 Escape 关闭 plugin-modal
  await page.keyboard.press('Escape');
  await page.waitForTimeout(800);

  // 再次按 Escape 关闭 convert-modal
  await page.keyboard.press('Escape');
  await page.waitForTimeout(800);

  // 切换为明亮模式并重新打开一次插件中心，捕获明亮模式截图
  await page.evaluate(() => {
    if (typeof state !== 'undefined') {
      state.theme = 'light';
      if (typeof applySettings === 'function') applySettings();
    }
    document.body.dataset.theme = 'light';
  });
  await page.waitForTimeout(600);

  await page.locator('#btn-more').click();
  await page.waitForTimeout(500);
  await page.locator('#btn-convert').click();
  await page.waitForTimeout(500);
  await page.locator('#btn-open-plugins').click();
  await page.waitForTimeout(1000);

  const shot5 = path.join(OUTPUT_DIR, '05-plugin-center-modal-light.png');
  await page.screenshot({ path: shot5, fullPage: false });
  console.log('✓ 已生成截图: 05-plugin-center-modal-light.png');

  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);

  // 切回暗黑模式平滑滚屏
  await page.evaluate(() => {
    if (typeof state !== 'undefined') {
      state.theme = 'dark';
      if (typeof applySettings === 'function') applySettings();
    }
    document.body.dataset.theme = 'dark';
  });
  await page.waitForTimeout(800);

  // 页面平滑向下滚动，展示公式与表格
  await page.mouse.wheel(0, 450);
  await page.waitForTimeout(2000);

  console.log('[6/6] 完成录制并处理视频...');
  await page.close();
  await context.close();
  await browser.close();

  try {
    server.kill();
  } catch (e) {}

  const files = fs.readdirSync(videoDir);
  const webmFile = files.find(f => f.endsWith('.webm'));
  if (!webmFile) {
    throw new Error('未找到录制的 webm 视频文件');
  }

  const rawWebmPath = path.join(videoDir, webmFile);
  const outputMp4Path = path.join(OUTPUT_DIR, 'plugin-center-and-capabilities-showcase.mp4');

  console.log('使用 ffmpeg 将 WebM 转码为高质量 MP4 (H.264 / 60fps)...');
  execSync(`ffmpeg -y -i "${rawWebmPath}" -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p "${outputMp4Path}"`, {
    stdio: 'inherit'
  });

  const artifactsToCopy = [
    '01-convert-workbench-entry-dark.png',
    '02-plugin-center-modal-dark.png',
    '03-plugin-center-interactive-dark.png',
    '04-reader-latex-math-dark.png',
    '05-plugin-center-modal-light.png',
    'plugin-center-and-capabilities-showcase.mp4',
  ];

  for (const item of artifactsToCopy) {
    const src = path.join(OUTPUT_DIR, item);
    const dest = path.join(ARTIFACT_DIR, item);
    if (fs.existsSync(src)) {
      fs.copyFileSync(src, dest);
      console.log(`✓ 复制产物到 Artifacts 目录: ${item}`);
    }
  }

  console.log('全部录制与高保真截图生成完毕！');
}

main().catch(err => {
  console.error('Showcase generation failed:', err);
  process.exit(1);
});
