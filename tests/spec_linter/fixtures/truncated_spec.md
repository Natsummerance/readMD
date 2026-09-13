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
- **任务栏隐藏真实 API**：彻底清除虚构的 `set_skip_taskbar`，统一调用 `metaWindow.hide_from_window_list()`。
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