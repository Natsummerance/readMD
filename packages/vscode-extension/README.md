# ReadMD for VS Code · V2.3.7

ReadMD for VS Code 是桌面 ReadMD 的编辑器入口：在 VS Code、Cursor 和兼容的 Extension Host 中复用同一个本地 Core Service，提供 Markdown 预览、修复、转换、AI/Skills 与导出能力。扩展不会把文档上传到 ReadMD；AI、网页抓取和 GitHub Skill 导入是否联网，取决于你主动启用的 Provider 或操作。

ReadMD for VS Code is the editor front end for the ReadMD desktop app. It connects to one local Core Service so the desktop app, VS Code extension and MCP use the same document, provider, credential, Skill and history model. No document is sent to ReadMD by default. AI providers, web extraction and GitHub imports require the network only when you explicitly use them.

## 安装 / Installation

1. 在 Releases 下载与 VS Code 版本匹配的 `readmd-vscode-2.3.7.vsix`，打开命令面板执行 **Extensions: Install from VSIX...**。
2. 重载窗口（Developer: Reload Window），打开一个 `.md` 文件。
3. 第一次运行任意 ReadMD 命令时，扩展会自动启动本地 Core；也可以在设置中填写已有 Core 地址。
4. 卸载前先关闭 ReadMD 视图。卸载扩展不会删除文档或凭据；要清除数据，请在桌面应用的设置中执行数据清理。

Install the matching `readmd-vscode-2.3.7.vsix` from Releases with **Extensions: Install from VSIX...**, reload the window, and open a Markdown file. The extension starts the bundled local Core on first use, or can connect to an existing Core configured in Settings. Uninstalling the extension does not delete documents or credentials.

## 21 个命令 / Command reference

所有命令都可在 `Ctrl/Cmd+Shift+P` 搜索，也会按上下文出现在编辑器标题栏或右键菜单中。

| 命令 | 用途 |
| --- | --- |
| `ReadMD: 打开 AI Skill 工作台` (`readmd.openAiWorkbench`) | 选择 Provider、会话和 Skill，预览、应用、插入或撤销 AI 结果。 |
| `ReadMD: 浏览 Skills` (`readmd.openSkills`) | 浏览内置、用户和项目 `.readmd/skills`，查看来源与权限。 |
| `ReadMD: 打开智能实时双向预览` (`readmd.preview`) | 在分栏打开与当前编辑器绑定的实时预览。 |
| `ReadMD: 智能诊断并自愈修复格式错误` (`readmd.fixCurrentDocument`) | 生成差异并将 Markdown 修复应用到当前编辑器。 |
| `ReadMD: 插入交互式代码块` (`readmd.insertCodeChunk`) | 插入受限的代码块模板；执行前始终显示确认。 |
| `ReadMD: 插入科学与工程图表` (`readmd.insertDiagram`) | 插入 PlantUML、TikZ、Vega 等图表块。 |
| `ReadMD: 插入子文档引用` (`readmd.insertDocImport`) | 插入 `@import` 模块引用。 |
| `ReadMD: 插入文档样式与演示元数据` (`readmd.insertFrontmatter`) | 插入 frontmatter 与样式字段。 |
| `ReadMD: 开启全屏 Reveal.js 演说模式` (`readmd.openPresentation`) | 以当前文档打开演示模式。 |
| `ReadMD: 导出 Reveal.js 演说 HTML` (`readmd.exportPresentation`) | 写出可离线播放的演示 HTML。 |
| `ReadMD: 插入 [TOC] 自动目录` (`readmd.insertToc`) | 根据标题生成或更新目录。 |
| `ReadMD: 插入 <!-- slide --> 幻灯片分页符` (`readmd.insertSlide`) | 在光标处插入分页标记。 |
| `ReadMD: 展平并编译 @import 模块化引用` (`readmd.processImports`) | 预览并确认后生成展平文档。 |
| `ReadMD: 安全运行光标所在 Python 代码块` (`readmd.runCodeChunk`) | 在限制目录和超时内执行当前代码块。 |
| `ReadMD: 排版级导出文档` (`readmd.exportDocument`) | 导出 PDF、Word、HTML、LaTeX 等格式。 |
| `ReadMD: 转换为 Markdown` (`readmd.convertFileToMarkdown`) | 将已选本地文件转换为 Markdown。 |
| `ReadMD: 转换本地文档为 Markdown...` (`readmd.convertAnyFilePrompt`) | 选择文件并指定转换选项。 |
| `ReadMD: 抓取网页 URL 为 Markdown` (`readmd.fetchWebToMarkdown`) | 明确确认后抓取网页正文。 |
| `ReadMD: 一键编译转为学术 LaTeX 源码` (`readmd.convertToLatex`) | 生成可继续编辑的 `.tex` 文件。 |
| `ReadMD: 解析 BibTeX 参考文献` (`readmd.parseBibtex`) | 解析当前 BibTeX 并返回引用数据。 |
| `ReadMD: 一键配置工作区 MCP Server` (`readmd.setupMcpServer`) | 在确认后写入工作区 MCP 配置。 |

