# Rust 内核重写 · 交接文档（致 Qwen3.8max 接手会话）

日期：2026-09-22
仓库：`T:\Programming\Project\codex\creator\readmd`，分支 `main`，HEAD `0c3e501`
交接人：上一轮 Qoder 会话（已完成实测核查，未提交任何 Rust 代码）

---

## 0. 先做这件事：把 `rust/` 提交

`git ls-files rust/` 返回 **0**。整个 Rust 内核树是未跟踪状态——包括 2.2 万行源码、`Cargo.lock`、构建脚本。任何一次误删、`git clean`、或换机器，重写成果就没了。

接手后第一条命令应该是（确认 `.gitignore` 没有把 `rust/target/` 之外的东西排除）：

```bash
git add rust/ && git status   # 先看清 staged 内容，排除 target/
git commit -m "feat(kernel): add rust rewrite workspace"
```

`rust/target/` 不要提交（23MB debug + 8.8MB release 二进制）。

---

## 1. 任务目标（用户原始要求，未变）

> 直接用 rust 重写整个项目……目标是用 rust 从零重写整个项目，搞个 rust 内核原生多平台版本出来；写完功能自己验证一下，要和我 GitHub 上面的所有功能保持一致，如果能升级，做到更加轻量化是最好的；没完成之前不要停止。

四条硬约束：
1. **从零重写**，不是包裹 Python。
2. **功能与 GitHub `main` 100% 对齐**（Python 侧 `readmd.py` + `assets/js/` 是验收基准）。
3. **自己验证**，不接受"写完即完成"。
4. 更轻量化优先（已见成果：release 二进制 8.8 MB）。

---

## 2. 唯一的权威代码树

| 路径 | 状态 |
|---|---|
| `rust/readmd-kernel/` | ✅ **权威**。workspace 唯一 member，编译、测试、跑起来都在这里 |
| `rust/src/` | ⚠️ **孤立重复树**，完全没接入构建，从未编译或测试 |
| `packages/readmd-pet-rust/` | 桌宠 overlay 进程（tao/wry），与内核是**两个独立进程**，见 §9 |

`rust/Cargo.toml` 证据：

```toml
[workspace]
resolver = "2"
members = ["readmd-kernel"]        # ← rust/src 不在这里
[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
opt-level = "s"                     # ← 轻量化已配置
```

`rust/src/` 里有 `convert.rs`(286) `ai.rs`(315) `bibtex.rs`(415) `code_chunk_runner.rs`(478) `ocr.rs`(559) `diagrams.rs`(462) `latex2omml.rs`(936) `mdexport_docx.rs`(862) `pdf_editor.rs`(824) `skill_import.rs`(729) `plugin_manager.rs`(675) `protocol.rs`(742) `security/mod.rs`(602) `runtime.rs`(594) `bridge/*` `webview/*`。

**接手必读警告**：这些文件名字和 `readmd-kernel/src/` 里的模块同名。grep 命中时先确认路径，否则会读到死代码并据它做判断。它们与权威实现行数差异极大（如 `convert.rs` 286 vs 4236），说明是早期草稿。**建议第一件事就是 `git rm -r rust/src` 或移入 `rust/_archive/`**，但先 diff 一遍确认没有权威树缺失的能力。

### 模块清单（`wc -l`，权威树）

```
convert.rs            4236   server.rs   4124   batch2.rs   1921
mdexport.rs           1331   content.rs  1046   code_chunk_runner.rs 971
link_indexer.rs        908   main.rs      877   latex2omml.rs  861
readmd_fix.rs          835   ocr.rs       697   lib.rs         684
store.rs               658   ai.rs        656   import_processor.rs 454
validators.rs          349   crypto.rs    343   transcribe.rs  250
plugin_manager.rs      199   headless_renderer.rs 108   build.rs 10
```

`lib.rs:7-25` 声明 19 个 `pub mod`，另有 4 个内联模块：`process`(30) `error`(47) `paths`(157) `settings`(487)。

**结构缺陷**：`import_processor.rs`（454 行）**没有在 `lib.rs` 里声明**，是死文件。要么补 `pub mod import_processor;` 并接进 `/api/import/process`，要么删掉。`/api/import/process` 已在路由表且报 implemented，所以先查它实际走的是哪份实现。

**没有 `batch3.rs`**（部分历史总结误记为 `pub mod batch3;`）。

---

## 3. 实测状态（本轮全部重新量过，非引用旧结论）

### 编译与测试

