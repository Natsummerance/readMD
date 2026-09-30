# Rust 原生化 · 桌宠 desktop 模块交接文档

**收件人**：Gemini 3.8 Flash（运行于 Google Antigravity）
**交接人**：上一轮 Qoder 会话（所有事实均在本机实测核查，未凭记忆下结论）
**日期**：2026-09-23
**仓库**：`<repo>/`
**分支**：`main` **HEAD**：`81a322e`（`feat(kernel): serve skill-import source routes from the parity owner`，已提交，**未 push**）

> ⚠️ 本文所有行号均为 **2026-09-23 HEAD `81a322e` + 当前工作树未提交改动** 的快照。
> 你动手前必须重新 grep 复现（§17 给了命令）。上一轮交接事故里，接手方照着简报行号改代码，
> 结果前提早已不成立，白跑一整轮。**行号是路标，不是事实；事实只有你亲眼 grep 到的那一行。**

---

## 0. 你是怎么被使用的：Gemini 3.8 Flash × Antigravity 操作手册

这一节不是客套，是**为了让你的特性用在这个任务上**。

### 0.1 模型侧（已核实，来源见 §18）

| 特性 | 数值 | 对本任务的意义 |
|---|---|---|
| 上下文窗口 | **1M tokens** | 你可以**整文件读入**而不是切片读。`parity_pets.rs` 5761 行、`pet_launcher.rs` 4331 行、`protocol.rs` 747 行——直接整读。**不要**用 grep 片段推断全局结构，那是小上下文模型才需要的妥协，你不需要，用了反而会误判。 |
| thinking level | `low` / `medium`（默认）/ `high` | 见 §0.2 分配表。**不要全程 high**——机械接线用 high 是纯浪费；也不要在 Win32 样式位运算上用 low，那里错一个位就是"窗口全黑"这种极难调试的故障。 |
| 官方标注强项 | "long-horizon software engineering"、"complex multi-file refactoring" | 这正是本任务：跨 2 个 crate + Python 权威树 + 前端 JS 的多文件对齐重构。你的强项对得上。 |
| 已废弃参数 | `temperature`、thinking level `minimal` | 如果你的 harness 配置或任何脚本里还带着这两项，删掉，否则可能报错或被静默忽略。 |

### 0.2 thinking level 分配建议（按本任务的真实风险分布）

| 工作类型 | 建议 level | 理由 |
|---|---|---|
| §7 的 Win32/DWM 样式位、`SetWindowPos` 标志组合 | **high** | 位运算错一处 = 窗口不透明/黑底/无法点击穿透，且**单元测试测不出来**，只能肉眼看。 |
| `protocol.rs::normalise_command` 与 `hermes_adapter.py` 的逐字段对齐 | **high** | 舍入规则（scale 0.18–0.72 round-half-even）、bounds 夹取、interact 动作白名单，差一位就是行为分叉。 |
| JSON 响应键集/嵌套/键序 parity | **high** | 键序在 `assert_eq!` 下**不可见**（§10.7），必须靠人脑对着 Python dict 字面量比。 |
| 把已写完的孤立模块接进 `ROUTES` | **low** | 纯机械，风险在"忘了声明 `mod`"（§14 Phase 3 有专门检查）。 |
| 删死代码 / 清 warning | **low** | 但**必须**先走 §16 的删除三证，不许凭"看起来没用"就删。 |
| 写交付报告 | **medium** | 格式固定（§15），但"未通过+原因"那一段需要诚实判断。 |

### 0.3 Antigravity harness 侧（已核实特性名，来源见 §18）

Antigravity 2.0 是 agent 的 "central command center"，会产出三类 artifact：**Task List**、**Implementation Plan**、**Walkthrough**；用 `/browser` 指令驱动浏览器；有 **Agent Panel** 做代码审查；模型可选且带 thinking/effort 档位。

**具体怎么用在这个任务上：**

1. **先出 Implementation Plan，再动手。** 本文 §14 已经给了 Phase 划分，把它转成你的 Implementation Plan artifact，每个 Phase 一条可验收项。**不要**把整份文档变成一个巨大的 Task List 然后一路闷头做——用户在上一轮明确说过"不许中途停在阶段之间"，但也说过"阶段 1 未完成不许开始阶段 2"。这两条合起来的意思是：**每个 Phase 内部不许停，Phase 之间必须有全量测试证据。**
2. **`/browser` + 截图是桌宠模块唯一可行的验收手段。** 桌宠是一个**透明、无边框、always-on-top、点击穿透**的 WebView2 窗口。这类窗口的正确性**无法用 `cargo test` 证明**（62 个测试全是纯逻辑，没有一个能验证"窗口真的是透明的"）。你必须：起进程 → 截图 → 肉眼比对 → 记录到 Walkthrough artifact。仓库里已有前一轮留下的截图样本 `current_screen.png`、`screenshot_test.png`、`tmp_pet_screen.png`（未跟踪），可作对照基线。
3. **Walkthrough artifact 要写"我看到了什么"，不是"我改了什么"。** 用户被"写完即完成"的交付坑过，明确说过"自己验证一下"。`git diff` 他自己会看；他要看的是**运行结果**。
4. **Agent Panel 自审。** 提交前用 Agent Panel 过一遍 diff，重点自查三件事：有没有新增 `Command::new`（§11 红线）、有没有 `todo!`/`unimplemented!`（§16 禁令）、有没有改到 §7 的不变量。
5. **上下文预算。** 1M 够用，但 `cargo test --offline -p readmd-kernel --lib` 的**完整输出**很长（1673 个测试，上一轮日志 198 KB）。把 cargo 输出重定向到文件，然后只 grep `test result:` 和 `FAILED`，别把整个日志灌进上下文。

---

## 1. 任务目标（用户原话，一字未改）

> 直接用 rust 重写整个项目，我原项目分支在 main，应该是没动的，你可以看 git 记录，我的目标是用 rust 从零重写整个项目，搞个 rust 内核原生无依赖多平台版本出来；你写完功能自己验证一下，要和我 GitHub 上面的所有功能保持一致，如果能升级，做到更加轻量化是最好的；没完成之前不要停止。

> 继续执行，不能停，除非我叫你停，绝对不能停。

四条硬约束：

1. **从零重写**，不是包 Python、不是 spawn Python。
2. **与 GitHub `main` 的 Python 实现 100% 行为对齐**：状态码、参数读取方式、响应键集与**嵌套形状与键序**、`error_code` 字符串、降级路径。
3. **自己验证**，不接受"写完即完成"。
4. **更轻量化**优先（release profile 已配 `lto="thin"` / `codegen-units=1` / `strip="symbols"` / `opt-level="s"`）。

用户补充的纪律（同样硬性）：

- **只提交 Rust 树，永远不 push。**
- **每个 Phase 结束跑一次全量 `cargo test` 确认无回归。**
- **交付报告固定 5 段**，含"未通过 + 原因"（§15）。
- **禁止用 `// TODO` 注释消音编译器警告。**

