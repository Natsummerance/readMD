import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
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

  constructor(context: vscode.ExtensionContext) {
    this.extensionPath = context.extensionPath;
  }

  private getMcpServerPath(): string {
    return path.join(this.extensionPath, '..', 'mcp-server', 'readmd_mcp_server.py');
  }

  /**
   * 调用 MCP 工具调度器执行核心能力。
   */
  public async callMcpTool(name: string, args: Record<string, any>): Promise<any> {
    const pythonExe = await findPythonPath();
    const serverScript = this.getMcpServerPath();

    return new Promise((resolve, reject) => {
      const proc = cp.spawn(pythonExe, [serverScript], {
        env: { ...process.env, PYTHONIOENCODING: 'utf-8' },
      });

      let stdoutData = '';
      let stderrData = '';

      proc.stdout.on('data', chunk => {
        stdoutData += chunk.toString();
      });

      proc.stderr.on('data', chunk => {
        stderrData += chunk.toString();
      });

      const request = {
        jsonrpc: '2.0',
        id: 1,
        method: 'tools/call',
        params: {
          name,
          arguments: args,
        },
      };

      proc.stdin.write(JSON.stringify(request) + '\n');
      proc.stdin.end();

      const timeout = setTimeout(() => {
        proc.kill();
        reject(new Error(`ReadMD 操作超时 (${name})`));
      }, 60000);

      proc.on('close', code => {
        clearTimeout(timeout);
        if (code !== 0 && !stdoutData) {
          reject(new Error(stderrData || `进程异常退出: 退出码 ${code}`));
          return;
        }

        try {
          const lines = stdoutData.trim().split('\n').filter(Boolean);
          const lastLine = lines[lines.length - 1];
          const response = JSON.parse(lastLine);

          if (response.error) {
            reject(new Error(response.error.message || 'MCP 执行错误'));
            return;
          }

          const result = response.result;
          if (result?.isError) {
            const msg = result.content?.[0]?.text || '执行失败';
            reject(new Error(msg));
            return;
          }

          const contentText = result?.content?.[0]?.text;
          if (contentText) {
            try {
              resolve(JSON.parse(contentText));
            } catch {
              resolve(contentText);
            }
          } else {
            resolve(result);
          }
        } catch (e) {
          reject(new Error(`解析 MCP 响应失败: ${stdoutData}`));
        }
      });
    });
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
    return this.callMcpTool('readmd_web_to_markdown', { url });
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
    });
  }

  /**
   * 解析 BibTeX 文件。
   */
  public async parseBibtex(bibPath: string): Promise<any> {
    return this.callMcpTool('readmd_parse_bibtex', { bib_file_path: bibPath });
  }
}
