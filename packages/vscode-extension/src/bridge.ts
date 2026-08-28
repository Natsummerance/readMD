import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import { findPythonPath } from './pythonFinder';

export interface FixResult {
  ok: boolean;
  repaired_content: string;
  fixes_count: number;
  fixes_details: string[];
  stats: Record<string, number>;
}

export interface WebResult {
  ok: boolean;
  title: string;
  markdown: string;
  url: string;
  images_count: number;
}

export class ReadMDBridge {
  private extensionPath: string;
  private proc?: cp.ChildProcessWithoutNullStreams;
  private starting?: Promise<void>;
  private nextId = 1;
  private buffer = '';
  private pending = new Map<number, { resolve: (value: any) => void; reject: (reason?: any) => void; timer: NodeJS.Timeout }>();

  constructor(context: vscode.ExtensionContext) {
    this.extensionPath = context.extensionPath;
  }

  private getMcpServerPath(): string {
    const configured = vscode.workspace.getConfiguration('readmd').get<string>('mcpServerPath', '');
    if (configured) return configured;
    const packaged = path.join(this.extensionPath, 'core', 'mcp-server', 'readmd_mcp_server.py');
    if (fs.existsSync(packaged)) return packaged;
    return path.join(this.extensionPath, '..', 'mcp-server', 'readmd_mcp_server.py');
  }

  public getServerPath(): string { return this.getMcpServerPath(); }