---

## 2. 最重要的一个纠正：desktop 模块**不在**你以为的地方

> 这一节是本文存在的核心理由。如果你只读一节，读这节。

内核树里有一个文件叫 **`rust/readmd-kernel/src/desktop_pet.rs`（775 行）**。名字里有 "desktop"。
**它不是桌宠的 desktop 模块。它是一个未接线的纯决策函数集合**，而且是 `readmd.py` 里
`_drain_pet_command` / `_open_pet_clipboard` / `_publish_pet_runtime` / `_start_pet_fullscreen_loop` /
`_start_tray`（仅图标选择部分）的**纯逻辑移植**——没有一行窗口代码。

**桌宠真正的 desktop 模块在这里：**

```
packages/readmd-pet-rust/src/
├── platform/          ← ★ desktop 模块本体
│   ├── mod.rs         (119)  trait PlatformBackend + create_backend()
│   ├── windows.rs     (471)  ★ DWM/样式/单实例互斥/多显示器工作区
│   ├── linux.rs       (284)  GNOME companion / wlr-layer-shell / X11
│   ├── macos.rs       (123)  objc2 NSPanel 浮动面板
│   └── fallback.rs    (46)   通用 Tao 后端（全 no-op）
├── runtime.rs         (828)  ★ Tao 事件循环、快照应用、崩溃恢复、健康/拆除
├── webview/
│   ├── mod.rs         (353)  ★ WRY + 自定义协议 + Electron preload ABI 仿真
│   └── ipc.rs         (68)      renderer → host 消息解析
├── input.rs           (477)  ★ 鼠标轮询、命中测试、拖拽（未跟踪！见 §3）
├── clipboard.rs       (626)  ★ Win32 剪贴板截图 + 手写 PNG 编码器（未跟踪！见 §3）
├── protocol.rs        (747)  ★ 命令规范化，逐字段镜像 hermes_adapter.py
├── bridge/            (679)  文件桥 IPC：snapshot/commands/health/parent_liveness
├── security/mod.rs    (381)  资产沙箱、路径逃逸防护、SHA-256 校验
├── error.rs           (21)   HostError（thiserror）
├── lib.rs             (19)   / main.rs (19)  --version + HostConfig::from_env() + PetHost::run
```

★ = desktop 模块的核心文件。

**两个 crate 的关系（进程模型）：**

```
readmd-kernel (rust/)            readmd-pet-rust (packages/)
HTTP 服务，替代 readmd.py   ──►  独立桌宠宿主进程
                                  tao + wry + WebView2
     │                                   │
     │ parity_pets.rs::start_pet_host     │ 轮询 READMD_PET_BRIDGE_FILE
     └── 写 bridge 文件 ─────────────────►│ （80ms，SHA-256 去重）
         pet_host.rs                      │
                                          └─► 透明置顶窗口 + 精灵图渲染
```

**内核只负责"启动并监督"宿主进程**，一像素的桌面渲染都不做。`rust/readmd-kernel/src/main.rs`
里有一句注释直接写明："The kernel has no tray"。

所以：**"做好桌宠的 desktop 模块" = 做好 `packages/readmd-pet-rust/src/platform/` + `runtime.rs` + `webview/` + `input.rs` + `clipboard.rs`。**

---

## 3. Phase 0（动手第一件事）：把桌宠 crate 的未提交改动入库

**这是当前仓库最大的单点风险。** 桌宠 crate 只有一个提交：

```
0c3e501 feat(pet): ship native rust desktop runtime    （已跟踪 25 个文件）
```

在它之上，工作树里有：

**13 个已修改文件，+2131 / −294：**

| 文件 | 增行 |
|---|---|
| `src/protocol.rs` | +561 |
| `src/platform/windows.rs` | +396 |
| `src/runtime.rs` | +362 |
| `src/security/mod.rs` | +310 |
| `src/bridge/parent_liveness.rs` | +258 |
| `src/webview/mod.rs` | +153 |
| `src/bridge/health.rs` | +131 |
| `src/bridge/commands.rs` | +124 |
| `src/bridge/snapshot.rs` | +56 |
| `src/webview/ipc.rs` | +47 |
| `src/platform/mod.rs` | +19 |
| `Cargo.toml` | +4 |
| `src/lib.rs` | +4 |

**2 个完全未跟踪的源文件（`git ls-files --others` 实测）：**

```
packages/readmd-pet-rust/src/clipboard.rs    626 行
packages/readmd-pet-rust/src/input.rs        477 行
```

**合计约 3234 行未受版本控制的工作成果。** 一次 `git clean -fd`、一次误操作、一次换机器，
桌宠的输入系统和剪贴板系统就没了。

**已核实的准确状态**（`git show HEAD:packages/readmd-pet-rust/src/lib.rs` 对比工作树）：

- HEAD 的 `lib.rs` **没有**声明 `pub mod clipboard;` / `pub mod input;`，这两个文件在 HEAD 也不存在
  ⇒ **HEAD `81a322e` 本身是自洽、可编译的。**
- 工作树的 `lib.rs` **已经**声明了这两个 mod（`+pub mod clipboard;`、`+pub mod input;`），
  并多导出了一个 `ClipboardCommand`，文件也确实存在 ⇒ **工作树也是自洽的。**

> 🔴 **但这两种自洽之间隔着一个陷阱**：如果你用 `git add -u`（只加已跟踪文件的修改）提交，
> 产出的那个提交会声明 `pub mod clipboard;` 却没有 `clipboard.rs` —— **那个提交编译不过**，
> 而且它会被 push 到别人手里（虽然本项目禁止 push）。
> **必须显式把两个未跟踪文件一起 add**，见下面的命令。

**Phase 0 操作：**

```bash
cd "<repo>/"
git status --porcelain -- packages/readmd-pet-rust     # 先看清要提交什么
git add packages/readmd-pet-rust/Cargo.toml packages/readmd-pet-rust/src
git status                                              # 复核 staged：不得含 target/ 或 dist/
git commit -m "feat(pet): wire native input, clipboard and bridge hardening"
```

**不要 `git add packages/readmd-pet-rust`（裸目录）**——那样会把 `target/` 和 `dist/`
（已构建的 `ReadMD-Pet-Rust.zip` + windows-x86_64 目录）一起吞进去。提交前用 `git status` 复核。

**Phase 0 验收**：`git status --porcelain -- packages/readmd-pet-rust` 只剩 `target/`、`dist/`、
`Cargo.lock`、`gnome-companion/`、`scripts/` 这类你**主动决定**不提交的东西，且你能说出每一项的理由。

---

## 4. 权威基准：Python 侧真相

**验收基准永远是这三处，不是任何 Rust 代码、不是任何文档（包括本文）：**