- `cargo check` → **0 error，81 warning**（其中 55 个可 `cargo fix --lib -p readmd-kernel` 自动修）
- `cargo test` → **115 passed / 0 failed / 0 ignored**，14.30s；`main.rs` 0 测试，2 个 doc-test ignored
- 测试覆盖：python exec、SQL 行/单元格上限、输出截断、10s 超时 kill、真实 HTTP 回环、路径穿越拒绝、token 注入、404-vs-501 路由、PDF 转换（含 `test_bjtu_internship_documents`）、vega 校验、capabilities 探测、whisper 探测

上一轮记录的 5 个编译错误（`code_chunk_runner.rs` E0308/E0277/E0282/E0597/E0382）**已不存在**，该文件 4 个测试全绿。历史总结中"无法做功能验证"的结论是错的。

### 具体 warning（值得清）

```
diagrams.rs:66    DiagramEngineInvalid  variant never constructed
diagrams.rs:83    MAX_SVG_SIZE  unused constant
link_indexer.rs:240  db_path field never read
server.rs:1455    fn module(id,state,note) never used
server.rs:3344    struct ShareSession.root field never read
```

---

## 4. 真机 HTTP 验证（本轮新增，之前只有进程内单测）

release 二进制确实能作为独立服务器跑起来并正确响应：

```bash
./rust/target/release/readmd.exe --no-window --host 127.0.0.1 --port 8791 --data-dir "$TEMP/readmd-probe"
# ReadMD rust kernel 2.4.0 listening on http://127.0.0.1:8791/
#   data / workspace / assets 路径 + 随机 token 都会打印到 stdout
```

CLI 解析在 `main.rs:52` `fn parse_args()`，可用 flag（`main.rs:71-85`）：`--host --port --data-dir --workspace|--workspace-dir --assets|--assets-dir --no-window|--browser --require-token -h|-V`。

| 端点 | 结果 |
|---|---|
| `GET /api/ping` | 200，`{engine:"rust",pid,port,uptimeMs,version:"2.4.0",dataDir,workspace,pong:true}` |
| `GET /api/kernel/status` | 200，见下 |
| `GET /api/list` | 200，返回目录扫描结果（含 kind/ext/mtime） |
| `GET /api/modules` | 200，`{ai:convert:ocr:web = "ready", win7:false}` |
| `GET /api/diagram/capabilities` | 200，`mermaid/katex/mathjax:true`，`graphviz/plantuml/serverSideRender:false`，note 明确"服务端渲染未移植" |
| `GET /api/pets` | 200，`active:"hermes"` + catalog（**已确认为合法 UTF-8**，见 §7 编码条目） |
| `GET /api/autostart/get` | 200，`{enabled:false}` |
| `POST /api/code/run` | **200**，`{"ok":true,"stdout":"42\r\n","exit_code":0,"lang":"python"}` |
| `POST /api/convert` | **200**（csv→markdown 表格正确） |
| `GET /api/tags` | **404 `unknown_route`** ← 唯一确认的功能缺口 |

`/api/kernel/status` 返回：`implementedCount: 107`，`legacyTotal: 107`，`pending: []`，`pendingCount: 0`。

### 端点契约（实测踩到的两个坑，务必传给前端）

1. **`/api/code/run` 需要确认字段**：缺确认时返回 `400 {"error_code":"confirmation_required"}`。实测 **`confirm: true`** 可通过（`confirmed/allow/acknowledged` 未验证，别猜，读 handler）。
2. **`/api/convert` 只有入参 `path`（+ 可选 `form_tables`），没有出参 `format`**。它是"任意文件 → Markdown"单向转换，输出格式由**文件扩展名**推断（`convert.rs:212` `convert_verbose`，`convert.rs:259` 落 `Unknown` 时报 422 `Unsupported format`）。传 `{"path":x,"format":"html"}` 会被忽略 `format`；传 `.md` 文件会 422。
   → **待核**：Python 侧对 `.md` 输入是否为恒等直通。若是，这是 parity 缺口。

---

## 5. 端点 parity：数字与真实含义

- 消费侧（Python + `assets/js/`）出现 **84** 个不同 `/api/` 端点
- 服务端（`readmd-kernel/src/*.rs`）出现 **119** 个 `/api/` 字面量
- 差集里唯一真缺口：**`/api/tags`**（实测 404）。`/api/recent/` 是 trailing-slash 假阳性。
- Rust 反向多出约 35 个端点（Python 侧无调用方），属"新增未接线"，不是缺口。

### `pendingCount: 0` 低报了真实状态 —— 关键机制

`server.rs:533`：

```rust
pub const PENDING: &[&str] = &[];          // 空
```

