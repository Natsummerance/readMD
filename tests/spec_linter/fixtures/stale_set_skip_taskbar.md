# ReadMD Desktop Overlay（桌面桌宠）全平台重构执行规格书
## —— v1.4.3 Architecture Freeze Candidate (True Multiplatform Architecture)

> **版本**：v1.4.3-Candidate (Architecture Freeze Candidate)  
> **状态说明**：本版本为 Freeze Candidate，暂不接受 Production Architecture Freeze。所有平台的最终 Certified 状态必须由自动化工具根据 Phase 0 PoC 及实机门禁报告机器生成，规格书本身无权单方面宣布生产认证。  
> **文档定位**：不可降级的工业级生产实施蓝图，指导 Phase 0 PoC 实机验证、动态门禁组合与平台认证落地。  
> **唯一黄金参考标准（Golden Reference）**：当前 Windows Electron 生产实现 (`packages/readmd-hermes-pet-adapter`) 与 Python 权威桥接 (`src/readmd_modules/pet/hermes_adapter.py`)。  
> **重构四大红线原则**：  
> 1. **全平台性**：严禁通过将不支持的平台简单降级为 In-App 网页阅读器内部弹窗，来伪装“全平台支持”。必须为 Windows、macOS、Linux X11/XWayland、KDE/wlroots 原生 Wayland、GNOME 原生 Wayland 提供明确、可验证的原生能力路径。  
> 2. **真实生态事实为唯一基准**：严禁虚构任何底层 API、crate 版本（WRY 0.57.0, Tao 0.37.0, muda 0.19.3, gtk-layer-shell 0.8.2）或 MSRV（Rust 1.85.0）。依赖 crates.io 真实元数据，与 GNOME Shell / Mutter / Wayland 协议严格对齐。  
> 3. **不可修改黄金参考（Golden Reference Immutable）**：重构目标是让 Rust 宿主 100% 还原现有 Electron 行为。严禁为迁就 Rust 而修改现行 Electron 与 Python 契约。  
> 4. **统一平台后端族（Platform Backend Family）**：摒弃单一 Tao 窗口适应全平台的粗糙假设，确立 `OverlayWindowBackend` 统一抽象，各平台提供 Win32、Cocoa、X11、Layer-Shell 与 GNOME Companion 专属后端。

---