## AI、Provider 与凭据 / AI, providers and credentials

打开 **AI Skill 工作台** 后，Provider 卡片和模型列表来自 Core 的 Provider catalog，不在扩展中写死。创建自定义连接时选择协议（OpenAI Chat/Responses/Completions 或 Anthropic Messages）、Base URL、模型和能力标签；Base URL 会规范化，密钥只通过 `credential_id` 从系统凭据库读取，不会写入设置、日志、历史或 URL。连接测试只返回状态、延迟和脱敏错误。

AI 请求以统一 SSE 事件（`meta`、`delta`、`usage`、`error`、`done`）处理。生成结果默认仅预览；使用 **Apply**、**Insert** 或 **Undo** 明确决定是否修改文档。取消请求可随时停止当前会话。

After opening the AI workbench, provider cards and models are loaded from the Core catalog. Secrets are stored by the operating system credential service and referenced only by `credential_id`. The extension never writes an API key to settings, history, logs, URLs or exports. Results are previewed before Apply/Insert, and a request can be cancelled.

## Skills 与 GitHub 导入 / Skills and GitHub import

Skill 解析顺序为项目 `.readmd/skills` > 用户目录 > 内置目录。每个 Skill 是 `SKILL.md` 加可选 `readmd.skill.json`，只允许六种受限变量：`document`、`selection`、`request`、`language`、`context`、`output_format`。普通用户 Skill 的脚本永远禁用。

在桌面 ReadMD 的 Skill 工作台选择 GitHub 导入，粘贴仓库、`tree` 子目录或 `SKILL.md` 的 `blob` 链接，先预览再勾选多个 Skill。导入使用 GitHub API/归档下载，不执行 clone、hooks 或仓库脚本；固定解析后的 commit，并将来源、文件哈希和更新状态写入本地 `skills.json`。私有仓库只填写凭据 ID，Token 存入系统凭据库。更新必须手动检查、查看差异并再次确认。

Skills resolve in project, user and built-in scopes. GitHub imports are previewed, selected, pinned to a commit and copied as data; scripts remain disabled. Private repositories use a credential ID rather than a token in the config. Updates are manual and show the commit/file changes before replacement.

## MCP 与网络边界 / MCP and network boundaries

`setupMcpServer` 会为当前工作区生成 MCP 配置。MCP 暴露只读资源、动态 Skill prompts 和文档工具；写文件、覆盖、导出、联网抓取与代码执行都要求明确目标及 `confirm=true`。桌面更新、托盘、通知和窗口控制不会通过 MCP 暴露。

完全离线可用：预览、编辑、目录、公式、转换（已安装依赖范围内）、本地导出和已缓存 Skill。需要联网的操作：GitHub 导入/更新、网页抓取、检查更新，以及调用远程 AI Provider。网络失败不会删除本地文档或凭据。

The MCP setup command writes a workspace-scoped configuration. Read-only resources and dynamic Skill prompts are safe by default; side effects require an explicit confirmation. Offline editing and local rendering remain available when GitHub, web extraction or a remote AI provider is unavailable.

## 数据目录与故障排查 / Data and troubleshooting

- Core 数据目录由桌面应用决定（Windows 通常位于 `%APPDATA%\\ReadMD`，macOS 位于 `~/Library/Application Support/ReadMD`，Linux 位于 `${XDG_DATA_HOME:-~/.local/share}/ReadMD`）。扩展设置只保存连接方式，不复制凭据。
- “Core 无法连接”：执行 `Developer: Reload Window`，确认没有第二个实例占用端口，再从设置中重新连接或选择自动启动。
- “没有模型”：检查 Provider 的协议、Base URL、模型发现权限和凭据 ID；先运行连接测试。
- “Skill 不显示”：检查 `SKILL.md` frontmatter、目录名、description 是否以 `Use when` 开头，以及是否被禁用；GitHub 导入后可在桌面工作台启用。
- “导入被阻止”：重新预览并检查 commit、许可证、路径和脚本提示。压缩包超限、符号链接、目录穿越和无许可证内容会被拒绝。
- “导出失败”：确认目标目录可写，并安装对应的系统转换依赖；错误消息不会包含 API Key。

See the desktop application's diagnostics log for a redacted error and timing record. Do not paste tokens or complete local paths into issues.

## 许可证 / License

ReadMD for VS Code 使用 MIT License。上游 Skill 与 Provider 原文在发行包中随许可证和归属信息离线保存；参考项目链接仅用于归属，不是运行时依赖。反馈请提交到 [ReadMD Issues](https://github.com/Natsummerance/readMD/issues)。
