# Rust 内核 parity 复刻 · 10 并行工作包（共享规约）

生成时间 2026-09-22。上游产物：`scratch/rust_parity/contract.json`（从 `readmd.py` AST 抽取的 83 条路由权威契约）。
本文件是 10 个复刻子智能体的唯一规约来源。每个子智能体只读全文 + 执行自己那一节。

## 0. 总目标（不可协商）

把 Rust 内核 `rust/readmd-kernel` 的 HTTP 行为复刻到与 `readmd.py`（唯一权威参考实现）**逐行精度一致**。
四条硬约束：

1. **从零 Rust 实现**：禁止调用、包装、subprocess 依赖 Python；禁止新增任何需要联网构建或 C 运行时之外的依赖。
2. **100% 功能对齐**：状态码、请求参数读取方式、响应 JSON 的**键集合与嵌套形状**、错误 `error_code` 字符串、边界与降级路径，都要和 Python 一致。宁可与 JS 前端当前用法冗余，也不许先偏离 Python。
3. **写完必须自证**：给出可复跑的验证证据（单元测试通过 + 真实 HTTP 响应对照），不接受"应该可以"。
4. **更轻量化优先**：在 1、2 满足后，删除死代码、减少重复、避免不必要的分配与拷贝。不得为了轻量化牺牲 parity。

## 1. 权威契约怎么读

`scratch/rust_parity/contract.json` 每个路由一条记录：

```json
"/api/convert": {
  "verb": "?",                  // "?" = readmd.py 用同一个 dispatcher 处理 GET/POST，不要据此猜方法
  "dispatch_line": 1213,        // readmd.py 中该 elif 的行号
  "query": ["p"],               // Python 从 query string 读这些键
  "body_keys": [],              // Python 从 JSON body 读这些键（含 handler 函数内递归收集）
  "statuses": [200,400,404],    // Python 可能发出的状态码
  "resp_keys": ["content", ...],// Python 响应体里出现过的键
  "handlers": [{"py_fn":"_api_convert","py_line":2050,"body_keys":[],"query":[]}]
}
```

用法：**先用 `dispatch_line` / `handlers[].py_line` 定位 Python 函数，整函数读完（从 `py_line` 到该函数结束），再动 Rust。**
`py_line` 只给起点，函数边界用 `readmd.py` 里下一个 `def ` 或缩进回退来确定。
Python 里出现的每个 `qs.get('k')` 都必须能在 Rust 侧被接受；每个 `payload.get('k')`/`data['k']` 同理。

## 2. 已实测确认的系统性缺口（所有包共用的判断依据）

差分测量（同一请求同时打 Python 8890 与 Rust 8891）已确认：

- **请求侧契约不一致**：Python 有 15 条路由从 **query string** 取参（`/api/file?p=`、`/api/list?p=`、`/api/convert?p=`、`/api/ocr?p=`、`/api/url?u=`、`/api/bibtex?p=`、`/api/links/graph?dir&max_nodes`、`/api/links/backlinks?path`、`/api/links/deadlinks?dir`、`/api/convert/progress?job`、`/api/pets/thumb?slug`、`/api/recent/status?p`、`/api/ai/history?id`、`/api/upload?ext&name&filename`、`/api/ping?t`）。Rust 只认 JSON body 的 `path` → Python 视为缺参返回 400/404。**Rust 必须同时接受 query 与 body，query 优先，键名与 Python 完全一致。**
- **响应信封形状不一致**：`/api/ping` Python 只回 `{ok}`，Rust 多回 `engine/pid/port/uptimeMs/version/workspace/dataDir` 等。多键与缺键同样算失败。
- **状态码语义偏差**（已知样本）：`control/open` Python 403 / Rust 200（安全门缺失）；`url`、`ai/chat` 未配置、`bibtex` GET、`diagram/render` 未知引擎出现 Rust 500 或 Python 409/422 而 Rust 200/500 的错配；`pets/thumb` 缺文件 Python 404 / Rust 400。
- **Rust 侧无响应**：`export/epub` 曾出现连接被直接掐断（拿不到状态码）。这类"崩溃/断连"优先级最高。
- **`/api/kernel/status` 是 Rust 独有**，Python 404；`/api/tags` 两边都 404。不要把它们当缺口，也不要因为 Python 没有就新增未审计端点。

## 3. 环境与工具链（违反会浪费所有人的时间）