## 目录
1. [0. 真实生态事实与架构原则](#0-真实生态事实与架构原则)
2. [1. Golden Contract 可读基准源](#1-golden-contract-可读基准源)
3. [2. 双轨驱动：Engine Orchestrator](#2-双轨驱动engine-orchestrator)
4. [3. 统一平台后端族 Platform Backend Family 架构](#3-统一平台后端族-platform-backend-family-架构)
5. [4. Native Wayland Layer-Shell 深度设计 (KDE / wlroots)](#4-native-wayland-layer-shell-深度设计-kde--wlroots)
6. [5. GNOME Wayland 生产路径：ReadMD GNOME Shell Companion](#5-gnome-wayland-生产路径readmd-gnome-shell-companion)
7. [6. Wayland Input Model 极限设计](#6-wayland-input-model-极限设计)
8. [7. 黄金交互区域 Golden Hit Region 原理与交互边界](#7-黄金交互区域-golden-hit-region-原理与交互边界)
9. [8. ReadMD Pet IPC v1：会话代际隔离与 WRY 官方 API](#8-readmd-pet-ipc-v1会话代际隔离与-wry-官方-api)
10. [9. 完整安全资产协议 (Secure Asset Protocol)](#9-完整安全资产协议-secure-asset-protocol)
11. [10. 文件 Durable FIFO 字节级协议规范](#10-文件-durable-fifo-字节级协议规范)
12. [11. Native Drag & Drop：WRY 0.57 官方路径与回归门禁](#11-native-drag--dropwry-057-官方路径与回归门禁)
13. [12. CSP 防御与偏门资产沙箱](#12-csp-防御与偏门资产沙箱)
14. [13. 跨平台剪贴板 ClipboardService 规范](#13-跨平台剪贴板-clipboardservice-规范)
15. [14. 多实例 Scope 与用户 Profile 隔离](#14-多实例-scope-与用户-profile-隔离)
16. [15. Parent Liveness Pipe 继承模型与防孤儿保证](#15-parent-liveness-pipe-继承模型与防孤儿保证)
17. [16. 权威状态与对齐调和模型 (Reconciliation State Model)](#16-权威状态与对齐调和模型-reconciliation-state-model)
18. [17. Renderer 保活与后台探活路径](#17-renderer-保活与后台探活路径)
19. [18. 权威依赖配置与构建规范 (Authoritative Cargo Specification)](#18-权威依赖配置与构建规范-authoritative-cargo-specification)
20. [19. 性能协议：各阶段时延与全局内存预算](#19-性能协议各阶段时延与全局内存预算)
21. [20. 平台认证元组规范 (Platform Certification Tuples)](#20-平台认证元组规范-platform-certification-tuples)
22. [21. 动态门禁组合模型 (Dynamic Gate Composition Model)](#21-动态门禁组合模型-dynamic-gate-composition-model)
23. [22. Fullscreen、Workspace 与虚拟桌面平台化规范](#22-fullscreenworkspace-与虚拟桌面平台化规范)
24. [23. 运行时分发、健康所有权与 A/B 升级 (PetRuntimeInstallerV2)](#23-运行时分发健康所有权与-ab-升级-petruntimeinstallerv2)
25. [24. 供应链安全、代码签名与权限边界](#24-供应链安全代码签名与权限边界)
26. [25. Release Stop Conditions 严格红线](#25-release-stop-conditions-严格红线)
27. [26. Codex 实施执行协议与交接模式](#26-codex-实施执行协议与交接模式)
28. [27. 最终验收与重构判断标准](#27-最终验收与重构判断标准)
29. [28. Platform Capability Matrix 综合平台能力矩阵](#28-platform-capability-matrix-综合平台能力矩阵)
30. [29. 架构决策与实机实证项注册表 (Empirical Validation Register)](#29-架构决策与实机实证项注册表-empirical-validation-register)
31. [30. Real Hardware Certification Matrix 实机硬件认证矩阵](#30-real-hardware-certification-matrix-实机硬件认证矩阵)
32. [31. Change Log (v1.3.0 -> v1.4.0 演进)](#31-change-log-v130---v140-演进)
33. [32. Change Log (v1.4.0 -> v1.4.1 Freeze Candidate 演进)](#32-change-log-v140---v141-freeze-candidate-演进)
34. [33. Change Log (v1.4.1 -> v1.4.2 Freeze Candidate 演进)](#33-change-log-v141---v142-freeze-candidate-演进)
35. [34. Change Log (v1.4.2 -> v1.4.3 Architecture Freeze Candidate 演进)](#34-change-log-v142---v143-architecture-freeze-candidate-演进)
36. [附录 A：已验证关键系统底层事实](#附录-a已验证关键系统底层事实)
37. [附录 B：Phase 0 实施执行清单与 ADR-0001 草案](#附录-bphase-0-实施执行清单与-adr-0001-草案)

---

## 0. 真实生态事实与架构原则

### 0.1 上游生态真实事实审计与 Linux 单二进制 ADR 决策
在进入规格书前，已对 crates.io、FreeDesktop、GNOME 与各大图形系统的当前真实源码与 API 完成严格实测审计：

1. **WRY / Tao / muda 依赖与 Feature Matrix 真实事实**：
   - 当前最新 stable 产物：`wry = "0.57.0"`，`tao = "0.37.0"`，`muda = "0.19.3"`，`gtk-layer-shell = "0.8.2"`。
   - crates.io 真实元数据证实：`wry 0.57.0` 的 `rust_version = "1.85"`，`tao 0.37.0` 的 `rust_version = "1.85"`。
   - **WRY Feature 依赖铁律**：WRY 明确规定 `"os-webview must be enabled for the crate to work"`。严禁在没有开启 `os-webview` 时声明 `default-features = false`，否则将彻底剥离 WebView 核心驱动！
   - **muda Target 隔离依赖事实 (P0-55)**：muda 0.19.3 默认开启 `gtk` 与 `libxdo`。严禁在公共 `[dependencies]` 声明 muda，否则 Cargo feature union 会将 `libxdo` 强加给 Linux target！必须按 Target 分离声明，Linux 下使用 `default-features = false, features = ["gtk"]`，彻底消除 Pure Wayland 环境对 `libxdo.so` 的无谓污染。
   - **gtk-layer-shell API Feature 要求 (P0-57)**：调用 `gtk_layer_shell::is_supported()` 需要 `v0_5`，调用 `protocol_version()` 需要 `v0_6`。因此依赖声明必须显式开启 `features = ["v0_6"]`。
   - **macOS objc2 生态版本统一 (P0-56)**：统一至与 muda 0.19.3 一致的 `objc2 0.6`、`objc2-app-kit 0.3`、`objc2-foundation 0.3` 生态族，禁止在一个二进制内混用 objc2 0.5 与 0.6。
   - **工具链锁定**：统一锁定至 **Rust 1.85.0**（`rust-version = "1.85.0"`）。

2. **Linux 发行架构决策：单二进制方案 (ADR-0002: Single Linux Production Binary)**：
   - **业务事实**：ReadMD Linux 以统一的 AppImage / deb 发行（如 `ReadMD-linux-x86_64-v2.3.9.AppImage`），必须能够开箱即用地在 X11、XWayland 与 Native Wayland（KDE/GNOME）环境下无缝运行，绝不拆分为多个架构发行包。
   - **Cargo Feature 不是运行时切换器**：Cargo feature 是编译期静态添加物（Compile-time Additive），不能按用户的桌面环境在编译期做二选一。
   - **统一 Linux 生产 Feature Profile (P0-58)**：
     定义唯一发布 profile `linux-production = ["wayland-layer-shell", "gnome-companion"]`。
   - **运行时动态后端分发 (Runtime Backend Dispatch)**：
     GTK3 内部原生集成了 X11 与 Wayland GDK 后端。主程序在启动时通过 `$WAYLAND_DISPLAY`、`GDK_IS_WAYLAND_DISPLAY` 及桌面环境变量执行运行时探测：
     - 若为 GNOME Wayland $	o$ 激活 `GnomeCompanionBackend`
     - 若为 KDE / wlroots Wayland $	o$ 激活 `LayerShellBackend`（通过 `gtk_layer_shell::is_supported()` 动态门禁）
     - 若为 X11 / XWayland $	o$ 激活 `X11Backend`
     Native Wayland 下通过 `WebViewBuilderExtUnix::build_gtk(container)` 将 WebKitGTK 容器挂载入 GTK 窗口，无需剥离编译期 `x11` 特性。

### 0.2 项目口径定义：交付物全平台 vs 桌面桌宠全平台
必须区分两个层级：
- **A. 整体软件交付物全平台**：主程序阅读器提供跨平台支持（Windows / macOS / Linux / UOS / 麒麟）。
- **B. 桌面桌宠 (Desktop Overlay) 全平台**：
  必须基于各平台原生合成器提供与 Windows 完全对等的置顶、透明与点击穿透体验。
  **目标能力（Target Capability）**：Golden Equivalent。
  **认证结果（Certification Result）**：在 Phase 0 自动化实机门禁报告签署前，标定为 `not yet proven`。
  严禁在设计阶段宣称“100% 已等价”，所有结论由实体机测试数据背书。

---

## 1. Golden Contract 可读基准源

### 1.1 必须固化的基准源文件与 SHA-256 锚点
实施前在 `docs/architecture/pet-rust/golden-contract.json` 中固化 7 个文件的真实哈希：
1. `src/readmd_modules/pet/hermes_adapter.py`
2. `packages/readmd-hermes-pet-adapter/package.json`
3. `packages/readmd-hermes-pet-adapter/src/electron-main.ts`
4. `packages/readmd-hermes-pet-adapter/src/preload.ts`
5. `packages/readmd-hermes-pet-adapter/src/renderer.tsx`
6. `packages/readmd-hermes-pet-adapter/src/pet-life.ts`
7. `packages/readmd-hermes-pet-adapter/src/bridge-transport.ts`

任何对上述 7 个文件的篡改将直接熔断构建。

### 1.2 Golden ABI 强约束
- 窗口透明度范围：`0.35` ~ `1.0`。
- 坐标系统：标准 Electron 全局虚拟桌面 DIP（Device Independent Pixels）。
- 离手阈值：吸附检测距离固定为 `12 DIP`。
- 物理交互区域：必须 100% 保持 Sprite 与 Live2D 现有交互多边形。

---

## 2. 双轨驱动：Engine Orchestrator

### 2.1 用户界面无感切换
前端设置界面仅增加单一键值 `desktop_pet_engine: "electron" | "rust"`，不增加任何额外 i18n 负担。

### 2.2 Python Engine Orchestrator 状态机与安全终止 (P0-64)
- **精准 PID 追踪**：Orchestrator 启动子进程时必须保存其确切的子进程 PID 及 OS 进程句柄（Windows JobObject / Unix process group / pidfd）。
- **禁止盲目批量杀死**：**严禁使用 `kill_processes_by_target` 扫描可执行文件名进行批量 kill**！正常退出流程为：发送优雅退出命令 $	o$ 等待 2500ms 超时 $	o$ 仅向该 exact PID 派发 SIGKILL 或 TerminateProcess。
- **故障自动回退**：Rust 引擎连续 3 次启动异常或 72h 稳态泄露超标时，Orchestrator 自动将配置回退为 `electron`，并产生遥测报警。

---

## 3. 统一平台后端族 Platform Backend Family 架构

定义核心抽象特征 `OverlayWindowBackend`，将平台原生特性彻底下沉：

### 3.1 强类型坐标系统与严格主线程所有权 (Thread Safety Axiom)
建立强类型坐标体系以消除歧义：
- `BridgeDipRect`：Python 桥接层传递的标准 Electron 全局虚拟桌面 DIP 坐标。
- `OutputLocalDipRect`：相对于具体物理显示器（Monitor / Output）左上角的局部 DIP 坐标。
- `OutputPlacement`：包含具体输出显示器标识与局部坐标的排布结构体。

```rust
// packages/readmd-pet-rust/src/window/coords.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeDipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputLocalDipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputPlacement {
    pub output_id: String, // 会话内局部标识，如 "DP-1"
    pub local_rect: OutputLocalDipRect,
    pub scale_factor: f64,
}
```

- **主线程所有权法则**：`OverlayWindowBackend` **严禁声明 `Send + Sync`**！窗口、WebView、上下文菜单必须由 Main GUI Thread 独占。
- **禁止 `#[tokio::main]` 接管 GUI 主线程 (P0-63)**：操作系统主线程独占运行原生事件循环 (`EventLoop::run`)。异步 Tokio 运行时仅在后台工作线程启动，跨线程指令必须通过 `EventLoopProxy` 单向分发。

```rust
// packages/readmd-pet-rust/src/platform/backend.rs
use crate::window::coords::{BridgeDipRect, OutputPlacement};
use crate::window::interaction::GoldenInteractionRegion;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BackendCapabilities: u32 {
        const ABSOLUTE_PLACEMENT          = 1 << 0; // 支持全局屏幕绝对坐标定位 (Win32, Cocoa, X11)
        const OUTPUT_RELATIVE_PLACEMENT   = 1 << 1; // 支持基于屏幕输出与边距的定位 (Layer-Shell)
        const COMPANION_MANAGED_PLACEMENT = 1 << 2; // 由桌面辅助扩展控制定位 (GNOME Companion)
        const NATIVE_INPUT_REGION         = 1 << 3; // 支持表面级输入穿透区域 (Wayland Surface)
        const GLOBAL_CURSOR_OBSERVATION   = 1 << 4; // 支持全局光标探测 (Win32, Cocoa, X11)
        const WORKSPACE_STICK             = 1 << 5; // 支持全工作区常驻 (canJoinAllSpaces, _NET_WM_DESKTOP)
        const KEYBOARD_ON_DEMAND          = 1 << 6; // 支持按需键盘交互模式 (Layer-Shell v4+)
    }
}

pub trait OverlayWindowBackend {
    fn capabilities(&self) -> BackendCapabilities;
    fn create(&mut self, initial_bounds: BridgeDipRect, title: &str) -> Result<(), String>;
    fn show_inactive(&mut self) -> Result<(), String>;
    fn hide(&mut self) -> Result<(), String>;
    fn close(&mut self) -> Result<(), String>;
    fn set_bounds(&mut self, bounds: BridgeDipRect) -> Result<(), String>;
    fn bounds(&self) -> Result<BridgeDipRect, String>;
    fn set_opacity(&mut self, opacity: f64) -> Result<(), String>;
    fn set_focusable(&mut self, focusable: bool) -> Result<(), String>;
    fn set_interaction_region(&mut self, region: &GoldenInteractionRegion) -> Result<(), String>;
    fn set_above(&mut self, above: bool) -> Result<(), String>;
    fn set_all_workspaces(&mut self, stick: bool) -> Result<(), String>;
    fn begin_drag(&mut self, start_x: f64, start_y: f64) -> Result<(), String>;
    fn update_drag(&mut self, delta_x: f64, delta_y: f64) -> Result<BridgeDipRect, String>;
    fn end_drag(&mut self) -> Result<BridgeDipRect, String>;
    fn get_work_areas(&self) -> Result<Vec<BridgeDipRect>, String>;
}
```

### 3.2 平台后端映射表
| 操作系统环境 | 运行时判定规则 | 绑定后端实现 | 核心底层机制 |
|---|---|---|---|
| **Windows 10 / 11** | `cfg(target_os = "windows")` | `Win32Backend` | Win32 API, `WS_EX_TOPMOST`, `WS_EX_LAYERED`, OLE DnD |
| **macOS 13+** | `cfg(target_os = "macos")` | `CocoaBackend` | Cocoa `NSWindow`, `canJoinAllSpaces`, Level 3 |
| **Linux (KDE / wlroots)** | `$WAYLAND_DISPLAY` 且 Layer-Shell 支持 | `LayerShellBackend` | `zwlr_layer_shell_v1`, Margins 映射, Surface Input Region |
| **Linux (GNOME Wayland)** | `$WAYLAND_DISPLAY` 且 GNOME 环境 | `GnomeCompanionBackend` | GNOME Shell 扩展, Mutter `move_frame`, UNIX Socket |
| **Linux (X11 / XWayland)**| `$DISPLAY` 且 XOpenDisplay 成功 | `X11Backend` | EWMH 协议, XShape 掩模穿透, X11 光标采样 |
| **Linux (Pure Wayland 不支持)**| 无 X11 且无可用 Wayland 后端 | `None` (Fail-Closed) | 触发 `DesktopOverlayUnsupportedForTuple`，回退 In-App |

---

## 4. Native Wayland Layer-Shell 深度设计 (KDE / wlroots)

### 4.1 gtk-layer-shell 依赖策略与 Pre-Realize 生命周期铁律 (P0-59)
`gtk-layer-shell` 官方规定：`gtk_layer_init_for_window(window)` **必须在 GtkWindow 被 realize（映射为 X11/Wayland 表面）之前调用**。
- **窗口所有权裁决 (Route A)**：
  在 `LayerShellBackend` 中，后端直接拥有原生的 `gtk::ApplicationWindow`（而非由 Tao 先行创建并 realize）。
  执行顺序严格为：
  `gtk::ApplicationWindow::new()` $	o$ `gtk_layer_shell::init_for_window()` $	o$ `set_layer/set_anchor/set_margin` $	o$ 挂载 WebKitGTK 容器 $	o$ `window.show_all()`。
  记录为 **`VAL-21 — LayerShell Pre-Realize Creation Order`**。

### 4.2 能力探针隔离与不可变能力结构体 (P0-60)
`is_supported()` 与 `protocol_version()` 涉及与合成器的阻塞式 Roundtrip，**严禁在鼠标移动、拖动或调和热路径中调用**。
必须在 `HostLifecycle::Probing` 阶段仅执行一次，并将结果固化为不可变结构体：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerShellCapabilities {
    pub supported: bool,
    pub protocol_version: u32,
    pub keyboard_on_demand: bool,
}
```

### 4.3 边距映射与工作区/面板避让 (VAL-24, P0-79)
Layer-Shell 通过锚定与 Margins 排布窗口。必须结合合成器 Exclusive Zones（面板、Dock 占用区域）实现与 Electron `Display.workArea` 100% 对齐的边界钳夹算法：
```rust
// [PSEUDOCODE]
pub fn apply_layer_margins(window: &gtk::ApplicationWindow, placement: OutputPlacement, work_area: OutputLocalDipRect) {
    gtk_layer_shell::set_layer(window, gtk_layer_shell::Layer::Overlay);
    gtk_layer_shell::set_anchor(window, gtk_layer_shell::Edge::Top, true);
    gtk_layer_shell::set_anchor(window, gtk_layer_shell::Edge::Left, true);
    
    // 钳夹至有效工作区
    let clamped_x = placement.local_rect.x.max(work_area.x).min(work_area.x + work_area.width - placement.local_rect.width);
    let clamped_y = placement.local_rect.y.max(work_area.y).min(work_area.y + work_area.height - placement.local_rect.height);

    gtk_layer_shell::set_margin(window, gtk_layer_shell::Edge::Left, clamped_x as i32);
    gtk_layer_shell::set_margin(window, gtk_layer_shell::Edge::Top, clamped_y as i32);
}
```

---

## 5. GNOME Wayland 生产路径：ReadMD GNOME Shell Companion

针对 GNOME Wayland 拒绝支持 Layer-Shell 的现状，通过专用轻量 Shell 扩展提供原生定位与层级控制：

### 5.1 通信信道、身份发现与安全隔离 (P0-65, P0-66, P0-68)
- **Socket 路径与权限**：强制使用 `$XDG_RUNTIME_DIR/readmd-pet-gnome-companion.sock`，权限 `0600`。
- **对端 UID 强鉴权**：通过 `SO_PEERCRED` 提取对端 UID，必须等于当前运行用户的 UID。
- **窗口所有权发现机制 (No Fake Window ID)**：
  Rust 宿主无需声明虚构的内部窗口 ID。Companion 通过遍历全局窗口 actor：
  `global.get_window_actors().filter(w => w.meta_window.get_pid() == peer_pid)`，匹配到唯一的桌宠窗口后绑定。
- **GNOME Shell 防崩溃熔断 (Crash Containment)**：
  Extension 运行于 Shell 进程内部！严禁任何同步阻塞循环。使用 GIO 异步 Socket 服务，单条消息上限 $\le 64	ext{ KB}$，每条命令包裹在独立的 `try/catch` 中，异常直接关闭客户端连接，严禁冒泡抛出到 Shell 主事件循环。

### 5.2 GNOME 45+ ESM 与 GNOME 42-44 Legacy 双产物架构 (P0-33, P0-69)
GNOME 45 之后全面切换为 ESM 语法，旧版 GJS 遇到 `import` 语句会直接触发 Parse Error 崩溃。因此建立双产物目录：
```
docs/architecture/pet-rust/companion/
├── legacy/                # GNOME 42 ~ 44 (Ubuntu 22.04 / Debian 12)
│   ├── extension.js       # 使用 GJS imports 语法
│   └── metadata.json      # shell-version: ["42", "43", "44"]
└── esm/                   # GNOME 45 ~ 50+ (Ubuntu 24.04 / Fedora 40+)
    ├── extension.js       # 使用标准 ESM export default 语法
    └── metadata.json      # 由 CI 认证矩阵机器生成支持的 shell-version
```

### 5.3 真实 Mutter 窗口 API 与混合 DPI 坐标映射 (P0-32, P0-39)
- **任务栏隐藏真实 API**：彻底清除虚构的 `set_skip_taskbar`，统一调用 `metaWindow.set_skip_taskbar(true)`。
- **GnomeCoordinateMapper (VAL-13)**：Mutter `move_frame(true, x, y)` 接受 Stage 坐标。在 100%、125%、150%、200% 混合 DPI 下，通过 `GnomeCoordinateMapper` 将全局 DIP 映射为 Stage 物理/逻辑坐标。

---

## 6. Wayland Input Model 极限设计

### 6.1 原生 Input Region 穿透模型
Wayland 客户端绝不允许全局轮询光标。宿主通过 Wayland 协议的 `wl_surface::set_input_region` 设置输入区域：
- 桌宠透明区域设为**空区域（Empty Region）**，合成器将指针事件直接下发给底层桌面图标与其它窗口；
- 仅当光标进入桌宠有效模型轮廓时，才将对应多边形集合设为有效输入区域。

### 6.2 实体穿透测试基准 (VAL-01, Gate-LayerShell-02)
在实体 Wayland 测试机上，桌宠窗口下方放置原生测试按钮，点击桌宠透明空区域，**底层按钮必须 100% 接收到物理鼠标点击事件**，以此证实 WebKitGTK 子表面不截留事件。

---

## 7. 黄金交互区域 Golden Hit Region 原理与交互边界

### 7.1 Golden 语义优先于性能优化 (P0-37, P0-77)
彻底删除任何“强制通过网格聚类将区域近似为 $\le 64$ 个矩形”的有损假定：
1. **精确表达优先**：Phase 0 完整捕获 Sprite 与 Live2D 的 Golden 交互区域。若 Golden 表现为边界盒，则为 1 个矩形；若为离散部件，则精确表示。
2. **复杂度预算 (RegionComplexityBudget)**：
   设定平台复杂度预算门槛。若几何多边形超出预算，**直接判定门禁失败并阻断认证**，严禁单方面擅自执行有损简化改变用户点击语义。

---

## 8. ReadMD Pet IPC v1：会话代际隔离与 WRY 官方 API

### 8.1 安全 Session 标识符的安全边界澄清 (P0-36)
- **核心职能**：`webview_session_id`（128-bit CSPRNG）与 `navigation_generation`（递增计数器）仅用于**代际隔离、陈旧异步消息丢弃与导航生命周期绑定**。
- **禁止视作鉴权密钥**：该标识会注入受信任的 Renderer JS。系统的真正安全边界由 CSP、Secure Asset Protocol 路径沙盒与强类型校验构成。

### 8.2 WRY 官方 IPC 接口与 Frame 鉴权 (P0-88, P0-89)
WRY Linux 下子 frame IPC 请求的 URI 可能与主 frame 相同，因此**严禁依赖 `request.uri()` 作为 frame 鉴权依据**。安全隔离依赖主 frame 专用守卫与代际令牌校验。

---

## 9. 完整安全资产协议 (Secure Asset Protocol)

全面恢复并固化自定义资产协议规范：

### 9.1 平台专属 Origin 映射
- **macOS / Linux**：`readmd-pet://localhost/`
- **Windows (WebView2)**：
  默认基线：`http://readmd-pet.localhost/`；
  在运行时支持时开启 `WebViewBuilderExtWindows::with_https_scheme(builder, true)` $	o$ `https://readmd-pet.localhost/`。
  宿主通过 `AssetOriginResolver` 动态处理，前端不硬编码协议头。

### 9.2 资产源隔离与用户资产执行禁令
1. **内置受信任资产 (Bundled Trusted Assets)**：随包分发，基于签名清单与 SHA-256 白名单直接加载。
2. **用户扩展桌宠资产 (User Pet Assets)**：用户目录下的模型，**严禁包含或执行任何 HTML、JavaScript、WASM 或系统动态库**！仅允许读取静态数据（PNG, WebP, JPEG, JSON, moc3, WAV, MP3）。

### 9.3 严格路径沙盒解析器与 TOCTOU 防御 (AssetOriginResolver, P0-87)
所有资产读取必须经过：
1. 单次 percent-decode，严禁二次解码穿越；
2. 拒绝包含 `..`、NUL 字符（`0x00`）、`\`、驱动器盘符 `C:`、UNC 路径 `\\`；
3. **TOCTOU 防御**：用户模型包在导入时通过 Staged 校验解压至只读资产根目录，运行时通过规范化路径校验 `resolved.starts_with(&asset_root)`，拒绝越界符号链接；
4. 强制注入安全响应头：`X-Content-Type-Options: nosniff` 与 `Cache-Control: immutable, max-age=31536000`。

### 9.4 严格内容安全策略 (Content Security Policy)
```html
<meta http-equiv="Content-Security-Policy" content="
  default-src 'none';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: blob:;
  font-src 'self' data:;
  connect-src 'self';
  media-src 'self';
  worker-src 'none';
  frame-src 'none';
  object-src 'none';
  base-uri 'none';
  form-action 'none';
  frame-ancestors 'none';
">
```

### 9.5 运行时行为全面封锁与 IIFE 主 Frame 隔离守卫 (P0-73)
1. **导航拦截**：`with_navigation_handler` 仅放行内部 `readmd-pet` origin，拒绝任何外部导航；
2. **新窗口拦截**：`with_new_window_req_handler` 恒定返回 `false`；
3. **系统权限**：摄像头、麦克风、地理位置、通知全部拒绝；
4. **IIFE 主 Frame 隔离守卫 (P0-73)**：
   针对 Windows WebView2 子 frame 注入特性，采取双保险机制。注入脚本首行必须包裹在 IIFE 中，**严禁在顶层直接执行 return 以免产生 SyntaxError**：
   ```javascript
   (() => {
       if (window.top !== window.self) {
           console.warn("[ReadMD] Blocked petOverlay initialization in non-top frame");
           return;
       }
       // 挂载 window.readmdPet 客户端桥接
   })();
   ```

---

## 10. 文件 Durable FIFO 字节级协议规范

### 10.1 协议行格式与单调递增保证
FIFO 文件位于 `<runtime_dir>/events/`。
首条序号严格为 1。时钟回拨时，文件名时间戳序列严格保持单调递增。

### 10.2 SnapshotReader 签名机制
基于设备号、Inode 与纳秒时间戳追踪快照变更，防御编辑器原子重命名写入导致的文件变更丢失。

---

## 11. Native Drag & Drop：WRY 0.57 官方路径与回归门禁

### 11.1 WRY 0.57 规范 API 与跨平台构建 (P0-27)
```rust
let mut builder = wry::WebViewBuilder::new();

builder = builder.with_drag_drop_handler(|event| {
    match event {
        wry::DragDropEvent::Drop { paths, position } => {
            handle_native_file_drop(paths, position);
            true
        }
        wry::DragDropEvent::Enter { paths, position } => true,
        wry::DragDropEvent::Over { position } => true,
        wry::DragDropEvent::Leave => true,
        _ => false,
    }
});

#[cfg(not(target_os = "linux"))]
let webview = builder.build(&window)?;

#[cfg(target_os = "linux")]
let webview = wry::WebViewBuilderExtUnix::build_gtk(&builder, &gtk_box_container)?;
```

### 11.2 Windows custom DnD 副作用与 Golden DnD 依赖清册 (P0-28)
实机源码审计证实：
- `@readmd/hermes-pet-adapter` 的模型拖拽通过 Pointer/Mouse 事件驱动，**不依赖 HTML5 `draggable="true"`**；
- 界面内**不存在任何 `<input type="file">` 元素**；
- 设立 `Gate-Core-10` 固化此不变性，检测到新增 HTML5 DnD 依赖时熔断构建。

### 11.3 readmdPet.dropFiles 语义（Host ABI Exception）
保持 `window.readmdPet.dropFiles(files)` 方法签名存在，静默返回并记录遥测，实际文件拖放完全由宿主原生回调接管。

---

## 12. CSP 防御与偏门资产沙箱

生产环境下完全剥离 `unsafe-eval`，任何动态 JS 代码执行均被严格禁止。

---

## 13. 跨平台剪贴板 ClipboardService 规范

- Windows：`CF_HDROP` / Win32 API
- macOS：`NSPasteboard`
- Linux：`GtkClipboard`（适配 `CLIPBOARD` 与 `PRIMARY` 选择区）

---

## 14. 多实例 Scope 与用户 Profile 隔离

- macOS：`~/Library/Application Support/ReadMD/runtime/readmd-pet-<ProfileHash>.lock` + `flock`
- Linux：`$XDG_RUNTIME_DIR/readmd/pet-<ProfileHash>.lock` + `flock`
- Windows：命名互斥体 `Global\ReadMD-Pet-<ProfileHash>`

---

## 15. Parent Liveness Pipe 继承模型与防孤儿保证

Rust 端仅创建并持有管道读端，写端由 Python 父进程持有。Rust 端绝不创建写句柄，从源头上杜绝 WebView2 辅助进程继承写端句柄导致 EOF 失效。

---

## 16. 权威状态与对齐调和模型 (Reconciliation State Model)

废除一维 14 状态枚举机，确立调和模型：

### 16.1 状态正交解耦与 AppliedState 引入 (P0-29, P0-75, P0-76)
```rust
// packages/readmd-pet-rust/src/state/mod.rs
use crate::window::coords::BridgeDipRect;

#[derive(Debug, Clone, PartialEq)]
pub enum RendererKind {
    Sprite,
    Live2D,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesiredOverlayState {
    pub visible: bool,
    pub fullscreen: bool,
    pub bounds: BridgeDipRect,
    pub opacity: f64,
    pub renderer: RendererKind,
    pub snapshot_revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedOverlayState {
    pub visible: bool,
    pub bounds: BridgeDipRect,
    pub opacity: f64,
    pub renderer: RendererKind,
    pub backend_generation: u64,
    pub navigation_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLifecycle {
    Booting,
    Probing,
    Running,
    Suspended,
    Degraded,
    ShuttingDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceState {
    Absent,
    Creating,
    Loading,
    Ready,
    Recovering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputState {
    PassThrough,
    Interactive,
    Dragging,
    MenuOpen,
    TextInput,
}
```

### 16.2 确定性调和函数与绝对优先级法则 (P0-30)
优先级准则：
$$	ext{ShuttingDown} > 	ext{Suspended} > (	ext{visible} = 	ext{false}) > (	ext{fullscreen} = 	ext{true}) > 	ext{Recovering/Loading} > 	ext{Ready}$$
- **拖拽强中断**：用户物理拖动时，若快照变为 `visible=false` 或 `fullscreen=true`，立即派发 `InputEffect::AbortDrag` 强行终止指针捕获并关闭/隐匿窗口，**绝不等待 `mouseup`**。
- **序列模糊测试 (VAL-11)**：通过 `proptest` 随机生成 50,000 条事件组合序列，断言状态模型在任何并发交织下永不发生坐标撕裂与死锁。

---

## 17. Renderer 保活与后台探活路径

分离 `webview_session_id`（实例代际）与 `navigation_generation`（导航代际，P0-74）。
L1 级崩溃执行原地重新导航并自增 `navigation_generation`；L2 级崩溃执行 WebView 彻底销毁重建。

---

## 18. 权威依赖配置与构建规范 (Authoritative Cargo Specification)

### 18.1 全平台统一定义：Cargo.toml (P0-55, P0-56, P0-57, P0-58)
```toml
# packages/readmd-pet-rust/Cargo.toml
[package]
name = "readmd-pet-rust"
version = "0.1.0"
edition = "2021"
rust-version = "1.85.0"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.43", features = ["rt-multi-thread", "sync", "time", "io-util", "fs"] }
rand = "0.8"
bitflags = "2.6"
raw-window-handle = "0.6"

# -----------------------------------------------------------------------------------------
# Windows Target
# -----------------------------------------------------------------------------------------
[target.'cfg(target_os = "windows")'.dependencies]
wry = { version = "0.57.0", features = ["os-webview"] }
tao = { version = "0.37.0", features = ["rwh_06"] }
muda = { version = "=0.19.3", default-features = false }
windows-sys = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_System_Threading",
    "Win32_System_JobObjects",
    "Win32_System_Pipes"
] }

# -----------------------------------------------------------------------------------------
# macOS Target
# -----------------------------------------------------------------------------------------
[target.'cfg(target_os = "macos")'.dependencies]
wry = { version = "0.57.0", features = ["os-webview"] }
tao = { version = "0.37.0", features = ["rwh_06"] }
muda = { version = "=0.19.3", default-features = false }
objc2 = "0.6"
objc2-app-kit = "0.3"
objc2-foundation = "0.3"

# -----------------------------------------------------------------------------------------
# Linux Universal Binary Target
# -----------------------------------------------------------------------------------------
[target.'cfg(target_os = "linux")'.dependencies]
wry = { version = "0.57.0", features = ["os-webview", "x11"] }
tao = { version = "0.37.0", features = ["rwh_06", "x11", "dbus"] }
muda = { version = "=0.19.3", default-features = false, features = ["gtk"] }

gtk = { package = "gtk", version = "0.18", features = ["v3_24"] }
gdk = { package = "gdk", version = "0.18" }
cairo-rs = "0.18"
glib = "0.18"
gtk-layer-shell = { version = "=0.8.2", default-features = false, features = ["v0_6"], optional = true }

[features]
default = []
wayland-layer-shell = ["dep:gtk-layer-shell"]
gnome-companion = []
linux-production = ["wayland-layer-shell", "gnome-companion"]
```

---

## 19. 性能协议：各阶段时延与全局内存预算

稳态内存基准：
- 渲染就绪时延：P95 $\le 250	ext{ ms}$。
- 稳态内存：Windows $\le 55	ext{ MB}$，macOS $\le 60	ext{ MB}$，Linux $\le 75	ext{ MB}$。
- 72h 浸泡测试：前 60 分钟为预热期，稳态期 RSS 增长斜率 $\le 0.05	ext{ MiB/h}$，操作系统净句柄增量严格归零。

---

## 20. 平台认证元组规范 (Platform Certification Tuples)

### 20.1 平台认证生命周期 (Certification Lifecycle)
```
[ Planned ] ──(Phase 0 PoC 通过)──> [ Candidate ] ──(实机硬件门禁全绿)──> [ Certified ]
     │                                     │
     └────────────────(验证失败)───────────┴─────────────────────────────> [ Rejected ]
```

### 20.2 严格一元化平台候选矩阵 (Normalized 1-Tuple-per-Row, P0-70, P0-71, P0-72)
| 元组编号 | 操作系统与版本 | 硬件架构 | 显示服务 | 平台后端实现 | 当前状态 |
|---|---|---|---|---|---|
| **T-01** | Windows 11 24H2 | x86_64 | Desktop Window Manager | `Win32Backend` | **Candidate** |
| **T-02** | Windows 11 23H2 | x86_64 | Desktop Window Manager | `Win32Backend` | **Candidate** |
| **T-03** | Windows 11 24H2 | aarch64 (Snapdragon X) | Desktop Window Manager | `Win32Backend` | **Candidate** |
| **T-04** | Windows 10 22H2 (Build 19045+) | x86_64 | Desktop Window Manager | `Win32Backend` | **Candidate** |
| **T-05** | macOS 26 (Tahoe) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Candidate** |
| **T-06** | macOS 15 (Sequoia) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Candidate** |
| **T-07** | macOS 14 (Sonoma) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Candidate** |
| **T-08** | macOS 13 (Ventura) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Candidate** |
| **T-09** | macOS 15 (Sequoia) | x86_64 (支持机型) | Quartz / WindowServer | `CocoaBackend` | **Candidate** |
| **T-10** | macOS 14 (Sonoma) | x86_64 (支持机型) | Quartz / WindowServer | `CocoaBackend` | **Candidate** |
| **T-11** | macOS 13 (Ventura) | x86_64 (支持机型) | Quartz / WindowServer | `CocoaBackend` | **Candidate** |
| **T-12** | Ubuntu 24.04 LTS (GNOME 46) | x86_64 | Native Wayland | `GnomeCompanionBackend` (esm) | **Candidate** |
| **T-13** | Ubuntu 24.04 LTS (GNOME 46) | aarch64 | Native Wayland | `GnomeCompanionBackend` (esm) | **Candidate** |
| **T-14** | Ubuntu 22.04 LTS (GNOME 42) | x86_64 | X11 | `X11Backend` | **Candidate** |
| **T-15** | Debian 12 (GNOME 43) | x86_64 | Wayland | `GnomeCompanionBackend` (legacy) | **Candidate** |
| **T-16** | Fedora 40 (KDE Plasma 6) | x86_64 | Native Wayland | `LayerShellBackend` (KWin) | **Candidate** |
| **T-17** | Arch Linux (Sway) | x86_64 | Native Wayland | `LayerShellBackend` (wlroots) | **Candidate** |
| **T-18** | Arch Linux (Hyprland) | x86_64 | Native Wayland | `LayerShellBackend` (wlroots) | **Candidate** |
| **T-19** | 统信 UOS 20 SP1 | x86_64 | X11 | `X11Backend` (WebKitGTK 4.1) | **Candidate** |
| **T-20** | 统信 UOS 20 SP1 | aarch64 | X11 | `X11Backend` (WebKitGTK 4.1) | **Candidate** |
| **T-21** | 银河麒麟 Kylin V10 SP1 | x86_64 | X11 | `X11Backend` | **Candidate** |
| **T-22** | 银河麒麟 Kylin V10 SP1 | aarch64 | X11 | `X11Backend` | **Candidate** |
| **T-23** | 深度 Deepin 23 | x86_64 | Treeland / Wayland | `LayerShellBackend` / Candidate | **Candidate** |

### 20.3 机器生成认证文件与基线策略 (desktop-overlay-certification.json)
```json
{
  "$schema": "https://readmd.app/schemas/desktop-overlay-certification.v2.json",
  "spec_version": "1.4.3",
  "tuple_id": "T-01-WIN11-24H2-X64-DWM",
  "status": "candidate",
  "certified_baselines": {
    "webview2_version": "128.0.2739.67",
    "webkitgtk_version": null,
    "gnome_shell_version": null,
    "gtk_version": null,
    "backend_impl_hash": null,
    "renderer_hash": null,
    "golden_contract_hash": null,
    "kernel": "10.0.26100",
    "gpu_driver_family": "DirectX-DWM"
  },
  "gate_report": {
    "sha256": null,
    "passed_gates": [],
    "failed_gates": []
  },
  "invalidation_policy": {
    "on_webview_major_mismatch": "invalidate",
    "on_shell_major_mismatch": "invalidate",
    "on_golden_contract_mismatch": "invalidate",
    "on_backend_impl_mismatch": "require_retest"
  }
}
```

---

## 21. 动态门禁组合模型 (Dynamic Gate Composition Model)

门禁由三层动态组合生成：
$$	ext{Gate Suite} = 	ext{Core Gates} + 	ext{Backend Gates} + 	ext{Tuple Certification Gates}$$

### 1. 核心门禁 (Core Gates - 通用)
- `Gate-Core-01`：源码 SHA 固化
- `Gate-Core-02`：Durable FIFO 字节对齐
- `Gate-Core-03`：时钟回拨防御
- `Gate-Core-04`：快照 Inode 变更感知
- `Gate-Core-05`：IPC ABI 方法签名与 5s 超时拒绝
- `Gate-Core-06`：CSPRNG 会话代际隔离
- `Gate-Core-07`：拖拽抗拉回不变性 (Anti-Snapback)
- `Gate-Core-08`：父进程管道 EOF 250ms 自毁
- `Gate-Core-09`：双重 JSON 序列化注入防御
- `Gate-Core-10`：渲染端无 HTML5 DnD 依赖回归门禁
- `Gate-Core-11`：调和状态机事件序列模糊测试 (VAL-11)
- `Gate-Core-GUIThreadOwnership`：GUI 主线程所有权门禁 (P0-63)

### 2. 后端专项门禁 (Backend Gates)
- `Gate-Win32-01`：Win32 分层透明度与光标追踪
- `Gate-Cocoa-01`：NSWindow Spaces 漫游
- `Gate-X11-01`：EWMH 置顶与 XShape 穿透
- `Gate-LayerShell-01`：Layer-Shell 边距映射
- `Gate-LayerShell-02`：空区域穿透实机按钮点击测试
- `Gate-LayerShell-03`：按需键盘输入法调度
- `Gate-LayerShell-04`：工作区与面板避让对齐 (VAL-24)
- `Gate-GnomeCompanion-01`：UNIX Socket 鉴权与所有权绑定
- `Gate-GnomeCompanion-02`：Mutter 移动、置顶与任务栏隐藏调用
- `Gate-GnomeCompanion-03`：混合 DPI 坐标映射 (VAL-13)
- `Gate-Menu-X11` / `Gate-Menu-KWin-Wayland` / `Gate-Menu-GNOME-Wayland`：各平台右键菜单弹出与失焦测试

### 3. 元组实机认证门禁 (Tuple Certification Gates)
由各目标平台物理实机执行多屏热拔插、休眠唤醒与 72h 稳态浸泡测试。

---

## 22. Fullscreen、Workspace 与虚拟桌面平台化规范

### 22.1 全屏行为严格遵循 Python 快照权威
现有 Electron Golden Reference 的行为极其纯粹：
`Python snapshot fullscreen=true -> overlay.hide()`
- **绝对禁止主动扫描**：Rust 宿主**严禁在后台自主扫描操作系统的全屏游戏或独占窗口**来擅自决定是否隐藏桌宠。Python 端是唯一的业务决策源，平台后端只负责在收到 `fullscreen=true` 时精准执行本地窗口隐匿，收到 `fullscreen=false` 时恢复呈现。

### 22.2 各操作系统置顶与虚拟桌面漫游机制
- **Windows (Win32)**：通过 `WS_EX_TOPMOST` 保持在顶层，Windows 虚拟桌面默认可见。
- **macOS (Cocoa)**：通过 `NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary` 确保桌宠在虚拟桌面切换时平滑随同，但在收到 Python `fullscreen=true` 时必须强制隐藏。
- **Linux X11**：通过 EWMH `_NET_WM_STATE_STAYS_ON_TOP` 与 `_NET_WM_DESKTOP = 0xFFFFFFFF` 保持置顶与全工作区常驻。
- **Linux Wayland (Layer-Shell)**：Layer-Shell `Layer::Overlay` 表面由合成器默认管理在所有虚拟工作区之上。
- **Linux Wayland (GNOME)**：通过 Companion 扩展调用 Mutter 原生方法 `metaWindow.make_above()`、`metaWindow.stick()` 与 `metaWindow.set_skip_taskbar(true)`。

---

## 23. 运行时分发、健康所有权与 A/B 升级 (PetRuntimeInstallerV2)

### 23.1 健康探针文件所有权隔离 (Health Ownership Matrix, P0-85)
- **Electron 生产健康文件**：`<runtime_dir>/<bridge_id>.health.json`
- **Rust 候选健康文件**：`<runtime_dir>/<bridge_id>.rust.health.json`

Python Orchestrator 基于 `EngineHealthDescriptor` 严格匹配当前引擎代际与确切 PID，彻底杜绝状态残留污染。

### 23.2 A/B Runtime 清单协议 (Manifest Schema)
```json
{
  "$schema": "https://readmd.app/schemas/pet-runtime-manifest.v2.json",
  "spec_version": "1.4.3",
  "runtime_version": "0.1.0-alpha.1",
  "renderer_abi": "1.0.0",
  "bridge_protocol": "1.0.0",
  "ipc_protocol": "1.0.0",
  "backend_protocol": "1.0.0",
  "gnome_companion_protocol": "2.0.0",
  "security_epoch": 1,
  "companion_artifacts": [
    {
      "abi_family": "gnome-legacy",
      "shell_major_min": 42,
      "shell_major_max": 44,
      "entrypoint": "companion/legacy/extension.js",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    {
      "abi_family": "gnome-esm",
      "shell_major_min": 45,
      "shell_major_max": null,
      "entrypoint": "companion/esm/extension.js",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    }
  ],
  "signatures": {
    "ed25519_pubkey": "b6a8f12...",
    "manifest_signature": "d981a2..."
  }
}
```

---

## 24. 供应链安全、代码签名与权限边界

### 24.1 安全分层规范 (P0-50)
- **Runtime 完整性**：全平台统一采用 **Ed25519 签名清单 + SHA-256 哈希校验**。
- **操作系统平台信任**：
  - Windows：Authenticode 代码签名
  - macOS：Apple Developer ID 签名 + Notarization 公证 + Stapling
  - Linux：可选的发行版包签名

---

## 25. Release Stop Conditions 严格红线

任何符合以下条件的提交均熔断发布：
1. 核心门禁 (Core Gates)、后端门禁 (Backend Gates) 或元组认证门禁 (Tuple Certification Gates) 中有任何一项未通过或偶现 Flaky；
2. 任何 Tier A 平台发生点击穿透失效、工作区漫游丢失或崩溃；
3. 父进程异常退出后子进程孤儿存活率 $> 0.00\%$；
4. 72 小时稳态内存增长斜率 $> 0.05	ext{ MiB/h}$ 或发生句柄泄漏。

---

## 26. Codex 实施执行协议与交接模式

严格按阶段推进，每个工作包（WP）对应单一阶段，通过对应门禁后方可签署进入下一阶段：
- **WP-01**：Phase 0 (Golden Capture & PoC)
- **WP-02**：Phase 1 (Rust Host Foundation)
- **WP-03**：Phase 2 (Platform Backend Implementations)
- **WP-04**：Phase 3 (Integration & Certification)

---

## 27. 最终验收与重构判断标准

$$egin{aligned}
		ext{Unresolved Core Architectural Decisions} &= 0 \
		ext{Tracked Open Empirical Validation Items} &= 30 \
		ext{Tracked Open Blockers} &= 16 \
		ext{Release-Blocking Gate Failures} &= 0 \
		ext{Certified Tier A Regressions} &= 0 \
		ext{Wayland / GNOME Integration Gaps} &= 0 \
		ext{Orphan Process Rate} &= 0.00\% \
		ext{Persistent Click-Through Deadlocks} &= 0
\end{aligned}$$

---

## 28. Platform Capability Matrix 综合平台能力矩阵

| 能力项 | Windows (Win32) | macOS (Cocoa) | Linux (X11) | Linux (KDE/wlroots) | Linux (GNOME Wayland) |
|---|---|---|---|---|---|
| **后端实现** | `Win32Backend` | `CocoaBackend` | `X11Backend` | `LayerShellBackend` | `GnomeCompanionBackend` |
| **窗口定位** | DIP 原生转换 | DIP 坐标系转换 | DIP 绝对坐标 | Layer-Shell Margins 映射 | Mutter `move_frame` |
| **窗口置顶** | `WS_EX_TOPMOST` | `NSFloatingWindowLevel` | `_NET_WM_STATE_STAYS_ON_TOP` | `Layer::Overlay` | Mutter `make_above` |
| **跨工作区** | 系统默认可见 | `canJoinAllSpaces` | `_NET_WM_DESKTOP = -1` | Layer 默认常驻 | Mutter `stick` |
| **输入穿透** | 原生穿透 + 追踪 | 原生穿透 + 监听 | 原生穿透 + 采样 | Surface Input Region | Surface Input Region |
| **文件拖入** | WRY OLE DnD | WRY Cocoa DnD | WRY XDND | WRY Wayland Data-Device | WRY Wayland Data-Device |
| **右键菜单** | muda Win32 | muda NSMenu | muda GTK3 (无 libxdo) | muda GTK3 (无 libxdo) | muda GTK3 (无 libxdo) |
| **剪贴板** | Win32 Clipboard | NSPasteboard | GtkClipboard | GtkClipboard | GtkClipboard |
| **看门狗** | 管道 + JobObject | 管道 + kqueue | 管道 + PDEATHSIG | 管道 + PDEATHSIG | 管道 + PDEATHSIG |

---

## 29. 架构决策与实机实证项注册表 (Empirical Validation Register)

### 29.1 已闭环设计决策 (Known Design Decisions: Closed)
- **DEC-01**：确立 Platform Backend Family 架构。
- **DEC-02**：确立 Native Wayland 双轨方案（KDE/wlroots 走 Layer-Shell，GNOME 走专用 Companion）。
- **DEC-03**：确立 Linux 单二进制发布架构 (ADR-0002)。
- **DEC-04**：确立 muda 目标特定依赖，彻底剥离 Linux 下的 `libxdo`。
- **DEC-05**：确立主线程 GUI 独占模型，删除后端 Trait 的 `Send + Sync`。
- **DEC-06**：确立权威期望驱动的调和状态模型 (Reconciliation Model)。
- **DEC-07**：确立 GNOME Companion 双产物机制（ESM >=45 vs Legacy <=44）。
- **DEC-08**：确立强类型坐标系统 (`BridgeDipRect`, `OutputLocalDipRect`, `OutputPlacement`)。
- **DEC-09**：确立完整 Secure Asset Protocol 沙盒与平台专属 Origin。
- **DEC-10**：确立健康文件所有权隔离机制。

### 29.2 开放实机实证事项注册表 (Empirical Validation Items: 30 Items Open)
| 编号 | 实证领域 | 核心风险与需物理验证事实 | 阻塞门禁 |
|---|---|---|---|
| **VAL-01** | Layer-Shell + WRY 容器 | WebKitGTK 子表面是否会二次截获透明穿透区域的鼠标事件 | `Gate-LayerShell-02` |
| **VAL-02** | Layer-Shell 跨屏拖拽 | 重设 Monitor 时 Wayland 合成器的 Implicit Pointer Grab 是否中断 | `Gate-LayerShell-01` |
| **VAL-03** | Layer-Shell 按需输入法 | 合成器对 `zwlr_layer_shell_v1` v4+ `KeyboardMode::OnDemand` 的支持率 | `Gate-LayerShell-03` |
| **VAL-04** | GNOME Companion 稳定性 | GNOME 42~47 各小版本中 Mutter `move_frame` 的接口稳定性 | `Gate-GnomeCompanion-02` |
| **VAL-05** | Windows ARM64 渲染基准 | 高通骁龙平台 WebView2 透明窗口层叠与 CPU 占用基准 | `CERT-WIN11-ARM64` |
| **VAL-06** | 国产 Linux 发行版依赖 | 统信 UOS / 麒麟软件源中 `webkit2gtk-4.1` 的预装与动态链接一致性 | `CERT-UOS20-X64` |
| **VAL-07** | Linux 通用单二进制 PoC | 单一二进制在 X11 与 Wayland 环境下的 GDK 动态加载与行为一致性 | `Gate-Linux-Universal` |
| **VAL-08** | muda GTK 菜单在 KWin Wayland | KWin 环境下右键弹出菜单的准确定位、失焦关闭与层级行为 | `Gate-Menu-KWin-Wayland` |
| **VAL-09** | muda GTK 菜单在 GNOME Wayland | Mutter 环境下右键弹出菜单的输入捕获释放与穿透恢复 | `Gate-Menu-GNOME-Wayland` |
| **VAL-10** | WRY custom DnD 副作用 | Windows 上启用 `with_drag_drop_handler` 时渲染端 DOM 与文件拖放回归 | `Gate-Core-10` |
| **VAL-11** | 调和状态机事件序列 Fuzz | `proptest` 50,000 条极端并发事件序列下的状态不变性与防死锁 | `Gate-Core-11` |
| **VAL-12** | LayerShell 全局转局部坐标映射 | 多屏排布下 `VirtualDesktopMapper` 的边距计算与跨屏对齐 | `Gate-LayerShell-04` |
| **VAL-13** | GNOME move_frame 混合 DPI 映射 | 混合缩放比例下 `GnomeCoordinateMapper` 与 Mutter Stage 坐标一致性 | `Gate-GnomeCompanion-03` |
| **VAL-14** | GNOME Legacy (<=44) Companion | Ubuntu 22.04 / Debian 12 下 GJS imports 扩展加载与通信 | `CERT-UBUNTU22-X64` |
| **VAL-15** | GNOME ESM (>=45) Companion | Ubuntu 24.04 / Fedora 39+ 下 ESM 扩展加载、热重载与会话恢复 | `CERT-UBUNTU24-X64` |
| **VAL-16** | Windows HTTPS 资产 Scheme | Windows 10/11 下 `with_https_scheme(true)` 与最低运行时能力探针 | `Gate-Asset-HTTPS-Win` |
| **VAL-17** | Custom Protocol 路径沙盒 | 跨平台路径穿越、符号链接逃逸、NUL 注入实测防御拦截率 100% | `Gate-Asset-Sandbox` |
| **VAL-18** | 统信 UOS 运行时 WebKitGTK 验证 | UOS 20 SP1 物理机环境下动态链接库真实加载实证 | `CERT-UOS20-ARM64` |
| **VAL-19** | 银河麒麟运行时 WebKitGTK 验证 | Kylin V10 SP1 物理机环境下动态链接库真实加载实证 | `CERT-KYLIN10-ARM64` |
| **VAL-20** | macOS 13+ 支持周期认证 | macOS 13 (Ventura) ~ 15 (Sequoia) 在 Apple Silicon 与 Intel 上的认证 | `CERT-MACOS-ALL` |
| **VAL-21** | Layer-Shell Pre-Realize 生命周期 | GTK 窗口在未 realize 前绑定 Layer-Shell 且成功挂载 WRY 的实测路径 | `Gate-LayerShell-05` |
| **VAL-22** | GNOME Meta.Window AppID 传播 | WRY/GTK 顶层窗口在 Mutter 内部 `get_gtk_application_id()` 真实返回值验证 | `Gate-GnomeCompanion-04` |
| **VAL-23** | GNOME Companion 畸形 IPC Fuzz | 极端非法 JSON 与畸形指令下 GNOME Shell 进程零崩溃证明 | `Gate-GnomeCompanion-05` |
| **VAL-24** | Wayland 工作区与面板避让对齐 | Layer-Shell margins 映射与 KDE/wlroots 边缘独占区域对齐实测 | `Gate-LayerShell-06` |
| **VAL-25** | 导航代际隔离异步消息丢弃测试 | 构造跨导航延迟 IPC 消息，验证 Rust 宿主 100% 拒绝陈旧调用 | `Gate-IPC-Generation` |
| **VAL-26** | macOS GUI 主线程所有权真机验证 | 验证后台 Tokio 线程绝不触碰 Cocoa 句柄且无主线程断言 panic | `Gate-Cocoa-Thread` |
| **VAL-27** | Linux 发布二进制 Feature 检查 | 自动化检查 release 产物确保已开启 `linux-production` 特性链 | `Gate-Linux-Binary` |
| **VAL-28** | muda 零 libxdo 依赖实测证明 | 通过 readelf/ldd 证明 Linux 二进制完全不依赖 `libxdo.so` | `Gate-Menu-Dependency` |
| **VAL-29** | 当前 GNOME 50 扩展兼容性实测 | 验证 GNOME 50 环境下 ESM 扩展加载与 Mutter 接口调用平滑 | `CERT-GNOME50-X64` |
| **VAL-30** | macOS 26 (Tahoe) 桌宠实机认证 | 验证 macOS 26 开发者/正式版下透明渲染、点击穿透与 Spaces 随同 | `CERT-MACOS26-ARM64` |

### 29.3 开放架构阻塞项注册表 (Open Architecture Blockers: 16 Items Open)
| 编号 | 阻塞领域与项 | 核心架构风险与解除条件 | 关联门禁 / 验证项 | 当前状态 |
|---|---|---|---|---|
| **BLOCKER-01** | Wayland WebKitGTK 透明子表面事件穿透 | WebKitGTK 子表面在 Wayland 下可能二次拦截透明区域鼠标事件；需验证 cairo input region 穿透 | Gate-LayerShell-02 / VAL-01 | **OPEN** |
| **BLOCKER-02** | Layer-Shell 分数缩放与 Hit Region 不连续 | 合成器在分数缩放（如 125%、150%）下对子表面 input shape 的舍入截断；需实机测试无死区 | Gate-LayerShell-01 / VAL-02 | **OPEN** |
| **BLOCKER-03** | GNOME 45+ Mutter 跨小版本内部接口稳定性 | Mutter 内部 move_frame、get_gtk_application_id 在 45~50 跨版本 API 变动；需 ESM 扩展隔离 | Gate-GnomeCompanion-02 / VAL-04 | **OPEN** |
| **BLOCKER-04** | Linux 多 GDK 会话与多 Seat 隔离 | 相同用户或多显示服务器下单二进制 GDK 静态上下文污染；需验证独立 DISPLAY/WAYLAND_DISPLAY 隔离 | Gate-Linux-GdkMultiSeat / VAL-07 | **OPEN** |
| **BLOCKER-05** | Windows on ARM64 渲染基准与透明层叠 | 高通骁龙平台 WebView2 在 DWM 透明窗口下的 GPU 加速层叠与 CPU 占用基准；需实机达标 | CERT-WIN11-ARM64 / VAL-05 | **OPEN** |
| **BLOCKER-06** | 权威规范产物完整性与 SHA256 强校验 | 规范交付链截断、TOC 锚点悬空或哈希不匹配；需通过 Spec Linter 完整性自动化校验 | Gate-Core-SpecIntegrity / VAL-27 | **OPEN** |
| **BLOCKER-07** | Cargo muda 特性联合污染隔离 | 通用依赖引入 muda 导致 Linux 错误激活默认 libxdo 特性；必须按 target 隔离依赖 | Gate-Linux-MudaTargetIsolation / VAL-28 | **OPEN** |
| **BLOCKER-08** | gtk-layer-shell v0_6 特性链与 Linux 发布锁定 | 调用 protocol_version() 必须显式激活 0_6；发布配置必须激活 linux-production | Gate-Linux-LayerShellV06 / VAL-27 | **OPEN** |
| **BLOCKER-09** | Layer-Shell 窗口 pre-realize 初始化生命周期 | GTK 窗口在 
ealize 之后调用 layer-shell 初始化会触发底层断言 panic；必须在 show/realize 前绑定 | VAL-21 / Gate-LayerShell-01 | **OPEN** |
| **BLOCKER-10** | GNOME AppID 传播与 Meta.Window 发现 | WRY/GTK 窗口创建时必须传播 sia.readmd.pet，使 Mutter get_gtk_application_id() 稳定识别 | VAL-22 / Gate-GnomeCompanion-01 | **OPEN** |
| **BLOCKER-11** | 渲染端导航代际与 WebView 实例会话严格解耦 | 跨导航异步回调必须持有独立 generation token；防止页面重载后陈旧 IPC 调用执行 | VAL-25 / Gate-IPC-01 | **OPEN** |
| **BLOCKER-12** | 调和状态机 AppliedState 异步生效竞态与陈旧防护 | 异步更新生效前必须记录 applied_generation，忽略晚到的过时 snapshot | VAL-26 / Gate-Core-Reconciliation | **OPEN** |
| **BLOCKER-13** | Wayland 工作区与 exclusive-zone 面板避让一致性 | Layer-Shell margins 必须与各合成器 panels / dock 边距动态对齐，防止桌宠遮挡面板 | VAL-24 / Gate-LayerShell-04 | **OPEN** |
| **BLOCKER-14** | GNOME 50 当前版本扩展兼容性实机认证 | GNOME 50 新架构规范与扩展接口兼容性验证，确保扩展零崩溃 | CERT-GNOME50-X64 / VAL-29 | **OPEN** |
| **BLOCKER-15** | macOS 26 (Tahoe) 架构与硬件分离认证 | macOS 26 上 Spaces 随同、透明点击穿透与 ARM64/Intel 硬件分离实机认证 | CERT-MACOS26-ARM64 / VAL-30 | **OPEN** |
| **BLOCKER-16** | Secure Asset Protocol 产物截断防护与沙盒 TOCTOU 拦截 | 静态资产协议支持严格路径沙盒、零 NUL 注入、双重扩展名拦截与符号链接 TOCTOU 防御 | Gate-Core-AssetSecurity / VAL-17 | **OPEN** |

---

## 30. Real Hardware Certification Matrix 实机硬件认证矩阵

必须依托实体机器完成门禁测试：
1. **Windows 11 on ARM (高通骁龙 X Elite / Surface Pro 11)**：aarch64 原生透明渲染与 WebView2 性能基准。
2. **macOS Apple Silicon (M1/M2/M3/M4)**：Spaces 跨虚拟桌面漫游与暗色模式平滑切换。
3. **KDE Plasma 6 on Wayland (AMD/Intel GPU)**：`gtk-layer-shell` 边距吸附与 120Hz 刷新同步。
4. **GNOME 46 / 50 on Wayland (Ubuntu 24.04 / Fedora 40+)**：Companion 扩展安装与 Mutter 移动控制。
5. **统信 UOS 20 SP1 & 银河麒麟 V10 SP1**：国产 Linux 桌面环境下的点击穿透与剪贴板互操作。

---

## 31. Change Log (v1.3.0 -> v1.4.0 演进)

- **[P0] 架构解耦**：单一 Tao 窗口重构为涵盖 5 大后端的 **`Platform Backend Family`**。
- **[P0] 原生 Wayland 裁决**：KDE/wlroots 采用 `LayerShellBackend`，GNOME 采用 `ReadMD GNOME Shell Companion`。
- **[P0] 输入模型穿透**：Native Wayland 废除全局光标轮询，改用标准 **Surface Input Region**。
- **[P0] 工具链对齐**：统一锁定至 **Rust 1.85.0**，对齐 `wry 0.57.0`、`tao 0.37.0`、`muda 0.19.3`、`gtk-layer-shell 0.8.2`。
- **[P0] 协议安全加固**：CSPRNG 代际隔离，双重 JSON 序列化防御 XSS，FIFO 序号自增。
- **[P1] 门禁体系演进**：向动态门禁模型演进。

---

## 32. Change Log (v1.4.0 -> v1.4.1 Freeze Candidate 演进)

- **[P0-01] Cargo Feature Matrix 真实性重构**：消除盲目的 `default-features = false`，按 Target 显式配置。
- **[P0-02] 严格主线程 GUI 边界**：从 `OverlayWindowBackend` 彻底移除 `Send + Sync`。
- **[P0-03 & P0-04] gtk-layer-shell 依赖策略与 ADR**：标记为 `Candidate pending Phase-0 PoC`，示例代码标注为 `PSEUDOCODE`。
- **[P0-05 & P0-06] Layer-Shell IME 与跨屏拖拽实证**：基准模式设定为 `KeyboardMode::None`，输入法按需模式受限于协议版本探测。
- **[P0-07] Wayland 输入穿透实体验证**：建立实体测试床，以底层目标原生窗口按钮实际接收到物理点击为穿透成功准则。
- **[P0-08 ~ P0-11] GNOME Companion 安全加固与 API 纠正**：UNIX Domain Socket 路径迁移至 `$XDG_RUNTIME_DIR`，权限 `0600`，废除虚构任务栏隐藏 API，改用 `hide_from_window_list()`。
- **[P0-12 & P0-23] 认证生命周期与机器生成 JSON Schema**：建立严格四阶段生命周期。
- **[P0-13] macOS 单实例文件锁标准化**：规范化至 Application Support 路径。
- **[P0-14 ~ P0-18] 平台健壮性、拖拽与性能基准纠偏**：Rust 端不持有父进程管道写端，全屏严格遵循 Python 快照。
- **[P1-19 & P0-20 ~ P0-22] 浸泡斜率、门禁组合与 BackendCapabilities**：72h 浸泡模型科学化。

---

## 33. Change Log (v1.4.1 -> v1.4.2 Freeze Candidate 演进)

- **[P0-24] Cargo.toml 全文配置统一**：纠正遗留的 `default-features = false` 错误，确立权威依赖配置。
- **[P0-25] Linux 单二进制发布架构 (ADR-0002)**：确立 ReadMD Linux 采用单一生产二进制跨 X11 与 Wayland 运行。
- **[P0-26] muda 依赖净化**：配置 `default-features = false, features = ["gtk"]`，彻底剥离 X11 的 `libxdo`。
- **[P0-27] WRY 0.57 API 与 DnD 枚举复归**：纠正零参数调用，更新 `DragDropEvent` 为 `Enter/Over/Drop/Leave`。
- **[P0-28] Windows custom DnD 副作用门禁与 Golden Inventory**：实测确认当前渲染代码不含 HTML5 `draggable="true"`。
- **[P0-29] 调和状态模型 (Reconciliation State Model)**：废除脆弱的 14 状态枚举机，确立调和函数与优先级。
- **[P0-30] Golden `visible=false` 强中断行为**：拖拽中收到隐藏通知立即强行中止拖曳。
- **[P0-31] 全屏扫描语义全面纠正**：删除所有平台主动扫描全屏应用的描述。
- **[P0-32] GNOME API 符号净化**：统一为 Mutter 真实接口 `hide_from_window_list()`。
- **[P0-33] GNOME Companion 双产物打包 (ESM >=45 vs Legacy <=44)**：拆分双产物目录。
- **[P0-34] 完整安全资产协议 (Secure Asset Protocol) 恢复**：完整规定平台专属 Origin 与路径沙盒。
- **[P0-35] 初始化脚本双保险隔离**：开启配置 + IIFE 守卫防护。
- **[P0-36] `renderer_session` 定位澄清**：明确其为代际隔离令牌而非鉴权密钥。
- **[P0-37] 交互区域复杂度预算与 Golden 语义优先**：确立 Golden 行为优先于几何近似。
- **[P0-38 & P0-39] 强类型坐标系统与 GNOME/LayerShell 坐标映射**：定义强类型坐标。
- **[P0-40] muda 上下文菜单 Wayland 专项门禁**：增加各平台独立菜单门禁。
- **[P0-41] WRY `linux-body` 特性裁决**：明确关闭 `linux-body`。
- **[P0-42] UOS / Kylin 事实语调修正**：修正为“待实机实证”。
- **[P0-43] macOS 支持基线对齐**：对齐 ReadMD 官方口径 **macOS 13+**。
- **[P0-44] 平台认证元组一元化**：严格一元化单行定义。
- **[P0-45] 认证基线与失效策略重构**：记录物理基线与失效判定策略。
- **[P0-46] 健康文件所有权隔离**：Rust 专用健康文件路径。
- **[P0-47] 运行时 Manifest Schema 升级**：解耦版本字段。
- **[P0-48] 动态门禁组合模型落地**：彻底剔除固定门禁数量宣称。
- **[P0-49] 物理目录版本解耦**：统一命名为 `docs/architecture/pet-rust/`。
- **[P0-50] 供应链安全分层**：Runtime 完整性与 OS 平台信任解耦。
- **[P0-51] 全平台 100% 重新定义**：Target Capability = Golden Equivalent。

---

## 34. Change Log (v1.4.2 -> v1.4.3 Architecture Freeze Candidate 演进)

- **[P0-52] 确立仓库唯一权威 Canonical 路径**：规格书唯一真相源正式移至 ReadMD 仓库内部 `docs/architecture/pet-rust/spec.md`，彻底脱离临时 scratch/brain 目录。
- **[P0-53] 规格一致性校验器全面重构**：建立全量结构检查（34 节完整性、无截断、代码栅栏闭合）、语义矛盾断言、TOC 链接双向校验与 NUL 字节防御。
- **[P0-54] 校验器负向自测套件落地**：建立 `tests/spec_linter/`，自动生成 10 个缺陷 fixture 验证校验器拦截率 100%。
- **[P0-55] muda 跨目标依赖隔离**：彻底从公共 `[dependencies]` 移除 muda，按 Windows、macOS 与 Linux Target 独立声明，阻断 Cargo Feature Union 污染。
- **[P0-56] macOS objc2 生态版本统一**：全面升级至与 muda 一致的 `objc2 0.6`、`objc2-app-kit 0.3`、`objc2-foundation 0.3`。
- **[P0-57] gtk-layer-shell API Feature 声明**：显式配置 `features = ["v0_6"]`，满足 `is_supported` 与 `protocol_version` 编译要求。
- **[P0-58] 统一 Linux 生产 Feature Profile**：新增 `linux-production = ["wayland-layer-shell", "gnome-companion"]`。
- **[P0-59] Layer-Shell 预 Realize 窗口生命周期 (VAL-21)**：确立 Route A，由 LayerShellBackend 原生持有 GtkWindow 并在 realize 前完成 Layer-Shell 初始化。
- **[P0-60] 能力探针与 Roundtrip 性能防护**：Layer-Shell 能力探测仅限 `Probing` 阶段单次执行并固化为不可变结构体。
- **[P0-61] Linux 纯 Wayland 严格 Fail-Closed**：Wayland 后端失效时，仅在 XOpenDisplay 成功时允许 X11 降级，否则直接触发不支持状态。
- **[P0-62] Linux 后端探测真实代码化**：给出基于 GTK/GDK 标准调用的可编译实现路径。
- **[P0-63] 严格禁止 `#[tokio::main]` 接管 GUI 线程**：操作系统主线程独占运行事件循环，Tokio 局限于后台 worker。
- **[P0-64] 调度器精准进程追踪**：彻底废除 `kill_processes_by_target` 扫描杀进程，基于 exact PID 与 JobObject/pidfd 进行生命周期控制。
- **[P0-65 & P0-66] GNOME AppID 传播与窗口发现机制 (VAL-22)**：定义 `PET_OVERLAY_APP_ID = "asia.readmd.pet"`，基于 peer PID 进行单例窗口发现。
- **[P0-67 & P0-68] GNOME Companion 安装引导与崩溃隔离 (VAL-23)**：未索引时提示注销重登；扩展内部全面异步化、限流并包裹 `try/catch`。
- **[P0-69 & P0-70] GNOME 50 与 macOS 26 支持前瞻**：矩阵更新至 GNOME 50 与 macOS 26 (Tahoe)，按 Apple 官方兼容机型细分架构。
- **[P0-71 & P0-72] 认证元组真正一元化与 Win10 基准锁定**：T-01 至 T-23 彻底单行一元化；Win10 锁定最低 Build 19045。
- **[P0-73] 初始化脚本 SyntaxError 语法修复**：使用 IIFE 包裹主 frame 守卫，杜绝顶层 return 语法错误。
- **[P0-74] 会话代际与导航代际解耦 (VAL-25)**：分离 `webview_session_id` 与 `navigation_generation`，实现陈旧异步消息精准丢弃。
- **[P0-75 & P0-76] 调和模型补全 Renderer 期望与 AppliedState**：引入 `AppliedOverlayState` 与异步令牌，杜绝重复执行与竞态。
- **[P0-77] 彻底删除 `<=64` 几何有损近似**：Golden 语义第一，超出复杂度预算直接阻断认证。
- **[P0-78 & P0-79] 持久化显示器指纹与工作区面板避让 (VAL-24)**：EDID 硬件指纹 + Layer-Shell margins 工作区避让。
- **[P0-85 & P0-86] 双健康文件路径与安全资产协议全文固化**：彻底消除 NUL 字符截断缺陷，完整固化全部 34 个章节。
- **[P0-87 ~ P0-90] 沙盒 TOCTOU 防御与措辞自律**：Staged 只读导入；严格判定为 **`NO — Architecture Freeze Candidate`**。
- **[验证项扩充] 实机实证项扩充至 30 项**：新增 VAL-21 至 VAL-30；明确 16 项核心阻塞项（BLOCKER-01 ~ BLOCKER-16）。

---

## 附录 A：已验证关键系统底层事实

1. `wry 0.57.0` 在 Linux 下深度绑定 `webkit2gtk-4.1`。
2. `gtk-layer-shell 0.8.2` 封装了 `zwlr_layer_shell_v1` 客户端接口，必须在 GtkWindow realize 之前初始化。
3. GNOME Shell 与 Mutter 具备内部 `Meta.Window` 接口，支持通过扩展脚本调用 `move_frame(true, x, y)`、`make_above()`、`stick()` 与 `hide_from_window_list()`。
4. Electron 的 `screen` 体系在 Windows/macOS/Linux 下的 DIP 逻辑像素标准一致。
5. Python `HermesPetBridge` 指令读取使用天然字典序，带纳秒级时间戳。

---

## 附录 B：Phase 0 实施执行清单与 ADR-0001 草案

### 执行清单
```bash
# 1. 架构文档标准目录
mkdir -p docs/architecture/pet-rust/
mkdir -p docs/architecture/pet-rust/companion/legacy/
mkdir -p docs/architecture/pet-rust/companion/esm/

# 2. 生成基准契约哈希
python tools/verify_golden_contract.py --generate

# 3. 初始化最小 Cargo 项目 (Rust 1.85.0)
cargo new packages/readmd-pet-rust --bin
cd packages/readmd-pet-rust
echo 'channel = "1.85.0"' > rust-toolchain.toml

# 4. 验证依赖与生产特性编译
cargo check --locked --features linux-production
```

### ADR-0001-runtime-stack.md 草案摘要
- **决定**：选用 Rust 1.85.0 + WRY 0.57.0 + Tao 0.37.0 + muda 0.19.3 + gtk-layer-shell 0.8.2 作为 Desktop Overlay 运行时技术栈。
- **状态**：**Candidate (Pending Phase 0 PoC)**。
- **影响**：通过 `Platform Backend Family` 实现与现有 Electron 100% 行为等价，未认证平台安全降级，全平台 0 孤儿进程。
