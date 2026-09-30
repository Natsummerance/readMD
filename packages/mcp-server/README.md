# ReadMD MCP Server

ReadMD 的 MCP 服务内置在桌面端可执行程序中：`readmd --mcp` 在标准输入输出上提供 JSON-RPC（MCP 2025-06-18 / 2025-03-26 / 2024-11-05）。它直接调用 Rust 内核里与桌面端相同的转换、导出、OCR、Skill Registry 和 AI Provider 代码，不需要 Python，也不需要打开窗口或监听端口。

## 快速开始

1. 安装 ReadMD 桌面端（或把 `readmd` 放到 PATH 中）。终端运行 `readmd --version` 确认可用。
2. 在 MCP 客户端配置中填入可执行文件的绝对路径，参数为 `--mcp`，然后重启客户端。

Windows：

```json
{
  "mcpServers": {
    "readmd": {
      "command": "C:\\Program Files\\ReadMD\\ReadMD.exe",
      "args": ["--mcp"]
    }
  }
}
```

macOS / Linux：

```json
{
  "mcpServers": {
    "readmd": {
      "command": "/usr/bin/readmd",
      "args": ["--mcp"]
    }
  }
}
```

VS Code 扩展的「ReadMD: 一键配置工作区 MCP Server」会自动探测 ReadMD 并写入同样的配置。其他客户端的模板见 `mcp_config_templates.json`。可以额外传 `--data-dir <dir>` 让 MCP 使用独立的数据目录。

## 工具

以客户端返回的 `tools/list` 为准，共 19 项：

`readmd_fix_markdown`、`readmd_convert_to_markdown`、`readmd_web_to_markdown`、`readmd_ocr_to_markdown`、`readmd_export_document`、`readmd_latex_to_md`、`readmd_md_to_latex`、`readmd_parse_bibtex`、`readmd_latex_to_omml`、`readmd_ai_assistant`、`readmd_ai_providers`、`readmd_ai_chat`、`readmd_process_imports`、`readmd_generate_toc`、`readmd_export_presentation`、`readmd_export_epub`、`readmd_run_code_chunk`、`readmd_pdf_audit`、`readmd_pdf_rollback`。

`resources/list` 公开 Skills（`readmd://skills/<id>`）、Provider 目录（`readmd://providers`，不含密钥）和本地会话记录（`readmd://sessions`）；`prompts/list` 与当前 Skill Registry 一一对应。旧的 workflow id（如 `polish`、`summary`）仍可在 `prompts/get` 和 `readmd_ai_assistant` 中使用。

## 安全边界

下列工具有副作用，参数必须包含 `"confirm": true`，否则返回 `confirmation_required`：

- `readmd_web_to_markdown`（联网）
- `readmd_export_document`、`readmd_export_presentation`、`readmd_export_epub`（写文件）
- `readmd_run_code_chunk`（执行代码）
- `readmd_pdf_rollback`（改写 PDF）

输出路径必须是绝对路径，扩展名须与格式一致，父目录必须存在，路径上不允许符号链接；目标已存在时只有 `"overwrite": true` 才会替换，否则返回 `output_exists`。`readmd_ai_chat` 只接受 `credential_id`，传入原始 API Key 会被拒绝。MCP 不暴露更新、托盘、开机启动、通知和窗口控制。

错误以 `{"ok": false, "error_code": "..."}` 返回（`isError: true`），协议错误使用标准 JSON-RPC 代码；同时运行的工具超过 8 个时返回 `-32001 server_busy`。客户端发送 `notifications/cancelled` 后，被取消的请求不再返回结果。

## 故障排查

- 客户端显示进程立即退出：在终端直接运行 `readmd --mcp`，输入一行 `{"jsonrpc":"2.0","id":1,"method":"ping"}` 应返回 `{"jsonrpc":"2.0","id":1,"result":{}}`。
- `--mcp 不能与 … 同时使用`：`--mcp` 不能与 `--browser`、`--selftest`、`--share` 等界面或自检参数混用。
- AI Provider 为空：先在同一用户账户的 ReadMD 桌面端保存 Provider 和凭据。
- OCR 返回 `ocr_no_engine`：当前平台没有系统 OCR 引擎（Windows 10+ 使用系统自带的 Windows.Media.Ocr）。

标准输出只承载协议消息，诊断信息写入标准错误。

旧的 Python 实现 `readmd_mcp_server.py` 已删除，由 `readmd --mcp` 取代。
