import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { ReadMDBridge } from './bridge';
import { ReadMDToolboxProvider } from './sidebarProvider';

let diagnosticStatusBarItem: vscode.StatusBarItem;
let coreStatusBarItem: vscode.StatusBarItem;
let coreWasDown = false;

/** Keep Core error codes out of the UI and out of user documents. */
function errorText(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error || '');
  const zh = vscode.env.language.toLowerCase().startsWith('zh');
  const messages: Record<string, [string, string]> = {
    core_process_exit: ['ReadMD Core 进程已退出，请重试', 'ReadMD Core stopped; try again'],
    core_start_timeout: ['ReadMD Core 启动超时', 'ReadMD Core startup timed out'],
    core_not_connected: ['ReadMD Core 未连接', 'ReadMD Core is not connected'],
    core_operation_timeout: ['操作超时，请重试', 'The operation timed out; try again'],
    core_closed: ['ReadMD Core 已关闭', 'ReadMD Core is closed'],
    mcp_request_failed: ['Core 请求失败', 'Core request failed'],
    mcp_tool_failed: ['Core 工具执行失败', 'Core tool failed'],
  };
  const pair = messages[code];
  return pair ? (zh ? pair[0] : pair[1]) : (zh ? '操作失败，请重试' : 'Operation failed; try again');
}