- Windows + Git Bash。**控制台输出不可信于中文**：涉及 CJK 的比较一律写文件再读回。
- `python3` 是静默 stub，必须用 `python`。
- 跑 `cargo` 之前先杀进程：`powershell -NoProfile -Command 'Get-Process readmd -EA SilentlyContinue | Stop-Process -Force'`（外层必须单引号，否则 bash 吞掉 `$_`）。
- 一律 `cargo test -p readmd-kernel --offline --lib <你的过滤词>`；构建产物在 `rust/target/`，与 9 个并行 agent 共享，**锁竞争时等待重试而不是改 target 目录**。整包 cargo 调用控制在 3 次以内。
- **禁止运行** `scratch/rust_parity/parity_diff.py`，也禁止监听 8890/8891 —— 那是主控方独占的差分台，并发跑会互相打死。
- 不要 `git add`/`commit`/`push`；不要提交任何人产出的改动。主控方统一集成。
- 不要修改 `Cargo.toml`、`Cargo.lock`、`rust/Cargo.toml`、`lib.rs`、`server.rs`、`batch2.rs`，除非该路由就是你的包（见 §4）。
- `rust/src/**` 是**不参与编译的孤儿树**，`import_processor.rs` 未在 `lib.rs` 声明同样不编译。禁止把它们当现状依据，也禁止在其中实现功能。

## 4. 文件所有权（互斥；越界即冲突）

| 包 | 只能写这些文件 | 其余全部只读 |
|---|---|---|
| P1 | `src/server.rs`, `src/lib.rs`, `src/content.rs`, `src/readmd_fix.rs` | ↓ |
| P2 | `src/convert.rs`, `src/parity_convert.rs`(新建) | |
| P3 | `src/mdexport.rs`, `src/parity_export.rs`(新建) | |
| P4 | `src/ai.rs`, `src/parity_ai.rs`(新建) | |
| P5 | `src/link_indexer.rs`, `src/parity_links.rs`(新建) | |
| P6 | `src/diagrams.rs`, `src/latex2omml.rs`, `src/parity_diagram.rs`(新建) | |
| P7 | `src/code_chunk_runner.rs`, `src/parity_code.rs`(新建) | |
| P8 | `src/store.rs`, `src/validators.rs`, `src/crypto.rs`, `src/parity_store.rs`(新建) | |
| P9 | `src/plugin_manager.rs`, `src/parity_pets.rs`(新建) | |
| P10 | `src/ocr.rs`, `src/transcribe.rs`, `src/headless_renderer.rs`, `src/parity_web.rs`(新建) | |

需要改 `server.rs` / `batch2.rs` 才能完成路由接线时：**在你自己的新建文件里实现正确的 handler，并在最终回复里逐条列出主控方必须做的接线**（`ROUTES` 条目、`use` 语句、`pub mod` 声明），不要自己动那两个文件。P1 例外：P1 独占它们。

## 5. 每个包的交付格式（严格照此返回，主控方要机械合并）

1. **改动清单**：文件 → 函数/路由，一行一条。
2. **parity 差异表**：`路由 | Python 行号 | 之前 Rust 行为 | 现在行为 | 证据`。
3. **验证证据**：真实跑过的命令 + 关键输出行（`test result: ok.` / 实际 HTTP 响应体片段）。没跑成功就写"未通过 + 原因"，禁止粉饰。
4. **待接线**：需要主控方在 `server.rs`/`batch2.rs`/`lib.rs` 应用的确切改动。
5. **未解决**：明确剩余缺口与阻塞原因（含 Python 侧因缺 `odf`/`ebooklib`/`tesseract`/`pyperclip` 而降级的项 —— 这类要 Rust 保持"无外部依赖也能给出与 Python 相同的 error_code"，不要假装功能可用）。

## 6. 逐包任务

### P1 — 分发层与请求契约（最关键，独占 server.rs）
范围：路由分发、参数抽取（query∪body，键名对齐 §2 的 15 条）、错误/成功信封构造器、状态码语义、token 校验、静态文件、`/api/ping`、`/api/modules`、`/api/modules/load`、`/api/file`、`/api/list`、`/api/save`、`/api/upload`、`/api/control/open`（Python 的 403 门）、`/api/dialog/*`、`/api/system/*`、`/api/clipboard/*`、`/api/readmd_fix`、未匹配路由的 404 体、`PENDING`/`prefix_pending`/`pendingCount` 的真实性。
另加：删除孤儿 `rust/src/**` 的误导性（只在报告里给出处置建议，不要执行删除）；把 `h_kernel_status` 之类的 Rust 私有能力标注为 Rust-only 扩展，不得混入 parity 断言。
验收：`/api/file?p=<fixture>`、`/api/list?p=<dir>`、`/api/convert?p=<file>`（走 P2 的 handler）三条 query 路径与 Python 响应键集合完全一致；`control/open` 无 token 时 403。

