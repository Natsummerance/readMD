import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';

export function activate(context: vscode.ExtensionContext) {
  // 注册预览命令
  const previewDisposable = vscode.commands.registerCommand('readmd.preview', () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showInformationMessage('Please open a Markdown document first');
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
      panel.webview.html = getWebviewContent(text);
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

  // 注册自动修复命令
  const fixDisposable = vscode.commands.registerCommand('readmd.fixCurrentDocument', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;

    const doc = editor.document;
    const text = doc.getText();

    // 尝试调用 Python 核心 readmd_fix
    const pythonScript = path.join(context.extensionPath, '..', '..', 'src', 'readmd_fix.py');
    const proc = cp.spawn('python', ['-c', `
import sys, json, os
sys.path.insert(0, os.path.dirname(os.path.dirname(r'${pythonScript}')))
from src import readmd_fix
content = sys.stdin.read()
repaired, fixes, stats = readmd_fix.fix_markdown_text(content)
print(json.dumps({'repaired': repaired, 'fixes': len(fixes)}))
    `]);

    let out = '';
    proc.stdout.on('data', data => { out += data.toString(); });
    proc.stdin.write(text);
    proc.stdin.end();

    proc.on('close', code => {
      if (code === 0 && out) {
        try {
          const res = jsonParse(out);
          if (res.repaired && res.repaired !== text) {
            editor.edit(editBuilder => {
              const fullRange = new vscode.Range(
                doc.positionAt(0),
                doc.positionAt(text.length)
              );
              editBuilder.replace(fullRange, res.repaired);
            });
            vscode.window.showInformationMessage(`ReadMD: Successfully auto-corrected ${res.fixes} formatting issue(s)!`);
          } else {
            vscode.window.showInformationMessage('ReadMD: Document format is correct, no issues detected.');
          }
        } catch (e) {
          vscode.window.showErrorMessage('Failed to parse repair results');
        }
      }
    });
  });

  context.subscriptions.push(previewDisposable, fixDisposable);
}

function jsonParse(str: string) {
  return JSON.parse(str);
}

function getWebviewContent(markdown: string): string {
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>ReadMD Preview</title>
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      padding: 24px 32px;
      line-height: 1.7;
      color: var(--vscode-editor-foreground);
      background-color: var(--vscode-editor-background);
      max-width: 860px;
      margin: 0 auto;
    }
    h1, h2, h3, h4 { color: var(--vscode-editor-foreground); font-weight: 600; }
    h1 { border-bottom: 1px solid var(--vscode-widget-border, #ddd); padding-bottom: 8px; }
    code { background: var(--vscode-textCodeBlock-background, rgba(127,127,127,0.1)); padding: 2px 6px; border-radius: 4px; font-family: monospace; }
    pre code { display: block; padding: 12px; overflow-x: auto; }
    blockquote { border-left: 4px solid var(--vscode-textBlockQuote-border, #3b82f6); margin: 12px 0; padding-left: 16px; color: var(--vscode-textBlockQuote-foreground, #666); }
    table { border-collapse: collapse; width: 100%; margin: 16px 0; }
    th, td { border: 1px solid var(--vscode-widget-border, #ddd); padding: 8px 12px; text-align: left; }
    th { background: var(--vscode-textCodeBlock-background, rgba(127,127,127,0.1)); }
  </style>
  <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
</head>
<body>
  <div id="content"></div>
  <script>
    const raw = ${JSON.stringify(markdown)};
    document.getElementById('content').innerHTML = marked.parse(raw);
  </script>
</body>
</html>`;
}

export function deactivate() {}