export function activate(context: vscode.ExtensionContext) {
  const bridge = new ReadMDBridge(context);
  context.subscriptions.push({ dispose: () => bridge.dispose() });

  // Native Skill/AI entry points share the same persistent ReadMD Core
  // connection as conversion and preview commands.
  const skillsDisposable = vscode.commands.registerCommand('readmd.openSkills', async () => {
    try {
      const skills = await bridge.listSkills();
      const pick = await vscode.window.showQuickPick(skills.map((s: any) => ({
        label: s.name || s.uri, description: s.description || '', uri: s.uri,
      })), { placeHolder: '选择 ReadMD Skill' });
      if (!pick) return;
      const text = await bridge.readSkill(pick.uri);
      const doc = await vscode.workspace.openTextDocument({ content: text, language: 'markdown' });
      await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
    } catch (err: any) { vscode.window.showErrorMessage(`ReadMD Skills 打开失败: ${errorText(err)}`); }
  });

  const aiWorkbenchDisposable = vscode.commands.registerCommand('readmd.openAiWorkbench', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) { vscode.window.showInformationMessage('请先打开 Markdown 文档'); return; }
    try {
      const prompts = await bridge.listPrompts();
      if (!prompts.length) {
        vscode.window.showWarningMessage('当前 Core 没有可用 Skill');
        return;
      }
      const workflow = await vscode.window.showQuickPick(prompts.map((prompt: any) => ({
        label: prompt.name || prompt.skill_id,
        description: prompt.description || prompt.skill_id || '',
        id: prompt.name || prompt.skill_id,
        skillId: prompt.skill_id || prompt.name,
      })), { placeHolder: '选择 ReadMD AI Skill 工作流' });
      if (!workflow) return;
      const providers = (await bridge.listProviders()).filter((p: any) => p.credential_id || p.key_source || p.name?.includes('Ollama'));
      if (!providers.length) {
        vscode.window.showWarningMessage('请先在 ReadMD 桌面端配置 AI 提供商和凭据');
        return;
      }
      const provider: any = await vscode.window.showQuickPick(providers.map((p: any) => ({
        label: p.name, description: p.has_key ? '已配置凭据' : '使用环境变量或本地服务', value: p,
      })), { placeHolder: '选择 AI 提供商' });
      if (!provider) return;
      const models = provider.value.models || [];
      const modelPick: any = models.length > 1
        ? await vscode.window.showQuickPick(models.map((m: string) => ({ label: m, value: m })), { placeHolder: '选择模型' })
        : models[0] ? { value: models[0] } : undefined;
      const model = modelPick?.value || '';
      if (!model) { vscode.window.showWarningMessage('当前提供商没有可用模型，请先刷新模型列表'); return; }
      const result = await bridge.aiChat({
        provider: provider.value.id, credential_id: provider.value.credential_id,
        model, skill_id: workflow.skillId, markdown_content: editor.document.getText(),
        language: vscode.env.language || 'en', stream: true,
      });
      const output = result?.content || '';
      if (!output) { vscode.window.showWarningMessage('AI 未返回可应用内容'); return; }
      const choice = await vscode.window.showInformationMessage('ReadMD AI 已生成结果', '替换选区', '插入末尾', '仅查看');
      if (choice === '替换选区') {
        await editor.edit(editBuilder => editBuilder.replace(editor.selection, output));
      } else if (choice === '插入末尾') {
        await editor.edit(editBuilder => editBuilder.insert(editor.document.positionAt(editor.document.getText().length), `\n\n${output}\n`));
      } else if (choice === '仅查看') {
        const doc = await vscode.workspace.openTextDocument({ content: output, language: 'markdown' });
        await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
      }
    } catch (err: any) { vscode.window.showErrorMessage(`ReadMD AI 工作台失败: ${errorText(err)}`); }
  });
  const openSkillByUriDisposable = vscode.commands.registerCommand('readmd.openSkillByUri', async (uri?: string) => {
    if (!uri) return;
    try {
      const text = await bridge.readSkill(uri);
      const doc = await vscode.workspace.openTextDocument({ content: text, language: 'markdown' });
      await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
    } catch (err: any) { vscode.window.showErrorMessage(`读取 Skill 失败: ${errorText(err)}`); }
  });
  context.subscriptions.push(skillsDisposable, aiWorkbenchDisposable, openSkillByUriDisposable);

  // 1. 注册侧边栏工具箱视图
  const toolboxProvider = new ReadMDToolboxProvider(() => bridge.listSkills());
  vscode.window.registerTreeDataProvider('readmdToolbox', toolboxProvider);

  // 2. 状态栏指示器
  diagnosticStatusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  diagnosticStatusBarItem.command = 'readmd.fixCurrentDocument';
  context.subscriptions.push(diagnosticStatusBarItem);

  const updateStatusBar = () => {
    const editor = vscode.window.activeTextEditor;
    if (editor && (editor.document.languageId === 'markdown' || editor.document.fileName.endsWith('.md'))) {
      diagnosticStatusBarItem.text = `$(wrench) ReadMD 自愈`;
      diagnosticStatusBarItem.tooltip = '点击运行 ReadMD 智能诊断并修复 Markdown 格式错误';
      diagnosticStatusBarItem.show();
    } else {
      diagnosticStatusBarItem.hide();
    }
  };

  vscode.window.onDidChangeActiveTextEditor(updateStatusBar, null, context.subscriptions);
  updateStatusBar();

  // Core 连接状态指示器：断线时提示，重连成功后自动隐藏
  coreStatusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 99);
  coreStatusBarItem.name = 'ReadMD Core';
  context.subscriptions.push(
    coreStatusBarItem,
    bridge.onDisconnected(() => {
      coreWasDown = true;
      coreStatusBarItem.text = '$(plug) ReadMD Core 已断开';
      coreStatusBarItem.tooltip = '下次执行操作时会自动重启并重连';
      coreStatusBarItem.show();
    }),
    bridge.onReady(() => {
      coreStatusBarItem.hide();
      if (coreWasDown) {
        coreWasDown = false;
        void vscode.window.showInformationMessage('ReadMD Core 已重新连接');
      }
    })
  );

  // 3. 命令：实时双向同步增强预览 (含 KaTeX、Mermaid、WaveDrom、代码高亮)
  const previewDisposable = vscode.commands.registerCommand('readmd.preview', () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showInformationMessage('请先在编辑器中打开一个 Markdown 文档');
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      'readmdPreview',
      `ReadMD: ${path.basename(editor.document.fileName)}`,
      vscode.ViewColumn.Beside,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      }
    );

    const updateWebview = () => {
      const text = editor.document.getText();
      panel.webview.html = getEnhancedWebviewContent(text, path.basename(editor.document.fileName));
    };

    updateWebview();
    const changeDocSubscription = vscode.workspace.onDidChangeTextDocument(e => {
      if (e.document.uri.toString() === editor.document.uri.toString()) {
        updateWebview();
      }
    });

    panel.onDidDispose(() => {
      changeDocSubscription.dispose();
    }, null, context.subscriptions);
  });

  // 4. 命令：智能自愈修复当前文档
  const fixDisposable = vscode.commands.registerCommand('readmd.fixCurrentDocument', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showWarningMessage('请先打开需要修复的 Markdown 文档');
      return;
    }

    const doc = editor.document;
    const text = doc.getText();

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: 'ReadMD 正在诊断并修复 Markdown 格式...',
      cancellable: false,
    }, async () => {
      try {
        const res = await bridge.fixMarkdown(text);
        if (res.ok && res.repaired_content && res.repaired_content !== text) {
          await editor.edit(editBuilder => {
            const fullRange = new vscode.Range(
              doc.positionAt(0),
              doc.positionAt(text.length)
            );
            editBuilder.replace(fullRange, res.repaired_content);
          });
          const detailMsg = res.fixes_count > 0 ? `（共修复 ${res.fixes_count} 处）` : '';
          vscode.window.showInformationMessage(`ReadMD: 文档格式已成功自愈！${detailMsg}`);
        } else {
          vscode.window.showInformationMessage('ReadMD: 当前文档格式规范，未检测到需要修复的语法问题。');
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`ReadMD 自愈失败: ${errorText(err)}`);
      }
    });
  });

  // 5. 命令：Reveal.js 全屏演说模式
  const presentationDisposable = vscode.commands.registerCommand('readmd.openPresentation', () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showWarningMessage('请先打开 Markdown 幻灯片文档');
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      'readmdPresentation',
      `演说: ${path.basename(editor.document.fileName)}`,
      vscode.ViewColumn.Active,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      }
    );

    const docText = editor.document.getText();
    const docTitle = path.basename(editor.document.fileName, path.extname(editor.document.fileName));
    panel.webview.html = getPresentationWebviewHtml(docText, docTitle);
  });

  // 6. 命令：导出演说 HTML
  const exportPresentationDisposable = vscode.commands.registerCommand('readmd.exportPresentation', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    const defaultUri = vscode.Uri.file(
      editor.document.fileName.replace(/\.[^/.]+$/, '') + '.slides.html'
    );

    const saveUri = await vscode.window.showSaveDialog({
      defaultUri,
      filters: { 'Reveal.js HTML Presentation': ['html'] },
      title: '导出 Reveal.js 演说 HTML',
    });

    if (!saveUri) return;

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: 'ReadMD 正在编译 Reveal.js 演说稿...',
      cancellable: false,
    }, async () => {
      try {
        const text = editor.document.getText();
        const docTitle = path.basename(editor.document.fileName, path.extname(editor.document.fileName));
        await bridge.exportPresentation(text, saveUri.fsPath, docTitle);
        const openBtn = '打开文件';
        const choice = await vscode.window.showInformationMessage(`ReadMD: 成功导出 Reveal.js 演说稿！`, openBtn);
        if (choice === openBtn) {
          vscode.env.openExternal(saveUri);
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`演说稿导出失败: ${errorText(err)}`);
      }
    });
  });

  // 7. 命令：插入 [TOC] 目录
  const insertTocDisposable = vscode.commands.registerCommand('readmd.insertToc', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    editor.edit(editBuilder => {
      editBuilder.insert(editor.selection.active, '\n[TOC]\n\n');
    });
    vscode.window.showInformationMessage('ReadMD: 已插入 [TOC] 自动目录标签');
  });

  async function getOrCreateEditor(): Promise<vscode.TextEditor | undefined> {
    let editor = vscode.window.activeTextEditor;
    if (!editor) {
      const doc = await vscode.workspace.openTextDocument({
        content: '# 新建文档\n\n',
        language: 'markdown',
      });
      editor = await vscode.window.showTextDocument(doc);
    }
    return editor;
  }

  // 8. 命令：插入分页符
  const insertSlideDisposable = vscode.commands.registerCommand('readmd.insertSlide', async () => {
    const editor = await getOrCreateEditor();
    if (!editor) return;

    editor.edit(editBuilder => {
      editBuilder.insert(editor.selection.active, '\n<!-- slide -->\n\n');
    });
  });

  // 8.1 命令：插入交互式代码块
  const insertCodeChunkDisposable = vscode.commands.registerCommand('readmd.insertCodeChunk', async () => {
    const editor = await getOrCreateEditor();
    if (!editor) return;

    const langPick = await vscode.window.showQuickPick([
      { label: 'python', description: 'Python (支持 Matplotlib 绘图与科学计算)', code: 'import matplotlib.pyplot as plt\nimport numpy as np\n\nx = np.linspace(0, 10, 100)\nplt.plot(x, np.sin(x), label="sin(x)")\nplt.legend()\nplt.show()', plot: true },
      { label: 'javascript', description: 'JavaScript (Node.js 执行环境)', code: 'const data = [10, 20, 30, 40];\nconsole.log("Sum:", data.reduce((a, b) => a + b, 0));', plot: false },
      { label: 'bash', description: 'Bash / Shell 脚本', code: '#!/usr/bin/env bash\necho "Hello ReadMD Code Chunk!"', plot: false },
      { label: 'r', description: 'R 语言科学统计与绘图', code: 'x <- seq(0, 10, by=0.1)\nplot(x, sin(x), type="l", col="blue")', plot: false },
      { label: 'go', description: 'Go 语言源码', code: 'package main\nimport "fmt"\nfunc main() {\n    fmt.Println("Hello ReadMD Go!")\n}', plot: false },
    ], { placeHolder: '请选择交互式代码块的编程语言' });

    if (!langPick) return;
    const flags = ['cmd=true'];
    if (langPick.plot) flags.push('matplotlib=true');
    const snippet = new vscode.SnippetString(`\`\`\`${langPick.label} {${flags.join(' ')}}\n\${1:${langPick.code}}\n\`\`\`\n$0`);
    editor.insertSnippet(snippet);
  });

  // 8.2 命令：插入科学与工程图表
  const insertDiagramDisposable = vscode.commands.registerCommand('readmd.insertDiagram', async () => {
    const editor = await getOrCreateEditor();
    if (!editor) return;

    const diagramPick = await vscode.window.showQuickPick([
      { label: 'plantuml', description: 'PlantUML (时序图 / 架构图 / 类图)', template: '@startuml\nautonumber\nClient -> Server: 发送请求\nServer --> Client: 返回响应 200 OK\n@enduml' },
      { label: 'tikz', description: 'TikZ / PGFPlots (LaTeX 矢量几何与函数图)', template: '\\begin{tikzpicture}\n\\draw[thick,->] (0,0) -- (4,0) node[anchor=north west] {x};\n\\draw[thick,->] (0,0) -- (0,3) node[anchor=south east] {y};\n\\draw[red,domain=0:3.5] plot (\\x,{0.2*\\x*\\x}) node[right] {$f(x)=\\frac{1}{5}x^2$};\n\\end{tikzpicture}' },
      { label: 'wavedrom', description: 'WaveDrom (数字电路时序波形图)', template: '{\n  signal: [\n    { name: "CLK",  wave: "p......" },\n    { name: "Data", wave: "x.345x.", data: ["head", "body", "tail"] },\n    { name: "Req",  wave: "0.1..0." },\n    { name: "Ack",  wave: "0..1.0." }\n  ]\n}' },
      { label: 'vega-lite', description: 'Vega-Lite (统计数据可视化图表)', template: '{\n  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",\n  "mark": "bar",\n  "data": { "values": [{"a": "A", "b": 28}, {"a": "B", "b": 55}] },\n  "encoding": { "x": {"field": "a", "type": "nominal"}, "y": {"field": "b", "type": "quantitative"} }\n}' },
      { label: 'graphviz', description: 'Graphviz DOT (网络拓扑与流程图)', template: 'digraph G {\n  rankdir=LR;\n  node [shape=box, style=rounded];\n  Start -> Process -> End;\n}' },
      { label: 'd2', description: 'D2 (现代声明式架构图)', template: 'Client -> Gateway: Request\nGateway -> Service: Process\nService -> Database: Query' },
      { label: 'bitfield', description: 'BitField (硬件寄存器与协议字段图)', template: '{\n  reg: [\n    {bits: 8, name: "IPO", type: 8},\n    {bits: 8, name: "Payload"},\n    {bits: 16, name: "CRC32", type: 2}\n  ]\n}' },
    ], { placeHolder: '请选择科学工程图表类型' });

    if (!diagramPick) return;
    const snippet = new vscode.SnippetString(`\`\`\`${diagramPick.label}\n\${1:${diagramPick.template}}\n\`\`\`\n$0`);
    editor.insertSnippet(snippet);
  });

  // 8.3 命令：插入子文档引用
  const insertDocImportDisposable = vscode.commands.registerCommand('readmd.insertDocImport', async () => {
    const editor = await getOrCreateEditor();
    if (!editor) return;

    const input = await vscode.window.showInputBox({
      prompt: '请输入被引用的相对 Markdown 文件路径 (例如 chapter1.md 或 ./sub/details.md)',
      value: 'chapter1.md'
    });
    if (!input) return;
    const snippet = new vscode.SnippetString(`@import "\${1:${input}}"\n$0`);
    editor.insertSnippet(snippet);
  });

  // 8.4 命令：插入 Frontmatter 样式与演示元数据
  const insertFrontmatterDisposable = vscode.commands.registerCommand('readmd.insertFrontmatter', async () => {
    const editor = await getOrCreateEditor();
    if (!editor) return;

    const doc = editor.document;
    if (doc.getText().startsWith('---')) {
      vscode.window.showWarningMessage('当前文档已包含 Frontmatter 头部');
      return;
    }
    const docTitle = doc.fileName ? path.basename(doc.fileName, path.extname(doc.fileName)) : '文档标题';
    const frontmatter = `---\ntitle: "${docTitle}"\nauthor: "Author"\npresentation:\n  theme: "black"\n  transition: "slide"\ncustom_css: |\n  /* 全文自定义样式 */\n---\n\n`;
    editor.edit(editBuilder => {
      editBuilder.insert(new vscode.Position(0, 0), frontmatter);
    });
    vscode.window.showInformationMessage('ReadMD: 已在文档顶部插入 Frontmatter 样式与演示元数据');
  });

  // 9. 命令：展平 @import 引用
  const processImportsDisposable = vscode.commands.registerCommand('readmd.processImports', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: 'ReadMD 正在展平并编译 @import 模块...',
      cancellable: false,
    }, async () => {
      try {
        const text = editor.document.getText();
        const baseDir = path.dirname(editor.document.fileName);
        const flattened = await bridge.processImports(text, baseDir);
        const doc = await vscode.workspace.openTextDocument({
          content: flattened,
          language: 'markdown',
        });
        await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
        vscode.window.showInformationMessage('ReadMD: 已成功展平编译全部 @import 模块！');
      } catch (err: any) {
        vscode.window.showErrorMessage(`展平模块失败: ${errorText(err)}`);
      }
    });
  });

  // 10. 命令：安全运行代码块
  const runCodeChunkDisposable = vscode.commands.registerCommand('readmd.runCodeChunk', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    const selection = editor.selection;
    let codeText = editor.document.getText(selection);

    if (!codeText.trim()) {
      // 提取光标所在的代码块
      const fullText = editor.document.getText();
      const cursorOffset = editor.document.offsetAt(selection.active);
      const codeBlockRegex = /```(?:python|py)\b[^\n]*\n([\s\S]*?)```/g;
      let match;
      while ((match = codeBlockRegex.exec(fullText)) !== null) {
        if (cursorOffset >= match.index && cursorOffset <= match.index + match[0].length) {
          codeText = match[1];
          break;
        }
      }
    }

    if (!codeText.trim()) {
      vscode.window.showInformationMessage('请将光标移至 Python 代码块内或选中需要运行的代码');
      return;
    }

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: 'ReadMD 正在安全执行代码块...',
      cancellable: false,
    }, async () => {
      try {
        const res = await bridge.runCodeChunk(codeText);
        if (res.ok) {
          let msg = res.stdout ? `输出:\n${res.stdout}` : '代码执行成功 (无标准输出)';
          if (res.images && res.images.length > 0) {
            msg += `\n[已生成 ${res.images.length} 张图表]`;
          }
          vscode.window.showInformationMessage(msg);
        } else {
          vscode.window.showErrorMessage(`代码运行异常：${errorText(res?.error_code || res?.error)}`);
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`运行失败: ${errorText(err)}`);
      }
    });
  });

  // 11. 命令：排版级导出文档 (PDF / Word / HTML / LaTeX)
  const exportDisposable = vscode.commands.registerCommand('readmd.exportDocument', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showWarningMessage('请先打开需要导出的 Markdown 文档');
      return;
    }

    const formatPick = await vscode.window.showQuickPick([
      { label: '$(file-pdf) PDF 文档 (.pdf)', value: 'pdf', description: '排版级高精度 PDF 导出' },
      { label: '$(file-text) Word 文档 (.docx)', value: 'docx', description: '包含原生 OMML 数学公式与样式' },
      { label: '$(book) EPUB 电子书 (.epub)', value: 'epub', description: '标准 EPUB 3.0 电子书 (支持微信读书/Apple Books)' },
      { label: '$(browser) 独立 HTML 网页 (.html)', value: 'html', description: '内置 KaTeX / 主题切换单文件' },
      { label: '$(file-code) 学术 LaTeX 源码 (.tex)', value: 'tex', description: '标准 pdflatex/xelatex 可编译源码' },
    ], { placeHolder: '请选择要导出的目标文件格式' });

    if (!formatPick) return;

    const presetPick = await vscode.window.showQuickPick([
      { label: 'minimal', description: '极简清爽 —— 适合日常阅读与通用笔记' },
      { label: 'academic', description: '学术论文 —— 严谨衬线排版与经典字号' },
      { label: 'report', description: '企业报告 —— 蓝调商务与结构化数据呈现' },
      { label: 'tech', description: '技术文档 —— 强调代码块与等宽阅读' },
      { label: 'warm', description: '温暖护眼 —— 暖色调与舒适字距' },
      { label: 'elegant', description: '典雅文集 —— 优雅人文质感' },
      { label: 'compact', description: '紧凑打印 —— 最大化页面空间利用率' },
    ], { placeHolder: '请选择排版样式预设' });

    if (!presetPick) return;

    const currentExt = `.${formatPick.value}`;
    const defaultUri = vscode.Uri.file(
      editor.document.fileName.replace(/\.[^/.]+$/, '') + currentExt
    );

    const saveUri = await vscode.window.showSaveDialog({
      defaultUri,
      filters: { [formatPick.label]: [formatPick.value] },
      title: `导出为 ${formatPick.value.toUpperCase()}`,
    });

    if (!saveUri) return;

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: `ReadMD 正在导出 ${formatPick.value.toUpperCase()}...`,
      cancellable: false,
    }, async () => {
      try {
        const text = editor.document.getText();
        const docTitle = path.basename(editor.document.fileName, path.extname(editor.document.fileName));
        if (formatPick.value === 'epub') {
          await bridge.exportEpub(text, saveUri.fsPath, docTitle);
        } else {
          await bridge.exportDoc(text, saveUri.fsPath, formatPick.value, presetPick.label, docTitle);
        }
        const openBtn = '打开文件';
        const choice = await vscode.window.showInformationMessage(`ReadMD: 成功导出至 ${path.basename(saveUri.fsPath)}！`, openBtn);
        if (choice === openBtn) {
          vscode.env.openExternal(saveUri);
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`导出失败: ${errorText(err)}`);
      }
    });
  });

  // 12. 命令：外部文件直接转 Markdown
  const convertFileDisposable = vscode.commands.registerCommand('readmd.convertFileToMarkdown', async (uri: vscode.Uri) => {
    let filePath = uri ? uri.fsPath : '';
    if (!filePath) {
      const picks = await vscode.window.showOpenDialog({
        canSelectMany: false,
        openLabel: '转换为 Markdown',
        filters: {
          'Supported Documents': ['docx', 'pdf', 'pptx', 'xlsx', 'tex', 'txt', 'html', 'png', 'jpg', 'webp'],
        },
      });
      if (picks && picks.length > 0) {
        filePath = picks[0].fsPath;
      }
    }

    if (!filePath) return;

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: `ReadMD 正在转换 ${path.basename(filePath)} 为 Markdown...`,
      cancellable: false,
    }, async () => {
      try {
        const markdown = await bridge.convertFile(filePath);
        const doc = await vscode.workspace.openTextDocument({
          content: markdown,
          language: 'markdown',
        });
        await vscode.window.showTextDocument(doc, vscode.ViewColumn.Active);
        vscode.window.showInformationMessage(`ReadMD: 已成功转换 ${path.basename(filePath)}！`);
      } catch (err: any) {
        vscode.window.showErrorMessage(`文档转换失败: ${errorText(err)}`);
      }
    });
  });

  const convertAnyPromptDisposable = vscode.commands.registerCommand('readmd.convertAnyFilePrompt', () => {
    vscode.commands.executeCommand('readmd.convertFileToMarkdown');
  });

  // 13. 命令：抓取网页为 Markdown
  const fetchWebDisposable = vscode.commands.registerCommand('readmd.fetchWebToMarkdown', async () => {
    const url = await vscode.window.showInputBox({
      prompt: '请输入待抓取的目标网页 URL 地址 (如 https://example.com/article)',
      placeHolder: 'https://...',
      validateInput: (text) => {
        return text && text.startsWith('http') ? null : 'URL 必须以 http:// 或 https:// 开头';
      },
    });

    if (!url) return;

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: `ReadMD 正在深度抽取网页正文...`,
      cancellable: false,
    }, async () => {
      try {
        const res = await bridge.fetchWeb(url);
        const doc = await vscode.workspace.openTextDocument({
          content: res.markdown,
          language: 'markdown',
        });
        await vscode.window.showTextDocument(doc, vscode.ViewColumn.Active);
        vscode.window.showInformationMessage(`ReadMD: 成功抓取文章《${res.title || url}》！`);
      } catch (err: any) {
        vscode.window.showErrorMessage(`抓取网页失败: ${errorText(err)}`);
      }
    });
  });

  // 14. 命令：一键编译转学术 LaTeX
  const convertLatexDisposable = vscode.commands.registerCommand('readmd.convertToLatex', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    await vscode.window.withProgress({
      location: vscode.ProgressLocation.Notification,
      title: 'ReadMD 正在编译为学术 LaTeX 源码...',
      cancellable: false,
    }, async () => {
      try {
        const text = editor.document.getText();
        const docTitle = path.basename(editor.document.fileName, path.extname(editor.document.fileName));
        const tex = await bridge.mdToLatex(text, docTitle);
        const doc = await vscode.workspace.openTextDocument({
          content: tex,
          language: 'latex',
        });
        await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
        vscode.window.showInformationMessage('ReadMD: 已成功生成标准学术 LaTeX 源码！');
      } catch (err: any) {
        vscode.window.showErrorMessage(`LaTeX 转换失败: ${errorText(err)}`);
      }
    });
  });

  // 15. 命令：扫描解析 BibTeX 参考文献
  const parseBibtexDisposable = vscode.commands.registerCommand('readmd.parseBibtex', async (uri: vscode.Uri) => {
    let bibPath = uri ? uri.fsPath : '';
    if (!bibPath) {
      const bibFiles = await vscode.workspace.findFiles('**/*.bib', '**/node_modules/**', 5);
      if (bibFiles.length > 0) {
        bibPath = bibFiles[0].fsPath;
      } else {
        const picks = await vscode.window.showOpenDialog({
          canSelectMany: false,
          filters: { 'BibTeX Database': ['bib'] },
          openLabel: '解析 BibTeX 数据库',
        });
        if (picks && picks.length > 0) {
          bibPath = picks[0].fsPath;
        }
      }
    }

    if (!bibPath) {
      vscode.window.showInformationMessage('未找到 .bib 文件');
      return;
    }

    try {
      const res = await bridge.parseBibtex(bibPath);
      const entries = res?.entries || res;
      const count = Object.keys(entries || {}).length;
      vscode.window.showInformationMessage(`ReadMD: 成功解析 BibTeX 数据库 (${path.basename(bibPath)})，共加载 ${count} 篇学术条目！`);
    } catch (err: any) {
      vscode.window.showErrorMessage(`BibTeX 解析失败: ${errorText(err)}`);
    }
  });

  // 16. 命令：一键配置工作区 MCP Server
  const setupMcpDisposable = vscode.commands.registerCommand('readmd.setupMcpServer', async () => {
    const wsFolders = vscode.workspace.workspaceFolders;
    const mcpScriptPath = bridge.getServerPath();

    const mcpConfig = {
      mcpServers: {
        readmd: {
          command: 'python',
          args: [mcpScriptPath],
          env: {
            PYTHONIOENCODING: 'utf-8',
          },
        },
      },
    };

    const choice = await vscode.window.showQuickPick([
      { label: '写入当前工作区 .vscode/mcp.json', value: 'vscode' },
      { label: '写入当前工作区 .cursor/mcp.json (Cursor IDE)', value: 'cursor' },
      { label: '复制 Claude Desktop 配置代码到剪贴板', value: 'clipboard' },
    ], { placeHolder: '请选择要配置的目标客户端' });

    if (!choice) return;

    if (choice.value === 'clipboard') {
      await vscode.env.clipboard.writeText(JSON.stringify(mcpConfig, null, 2));
      vscode.window.showInformationMessage('ReadMD: 已将 MCP 配置 JSON 复制到剪贴板，可直接粘贴进 Claude Desktop 配置文件中！');
      return;
    }

    if (!wsFolders || wsFolders.length === 0) {
      vscode.window.showWarningMessage('请先在 VSCode 中打开一个工作区文件夹');
      return;
    }

    const targetDir = path.join(wsFolders[0].uri.fsPath, choice.value === 'vscode' ? '.vscode' : '.cursor');
    const targetFile = path.join(targetDir, 'mcp.json');

    try {
      if (!fs.existsSync(targetDir)) {
        fs.mkdirSync(targetDir, { recursive: true });
      }
      fs.writeFileSync(targetFile, JSON.stringify(mcpConfig, null, 2), 'utf-8');
      vscode.window.showInformationMessage(`ReadMD: 已成功在 ${targetFile} 生成 MCP 服务配置！`);
    } catch (err: any) {
      vscode.window.showErrorMessage(`写入 MCP 配置失败: ${errorText(err)}`);
    }
  });

  context.subscriptions.push(
    previewDisposable,
    fixDisposable,
    insertCodeChunkDisposable,
    insertDiagramDisposable,
    insertDocImportDisposable,
    insertFrontmatterDisposable,
    presentationDisposable,
    exportPresentationDisposable,
    insertTocDisposable,
    insertSlideDisposable,
    processImportsDisposable,
    runCodeChunkDisposable,
    exportDisposable,
    convertFileDisposable,
    convertAnyPromptDisposable,
    fetchWebDisposable,
    convertLatexDisposable,
    parseBibtexDisposable,
    setupMcpDisposable
  );
}

