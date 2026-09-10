// ui-tests/test_graph_button_visibility.js
// 验证顶栏知识图谱按键的智能上下文感知显示/隐藏逻辑

const { chromium } = require('playwright');
const path = require('path');
const { spawn } = require('child_process');

(async () => {
  const rootDir = path.resolve(__dirname, '..');
  const port = 28599;
  console.log(`[Test] 启动 ReadMD 后端测试服务器 (端口: ${port})...`);
  
  const serverPy = path.join(rootDir, 'tools', 'ui_server.py');
  const pyServer = spawn('python', [serverPy, String(port)], {
    cwd: rootDir,
    stdio: ['ignore', 'pipe', 'inherit'],
    env: { ...process.env, READMD_UI_PORT: String(port) }
  });

  await new Promise((resolve) => {
    pyServer.stdout.on('data', (d) => {
      if (d.toString().includes('ReadMD UI test server ready')) {
        console.log('[Server] ReadMD UI server ready!');
        resolve();
      }
    });
    setTimeout(resolve, 3000);
  });

  console.log('[Test] 后端服务已就绪，启动无头浏览器测试...');
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();

  try {
    await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(600);

    // 1. 初始 Welcome 界面验证：#btn-graph 必须隐藏
    console.log('[Step 1] 验证 Welcome 初始界面下顶栏图谱按键隐藏态...');
    const graphBtn = page.locator('#btn-graph');
    const isHiddenInit = await graphBtn.evaluate(el => el.classList.contains('hidden') || window.getComputedStyle(el).display === 'none');
    console.log(`  -> Welcome 初始状态 #btn-graph 隐藏: ${isHiddenInit}`);
    if (!isHiddenInit) throw new Error('Welcome 页面下 #btn-graph 应该隐藏，但实际未隐藏！');

    // 2. 加载不含知识图谱的普通长文档 (README.md)
    const plainDocPath = path.resolve(rootDir, 'README.md');
    console.log('[Step 2] 打开不含知识图谱的文档 (README.md)...');
    await page.evaluate((p) => window.loadFile && window.loadFile(p), plainDocPath);
    await page.waitForTimeout(1000);

    const isHiddenPlain = await graphBtn.evaluate(el => el.classList.contains('hidden') || window.getComputedStyle(el).display === 'none');
    console.log(`  -> 普通文档下 #btn-graph 隐藏: ${isHiddenPlain}`);
    if (!isHiddenPlain) throw new Error('普通不含双链的文档下 #btn-graph 应该隐藏，但实际显示了！');

    // 3. 加载含知识图谱的文档 (knowledge_base/index.md)
    const kbDocPath = path.resolve(rootDir, 'showcase/v239_full_coverage/samples/knowledge_base/index.md');
    console.log('[Step 3] 打开含知识图谱的文档 (knowledge_base/index.md)...');
    await page.evaluate((p) => window.loadFile && window.loadFile(p), kbDocPath);
    await page.waitForTimeout(1200);

    const isVisibleKb = await graphBtn.evaluate(el => !el.classList.contains('hidden') && window.getComputedStyle(el).display !== 'none');
    console.log(`  -> 知识图谱文档下 #btn-graph 显示: ${isVisibleKb}`);
    if (!isVisibleKb) throw new Error('含双链知识库的文档下 #btn-graph 应该显示，但实际未显示！');

    // 4. 再次切换回普通文档
    console.log('[Step 4] 再次切换回普通文档，验证按键自动恢复隐藏...');
    await page.evaluate((p) => window.loadFile && window.loadFile(p), plainDocPath);
    await page.waitForTimeout(1000);

    const isHiddenAgain = await graphBtn.evaluate(el => el.classList.contains('hidden') || window.getComputedStyle(el).display === 'none');
    console.log(`  -> 切回普通文档后 #btn-graph 重新隐藏: ${isHiddenAgain}`);
    if (!isHiddenAgain) throw new Error('切回普通文档后 #btn-graph 未能自动隐藏！');

    // 5. 验证更多菜单中的全局入口不受影响
    console.log('[Step 5] 验证更多菜单中全局知识图谱入口始终可用...');
    const menuGraphBtn = page.locator('#btn-graph-menu');
    const menuExists = await menuGraphBtn.count() > 0;
    console.log(`  -> 更多菜单 #btn-graph-menu 全局存在: ${menuExists}`);
    if (!menuExists) throw new Error('更多菜单中的全局图谱入口丢失！');

    console.log('\n[SUCCESS] 顶栏知识图谱按键智能上下文感知显示/隐藏优化全部通过验证！\n');
  } finally {
    await browser.close();
    pyServer.kill();
  }
})();