1. `readmd.py`（HTTP 层 + 桌宠编排）
2. `src/readmd_modules/**/*.py`（业务实现）
3. `assets/js/**`（前端契约的消费方，键序/键集的最终裁判）

### 4.1 Python 桌宠路由表（`readmd.py` 实测）

Python 的路由是 `readmd.py:1165 _route()`，被 `do_GET:1054`、`do_POST:1067`、`do_DELETE:1086`
**三者共用**。这意味着——

> 🔴 **Python 的桌宠路由是方法无关的（method-agnostic）。任何方法都能命中同一个 handler。**
> Rust 侧对 `/api/pets/configure`、`/api/pets/interact` 的非 POST 请求返回 **405**，这是一处 parity 偏差。
> 处置见 §10.1。

| 路由 | 行号 | 委托到 |
|---|---|---|
| `/api/pets` | `:1245` | `_api_pets:1646` → `pet.store.list_pets` |
| `/api/pets/status` | `:1247` | `:1661` → `Api.get_pet_runtime_status` |
| `/api/pets/configure` | `:1249` | `:1673` → `Api.configure_pet` |
| `/api/pets/interact` | `:1251` | `:1690` → `pet.companion.PetCompanion` |
| `/api/pets/import` | `:1253` | `:1729` → `pet.store.register_local_pet` |
| `/api/pets/remove` | `:1255` | `:1756` |
| `/api/pets/active` | `:1257` | `:1776`（含 `shutil.copytree`） |
| `/api/pets/install` | `:1259` | `:1723` |
| `/api/pets/runtime/install` | `:1261` | `_handle_pet_lifecycle_action:1707` |
| `/api/pets/uninstall` | `:1263` | `:1726` |
| `/api/pets/thumb` | `:1265` | `:1809`（GET，返回精灵表原始字节） |
| `/api/pets/update_status` | `:1267` | `:1840` |
| `/api/pets/check_update` | `:1269` | `:1852` |
| `/api/pets/apply_update` | `:1271` | `:1866` |
| `/api/control/pet-batch` | `:1361` | `pop_pet_batch:402` |
| `/api/control/pet-menu` | `:1364` | `pop_pet_menu:431` |

`LAN_BLOCKED_PATHS`（`readmd.py:1018-1030`）拦截 12 条桌宠路径——局域网访问时这些路由直接不可达，
Rust 侧必须复刻这份名单，否则是一个**安全 parity 漏洞**（不只是行为差异）。

### 4.2 `src/readmd_modules/pet/` 清单

| 文件 | 行数 | 作用 |
|---|---|---|
| `runtime.py` | **763** | ★ 核心。`RustPetRuntimeInstaller:41`、`RustPetRuntime:379`、`ElectronPetRuntime:663`、`PetRuntimeOrchestrator:667` |
| `store.py` | 492 | 桌宠注册表、本地导入 |
| `updater.py` | 420 | 更新检查/应用 |
| `sprite_processor.py` | 247 | 精灵表处理 |
| `window_adapter.py` | 139 | pywebview 探测桥 |
| `companion.py` | 118 | `PetCompanion` 交互 |
| `controller.py` | 100 | 控制器 |
| `task_queue.py` | 92 | 批量命令队列 |
| `model_manifest.py` | 91 | 模型清单 |
| `fullscreen.py` | 66 | ★ 全屏检测（**Python 侧唯一的直接 win32 调用**） |
| `probe.py` | 24 | 手动探测入口 |
| `hermes_adapter.py` | **829** | ★ Electron 宿主适配，`protocol.rs` 的对齐基准 |
| `__init__.py` | 82 | |

### 4.3 Python **不创建任何桌宠窗口**（关键认知）

很多接手方会以为要跟 Python 的窗口代码对齐。**没有这种代码。** Python 只做三件事：

1. **拉起外部进程**：`runtime.py:588 subprocess.Popen([binary_path])`，cwd 在 `:573`，
   `STARTUPINFO` + `CREATE_NO_WINDOW` 在 `:574-583`，注入 7 个环境变量键。
2. **等健康文件**：`_health:410-435`、`_wait_health:508-536`。
3. **杀进程**：`stop:610-649`，走 `taskkill`（`:641-645`）。

Electron 降级路径：`hermes_adapter.py:363 Popen(electron.exe)`、`:384-387 taskkill /F /T /PID`、
`:68-71` PowerShell `Get-Process -Name electron | Stop-Process`（**这是桌宠路径里唯一的 PowerShell 外呼**）。

Python 侧唯一的直接 win32：`fullscreen.py:33-62` 用
`ctypes.windll.user32.GetForegroundWindow / GetWindowRect / MonitorFromWindow(h,2) / GetMonitorInfoW`，
被 `readmd.py:5172` 和 `:5732` 消费。

pywebview 探测：`window_adapter.py:57-59`
`create_window(min_size=(160,160), frameless=True, on_top=True, transparent=True)`，
带签名嗅探能力门 `:23-49` 和 `windows_transparency` 覆写 `:39-46`；**仅手动触发**（`probe.py:11-20`）。

状态桥：`readmd.py:5119 _publish_pet_runtime` → 精灵表 base64 + `frameW/frameH/framesPerState/stateRows`
+ bounds，经 `HermesPetBridge.publish`（`hermes_adapter.py:93`）。
拖拽：`window_adapter.py:88-127 PetProbeDragBridge`。

**托盘不在 `pet/` 里**：`readmd.py:6884 _start_tray` 用 `pystray`（`:6887`），菜单 `:6926-6932`，
受 `api._on_page_ready`（`:6556`）门控。

**Python 侧没有**：屏幕捕获、鼠标/键盘钩子、per-monitor DPI 处理。
（Rust 桌宠宿主**有**屏幕捕获和 per-monitor DPI——这是"升级/更轻量化"允许的超出项，不是偏差。）

---

## 5. 内核侧桌宠表面：什么已 live，什么是孤儿

`rust/readmd-kernel/src/` 里的桌宠模块（行数为 `wc -l` 实测）：

| 文件 | 行数 | 状态 | 说明 |
|---|---|---|---|
| `parity_pets.rs` | **5761** | ✅ **LIVE** | 14 个 handler，54 个测试 |
| `pet_host.rs` | 517 | ✅ **LIVE** | 唯一消费者 `parity_pets.rs:1936,3511,3542,3543`；13 测试 |
| `pet_paths.rs` | 378 | ✅ LIVE | 被 `parity_pets.rs:24` 导入；15 测试 |
| `pet_launcher.rs` | **4331** | 🔌 **未接线** | `hermes_adapter.py` 的移植，映射表在 `:8-24`；**84 测试全绿但无人调用** |
| `pet_queue.rs` | 1298 | 🔌 **未接线** | `pet/task_queue.py` 的移植；13 测试 |
| `pet_probe.rs` | 1053 | 🔌 **未接线** | `pet/window_adapter.py` + `pet/probe.py` 的移植；36 测试 |
| `desktop_pet.rs` | 775 | 🔌 **未接线** | 纯决策逻辑（见 §2）；API：`route_pet_command:130`、`classify_clipboard:243`、`resolve_renderer:322`、`pick_pet_slug:357`、`pick_tray_icon:433`；20 测试 |
| `pet_window_state.rs` | 568 | 🔌 **未接线** | `src/readmd_core/window_state.py` 的移植，**被错放在 pet 名下**；15 测试 |

