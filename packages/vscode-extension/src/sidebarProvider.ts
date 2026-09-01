import * as vscode from 'vscode';

export interface SkillEntry {
  name?: string;
  uri: string;
  description?: string;
}

export class ReadMDToolboxProvider implements vscode.TreeDataProvider<ToolboxItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<ToolboxItem | undefined | null | void> = new vscode.EventEmitter<ToolboxItem | undefined | null | void>();
  readonly onDidChangeTreeData: vscode.Event<ToolboxItem | undefined | null | void> = this._onDidChangeTreeData.event;

  constructor(private readonly listSkills?: () => Promise<SkillEntry[]>) {}

  refresh(): void {
    this._onDidChangeTreeData.fire();
  }

  getTreeItem(element: ToolboxItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: ToolboxItem): Thenable<ToolboxItem[]> {
    if (!element) {
      return Promise.resolve([
        new ToolboxItem('语法自愈与排版规范', vscode.TreeItemCollapsibleState.Expanded, 'group_fix', [
          new ToolboxItem('一键自愈当前 Markdown 文档', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.fixCurrentDocument',
            title: '一键自愈当前文档',
          }, 'wrench'),
          new ToolboxItem('插入 [TOC] 自动内嵌目录树', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.insertToc',
            title: '插入 TOC 目录',
          }, 'list-tree'),
        ], undefined, 'wrench'),
        new ToolboxItem('Reveal.js 演说与幻灯片', vscode.TreeItemCollapsibleState.Expanded, 'group_presentation', [
          new ToolboxItem('开启全屏 Reveal.js 演说模式', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.openPresentation',
            title: '开启全屏演说',
          }, 'screen-full'),
          new ToolboxItem('插入 <!-- slide --> 幻灯片分页符', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.insertSlide',
            title: '插入分页符',
          }, 'split-horizontal'),
          new ToolboxItem('导出独立 Reveal.js 演说 HTML...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.exportPresentation',
            title: '导出演说 HTML',
          }, 'file-media'),
        ], undefined, 'screen-full'),
        new ToolboxItem('全格式文档转换与模块化', vscode.TreeItemCollapsibleState.Expanded, 'group_convert', [
          new ToolboxItem('选择本地文档转为 Markdown (Word/PDF/PPT/Excel/TXT/TeX)...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.convertAnyFilePrompt',
            title: '转换本地文档',
          }, 'file-symlink-file'),
          new ToolboxItem('抓取网页 URL 转为 Markdown...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.fetchWebToMarkdown',
            title: '抓取网页为 Markdown',
          }, 'globe'),
          new ToolboxItem('展平并编译所有 @import 模块化引用', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.processImports',
            title: '展平 @import 引用',
          }, 'references'),
        ], undefined, 'references'),
        new ToolboxItem('多格式高质量排版导出', vscode.TreeItemCollapsibleState.Expanded, 'group_export', [
          new ToolboxItem('导出为排版级 PDF / Word (.docx) / HTML / LaTeX...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.exportDocument',
            title: '导出文档',
          }, 'export'),
          new ToolboxItem('编译为学术 LaTeX 源码', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.convertToLatex',
            title: '转为学术 LaTeX',
          }, 'file-code'),
        ], undefined, 'export'),
        new ToolboxItem('交互式代码与学术工具', vscode.TreeItemCollapsibleState.Collapsed, 'group_interactive', [
          new ToolboxItem('插入交互式代码块 (Python/JS/Bash/R)...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.insertCodeChunk',
            title: '插入交互代码块',
          }, 'play'),
          new ToolboxItem('插入科学工程图表 (PlantUML/TikZ/Vega/D2)...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.insertDiagram',
            title: '插入科学工程图表',
          }, 'graph'),
          new ToolboxItem('插入 @import 子文档引用...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.insertDocImport',
            title: '插入子文档引用',
          }, 'references'),
          new ToolboxItem('插入样式与演示元数据 (Frontmatter)...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.insertFrontmatter',
            title: '插入样式元数据',
          }, 'gear'),
          new ToolboxItem('安全运行光标所在 Python 代码块 (Code Chunk)', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.runCodeChunk',
            title: '运行代码块',
          }, 'play-circle'),
          new ToolboxItem('扫描并解析当前工作区 BibTeX 参考文献...', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.parseBibtex',
            title: '解析 BibTeX 参考文献',
          }, 'references'),
        ], undefined, 'code'),
        new ToolboxItem('AI 与 MCP 智能体接入', vscode.TreeItemCollapsibleState.Expanded, 'group_mcp', [
          new ToolboxItem('一键生成工作区 MCP 配置 (Cursor/Claude/VSCode)', vscode.TreeItemCollapsibleState.None, 'cmd', [], {
            command: 'readmd.setupMcpServer',
            title: '配置 MCP Server',
          }, 'hubot'),
        ], undefined, 'hubot'),
        new ToolboxItem('ReadMD Skills 技能库', vscode.TreeItemCollapsibleState.Collapsed, 'group_skills', [], undefined, 'library'),
      ]);
    }

    if (element.contextValue === 'group_skills') {
      return this.getSkillChildren();
    }

    if (element.children) {
      return Promise.resolve(element.children);
    }

    return Promise.resolve([]);
  }

  private async getSkillChildren(): Promise<ToolboxItem[]> {
    if (!this.listSkills) {
      return [new ToolboxItem('Skill 列表不可用（未连接 ReadMD Core）', vscode.TreeItemCollapsibleState.None, 'skill_unavailable')];
    }
    try {
      return buildSkillItems(await this.listSkills());
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      return [new ToolboxItem(`读取 Skill 失败：${message}`, vscode.TreeItemCollapsibleState.None, 'skill_error')];
    }
  }
}

export function buildSkillItems(skills: SkillEntry[]): ToolboxItem[] {
  if (skills.length === 0) {
    return [new ToolboxItem('未发现可用 Skill', vscode.TreeItemCollapsibleState.None, 'skill_empty')];
  }
  return skills.map(skill => new ToolboxItem(
    skill.name || skill.uri,
    vscode.TreeItemCollapsibleState.None,
    'skill',
    [],
    { command: 'readmd.openSkillByUri', title: '打开 Skill', arguments: [skill.uri] },
    'library',
    skill.description
  ));
}

export class ToolboxItem extends vscode.TreeItem {
  constructor(
    public readonly label: string,
    public readonly collapsibleState: vscode.TreeItemCollapsibleState,
    public readonly contextValue: string,
    public readonly children: ToolboxItem[] = [],
    public readonly command?: vscode.Command,
    public readonly iconName?: string,
    public readonly itemDescription?: string
  ) {
    super(label, collapsibleState);
    if (itemDescription) {
      this.description = itemDescription;
    }
    if (iconName) {
      this.iconPath = new vscode.ThemeIcon(iconName);
    }
  }
}