function getPresentationWebviewHtml(markdown: string, title: string): string {
  const safeMarkdown = (markdown || '').replace(/<\/textarea>/gi, '&lt;/textarea&gt;');
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <title>${title}</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/reveal.css">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/theme/black.css">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.css">
</head>
<body>
  <div class="reveal">
    <div class="slides">
      <section data-markdown>
        <textarea data-template>
${safeMarkdown}
        </textarea>
      </section>
    </div>
  </div>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/reveal.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/plugin/markdown/markdown.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/plugin/math/math.js"></script>
  <script>
    Reveal.initialize({
      controls: true,
      progress: true,
      center: true,
      hash: true,
      plugins: [ RevealMarkdown, RevealMath.KaTeX ]
    });
  </script>
</body>
</html>`;
}

function getEnhancedWebviewContent(markdown: string, docTitle: string): string {
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>ReadMD: ${docTitle}</title>
  <!-- KaTeX CSS -->
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.css">
  <style>
    :root {
      --rm-font: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif;
    }
    body {
      font-family: var(--rm-font);
      padding: 28px 36px;
      line-height: 1.75;
      color: var(--vscode-editor-foreground);
      background-color: var(--vscode-editor-background);
      max-width: 880px;
      margin: 0 auto;
      word-wrap: break-word;
    }
    h1, h2, h3, h4, h5, h6 {
      color: var(--vscode-editor-foreground);
      font-weight: 600;
      line-height: 1.35;
      margin-top: 1.5em;
      margin-bottom: 0.6em;
    }
    h1 { font-size: 2em; border-bottom: 1px solid var(--vscode-widget-border, rgba(127,127,127,0.2)); padding-bottom: 0.3em; }
    h2 { font-size: 1.5em; border-bottom: 1px solid var(--vscode-widget-border, rgba(127,127,127,0.15)); padding-bottom: 0.25em; }
    p, li { font-size: 15px; }
    code {
      background: var(--vscode-textCodeBlock-background, rgba(127,127,127,0.12));
      padding: 2px 6px;
      border-radius: 4px;
      font-family: "Consolas", "Courier New", monospace;
      font-size: 13.5px;
    }
    pre {
      background: var(--vscode-textCodeBlock-background, rgba(127,127,127,0.08));
      border: 1px solid var(--vscode-widget-border, rgba(127,127,127,0.2));
      border-radius: 6px;
      padding: 14px 16px;
      overflow-x: auto;
    }
    pre code { background: transparent; padding: 0; }
    blockquote {
      border-left: 4px solid var(--vscode-textBlockQuote-border, #3b82f6);
      background: rgba(59, 130, 246, 0.05);
      margin: 16px 0;
      padding: 10px 16px;
      border-radius: 0 4px 4px 0;
    }
    table {
      border-collapse: collapse;
      width: 100%;
      margin: 20px 0;
      font-size: 14.5px;
    }
    th, td {
      border: 1px solid var(--vscode-widget-border, rgba(127,127,127,0.25));
      padding: 8px 14px;
      text-align: left;
    }
    th {
      background: var(--vscode-textCodeBlock-background, rgba(127,127,127,0.15));
      font-weight: 600;
    }
    tr:nth-child(even) {
      background: rgba(127,127,127,0.03);
    }
    img { max-width: 100%; border-radius: 6px; }
    hr {
      border: none;
      border-top: 1px solid var(--vscode-widget-border, rgba(127,127,127,0.2));
      margin: 24px 0;
    }
    .katex-display { margin: 1.2em 0; overflow-x: auto; overflow-y: hidden; }
  </style>
  <link rel="stylesheet" type="text/css" href="https://tikzjax.com/v1/fonts.css">
  <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/contrib/auto-render.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/mermaid@10.3.0/dist/mermaid.min.js"></script>
  <script src="https://tikzjax.com/v1/tikzjax.js"></script>
</head>
<body>
  <div id="content"></div>
  <script>
    const raw = ${JSON.stringify(markdown)};
    const el = document.getElementById('content');
    el.innerHTML = marked.parse(raw);
    if (window.renderMathInElement) {
      renderMathInElement(el, {
        delimiters: [
          {left: '$$', right: '$$', display: true},
          {left: '$', right: '$', display: false},
          {left: '\\\\[', right: '\\\\]', display: true},
          {left: '\\\\(', right: '\\\\)', display: false}
        ],
        throwOnError: false
      });
    }
    if (window.mermaid) {
      mermaid.initialize({ startOnLoad: true, theme: 'default' });
    }
  </script>
</body>
</html>`;
}

export function deactivate() {}