`parity_pets.rs` 的 14 个 live handler（行号为快照）：
`h_pets:3654`、`h_pets_status:3667`、`h_pet_thumb:3679`、`h_pet_import:3720`、`h_pet_remove:3773`、
`h_pet_active:3808`、`h_pet_install:3980`、`h_pet_uninstall:3985`、`h_pet_runtime_install:3996`、
`h_pet_configure:4009`、`h_pet_interact:4031`、`h_pet_update_status:4070`、`h_pet_check_update:4096`、
`h_pet_apply_update:4127`；另有 `start_pet_host:3464`、`stop_pet_host:3587`、`RuntimeTree` 安装器 `:1318`。

### 5.0 🔴 这 5 个孤儿模块互相引用，构成一个"孤儿集群"（实测）

我 grep 了 `pet_launcher::|pet_queue::|pet_probe::|desktop_pet::|pet_window_state::`，
**命中 8 处**。如果你止步于此，会得出"它们被使用了"的**错误结论**。实际命中的是**集群内部的互相引用**：

```
pet_probe.rs:45       use crate::pet_launcher::{py_float_value, py_truthy, OrdValue};
pet_launcher.rs:283   crate::pet_queue::json_str(key)
pet_launcher.rs:335   crate::pet_queue::json_str(text)
pet_launcher.rs:812   crate::desktop_pet::py_truthy(value)
pet_launcher.rs:1110  crate::pet_queue::py_realpath(&text)
pet_launcher.rs:1123  crate::pet_queue::py_normcase(path)
pet_launcher.rs:1135  crate::pet_queue::py_realpath(exe)
pet_launcher.rs:2288  crate::pet_queue::py_normpath(&joined)
```

依赖方向：`pet_probe → pet_launcher → {pet_queue, desktop_pet}`；`pet_window_state` 完全孤立（0 引用）。

**结论：集群外（`server.rs` / `parity_pets.rs` / `main.rs` / 任何 live 路径）零调用者。**
它们能编译、测试能跑绿（84+13+36+20+15 = **168 个测试全在验证无人调用的代码**），
但从 HTTP 请求进不到这里一行。

**这就是为什么 §17 的复现命令 #4 必须排除集群自身**——否则你会看到一堆命中，
误以为上一轮的"孤儿"判断是错的，然后跳过整个 Phase 3。

### 5.1 `server.rs` 里的**死重复** handler（陷阱）

`server.rs` 里定义了这些函数，但 **`ROUTES` 绑定的是 `parity_pets::` 版本**，所以它们从未被调用：

```
h_pets:3243   h_pets_status:3276   h_pet_active:6412   h_pet_remove:6468
h_pet_thumb:6495   h_pet_update_status:6513   h_pet_uninstall:6551   h_pet_interact:6581
```

**grep `h_pet_` 会同时命中两套。** 改错那一套 = 你的修改永远不会执行，而测试还是绿的。
动手前先确认 `ROUTES`（`server.rs:572`）里那一行指向谁。

live 的还有：`h_control_pet_batch:4369`、`h_control_pet_menu:4378`；
辅助函数在 `batch2.rs:1840-2075`（`find_pet_host_exe`、`find_pet_state_file`、`stop_pet_process`、
`ensure_pet_state_file`、`is_pet_running`）。

### 5.2 路由表现状（好消息）

- `ROUTES`（`server.rs:572`）桌宠行：`:603-604`、`:620-621`、`:628-634`、`:664-668`
- `LEGACY_ROUTES`（`:705`）桌宠项：`:718-719`、`:740-753`
- `LEGACY_DYNAMIC_PREFIXES`（`:807`）：**无桌宠项**
- `PENDING`（`:847`）：**只有 1 项** `/api/file.txt-md-structuring`，**桌宠路由零 pending**
- `dispatch()`（`:862`）顺序：精确表 → 动态前缀 → pending → 404
- **整个内核里唯一的 501 是 `OPTIONS` 方法**（`:866-869`）
- **桌宠代码里没有任何 `todo!` / `unimplemented!` / 501**

所以：**桌宠 HTTP 表面的"路由接线"已经完成**。剩下的 parity 工作是**响应形状细节**（§10）
和**把 5 个孤儿模块接进来**（§14 Phase 3）。

### 5.3 唯一已声明的功能缺口

`pet_host.rs:40-49`：**`READMD_PARENT_PIPE_HANDLE` 从未被设置。**
需要 `STARTUPINFOEX` + `CreatePipe` 才能让子进程继承匿名管道；
目前 `stop()` 只能依赖子进程自己的存活轮询（`parent_liveness.rs` 的 `READMD_PARENT_PID`
+ `OpenProcess` 回退，250ms 周期）。

**后果**：内核被强杀（不是优雅退出）时，桌宠宿主最多要等一个轮询周期才发现，
而不是立刻收到管道 EOF。这是"孤儿桌宠窗口留在桌面上"的成因。
**注意**：`CreatePipe` 不是 spawn 外部程序，**不违反 §11 红线**，可以放心实现。

---

## 6. 桌宠 crate 的 IPC 契约（不是 HTTP，不是 stdio）

**全部基于文件。** 接手方常误以为要起个 HTTP 端口，错。

| 通道 | 机制 |
|---|---|
| **快照下行** | 宿主每 **80ms** 轮询 `READMD_PET_BRIDGE_FILE`，用 **SHA-256 签名去重**（内容没变就不重渲染） |
| **命令上行** | 持久命令队列文件，命名 `<20位毫秒时间戳>-<pid>-<seq>-<sha256>.json`，写入序列 `create_new` → `sync_all` → `rename`（原子） |
| **健康** | `<bridge>.rust.health.json`，状态机 `booting / loading / ready / degraded / failed / stopped`，`protocol_version = 1` |
| **父进程存活** | 继承的匿名管道 `READMD_PARENT_PIPE_HANDLE`（EOF = Win 错误 **109 / 232 / 233**），回退到 `READMD_PARENT_PID` + `OpenProcess`，250ms 周期 |

**`protocol.rs:190-202` `CONSUMER_COMMANDS`**（宿主→渲染页的消费类命令白名单）：
`bounds`、`clipboard`、`drop`、`open-app`、`open-menu`、`pop-in`、`scale`、`submit`、
`toggle-app`、`interact`、`character`。

