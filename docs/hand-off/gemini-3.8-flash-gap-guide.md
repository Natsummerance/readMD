# ReadMD v2.3.8 — Gemini 3.8 Flash 交接与补缺指南

## 基线与边界

- 当前修复候选 checkpoint：`bf3f07a`（`codex/v2.3.8-remediation`）。
- 本文所在提交合并到本地 `main` 后，必须以 `git rev-parse main` 重新记录最终 SHA。
- 本轮已提交 51 个 Gemini 产品/测试文件；图表相关的 `render.js`、`readmd.boot.js`、`style.css` 和 `VERSIONS.md` 原样保留。
- `.env.local`、用户虚拟环境、测试夹具和正式源文件不是交接资产，不得提交、移动或删除。
- 当前仍是 `2.3.8-preview.1`，结论是 NO-GO；本文不是正式发布批准。

## 已验证结果

- 全量 pytest：`915 passed, 1 skipped`；唯一 skip 是 Windows 无法提供 symlink。
- i18n 结构：46 个 locale、每个 1182 keys；新增文案不再直接复制英文，但尚未完成人工语义审校。
- Prompt 来源、版本同步、隐私扫描、自检：通过。
- VSCode 测试：26/26；只覆盖 stub/Extension Host 轻量路径。
- `assets/readmd.boot.js` 与 `build_startup_bundle()` 当前一致：644341 bytes，SHA-256 `ff2663f1591c49380c10bb226a80c71e5456bde1893458fbe4e5cd2ebd20849d`。

## 必须优先修复的缺口

### P1：VSCode 本地化与真实行为

1. `packages/vscode-extension/package.json` 缺少 VS Code `l10n` manifest 声明，导致新增 `bundle.l10n.*` 不能按预期加载。
2. `extension.ts` 仍有状态栏、进度标题、代码块/图表/导入提示和 MCP 成功消息的硬编码文本。
3. `btnOpen` 结果仍与中文字面 `打开文件` 比较；英文环境下导出/演示成功后的打开动作可能失效。使用稳定 action id，不比较显示文本。
4. bundle 缺少 `noBibFound`、`mcpCopiedClipboard`、`openWorkspaceFirst`、`writeMcpFailed` 等实际使用 key；为支持语言补齐并验证占位符。
5. 新测试 stub 自行实现了错误的 l10n 调用协议；必须增加真实 Extension Host 测试，不能只依赖 stub 变绿。

### P1：图表安全与诚实状态

1. `assets/index.html` 当前 CSP 含 `unsafe-eval`，与离线安全策略冲突。不要用放宽 CSP 掩盖 Vega 运行时限制；优先恢复安全 CSP，或将 Vega 固定放到受控 sidecar。
2. `render.js` 的 `(Online Proxy)` 是硬编码英文，应使用 i18n key、无障碍名称和可测试的状态枚举。
3. PlantUML 无本地 JAR 时会请求网络；必须在用户确认后才联网，或明确显示不可用，不能与“未经授权不上传源码”的文档矛盾。
4. D2/WSD/Ditaa 继续保持不可用/降级状态，不能在能力矩阵或宣传中标为离线可用。

### P2：测试真实性与证据隔离

1. `tests/test_stress_workload.py` 的多引擎用例必须真正调用解析器并断言每个引擎的结果、错误回退和 12 引擎清单；删除无条件 `assertTrue(True)`。
2. `tests/test_vega_client_render.py` 当前只调用服务端 `render_vega_svg`，不等价于浏览器端渲染测试；新增真实 WebView/Playwright 断言。
3. `tools/record_mcp_evidence.py` 的 `readline()` 会阻塞，超时循环无法生效；改为非阻塞读取或独立读取线程，并把输出默认放到外部临时目录。
4. MCP 证据必须区分“协议模拟”与 Claude/Cursor/Cline 真实客户端连接；不得把单一 Claude Desktop 握手写成三客户端 E2E。
5. 所有截图、日志、样本和输出均使用合成内容；报告不得包含 API Key、用户名、绝对路径或完整用户文档。

## 交接后实施顺序

1. 修复 VSCode manifest、l10n key、稳定 action id 和剩余硬编码文本；重新编译并执行 Extension Host 测试。
2. 恢复图表 CSP 安全边界，确定 Vega 与 PlantUML 的离线/联网状态，补齐 i18n 和 UI 状态测试。
3. 将压力测试和 MCP 证据脚本改为真实断言、非阻塞、外部输出；清理所有旧证据输出。
4. 对 46 个 locale 做人工语义审校，特别检查 zh-HK、zh-TW、RTL 和长文本布局。
5. 重新执行完整 pytest、完整 Playwright（Chromium/Firefox/WebKit/mobile）、VSCode、MCP、转换/OCR/AI/代码沙箱和压力测试。
6. 逐平台补齐当前 commit 的原生安装、冷启动、离线启动、文件关联、签名/公证和功能证据；没有真实设备证据就保持 `pending`。
7. 只有所有 P0–P2 关闭、平台矩阵无 `pending`、产物哈希与提交一致后，才允许考虑正式版本流程。

## 必跑命令

```powershell
git diff --check main...HEAD
python -m pytest -q
python readmd.py --selftest
python tools/sync_version.py --check
python tools/check_prompt_sources.py --check
python tools/privacy_scan.py
python tools/i18n_sync.py --validate-only
python -c "from src.readmd_core.static_assets import build_startup_bundle; import pathlib; assert build_startup_bundle('.') == pathlib.Path('assets/readmd.boot.js').read_bytes()"
Push-Location packages/vscode-extension
npm test
Pop-Location
Push-Location ui-tests
node node_modules/@playwright/test/cli.js test
Pop-Location
```

所有命令都要记录 commit、环境、结果、skip 原因和产物 SHA。任何失败都必须保留原始错误并在修复后重跑，禁止删弱断言或把未执行任务标成通过。