但 `prefix_pending()`（`server.rs:596-615`）硬编码 10 个家族前缀 + 特殊处理 `upstream-sources`：
`skill-imports, upstream-sources, pets, convert, update, share, export, diagram, plugins, modules`。

**它只在精确路由查找失败后的分支里触发**（`server.rs:536-573` 的 dispatch 顺序），也就是说：

```
OPTIONS → 静态文件 → token 校验 → 去尾斜杠查表 → 原路径查表 → 命中即执行
      └─ 未命中：upstream 动态 → prefix_pending ? 501 pending : 404 unknown_route
```

结论：`prefix_pending` 是 **"用 501 代替 404"的标记**，不是遮蔽门。已注册的路由照常执行。
副作用：**`pendingCount: 0` 不表示没有缺口**，它只表示 `PENDING` 常量数组为空。真正"某个家族里哪些子路径没实现"要按家族比对（pets 15 个、convert 7、update 6、plugins 5、upstream-sources 5、skill-imports 4、share 4、export 4、modules 4、diagram 3）。改 parity 探针时别只读 `pendingCount`。

唯一确认的**平台性真实缺口**：`ApiError::pending("autostart.non-windows")`（`server.rs:1397`、`1411`）——非 Windows 开机自启未实现。

---

## 6. 依赖：一条项目记忆已经过期

`rust/readmd-kernel/Cargo.toml` 实测已含（全部能编译）：

```
base64 0.22.1   flate2 1.1.0   curl 0.4.47   ureq 2.12.1 (json)
reqwest 0.12.26 (blocking)   aes-gcm 0.11.1
pdf-extract 0.12.1   lopdf 0.42.0   rusqlite 0.40.2 (bundled)
actix-web 4.11.0   tao 0.37.0   wry 0.57.0 (os-webview)
pulldown-cmark 0.13.4 (default-features=false, features=["html"])
dunce   tempfile(dev)   winres(build)
```

除 `pdf-extract/lopdf/aes-gcm/winres` 外全部 `=` 精确锁版。

项目记忆 `rust-kernel-offline-constraints.md` 声称"离线缓存没有 TLS/zip/base64，因此手写 HTTP 与 curl 出网"——**已不成立**，那些依赖都在并且构建通过。接手时**先更新或删掉这条记忆**，否则会照着它做无必要的手写实现。另一条 `handoff-luna-max-7-items.md` 引用的 `.qoder-scratch/handoff-plan-luna-max.md` 在磁盘上不存在，也已失效。

---

## 7. 已确认的一个缺陷（本轮发现，未修）

**`csv_to_md` 不剥离 UTF-8 BOM。**

```
无 BOM 的 bom-free.csv →  "# nobom.csv\n\n| name | qty | ..."     ✅
带 BOM 的 csv         →  "# bom.csv\n\n| \ufeffname | qty | ..."  ❌ BOM 混进首个表头单元格
```

Python 侧普遍用 `encoding="utf-8-sig"` 读 CSV，会自动吃掉 BOM，所以这是**行为背离**，不只是观感问题——导出后再传给下游会污染列名。修法：`csv_to_md` 读文本后 `trim_start_matches('\u{feff}')`，并把带 BOM 的用例补进 `convert.rs` 的测试。

同类待查：DOCX/XLSX/EPUB 路径是否有相同 BOM/编码假设。

**顺带排除一个假警报**：`/api/pets` 的中文在终端显示为乱码，是我 Git Bash 的 GBK 控制台所致；直接校验响应字节是**合法 UTF-8**，服务端无问题。在这个环境里看 CJK 输出请写文件再读，别凭终端观感下结论。

---

## 8. 这个环境里的操作陷阱（都是踩过的）

- `python3` 是静默 stub，只用 `python`。
- **`taskkill //F //IM readmd.exe` 会被 Bash 工具的路径校验拒绝**（`Command contains a UNC path`）。杀进程改用：
  `powershell -NoProfile -Command 'Get-Process readmd -ErrorAction SilentlyContinue | Stop-Process -Force'`
  注意用**单引号**包裹，双引号里 `$_` 会被 bash 吃掉。
- `cargo build` 前若有 readmd 进程持有 target 产物会链接失败——先杀进程。
- **绝不能用 `head` 看 cargo 输出**：曾因此把"编译失败"报成"编译成功"。一律 `| tail -40`，并确认看到 `Finished` / `test result: ok` 才允许下结论。
- 多行补丁写成 `.pl`/`.py` 文件执行，别塞进 shell 内联。
- **不要用 `grep '\.route("'` 找路由**——本服务端是静态元组表 `&(&str, Handler)`（`server.rs:417-520` 区段），不是 actix builder 链，那样查会得 0 命中并误判"没有路由"。改查 `"/api/` 字面量。
- 大小写盘符：路径写 `/t/Programming/...`。