**`protocol.rs:247-368` `normalise_command`** 逐字段镜像 `hermes_adapter.py`：
scale 夹取 0.18–0.72 且 **round-half-even**、bounds 安全夹取、interact 动作闭集
`pet / feed / play / rest / wake`。

**会话令牌 + 导航代号**：`protocol.rs:464-469` 打戳，`runtime.rs:503` 强制校验。
作用是丢弃上一次页面加载遗留的迟到消息——**不要为了"简化"去掉它**，去掉会复现
"切换桌宠后旧动画闪一下"的老 bug。

渲染页→宿主的消息种类在 `runtime.rs:507-718` 处理。

---

## 7. 🔴 不许"修好"的 Win32 / DWM 不变量

> 这一节的每一条都看起来像 bug 或遗漏。**它们不是。** 每一条都是一次调试的结论。
> 在你完整读完 `platform/windows.rs` 并理解 DWM 合成路径之前，**不要改动这一节的任何一项**。

### 7.1 透明 + 点击穿透的完整配方（`platform/windows.rs`）

| 步骤 | 位置 | 内容 |
|---|---|---|
| DWM 玻璃边框 | `:266-273` | `DwmExtendFrameIntoClientArea`，四个 margin 全 `-1` |
| 关掉 Win11 圆角 | `:276-284` | `DwmSetWindowAttribute(33 /*CORNER_PREFERENCE*/, 1 /*DONOTROUND*/)` |
| 空背景刷 | `:287-291` | `SetClassLongPtrW(GCLP_HBRBACKGROUND, GetStockObject(NULL_BRUSH))` |
| 样式手术 | `set_style:148-183` | **移除** `WS_EX_LAYERED`；加 `WS_POPUP \| WS_EX_TOOLWINDOW`；按需切换 `WS_EX_TRANSPARENT` / `WS_EX_NOACTIVATE` |
| 置顶 | 同上 | `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE \| SWP_NOMOVE \| SWP_NOSIZE \| SWP_FRAMECHANGED)` |
| 裸 FFI 声明 | `:23-35` | `#[link(name="dwmapi")] extern "system"` |

### 7.2 三个"看起来是 bug"的设计

**① `set_opacity` 在 Windows 上是 no-op（`:340-344`）——这是必须的。**
原因：实现透明度需要**移除** `WS_EX_LAYERED`（它会破坏 DirectComposition / WebView2 的合成路径）。
而 `SetLayeredWindowAttributes` 恰恰**要求** `WS_EX_LAYERED`。两者互斥，透明度优先。

后果你要知道：`protocol.rs:106-117` 把快照 opacity 夹到 0.35–1.0，
**但这个值在 Windows 上完全不生效**。这不是待修 bug，是平台约束。
如果用户报"调透明度没反应"，答案是 CSS 侧的 `opacity`，不是 Win32 侧。

**② `update_interaction_regions` 只存矩形、不做命中测试（`:369-376`）——命中测试在别处。**
真正的命中测试数学在 `input.rs`。platform 层只负责保存区域。
不要在 `windows.rs` 里"补全"命中测试，那会造成两套逻辑。

**③ 输入是轮询，不是钩子。**
`input.rs:223-380 run_watcher_loop` 轮询 `GetAsyncKeyState(VK_LBUTTON / VK_RBUTTON / VK_MBUTTON)`
+ `GetCursorPos` / `GetWindowRect`。
**整个 crate 里没有 `SetWindowsHookEx`。** 不要"升级"成全局钩子——
全局钩子需要 DLL 注入、会被杀软拦、且会让桌宠变成键盘记录器（安全红线）。
轮询是刻意选择。

### 7.3 其他平台后端

- **macOS**（`platform/macos.rs:91-123`）：objc2 `NSPanel`，non-activating 浮动面板、all-spaces。
  `set_opacity` = `setAlphaValue`（`:42-55`）**是真实现的**（与 Windows 不同）。
- **Linux**（`platform/linux.rs`）：`select_linux_backend:266-284` 三选一——
  `GnomeCompanionBackend`（`:208-263`，通过 Unix socket `$XDG_RUNTIME_DIR/readmd-pet-gnome.sock`
  走 JSON lines，配对 `gnome-companion/` 里的 JS 扩展）、
  `LayerShellBackend`（`:155-206`，wlr-layer-shell）、
  `X11Backend`（`:117-153`）。cairo 输入形状 `:70-102`。
  `set_opacity` 在 `:133` 和 `:186` 都是 no-op。
- **fallback**（`platform/fallback.rs`，46 行）：通用 Tao 后端，opacity / regions 全 no-op。

### 7.4 `trait PlatformBackend`（`platform/mod.rs:21-52`）

```
name / init / set_bounds / applied_bounds / set_visible / set_opacity /
set_click_through / set_focusable / update_interaction_regions
+ 默认实现 show_context_menu / drag_window / win32_hwnd
```

配套：`configure_builder`（`:72-99`）、`create_backend()`（`:101-119`）。

> 🔴 **这个 trait 故意不是 `Send + Sync`。** 所有方法都必须在 Tao 事件循环线程上调用。
> 如果你为了"并发优化"给它加 `Send`，会引入跨线程窗口操作——在 Windows 上是未定义行为，
> 在 macOS 上直接崩（AppKit 主线程约束）。**不要加。**

### 7.5 单实例互斥（`platform/windows.rs:294-321`）

per-user 命名互斥体，名字 = `SHA-256(USER_SID + data_dir)`，`CreateMutexW`，
失败重试 **30 次 × 50ms**。哈希里带 `data_dir` 是刻意的：允许同一用户跑多个不同数据目录的实例。
不要改成固定名字。

### 7.6 多显示器工作区（`:202-245`）+ 夹取（`place_in_work_area:77-94`）

优先 per-monitor 工作区，失败回退 `SPI_GETWORKAREA`。
**Python 侧没有 per-monitor DPI/工作区处理**（只有 `fullscreen.py` 的 `MonitorFromWindow`），
所以这里 Rust 是**超出**基准的——允许，但要保证回退路径的行为跟 Python 一致。

---

## 8. 窗口与渲染管线

| 环节 | 位置 | 要点 |
|---|---|---|
| Tao 窗口构建 | `runtime.rs:129-145` | transparent、undecorated、always-on-top、**不获取焦点**、320×380、标题 `ReadMD Desktop Pet` |
| WRY webview | `webview/mod.rs:117-165` | 自定义协议 `readmd-pet://localhost/index.html?renderer=&generation=&session=` |
| Windows 导航改写 | `webview/mod.rs:245-255` | Windows 上必须改写成 `http://readmd-pet.localhost`（WebView2 对自定义 scheme 的限制） |
| preload ABI 仿真 | `webview/mod.rs:15-69` `PRELOAD_ABI` | 仿真 Electron 的 `hermesDesktop.petOverlay` 表面——**前端 JS 不用改就能跑** |
| host → page | `webview/mod.rs:207-232` | `evaluate_script(__readmdRustDispatch)` |
| page → host | `webview/ipc.rs` | `parse_renderer_message` |
| 崩溃恢复 | `runtime.rs:415-439` | 退避 + 熔断器 |

