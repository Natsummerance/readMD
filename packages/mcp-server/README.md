# ReadMD MCP Server

ReadMD 的官方 MCP (Model Context Protocol) 插件服务，让 Claude Desktop、Cursor、Antigravity 等 AI 编程助手具备原生级 Markdown 智能自愈修复、学术 LaTeX 互转、BibTeX 交叉引用与多格式文档（Word/PDF/PPT/Excel）转 Markdown 能力。

## ️ 提供的 MCP Tools

1. **`readmd_fix_markdown`**：深度扫描并修复 Markdown 文本格式（修复公式、未闭合围栏、错位表格、标点空白等）。
2. **`readmd_convert_to_markdown`**：将 docx, pdf, pptx, xlsx, tex, html 等本地文件转换为标准 Markdown。
3. **`readmd_latex_to_md`**：LaTeX 源码转换为 Markdown。
4. **`readmd_md_to_latex`**：Markdown 转换为可直接编译的标准学术 LaTeX。
5. **`readmd_parse_bibtex`**：解析 `.bib` 文献数据库并提取标准引用。

## 配置示例

在 Claude Desktop 或 Cursor 的 MCP 配置文件（如 `claude_desktop_config.json`）中添加：

```json
{
 "mcpServers": {
 "readmd": {
 "command": "python",
 "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]
 }
 }
}
```