### P2 — 转换引擎
`/api/convert`, `/api/convert/batch`, `/api/convert/collect`, `/api/convert/progress`, `/api/convert/cancel`, `/api/batch/extract-zip`, `/api/import/process`。
必做：UTF-8 BOM 泄漏修复（`csv_to_md` 首格 `\u{feff}`）+ 回归测试；`.md` 直通语义与 Python 对齐；`Unsupported format` 的错误路径与状态码对齐；批量任务的 job 生命周期（progress/cancel 的键与状态语义）。

### P3 — 导出与样式
`/api/export`, `/api/export/epub`, `/api/export/presentation`, `/api/image/save`, `/api/style/get`, `/api/style/save`, `/api/render`。
必做：`export/epub` 绝不允许无响应/断连 —— 任何失败都必须是 Python 同状态码 + 同 `error_code`；`presentation` 的 `py=200 / rs=400` 差异；epub/pptx 手写 zip 产物的字节级结构自查。

### P4 — AI
`/api/ai/config`, `/api/ai/models`, `/api/ai/prompts`, `/api/ai/history`, `/api/ai/chat`。
必做：未配置时 Python 的 409 语义（Rust 曾回 200/500）；`stream` 分块语义与终止帧；`history` 的 `action` 分支；`models` 的多 provider 键集合。

### P5 — 链接图谱
`/api/links/index`, `/api/links/graph`, `/api/links/backlinks`, `/api/links/deadlinks`, `/api/links/extract`。
必做：query 参数（`dir`/`max_nodes`/`path`）读取；`max_nodes` 截断算法与 Python 一致；deadlinks 判定规则；返回排序稳定性。

### P6 — 图表与公式
`/api/diagram/capabilities`, `/api/diagram/render`。
必做：未知引擎 Python 422/409 而 Rust 曾 500/200；`allow_remote` 门；mermaid 等离线不可用时的降级 error_code 必须与 Python 一致；capabilities 列表逐项核对。

### P7 — 代码执行
`/api/code/run`。
必做：`confirm`/`lang`/`code`/`cwd`/`timeout` 全键对齐；被禁语言的 403 语义（当前唯一 MATCH 项，不得回退）；超时与截断输出协议。

### P8 — 存储/设置/更新/分享
`/api/recent/*`, `/api/settings*`, `/api/autostart/*`, `/api/share/*`, `/api/update/*`, `/api/system/language`, `/api/pin`, `/api/search`, `/api/stats`, `/api/wordcount`, `/api/tree`, `/raw`。
必做：SQLite/JSON 落盘键序与 Python 一致；autostart 非 Windows 的 `pending` 语义（`autostart.non-windows`）与 Python 在同类平台的返回一致；update 各阶段状态码。

### P9 — 桌宠 / Skill / 插件
`/api/pets*`, `/api/skills`, `/api/skill-imports*`, `/api/plugins/*`, `/api/upstream-sources`。
必做：`pets/thumb` 缺文件 404（非 400）；`pets/check_update` 的 `allow_network` 门；`skill-imports/preview`→`apply` 的 `selections`/`preview` 回传结构；`install`/`uninstall` 的 confirm 语义。

### P10 — 视觉/语音/网络抓取
`/api/ocr`, `/api/transcribe`, `/api/url`, `/api/web/extract`, `/api/web/cancel`。
必做：`ocr.rs` 的 `vision-not-implemented` 必须改为与 Python 缺 `tesseract` 时同状态码同 `error_code`；`url?u=&crawl=` query 读取（Rust 曾 500）；`web/extract` 与 `web/cancel` 的 409 语义。

## 7. 禁止事项清单

- 禁止"顺手重构"别人的文件；禁止为了消除 warning 而改语义。
- 禁止新增外部 crate。确有必要时：写进"未解决"，说明为什么手写实现不可行，由主控方决策。
- 禁止把 Python 的明显缺陷"修好"。缺陷也要复刻，把修复建议写进"未解决"。
- 禁止伪造证据输出。