`assets/` 里的前端 JS 通过 `PRELOAD_ABI` 看到的 API 与 Electron 时代**完全一致**。
这是"不重写前端"的关键，也是为什么 `webview/mod.rs` 那 69 行 ABI 不能随便改签名。

---

## 9. 安全边界（`security/mod.rs`，381 行）

| 组件 | 位置 | 契约 |
|---|---|---|
| `is_safe_relative_path` | `:22-54` | 镜像 Python `_safe_name` |
| `AssetSandbox` | `:58-126` | canonicalize 后必须在根目录内；**拒绝符号链接**；256 MiB 上限；SHA-256 `verify_digest` |
| `RendererAssetRouter` | `:199-242` | 只允许 renderer bundle 或其兄弟目录 `vendor` / `models` / `assets`（`:160-195`） |

自定义协议 `readmd-pet://` 是一个**本地文件服务器**。上面三道闸是它唯一的路径逃逸防护。
**不要为了"支持更多资源"放宽 `RendererAssetRouter` 的兄弟目录白名单**——
`../` 穿越 + 符号链接是这个设计的主要攻击面。

---

## 10. 已知偏差清单（逐条处置，或在交付报告里写明"未修 + 原因"）

### 10.1 桌宠路由方法严格性

Python `_route()` 方法无关；Rust 对 `/api/pets/configure`、`/api/pets/interact` 的非 POST 返 **405**。
**决策点**：要么放宽 Rust 到方法无关（严格 parity），要么保留 405 并在交付报告里声明为
"刻意收紧的安全偏差"。**必须显式选择并写进报告，不许默认放过。**
（注意 `assets/js/features/pet-batch.js` 是消费方，先看它实际发什么方法。）

### 10.2 非 Windows 剪贴板返回空捕获

`clipboard.rs:43-51`：非 Windows 平台永远返回空捕获。
后果：macOS / Linux 上 `toggle-app` 会发布一个**schema 合法但内容为空**的命令。
渲染页看不出区别，用户看到的是"点了没反应"。
**修法**：要么实现平台捕获，要么在命令里带上明确的"平台不支持"标记让前端能提示。
不允许保持静默空返回。

### 10.3 `runtime.rs:150` 用了已废弃的 `env::set_var`

用来把会话令牌传给 Linux GNOME 后端。Rust 2024 里 `env::set_var` 是 `unsafe`
（多线程下改环境变量是 UB）。**改成显式参数传递**，不要 `unsafe` 包住了事。

### 10.4 `webview/ipc.rs:52-67` 测试是假的

它调用 `parse_renderer_message` 但**丢弃返回值**，只有 65 字符 type 那一个用例做了断言。
补成真断言。这类"跑过了但什么都没验证"的测试比没有测试更危险。

### 10.5 Windows `set_opacity` 的静默失效（见 §7.2①）

**不是 bug**，但要在交付报告里写明，否则下一轮接手方会当成 bug 又"修"一遍。

### 10.6 `pet_host.rs:40-49` 管道句柄缺失（见 §5.3）

功能缺口，需要 `STARTUPINFOEX` + `CreatePipe`。

### 10.7 JSON 键序对 `assert_eq!` 不可见 🔴

`serde_json` 开了 `preserve_order`（提交 `16b7175`）。但 **`Map` 的相等比较忽略顺序**，
所以 `assert_eq!(body, expected)` **永远不会因为键序错误而失败**。

要断言键序，必须显式比对 `body.as_object().keys()` 的序列。
用户明确要求"响应键集**与嵌套形状与键序**"对齐——**这是唯一能证明键序的手段**。
写 parity 测试时，凡是涉及响应体的，都要额外加一条键序断言。

### 10.8 已记录但**尚未修**的其他 parity 项（非桌宠，但必须在报告里出现）

| 项 | 现状 |
|---|---|
| `read_request` 整路径 percent-decode | 会把 `%2F` 拆开，Python 不会 |
| `MAX_BODY = 32 MiB` | Python 是 2 MiB；Rust 先缓冲再答 `request_too_large` |
| >32 MiB body | Rust `read_body` 直接失败，Python 返 400 |
| `Location` 含控制字符 | Rust → `github_network_error` 400；Python 未捕获 `ValueError` → JSON 500 |
| 提交 `16b7175` 的 message | 写 "Nine Tests"，实际证据是 7 个（不改历史，但报告里说明） |

### 10.9 autostart 偏差（一组，非桌宠）

- `autostart.non-windows` 是 501 缺口
- `enabled` 用字符串匹配解析，Python 是从 body 直接取 JSON truthiness
- Rust 返 500 `error_code` 信封，Python 返 **HTTP 200** `{"ok":false,"error":str(e)}`
- Rust 写了一个 Python 从不写的 `autostart` 设置键
- `silent_command("reg")` **违反 §11 无 spawn 红线**

### 10.10 `batch2.rs:1291-1456` 的 `/api/skill-imports` handler 不忠实

凭空造 `"ok": true`、自铸 `src-<uuid8>` id、返回 `{ok, schema_version, sources}`，
而 Python 返回 `{schema_version, sources}`。**键集不一致 = parity 失败。**

### 10.11 命名 backlog

`check_upgrade` stub（`main.rs:3632`）、`sha_url` 原样回显偏差、约 35 处遮蔽重复、
`Store::graph` / `Store::deadlinks` 的删除证明、4 处残留 `unused doc comment` 警告、
`Command::new` 站点（powershell×3、cmd×3、xdg-open×2、open×2、explorer.exe×1）、
约 24 个从未审计的模块。

---

## 11. 🔴 无依赖红线

**交付的二进制不得 spawn 任何外部程序。**

现状违规点：
- 内核：`Command::new` powershell×3、cmd×3、xdg-open×2、open×2、explorer.exe×1
- `silent_command("reg")`（autostart 路径）
- Python 侧的 `taskkill`（`runtime.py:641-645`）和 PowerShell（`hermes_adapter.py:68-71`）——
  **Rust 移植时不许照抄这两个 shell-out**，要用 Win32 API 直接做（`TerminateProcess` / 进程枚举）。

**唯一被批准的例外：`wry`。**
Windows 上 wry 会拉起微软的 `msedgewebview2.exe`（Evergreen 运行时，首次可能需联网安装）。
这是**声明过的、用户知情的**例外，因为不用 WebView2 就得自带一个浏览器引擎（体积爆炸）。
桌宠 crate 本身：`src/` 里**零 `Command::new`、零 powershell、零 cmd、零网络调用**。
**保持这个状态。** 新增任何 spawn 都必须在交付报告里单列一条并说明理由。

---