---

## 9. 与桌宠 / Python 侧的关系（尚未收口）

- `packages/readmd-pet-rust/` 是**独立 Rust crate**（桌面 overlay），有自己的 `bridge/ webview/ runtime.rs protocol.rs security/ platform/`，与 `readmd-kernel` 不共享编译单元。它当前有大量未提交改动（`git status` 里 `M packages/readmd-pet-rust/**`）。
- Python 主程序 `readmd.py` 与 `assets/` 前端仍在仓中，是 parity 基准；**尚未验证前端是否已能纯靠 rust 内核跑通完整 UI 流程**（见 §10）。
- 未跟踪的临时产物不要提交：`tmp_pet_test/`、`test_copies/`、`current_screen.png`、`screenshot_test.png`、`tmp_pet_screen.png`、`ReadMD.exe`、`ReadMD.pyinstaller.bak.exe`、`migration/`、`tests/migration/`、`tools/*_temp.py`。

---

## 10. 明确未验证 / 未做的部分（不要当作已完成）

1. **窗口模式未实跑**：`tao`/`wry` 桌面窗口路径只编译过，没有真机开过窗验证。headless 路径已验证（§4）。
2. **Linux / macOS 零验证**：`rust-version = 1.85`，但从未在非 Windows target 上 `cargo check`。`platform/windows.rs`、`winres` build-dep、`#[cfg(windows)]` 分支是主要移植面。这是"原生多平台"目标当前**最大的未兑现项**。
3. **端到端 UI 未验**：前端 `assets/index.html` 打到 rust 内核的完整流程（打开文档、转换、导出、AI 对话、图表渲染）没跑过。
4. **未测端点**：`/api/export`、`/api/ocr`、`/api/transcribe`、`/api/ai/chat`（需真实 key）、`/api/batch/extract-zip`、`/api/share/*`、`/api/update/*`、`/api/skill-imports/*`、`/api/upstream-sources/*` 动态族。
5. **服务端图表渲染未移植**（`serverSideRender:false` 是内核自己承认的）。
6. **桌宠与内核的进程编排**未收口。
7. `legacyTotal` 与 `implementedCount` 都等于 107 —— 说明这个"legacy 总数"是从内核自己的表算出来的，**不是从 Python 侧真值取的**，因此它不能证明 parity。别拿它当验收证据；用 §5 的消费侧 diff。

---

## 11. 建议接手顺序

1. `git add rust/`（排除 `target/`）并提交 —— 解除 §0 的丢失风险。
2. 移除/归档 `rust/src/`，声明或删除 `import_processor.rs`。
3. 修 BOM 缺陷 + 补回归测试（§7）。
4. 清 81 个 warning（先 `cargo fix` 吃掉 55 个，剩下手判断）。
5. 补 `/api/tags`，把 parity diff 写成 CI 门禁（消费侧 84 端点逐个断言非 404）。
6. 让 `/api/kernel/status` 的 pending 计算改读 `prefix_pending` 家族而非空 `PENDING` 常量，使 `pendingCount` 不再低报。
7. **跨平台**：`cargo check --target x86_64-unknown-linux-gnu`（+ macOS），把 Windows-only 抽象到 `platform/` 后面。这是用户"原生多平台"要求的实质进度。
8. 真机开一次窗口模式，录一次端到端 UI 流程。
9. 逐族推过 501 门（pets/convert/update/plugins/upstream-sources/skill-imports/share/export/modules/diagram）。

---

## 12. 复核用的最短命令

```bash
cd /t/Programming/Project/codex/creator/readmd
git ls-files rust/ | wc -l                                   # 当前 0
cargo test -p readmd-kernel 2>&1 | tail -20                   # 期望 115 passed
cargo build --release -p readmd-kernel 2>&1 | tail -20
ls -l rust/target/release/readmd.exe                          # 8,823,296 B
./rust/target/release/readmd.exe --no-window --port 8791 --data-dir "$TEMP/probe" &
curl -s http://127.0.0.1:8791/api/kernel/status | head -c 400
curl -s -X POST http://127.0.0.1:8791/api/code/run \
     -H 'Content-Type: application/json' \
     -d '{"language":"python","code":"print(6*7)","confirm":true}'
powershell -NoProfile -Command 'Get-Process readmd -EA SilentlyContinue | Stop-Process -Force'
```
