# ReadMD MCP Server

ReadMD MCP 是随发布包离线提供的 stdio 服务。它复用桌面端同一套文档核心、AI Provider、Skill Registry 和错误模型，不需要启动浏览器，也不会读取或修改其他软件的配置。

## 快速开始

1. 解压 `readmd-mcp-server-<version>.zip`，确认目录中包含 `readmd_mcp_server.py`、`src/`、`assets/skills/` 和本文件。
2. 确认 Python 3.11 可用：`python --version`。
3. 将下面配置中的路径改成解压目录内脚本的绝对路径，然后重启 MCP 客户端。

Windows：

```json
{
  "mcpServers": {
    "readmd": {
      "command": "python",
      "args": ["C:\\Tools\\ReadMD-MCP\\readmd_mcp_server.py"]
    }
  }
}
```

macOS / Linux：

```json
{
  "mcpServers": {
    "readmd": {
      "command": "python3",
      "args": ["/opt/readmd-mcp/readmd_mcp_server.py"]
    }
  }
}
```

路径中不要使用示例占位符。Windows JSON 路径中的反斜杠必须写成 `\\`；也可以改用 `/`。

## 能力

- 文档：格式诊断与修复、目录生成、`@import` 展开。
- 转换：DOCX、PDF、PPTX、XLSX、HTML、TXT、LaTeX 转 Markdown。
- 导出：PDF、DOCX、HTML、LaTeX、EPUB 和 Reveal.js 演示文稿。
- 学术：LaTeX/Markdown 互转、LaTeX 转 OMML、BibTeX 解析。
- 本地能力：OCR、受限代码块执行。
- AI：动态读取桌面端 Provider，列出并调用同一组 Skills；MCP `prompts/list` 与当前 Skill Registry 一一对应，不内置另一套写死提示词。
- Resources：只读公开当前文档、会话、Skills 元数据和离线上游来源信息。

当前工具列表以客户端返回的 `tools/list` 为准。常用工具包括：

`readmd_fix_markdown`、`readmd_convert_to_markdown`、`readmd_web_to_markdown`、`readmd_ocr_to_markdown`、`readmd_export_document`、`readmd_latex_to_md`、`readmd_md_to_latex`、`readmd_parse_bibtex`、`readmd_latex_to_omml`、`readmd_ai_assistant`、`readmd_ai_providers`、`readmd_ai_chat`、`readmd_process_imports`、`readmd_generate_toc`、`readmd_export_presentation`、`readmd_export_epub`、`readmd_run_code_chunk`。

## 安全边界

读取、分析和内存内转换默认可用。下列操作有副作用，调用参数必须明确包含 `"confirm": true`：

- 联网抓取网页；
- 写入或覆盖导出文件；
- 生成演示文稿或 EPUB 文件；
- 执行代码块。

文件工具只处理调用中明确给出的路径。MCP 不暴露桌面应用更新、托盘、开机启动、通知和窗口控制。AI 密钥不会出现在工具结果、URL、历史或导出配置中；服务只使用 ReadMD 配置保存的 `credential_id`。

## 使用示例

让客户端先列出工具或 Skills，再发出任务，例如：

- “使用 ReadMD 检查这段 Markdown，只返回修复后的内容。”
- “列出 ReadMD Skills，并用选中的 Skill 处理当前文档。”
- “把 `report.docx` 转成 Markdown，先不要覆盖任何已有文件。”
- “将本文导出到明确路径；确认写入后设置 `confirm=true`。”

## 网络与离线说明

格式修复、本地转换、OCR、目录、LaTeX、BibTeX、Skills 枚举和本地导出可离线运行，但某些格式需要系统中已有的可选转换组件。网页抓取、GitHub Skill 导入和远程 AI Provider 需要网络。AI 功能还需要先在 ReadMD 桌面端配置 Provider 和凭据。

## 故障排查

- 客户端显示进程立即退出：在终端直接运行配置中的 `command` 和 `args`，检查 Python 与脚本路径。
- 找不到模块：必须从完整 MCP ZIP 解压运行，不能只复制单个 Python 文件。
- Windows 路径解析失败：使用绝对路径，并正确转义 JSON 反斜杠。
- AI Provider 为空：先在同一用户账户的 ReadMD 桌面端保存 Provider 设置。
- 写文件被拒绝：确认目标路径明确，并仅对预期副作用传入 `confirm=true`。
- 客户端缓存旧能力：完全退出并重新启动 Claude Desktop、Cursor 或 Cline，再重新连接服务。

服务使用标准输入输出通信。不要向标准输出添加调试文本；诊断信息应写入标准错误。