## 12. 环境陷阱（每一条都让上一轮损失过时间）

| 陷阱 | 事实 | 对策 |
|---|---|---|
| **T 盘空间** | 实测 **13 GB 可用 / 94% 已用**（184G 总量）。每跑一次 cargo 门禁 `target/` 增长 **约 1 GB** | `link.exe` 退出码 **1318** = 磁盘满。每次 cargo 之后剪 `target/*/incremental` 和陈旧 deps。改一次 feature = 整套依赖重新生成 |
| **`--offline` 只认 `~/.cargo` 缓存** | 已缓存：TLS、tempfile、lopdf、tao、wry、windows-sys（桌宠 crate 有 `Cargo.lock` + `target/` + `dist/` 构建产物，证明依赖能离线解析）。**未缓存**：zip、notify、async 框架 | 加依赖前先看 `Cargo.toml` 现状 + 试 `cargo build --offline`。**不要**假定能联网拉 crate |
| **`readmd.exe` 占用文件** | 运行中的进程会锁住二进制，`cargo build` 失败 | build 前先杀：`tasklist \| grep -i readmd`（**注意下一条**） |
| **Git Bash 拒绝 `//FI`** | `tasklist //FI "..."` 被当成 UNC 路径 | 用 `tasklist \| grep -i readmd` |
| **`python3` 是静默 stub** | Windows 上 `python3` 不报错也不干活 | **永远用 `python`** |
| **多行补丁** | 内联 heredoc/sed 在这个环境容易静默不匹配 | 写进 `.pl` 文件再执行；执行后**必须**验证真的改了（上一轮有一次 perl 重写静默匹配 0 处，因为字面 `(` `)` 被当成正则捕获组） |
| **`rust/.gitignore`** | 隐藏 `target/` 和 `_archive/` | **Grep 工具看不见 `_archive/`**。`rust/_archive/` 是被放弃的早期架构草稿，里面的声明在权威树和 Python 里都不存在，**不是 parity 缺口来源**，不要照着它补功能 |
| **`rust/src/` 已不存在** | 上一轮的孤立重复树已移入 `rust/_archive/` | 现在权威树只有 `rust/readmd-kernel/`（workspace 唯一 member），`git ls-files rust/` = 75 个文件 |

---

## 13. 门禁命令（逐字可复制，全部来自上一轮真实通过的日志）

**内核（在仓库根执行）：**

```bash
cargo test --offline -p readmd-kernel --lib          # 基线：1673 passed / 0 failed / 2 ignored，约 149s
cargo test --offline -p readmd-kernel --bin readmd   # 基线：112 passed / 0 failed
cargo build --offline -p readmd-kernel               # EXIT=0
python rust/tools/endpoint_live_probe.py             # EXIT=0（活体端点探测）
```

另有 `rust/tools/endpoint_parity_gate.py`（parity 门禁）。

**桌宠 crate（独立 crate，不在 `rust/` workspace 里）：**

```bash
cd packages/readmd-pet-rust && cargo test --offline  # 基线：62 个 #[test]，全部在文件内 #[cfg(test)]，无 tests/ 目录
```

62 个测试的分布：protocol 10、clipboard 10、input 7、platform/windows 7（仅 Windows）、
security 6（1 个仅 unix）、parent_liveness 5（1 个仅 Windows）、commands 4、runtime 3、
ipc 3、snapshot 3、health 2、webview 1。

**打包**：`packages/readmd-pet-rust/scripts/build-package.py`（产出 `dist/ReadMD-Pet-Rust.zip`）。

**证据纪律（上一轮的硬规则）：**
- 日志文件的 mtime **必须晚于**最后一次源码编辑。用 `ls -la` 核对，别自说自话。
- 把 cargo 输出重定向到文件再 grep，例如：
  ```bash
  cargo test --offline -p readmd-kernel --lib 2>&1 | tee .qoder-scratch/gate_$(date +%H%M).log
  grep -E "test result:|FAILED|^error" .qoder-scratch/gate_*.log
  ```
- 参考基线日志：`.qoder-scratch/gate_skill_src.log`（2026-09-23 19:35，198 KB）。

**GAP 判定规则（重要，避免误报）：**
内核是单一 `_route` 分派器，handler 可以**自己返回 404 + JSON**。
所以**"返回 404" ≠ "未实现"**。只有分派器吐出的**纯文本 `not found`** 才算 GAP。
同理，函数名/端点名清点判"未实现"的误报率很高——
例如 `/api/tags` 是**出站** Ollama 探测，不是服务端路由。

---

## 14. 建议的 Phase 划分

**规则：Phase 内部不许停；Phase 之间必须有全量 `cargo test` 证据。前一个 Phase 未验收，不许开始下一个。**

### Phase 0 — 入库（§3）
提交桌宠 crate 的 13 改 + 2 未跟踪文件。
**验收**：`git status --porcelain -- packages/readmd-pet-rust` 干净（或只剩你能逐项解释的东西）；
`git stash list` 为空；从 HEAD 干净 checkout 能编译。

### Phase 1 — 建立基线
跑 §13 全部命令，记录数字。跑 `endpoint_parity_gate.py`。
**验收**：一份基线日志，且 mtime 晚于任何编辑。
**如果基线本身就是红的，先修红，不许带着红进 Phase 2。**

### Phase 2 — 桌宠 desktop 模块的**可视**验收（本任务重点）
起内核 → 起桌宠宿主 → `/browser` 或截图 → 逐项核对：
透明背景真的透明（能看到桌面）、无边框、置顶、不抢焦点、
点击穿透区域正确（精灵图外的点击落到桌面）、拖拽跟手、
多显示器/缩放变化后位置正确、切换桌宠无旧动画残影（验证 §6 的会话令牌）。
**验收**：Walkthrough artifact 里每一项都有截图 + 结论。
**这是唯一能证明 §7 配方正确的环节，`cargo test` 证明不了。**

### Phase 3 — 接线 5 个孤儿模块（§5）
`pet_launcher.rs`(4331)、`pet_queue.rs`(1298)、`pet_probe.rs`(1053)、
`desktop_pet.rs`(775)、`pet_window_state.rs`(568) —— 合计 **8025 行已写完但从未被调用的代码**。
逐个：先确认 Python 侧对应行为有 owner，再接进 `ROUTES` / 相应调用点。
**每接一个就跑全量测试。**

> 🔴 **接线后必须验证"真的被编译且真的被调用"。**
> 上一轮的血泪教训：写了 `.rs` 文件但没在 `lib.rs` 声明 `mod` ⇒ cargo **从来没编译过它**，
> 而所有测试照样全绿。验收手段：临时在函数入口加一个必然 panic 的断言，跑测试，
> **确认它真的炸了**，然后撤掉。或者用 `rust/tools/` 下类似的可达性检查。
> `pet_window_state.rs` 尤其可疑——它移植的是 `src/readmd_core/window_state.py`，
> **被错放在 pet 名下**，接线时要先想清楚它归属哪个 owner。