  private async ensureProcess(): Promise<void> {
    if (this.proc && !this.proc.killed) return;
    if (this.starting) return this.starting;
    this.starting = (async () => {
      const pythonExe = await findPythonPath();
      const serverScript = this.getMcpServerPath();
      const proc = cp.spawn(pythonExe, [serverScript], {
        env: { ...process.env, PYTHONIOENCODING: 'utf-8' },
        stdio: ['pipe', 'pipe', 'pipe'],
      });
      this.proc = proc;
      proc.stdout.on('data', chunk => this.consumeOutput(chunk.toString()));
      proc.stderr.on('data', chunk => { /* protocol responses stay on stdout */ void chunk; });
      proc.on('error', err => this.failProcess(err));
      proc.on('close', code => this.failProcess(new Error(`ReadMD Core 进程异常退出: ${code ?? 'unknown'}`)));
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('ReadMD Core 启动超时')), 10000);
        proc.once('spawn', () => { clearTimeout(timer); resolve(); });
        proc.once('error', err => { clearTimeout(timer); reject(err); });
      });
    })().finally(() => { this.starting = undefined; });
    return this.starting;
  }

  private consumeOutput(chunk: string): void {
    this.buffer += chunk;
    let idx = this.buffer.indexOf('\n');
    while (idx >= 0) {
      const line = this.buffer.slice(0, idx).trim();
      this.buffer = this.buffer.slice(idx + 1);
      if (line) {
        try {
          const response = JSON.parse(line);
          const id = Number(response.id);
          const waiter = this.pending.get(id);
          if (waiter) {
            this.pending.delete(id); clearTimeout(waiter.timer);
            if (response.error) waiter.reject(new Error(response.error.message || 'MCP 执行错误'));
            else waiter.resolve(response.result);
          }
        } catch { /* ignore partial/non-protocol output */ }
      }
      idx = this.buffer.indexOf('\n');
    }
  }

  private failProcess(error: Error): void {
    if (this.proc && this.proc.exitCode === null) return;
    this.proc = undefined;
    for (const waiter of this.pending.values()) { clearTimeout(waiter.timer); waiter.reject(error); }
    this.pending.clear(); this.buffer = '';
  }

  /**
   * 调用 MCP 工具调度器执行核心能力。
   */
  public async callMcpTool(name: string, args: Record<string, any>): Promise<any> {
    const result = await this.callMcpMethod('tools/call', { name, arguments: args });
    if (result?.isError) throw new Error(result.content?.[0]?.text || '执行失败');
    const text = result?.content?.[0]?.text;
    try { return text ? JSON.parse(text) : result; } catch { return text || result; }
  }

  /** Call a persistent MCP JSON-RPC method (resources/prompts included). */
  public async callMcpMethod(method: string, params: Record<string, any> = {}): Promise<any> {
    await this.ensureProcess();
    const proc = this.proc;
    if (!proc || !proc.stdin.writable) throw new Error('ReadMD Core 未连接');
    const id = this.nextId++;
    const request = { jsonrpc: '2.0', id, method, params };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`ReadMD 操作超时 (${method})`)); }, 60000);
      this.pending.set(id, { resolve, reject, timer });
      proc.stdin.write(JSON.stringify(request) + '\n');
    });
  }

  public async listSkills(): Promise<any[]> {
    const result = await this.callMcpMethod('resources/list');
    return result?.resources || [];
  }

  /** Return the Core's current Skill-backed prompt descriptors. */
  public async listPrompts(): Promise<any[]> {
    const result = await this.callMcpMethod('prompts/list');
    return Array.isArray(result?.prompts) ? result.prompts : [];
  }

  public async readSkill(uri: string): Promise<string> {
    const result = await this.callMcpMethod('resources/read', { uri });
    return result?.contents?.[0]?.text || '';
  }

  public async getPrompt(workflowId: string, markdownContent: string, request = ''): Promise<any> {
    return this.callMcpMethod('prompts/get', { name: workflowId, arguments: { markdown_content: markdownContent, request } });
  }

  public async listProviders(): Promise<any[]> {
    const result = await this.callMcpTool('readmd_ai_providers', {});
    return result?.providers || [];
  }

  public async aiChat(args: Record<string, any>): Promise<any> {
    return this.callMcpTool('readmd_ai_chat', args);
  }

  public dispose(): void {
    for (const waiter of this.pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error('ReadMD Core 已关闭')); }
    this.pending.clear(); this.proc?.kill(); this.proc = undefined;
  }

  /**
   * 一键自愈当前 Markdown 文本。
   */
  public async fixMarkdown(content: string): Promise<FixResult> {
    return this.callMcpTool('readmd_fix_markdown', { content });
  }

  /**
   * 本地文件转 Markdown。
   */
  public async convertFile(filePath: string): Promise<string> {
    return this.callMcpTool('readmd_convert_to_markdown', { file_path: filePath });
  }

  /**
   * 网页 URL 抓取并转为 Markdown。
   */
  public async fetchWeb(url: string): Promise<WebResult> {
    return this.callMcpTool('readmd_web_to_markdown', { url, confirm: true });
  }

  /**
   * 导出文档。
   */
  public async exportDoc(markdown: string, outputPath: string, format: string, preset: string, title?: string): Promise<any> {
    return this.callMcpTool('readmd_export_document', {
      markdown_content: markdown,
      output_path: outputPath,
      output_format: format,
      style_preset: preset,
      title: title || 'ReadMD Document',
      confirm: true,
    });
  }

  /**
   * Markdown 转学术 LaTeX。
   */
  public async mdToLatex(markdown: string, title?: string): Promise<string> {
    return this.callMcpTool('readmd_md_to_latex', {
      markdown_content: markdown,
      doc_title: title || 'ReadMD Paper',
    });
  }

  /**
   * LaTeX 转 Markdown。
   */
  public async latexToMd(latex: string): Promise<string> {
    return this.callMcpTool('readmd_latex_to_md', { latex_content: latex });
  }

  /**
   * 解析并展平 @import 模块化导入。
   */
  public async processImports(content: string, baseDir: string): Promise<string> {
    return this.callMcpTool('readmd_process_imports', {
      markdown_content: content,
      base_dir: baseDir,
    });
  }

  /**
   * 生成 [TOC] 目录树。
   */
  public async generateToc(content: string, depthFrom = 1, depthTo = 6, ordered = false): Promise<string> {
    return this.callMcpTool('readmd_generate_toc', {
      markdown_content: content,
      depth_from: depthFrom,
      depth_to: depthTo,
      ordered_list: ordered,
    });
  }

  /**
   * 导出 Reveal.js 演说幻灯片 HTML。
   */
  public async exportPresentation(content: string, outputPath: string, title?: string, theme = 'black', transition = 'slide'): Promise<any> {
    return this.callMcpTool('readmd_export_presentation', {
      markdown_content: content,
      output_path: outputPath,
      title: title || 'ReadMD Presentation',
      theme,
      transition,
      confirm: true,
    });
  }

  /**
   * 导出标准 EPUB 3.0 电子书。
   */
  public async exportEpub(content: string, outputPath: string, title?: string, author?: string, language = 'zh-CN'): Promise<any> {
    return this.callMcpTool('readmd_export_epub', {
      markdown_content: content,
      output_path: outputPath,
      title: title || 'ReadMD 电子书',
      author: author || 'ReadMD Author',
      language,
      confirm: true,
    });
  }

  /**
   * 安全执行代码块。
   */
  public async runCodeChunk(code: string, language = 'python', capturePlot = true): Promise<any> {
    return this.callMcpTool('readmd_run_code_chunk', {
      code,
      language,
      capture_plot: capturePlot,
      confirm: true,
    });
  }

  /**
   * 解析 BibTeX 文件。
   */
  public async parseBibtex(bibPath: string): Promise<any> {
    return this.callMcpTool('readmd_parse_bibtex', { bib_file_path: bibPath });
  }
}