### Phase 4 — §10 偏差清单逐条处置
每条要么修，要么在报告里写"未修 + 原因"。**不许静默跳过。**
优先级建议：10.7（键序断言，影响所有 parity 测试的可信度）→ 10.4（假测试）→
10.2（跨平台静默失效）→ 10.6（孤儿窗口）→ 10.1（决策项）→ 10.3 → 10.9/10.10 → 10.8/10.11。

### Phase 5 — 无 spawn 红线清零（§11）
把 `taskkill` / PowerShell / `reg` 换成 Win32 API。逐个 `Command::new` 站点处置。

### Phase 6 — 轻量化 + 全量收尾
二进制体积、启动时间、内存占用对比 Python 版。全量测试。最终交付报告。

---

## 15. 交付报告固定格式（5 段，不可增删改名）

```
1. 本轮完成
2. 证据（命令 + 输出摘要 + 日志文件 mtime，必须晚于最后一次编辑）
3. 未通过 + 原因        ← 这一段不许省略，不许写"无"来敷衍；真的无就写"全量 N passed / 0 failed，命令逐条列出"
4. 未处置的已知偏差（引用 §10 编号 + 一句话原因）
5. 下一轮建议起点
```

**报告用中文。** 用户明确说过"中文告诉我"。

---

## 16. 禁止事项汇总

- ❌ **不 push**（提交可以，push 不行）
- ❌ **不提交 Rust 树以外的东西**（`assets/`、`readmd.py`、`tests/` 等一律不动）
- ❌ **不用 `// TODO` / `#[allow(...)]` 消音编译器警告**——修根因
- ❌ **不留 `todo!()` / `unimplemented!()`**（当前桌宠代码里一个都没有，保持）
- ❌ **不 spawn 外部程序**（§11，`wry` 是唯一已批准例外）
- ❌ **不改 §7 的 Win32 不变量**（除非你能引用 DWM 合成路径的具体理由）
- ❌ **不给 `PlatformBackend` 加 `Send`/`Sync`**（§7.4）
- ❌ **不用全局鼠标/键盘钩子**（§7.2③）
- ❌ **不放宽 `RendererAssetRouter` 白名单**（§9）
- ❌ **不删代码，除非满足删除三证**：`refs == 1`（只有定义处）+ `ROUTES` 里无限定名引用 + Python 侧对应行为有明确 owner。**`refs >= 2` 的同名项禁止批量删**（§5.1 那 8 个死重复 handler 就是 `refs>=2` 的典型，要一个一个证明）
- ❌ **不假定行号仍然有效**（§17）
- ❌ **不停在 Phase 中间**

---

## 17. 动手前的复现清单（把本文的事实变成你的事实）

```bash
cd "<repo>/"

# 1. 我在哪个提交
git rev-parse --short HEAD                       # 期望 81a322e

# 2. 桌宠 crate 的未提交状态（Phase 0 的对象）
git status --porcelain -- packages/readmd-pet-rust
git diff --stat -- packages/readmd-pet-rust
git ls-files --others --exclude-standard -- packages/readmd-pet-rust/src

# 3. desktop 模块真的在 packages/ 不在 rust/（§2）
grep -rn "DwmExtendFrameIntoClientArea" --include=*.rs .
grep -rn "SetWindowsHookEx" packages/readmd-pet-rust/src   # 期望：无输出（§7.2③）

# 4. 孤儿集群真的没有外部调用者（§5.0）
#    必须排除全部 5 个集群成员，否则只会看到集群内部互引（8 处）而误判为"已被使用"
CLUSTER='pet_launcher|pet_queue|pet_probe|desktop_pet|pet_window_state'
grep -rnE "($CLUSTER)::" rust/readmd-kernel/src --include=*.rs \
  | grep -vE "^rust/readmd-kernel/src/($CLUSTER)\.rs:"
# 期望：无输出（= 集群外零调用者）。若出现 server.rs / parity_pets.rs / main.rs 的命中，
# 说明有人已经接线了，本文 §5 过时，以你的输出为准。

# 5. 死重复 handler（§5.1）—— 确认 ROUTES 绑的是谁
grep -n "h_pet_interact\|h_pet_active\|h_pets\b" rust/readmd-kernel/src/server.rs | head -40

# 6. 桌宠路由零 pending、唯一 501 是 OPTIONS（§5.2）
grep -n "PENDING" rust/readmd-kernel/src/server.rs | head
grep -n "501" rust/readmd-kernel/src/server.rs | head

# 7. 无 spawn 红线现状（§11）
grep -rn "Command::new" rust/readmd-kernel/src packages/readmd-pet-rust/src

# 8. Python 权威：桌宠路由是方法无关的（§4.1）
sed -n '1050,1090p;1160,1175p' readmd.py
grep -n "/api/pets" readmd.py | head -30

# 9. 磁盘余量（§12）
df -h /t
```

**如果任何一条的输出与本文不符，以你的输出为准，并在交付报告第 3 段写明差异。**
本文可能已过时；代码不会。

---

## 18. 来源

**模型事实（本轮 WebFetch 实测）：**
- <https://ai.google.dev/gemini-api/docs/latest-model> — Gemini 3.8 Flash：1M 上下文、thinking level `low`/`medium`(默认)/`high`、强项 "long-horizon software engineering" 与 "complex multi-file refactoring"、`temperature` 与 `minimal` 已废弃
- <https://ai.google.dev/gemini-api/docs/models>
- <https://deepmind.google/models/model-cards/gemini-3-8-flash/>

**Antigravity harness 事实（本轮 WebFetch 实测）：**
- <https://codelabs.developers.google.com/getting-started-google-antigravity> — Antigravity 2.0 = agent 的 "central command center"；artifacts = **Task List** / **Implementation Plan** / **Walkthrough**；`/browser` 指令；**Agent Panel** 做代码审查；模型可选并带 thinking/effort 档位
- <https://codelabs.developers.google.com/agentic-ui-automation-with-antigravity>（已定位，**本轮未抓取**——UI 自动化细节请以你 harness 内的实际能力为准）

**代码事实**：全部来自本仓库 `81a322e` + 当前工作树的实测（`git status` / `git diff --stat` /
`wc -l` / `grep` / `ls`），以及上一轮已通过的门禁日志 `.qoder-scratch/gate_skill_src.log`。

**同目录下的其他交接文档（可交叉参考，但注意时效）：**
- `docs/handoffs/rust-kernel-handoff-qwen38max.md`（2026-09-22，内核侧；其中"`rust/` 未跟踪"
  和"`rust/src/` 孤立树"两条**已过时**——`rust/` 现有 75 个跟踪文件，`rust/src/` 已移入 `rust/_archive/`）
- `docs/handoffs/rust-parity-workpackages.md`（2026-09-22，parity 工作包拆分）
