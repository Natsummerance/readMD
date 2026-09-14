# ReadMD Desktop Overlay v1.4.5 Semantic Closure Candidate

> **版本**：v1.4.5-Candidate
> **状态说明**：Semantic Closure Candidate / Architecture Design Candidate ready for Phase 0
> **架构冻结裁决**：**NO — Production Architecture Freeze**（严禁进入正式生产冻结）
> **Phase 1 Rust Host 生产实现状态**：**严格禁止**（必须在 Phase 0 物理 PoC 实机验证完成且硬件门禁全部通过后方可解锁）
> **前序版本审计依据**：彻底修复 v1.4.4 中所有语义冲突、状态机遗漏、非原子元组及虚构行为。

---

## 目录
- [0. 核心架构约束与设计哲学](#0-核心架构约束与设计哲学)
- [1. Golden Contract 权威行为源与基准契约](#1-golden-contract-权威行为源与基准契约)
- [2. 桌面级 Shell 保真度契约](#2-桌面级-shell-保真度契约)
- [3. 跨平台后端抽象层：PlatformBackend trait](#3-跨平台后端抽象层platformbackend-trait)
- [4. Native Wayland Layer-Shell 后端 (KDE / wlroots)](#4-native-wayland-layer-shell-后端-kde--wlroots)
- [5. GNOME Wayland 专属扩展：ReadMD GNOME Shell Companion](#5-gnome-wayland-专属扩展readmd-gnome-shell-companion)
- [6. Wayland 交互区域与坐标强类型模型](#6-wayland-交互区域与坐标强类型模型)
- [7. Linux 平台桌面集成基准](#7-linux-平台桌面集成基准)
- [8. ReadMD Pet IPC v1：会话隔离与 WRY 官方 API 绑定](#8-readmd-pet-ipc-v1会话隔离与-wry-官方-api-绑定)
- [9. 安全资产加载协议与沙箱边界](#9-安全资产加载协议与沙箱边界)
- [10. 权威 Durable FIFO 与 SnapshotReader 规范](#10-权威-durable-fifo-与-snapshotreader-规范)
- [11. 原生文件拖拽：WRY 0.57 官方路径规范](#11-原生文件拖拽wry-057-官方路径规范)
- [12. 内容安全策略 (CSP) 与资源隔离](#12-内容安全策略-csp-与资源隔离)
- [13. 异常退出与崩溃自愈状态机](#13-异常退出与崩溃自愈状态机)
- [14. 实例互斥作用域与多用户隔离](#14-实例互斥作用域与多用户隔离)
- [15. 宿主生命周期管理：Parent Liveness 立即退出与优雅替换](#15-宿主生命周期管理parent-liveness-立即退出与优雅替换)
- [16. 权威状态对齐模型 (Reconciliation State Model)](#16-权威状态对齐模型-reconciliation-state-model)
- [17. 健康度监控所有权与原子写入规范](#17-健康度监控所有权与原子写入规范)
- [18. 权威构建与依赖规范 (Cargo Specification)](#18-权威构建与依赖规范-cargo-specification)
- [19. 临时测量参考指标与性能基线](#19-临时测量参考指标与性能基线)
- [20. 平台认证元组规范 (Platform Certification Tuples)](#20-平台认证元组规范-platform-certification-tuples)
- [21. 自动化质量门禁体系 (Quality Assurance Gates)](#21-自动化质量门禁体系-quality-assurance-gates)
- [22. 全屏感知与桌面环境业务规则](#22-全屏感知与桌面环境业务规则)
- [23. 运行时分发与离线验签 (PetRuntimeInstallerV2)](#23-运行时分发与离线验签-petruntimeinstallerv2)
- [24. Phase 0 物理概念验证计划 (PoC Spikes)](#24-phase-0-物理概念验证计划-poc-spikes)
- [25. 分级发布熔断条件 (Scoped Release Stop Conditions)](#25-分级发布熔断条件-scoped-release-stop-conditions)
- [26. Golden 差异对比测试与录制回放](#26-golden-差异对比测试与录制回放)
- [27. 验收标准与准出公式](#27-验收标准与准出公式)
- [28. 跨平台能力支撑矩阵](#28-跨平台能力支撑矩阵)
- [29. 闭环决策树、验证注册表与阻塞台账](#29-闭环决策树验证注册表与阻塞台账)
- [30. 规范审计历史记录](#30-规范审计历史记录)
- [31. 术语与概念定义表](#31-术语与概念定义表)
- [32. 跨平台架构差异快速索引](#32-跨平台架构差异快速索引)
- [33. 常见陷阱与反模式排查手册](#33-常见陷阱与反模式排查手册)
- [34. Phase 0 PoC 执行代码模板与命令指引](#34-phase-0-poc-执行代码模板与命令指引)
- [35. 规范签署与一致性哈希锁定](#35-规范签署与一致性哈希锁定)

---

## 0. 核心架构约束与设计哲学

### 0.1 依赖、MSRV 与技术栈约束 (P0-55, P0-56, P0-57, P0-58)
- **Rust MSRV**：`1.85.0`
- **核心窗口与渲染库**：
  - `tao = 0.37.0`（启用 `rwh_06`）
  - `wry = 0.57.0`（必须启用 `os-webview` 特性）
- **Linux 目标隔离原则**：
  - `gtk-layer-shell` 锁定为 `v0_6` 特性；
  - `muda` 仅在 Windows 与 macOS 引入，Linux 生产构建通过 target-specific cfg 严格剔除，彻底杜绝 `libxdo.so` 污染；
  - 生产构建配置专用 `linux-production` profile。

---

## 1. Golden Contract 权威行为源与基准契约

### 1.1 核心行为源与 Git 提交哈希锁定 (P0-91, P0-103, P0-158)
本项目严格以 Git Commit `4dcfd73ce81a14ace7e429791e0594bea47b24e5` 下的真实实现为黄金基准：
- **Golden Git Commit SHA**：`4dcfd73ce81a14ace7e429791e0594bea47b24e5`
- **机器可读契约源**：`docs/architecture/pet-rust/golden-contract.json`

<!-- GENERATED: golden-source-table -->
| 序号 | 行为源文件路径 | 承担的核心合约职责 | 文件大小 | 精确 SHA-256 哈希 |
|---|---|---|---|---|
| **1** | `packages/readmd-hermes-pet-adapter/src/electron-main.ts` | 主窗口生命周期、右键菜单模型、托盘与剪贴板捕获 | 14,929 B | `0a1b6473d155f8121d77d1463316a7968b0d973f76bb6080f4abb58de65a269d` |
| **2** | `packages/readmd-hermes-pet-adapter/src/preload.ts` | Preload ABI 上下文暴露 (window.__HERMES_PET__) | 1,883 B | `fafeb3c1e5241efe3c25646f4ec1cb818ca46a17e375f85e3e16710963df1179` |
| **3** | `packages/readmd-hermes-pet-adapter/src/bridge-transport.ts` | Durable FIFO 队列与 SnapshotReader 严格原子读取器 | 2,276 B | `7055deed1d644687fe8fc1a3adff39fba85185644903e334be6b502397100723` |
| **4** | `packages/readmd-hermes-pet-adapter/src/renderer.tsx` | 前端 React 挂载、状态驱动、错误边界与控制事件分发 | 2,002 B | `5bbbd06c222c572d75b68b10bb09e910a5e02e6f1e89475a9811d0934dccaa36` |
| **5** | `packages/readmd-hermes-pet-adapter/src/live2d/stage.ts` | Live2D 舞台命中判定 (hitTest || bounds.contains) | 19,525 B | `bec994ed0a299fd7f05156f54cef6fa06da750f96f6f931a547313bd3e64522a` |
| **6** | `third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc.ts` | 上游宠物 IPC 管道与窗口穿透控制 | 5,824 B | `5c99fce416fece34d0fb66fdb662af0fb0169b9c4e8aae71977f9a46ac171d8d` |
| **7** | `src/readmd_modules/pet/hermes_adapter.py` | Python 宿主控制逻辑、生命周期与剪贴板 FIFO 响应 | 33,603 B | `2a2f09188d3f9f6f52ac9a0a0571d3a94eaf2e385949184ef24058f3ec5b03ee` |
| **8** (证据) | `packages/readmd-hermes-pet-adapter/package.json` | 依赖版本与包元数据 | 709 B | `ee63a91062219ea13672d4440246745f5eb821573a3007b30a7385e857780600` |
| **9** (证据) | `packages/readmd-hermes-pet-adapter/src/pet-life.ts` | 宠物伴侣状态机与角色属性定义 | 12,123 B | `21ef9bf62592d4d00a3b99cd3fd6f50944245fb26ac1f51cd3f3ad2db20d1dd5` |

### 1.2 权威 Preload ABI 规范 (P0-92, P0-146)
逐字对照 `preload.ts`，锁定宿主与渲染层通信标准：
```typescript
export interface ReadMDPetPreloadABI {
  open(bounds: Bounds, renderer?: string): Promise<boolean>;
  close(): Promise<boolean>;
  setBounds(bounds: Bounds): void;
  setIgnoreMouse(ignore: boolean, options?: { forward?: boolean }): void;
  setFocusable(focusable: boolean): void;
  pushState(payload: unknown): void; // 渲染层向宿主上报状态 (renderer -> host)
  control(payload: { action?: string; type?: string; [key: string]: unknown }): void;
  onState(listener: (payload: any) => void): () => void;
  onControl(listener: (payload: any) => void): () => void;
  dropFiles(files: string[]): void;
}
```
**命名消歧 (P0-146)**：
- 前端暴露的 `petOverlay.pushState(payload)` 专用于渲染层向宿主提交增量；
- 宿主内部向前端同步全局状态的助手函数明确命名为 `host_publish_state()`（内部发送 `hermes:pet-overlay:state` 事件）。

### 1.3 窗口尺寸策略解耦与吸附逻辑清除 (P0-93, P0-94)
- 彻底剔除臆造的“12-DIP 贴边吸附”和“双轴强制收缩”设定，贴边吸附阈值永久锁定为 0。
- **HostSnapshotBoundsPolicy**（宿主物理安全窗口）：
  - 最小宽度 240 DIP，最小高度 300 DIP；最大宽度 640 DIP，最大高度 720 DIP；
  - 必须与工作区至少重叠 40 DIP；允许进入屏幕外负边距（上/左 -12 DIP，下/右 -24 DIP）。
- **RendererInteractiveBoundsPolicy**（前端交互缩放）：
  - 最小尺寸限制为 80x80 DIP；仅在用户开启交互缩放时允许临时越界。

### 1.4 Golden Hit Region 真实判定模型 (P0-95, P0-112, P0-160)
严格还原 Live2D 与 Sprite 渲染引擎原生穿透判定逻辑：
1. **Live2D 判定合约 (`live2d/stage.ts` L128-L129)**：
   ```typescript
   const areas = typeof model.hitTest === 'function' ? model.hitTest(x, y) : [];
   return (Array.isArray(areas) && areas.length > 0) || Boolean(model.getBounds?.().contains(x, y));
   ```
   **物理事实**：只要 `model.hitTest(x, y)` 命中了有效部件，或者坐标落在模型包围盒 `model.getBounds()` 内，即视为命中宠物本体（禁止私自改成仅包含 HitArea 的方案）。
2. **Sprite 判定合约 (`third_party/.../pet-overlay-app.tsx` L132-L165)**：
   - 使用 `document.elementFromPoint(x, y)` 检测拾取目标；
   - 若拾取目标不在宠物根容器 `petRef` 内，判定为透明穿透区域（返回 `false`）；
   - 若拾取目标为非 Canvas 交互 DOM 元素（如对话气泡 `PetBubble`、未读邮件图标 `Mail`、弹出式输入框 `composer`），直接信任 DOM 命中测试（返回 `true`）；
   - 若拾取目标为 `HTMLCanvasElement`，则获取 2D 上下文并在对应纹理坐标处进行像素采样：
     $$\text{Solid Pixel} \iff \text{ctx.getImageData}(px, py, 1, 1).\text{data}[3] \ge 16 \quad (\text{ALPHA\_HIT\_THRESHOLD} = 16)$$
   - 若 Canvas 受到污染（Tainted）或读取抛出异常，执行安全打开策略（fail-open，返回 `true`），确保桌宠依然可被鼠标抓取。

### 1.5 ReadMD 适配器 `toggle-app` 语义 (P0-96)
恢复 `hermes_adapter.py` 第 309-322 行逻辑：
- `toggle-app` 绝不操作操作系统的窗口最小化或显示；
- 它的物理职责是：读取系统剪贴板（文本、PNG 图片 Base64、文件路径），并封装为 `{"type": "clipboard", "text": ..., "image_png": ..., "paths": ...}` 写入命令 FIFO，供宠物进行语义消费。

### 1.6 右键菜单 Golden 契约 (P0-97, P0-147, P0-148)
恢复 `electron-main.ts` 第 205-224 行菜单树结构：
1. 状态头（禁用态）：`Level {level} · Energy {energy} · Mood {mood}`；
2. 分隔线；
3. **4 项互动动作**：
   - `Pet`（抚摸）；
   - `Feed`（喂食）；
   - `Play`（娱乐，仅在 `(energy ?? 80) >= 10` 时可点，否则禁用）；
   - `Rest` / `Wake up`：
     - **冷却键规则 (P0-148)**：当 `life.resting` 为 `true` 时，动作为 `'wake'`，检查 `cooldowns['wake']`；当 `life.resting` 为 `false` 时，动作为 `'rest'`，检查 `cooldowns['rest']`；
4. 分隔线；
5. `Characters` 子菜单：展示角色单选列表，最多 128 项；
6. `Open reader`：打开主阅读器（写入 `{"type": "open-menu"}`）。

### 1.7 剪贴板 Golden 边界阀值 (P0-98)
- 文本上限：4M 字符（4 * 1024 * 1024）；
- PNG 图片 Base64 上限：24M 字符（24 * 1024 * 1024）；
- Windows 路径列表上限：128 项（从 `CF_HDROP` / `FileNameW` 缓冲区解析）。

### 1.8 渲染层崩溃自愈与健康度报告 (P0-99, P0-149, P0-150)
恢复 `electron-main.ts` 第 165-172 行原生自愈时序：
1. **优先报告健康度 (P0-149)**：收到 `render-process-gone` 事件且非正常退出时，**立即上报健康度**：`reportHealth('failed', lastRenderer, 'pet_renderer_crashed')`；
2. **时间窗口过滤**：过滤出过去 60 秒内的崩溃时间戳列表：`recoveries = recoveries.filter(t => Date.now() - t < 60_000)`；
3. **熔断判定**：若 `recoveries.length >= 3`，触发熔断，停止自愈并保持静默；
4. **延迟重载**：若未熔断，记录当前时间戳并延时重启：`setTimeout(() => loadOverlayPage(lastRenderer), 500 * recoveries.length)`。
- **边界用例矩阵 (P0-150)**：
  - 第 1 次崩溃：延时 500ms 重载；
  - 第 2 次崩溃：延时 1000ms 重载；
  - 第 3 次崩溃：延时 1500ms 重载；
  - 同一 60 秒内发生第 4 次崩溃：熔断打开，不执行重载。

### 1.9 Fallback 雪碧图权威元数据 (P0-125)
- 帧宽度：192 DIP；帧高度：208 DIP；缩放系数：0.33；底部边距：24 DIP。

### 1.10 Bridge 轮询时序与 `pushState` 状态防倒灌 (P0-126, P0-127, P0-128, P0-132)
1. **轮询时序与退出检测**：
   - 每 100ms 检查宿主父进程存活性；若父进程已死亡，立即销毁窗口退出；
   - 通过 `SnapshotReader` 读取快照；
   - 若 `visible === false && !fullscreen`，关闭窗口并返回；
   - 若窗口未创建，执行创建；若 `bounds` 发生变化，应用 `clampBounds` 并更新窗口；
   - **全屏响应 (P0-132)**：若 `fullscreen === true`，执行 `hide()`；否则执行 `showInactive()`；
   - 若 `renderer` 变更，重载页面并返回；
   - 最终执行 `host_publish_state()`。
2. **防倒灌机制 (Anti-Snapback)**：前端拖拽过程中，渲染层通过 `petOverlay.pushState` 上报坐标前，宿主读取真实物理位置，严禁直接覆盖导致位置回弹跳变。

---

## 2. 桌面级 Shell 保真度契约

### 2.1 普通用户交互隔离 (P0-104)
普通用户界面严禁出现 `electron` / `rust` 底层技术名词，仅提供“随主程序内嵌”与“独立桌面浮窗”两种业务开关。

### 2.2 启动登录态原子传递 (P1-131)
启动子进程时，通过标准输入（stdin）或受控临时 IPC 管道注入会话凭证，严禁暴露在进程命令行参数中。

### 2.3 确定性子进程关闭 (P0-64)
宿主主进程退出时，向所有衍生进程发送 SIGTERM / TerminateProcess，并在 2.5 秒宽限期内强制清理残留进程。

---

## 3. 跨平台后端抽象层：PlatformBackend trait

```rust
pub trait PlatformBackend: Send + Sync {
    fn init(&mut self) -> Result<(), BackendError>;
    fn create_surface(&mut self, config: &SurfaceConfig) -> Result<(), BackendError>;
    fn update_geometry(&mut self, rect: &BridgeDipRect) -> Result<(), BackendError>;
    fn update_input_region(&mut self, snapshot: &InteractionRegionSnapshot) -> Result<(), BackendError>;
    fn set_visible(&mut self, visible: bool) -> Result<(), BackendError>;
    fn set_opacity(&mut self, opacity: f64) -> Result<(), BackendError>;
    fn destroy_surface(&mut self) -> Result<(), BackendError>;
}
```

---

## 4. Native Wayland Layer-Shell 后端 (KDE / wlroots)

### 4.1 Route A 初始化顺序 (P0-59)
GTK Window 必须在调用 `widget.show()` 或 `widget.realize()` **之前**完成 Layer-Shell 初始化：
```rust
let window = gtk::Window::new(gtk::WindowType::Toplevel);
gtk_layer_shell::init_for_window(&window);
gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay);
gtk_layer_shell::set_namespace(&window, "readmd-pet");
gtk_layer_shell::set_keyboard_mode(&window, gtk_layer_shell::KeyboardMode::None);
```

### 4.2 动态探针与能力冻结 (P0-60)
在启动探针阶段执行 `is_supported()` 与 `protocol_version()`，结果固化在 `LayerShellCapabilities` 结构中，严禁在渲染循环中频繁 IPC 查询。

### 4.3 合成器避让候选策略 (P0-113, P0-163)
Wayland 环境缺乏全局 `Display.workArea`。使用 `Layer::Overlay` 与 `exclusive_zone(0)` 作为候选避让策略，其具体行为依赖 `VAL-31` 物理实机验证。不同合成器（KWin, Sway, Hyprland）可能呈现差异行为，系统支持通过能力探针进行策略切换。

---

## 5. GNOME Wayland 专属扩展：ReadMD GNOME Shell Companion

### 5.1 AppID 绑定、窗口穿透与多窗口消歧 (P0-65, P0-66, P0-80, P0-81, P0-164, P0-165)
- **AppID 唯一标识**：`PET_OVERLAY_APP_ID = "asia.readmd.pet"`；
- **穿透实现路径 (P0-164)**：GNOME 环境下窗口置顶与图层控制由 Companion 扩展负责；点击穿透与输入区域控制通过 GTK/GDK client surface region 或 Shell 侧专用通道实现（关联 `VAL-01`, `VAL-13`, `VAL-22`, `VAL-45`, `VAL-46`）；
- **多窗口确定性消歧 (P0-165)**：当同一 Rust 进程创建辅助窗口时，Companion 结合 Peer PID、GTK 应用 ID (`asia.readmd.pet`) 以及窗口角色标签进行精确匹配：0 个匹配则安全关闭，1 个匹配直接绑定，多个匹配若无确定性标记则安全关闭（关联 `VAL-47`）；
- **去除废弃 API**：剔除已废弃的 `set_skip_taskbar`，统一调用 `metaWindow.hide_from_window_list()`。

### 5.2 双语法扩展包分发 (P0-67, P0-68, P0-69)
- GNOME 42~44（Ubuntu 22.04 / Debian 12）：分发 Legacy GJS imports 语法；
- GNOME 45~50（Ubuntu 24.04 / Fedora 40+）：分发 ESM 模块标准语法。

---

## 6. Wayland 交互区域与坐标强类型模型 (P0-111, P0-161, P0-162)

### 6.1 强类型坐标系统定义 (P0-162)
为了防止裸 `f64` 浮点数在不同坐标空间混淆，定义强类型几何结构：
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssPxPoint { pub x: f64, pub y: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssPxRect { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceLocalDipRect { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeGlobalDipRect { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputLocalDipRect { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalPxRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
```

### 6.2 Wayland 交互快照与局部坐标转换 (P0-111, P0-161)
化解 Wayland 下透明穿透导致无法感知鼠标进入事件的死锁，采用局部坐标快照主动同步协议：
```rust
pub struct InteractionRegionSnapshot {
    pub generation: u64,
    pub rects: Vec<SurfaceLocalDipRect>, // 必须采用以窗口自身左上角为原点的局部坐标
}
```
**坐标转换链条**：
$$\text{Renderer CSS/local} \longrightarrow \text{SurfaceLocalDipRect} \longrightarrow \text{Backend Scale} \longrightarrow \text{wl\_region} \longrightarrow \text{wl\_surface.commit()}$$
`BridgeGlobalDipRect` 仅用于全局窗口定位，严禁混入窗口内部命中区域。

---

## 7. Linux 平台桌面集成基准
- 符合 XDG 桌面规范，提供标准的 `.desktop` 文件与图标资源；
- 支持系统托盘集成与多显示器热插拔事件感知。

---

## 8. ReadMD Pet IPC v1：会话隔离与 WRY 官方 API 绑定 (P0-73, P0-74, P0-88, P0-89)

### 8.1 会话与世代隔离
- `webview_session_id`：为每个 WebView 实例分配全局唯一 UUID；
- `navigation_generation`：页面每次重新导航时递增世代计数；
- 丢弃所有携带过期世代令牌的异步回调。

### 8.2 IIFE 脚本封装
```javascript
(() => {
  if (window.top !== window.self) {
    console.warn("[ReadMD] Blocked petOverlay initialization in non-top frame");
    return;
  }
  // 注入 window.__HERMES_PET__ 与 window.readmdPet
})();
```

---

## 9. 安全资产加载协议与沙箱边界

### 9.1 平台专属 Origin
- Windows: `https://readmd.localhost/`
- macOS / Linux: `readmd://localhost/`

### 9.2 严格路径沙箱与 TOCTOU 防御 (P0-118)
只读受管根目录，禁止符号链接跨目录访问，拦截带有 NUL 字节或路径穿越符的请求。

### 9.3 生产构建 DevTools 禁用 (P0-119)
生产环境中严禁开启开发者工具与控制台端口。

---

## 10. 权威 Durable FIFO 与 SnapshotReader 规范 (P0-100, P0-142, P0-143, P0-144, P0-145)

### 10.1 FIFO 命令队列契约 (P0-100, P0-144, P0-145)
- **唯一权威目录路径**：`${bridge}.commands`（严禁使用虚构的 events 路径）；
- **精确 Envelope 封装 (P0-144)**：
  ```javascript
  JSON.stringify({
    command,
    created_at: Date.now()
  })
  ```
  禁止向原生命令体中强行插入未经验证的 envelope 版本号、session token 或引擎名称。
- **前置排队容量检查 (P0-145)**：
  ```javascript
  const queued = fs.readdirSync(directory).filter(name => name.endsWith('.json'));
  if (queued.length >= 128) throw new Error('pet_command_queue_full');
  ```
  写入前已存在文件数量必须在 0 到 127 之间，写入成功后目录内最大文件数不超过 128 个。
- **容量与原子写入**：单文件上限 32 MiB，总待处理容量上限 64 MiB；先写入 `<target>.tmp`，然后原子 `renameSync` 至 `<target>.json`。

## 11. 原生文件拖拽：WRY 0.57 官方路径规范 (P0-102)
- 提取文件绝对路径，过滤非空字符串（长度 <= 32768，数组上限 128 项）；
- 触发 `readmdPet.dropFiles(paths)` 并通过 FIFO 发送 `{"type": "drop", "paths": [...]}`。

---

## 12. 内容安全策略 (CSP) 与资源隔离
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

---

## 13. 异常退出与崩溃自愈状态机
- 支持进程异常退出时的原子日志记录与自动隔离；
- 与前述第 1.8 节熔断机制协同联动。

---

## 14. 实例互斥作用域与多用户隔离 (P0-105)
- Windows 严禁使用全局会话命名空间（禁止使用 `Global\`）；
- 必须使用用户私有会话命名空间：`Local\ReadMDPetOverlay_<UserSIDHash>_<DataDirHash>`，确保多用户会话隔离。

---

## 15. 宿主生命周期管理：Parent Liveness 立即退出与优雅替换 (P0-106, P0-135)

### 15.1 父进程存活性感知 (ParentDeathShutdown, P0-135)
- Python 宿主持有管道写入端，Rust 宿主启动时继承只读端；
- 当 Python 宿主退出或崩溃时，远端写入句柄关闭，Rust 读端立即收到 EOF；
- **立即退出规则**：Rust 收到 EOF 后，立即向主线程事件循环发送 `ShutdownParentGone` 事件，主线程关闭/销毁 Overlay 窗口并立即退出进程。**严禁人为等待 2.5 秒**；
- 可设置短时有界析构看门狗（如 <= 500ms），仅用于防止窗口析构挂死，该看门狗绝非 Golden 延迟。

### 15.2 引擎编排器优雅替换 (EngineReplacementGracePeriod, P0-135)
在 Python 主进程主动执行引擎热切换时：
- Python 主进程向旧子进程发出关闭请求；
- 等待最多 2500ms（优雅退出宽限期）；
- 若超时子进程仍未退出，则针对其具体 PID 执行强制终止（`taskkill` 或 `terminate/kill`）。

---

## 16. 权威状态对齐模型 (Reconciliation State Model) (P0-75, P0-76, P0-133, P0-134)

### 16.1 期望与已应用状态结构 (P0-133)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub surface_exists: bool,
    pub visible: bool,
    pub bounds: BridgeDipRect,
    pub opacity: f64,
    pub renderer: RendererKind,
    pub backend_generation: u64,
    pub navigation_generation: u64,
}
```

### 16.2 正交运行时状态枚举 (P0-134)
```rust
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

### 16.3 状态对齐函数与优先级仲裁 (P0-134)
```rust
pub fn reconcile(
    desired: &DesiredOverlayState,
    applied: &AppliedOverlayState,
    lifecycle: HostLifecycle,
    surface: SurfaceState,
    input: InputState,
    capabilities: &BackendCapabilities,
) -> Vec<Effect>
```
**仲裁优先级**：
$$\text{ShuttingDown} > \text{Suspended} > (\text{visible=false} \land \neg\text{fullscreen}) > (\text{fullscreen=true}) > (\text{Recovering/Loading}) > \text{Ready}$$
**拖拽保护规则**：若当前处于 `InputState::Dragging` 且收到了 `visible=false` 或 `fullscreen=true`，必须先发出 `Effect::AbortDrag` 中断拖拽，随后再执行隐藏或销毁操作。

---

## 17. 健康度监控所有权与原子写入规范 (P0-85, P0-136, P0-137, P0-151)

### 17.1 所有权边界清晰划分 (P0-136)
- **遗留 Electron 宿主健康文件**：`${bridge}.health.json`，**唯一所有者为 Electron 宿主主进程**（通过 `electron-main.ts` 中的 `reportHealth` 写入），渲染层严禁直接写入该文件；
- **Rust 宿主健康文件**：`${bridge}.rust.health.json`，唯一所有者为 Rust 宿主进程；
- **渲染层通信机制**：渲染层仅在完成挂载或遇到错误时，通过 Control IPC 发送 `renderer-ready` 或 `renderer-failed` 事件通知宿主。

### 17.2 健康度负载契约 (P0-137)
- **遗留 Schema (Legacy Schema)**：
  ```json
  {
    "state": "loading",
    "renderer": "live2d",
    "code": "optional_code",
    "pid": 12345,
    "updated_at": 1726300000000
  }
  ```
- **Rust 扩展 Schema (Rust Schema)**：
  ```json
  {
    "engine": "rust",
    "state": "ready",
    "renderer": "live2d",
    "code": null,
    "pid": 12345,
    "updated_at": 1726300000000,
    "engine_generation": 1,
    "protocol_version": 1
  }
  ```
- **原子写入契约 (P0-151)**：写入 `<target>.tmp` 临时文件后，通过原子 `rename` 替换目标文件，防止读取方解析到截断的残缺 JSON。

---

## 18. 权威构建与依赖规范 (Cargo Specification) (P0-55, P0-56, P0-57, P0-58)

### 18.1 全平台统一母体：Cargo.toml
```toml
[package]
name = "readmd-pet-rust"
version = "0.1.0"
edition = "2021"
rust-version = "1.85.0"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.43", features = ["sync", "time", "rt"] }
tao = { version = "0.37.0", default-features = false, features = ["rwh_06"] }
wry = { version = "0.57.0", default-features = false, features = ["os-webview"] }

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Dwm",
    "Win32_Security"
] }
muda = "0.15"

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-app-kit = "0.2"
objc2-foundation = "0.2"
muda = "0.15"

[target.'cfg(target_os = "linux")'.dependencies]
gtk = { version = "0.18", default-features = false }
gdk = { version = "0.18", default-features = false }
glib = "0.20"
gtk-layer-shell = { version = "0.4.0", features = ["v0_6"] }

[features]
default = []
linux-production = []
```

---

## 19. 临时测量参考指标与性能基线 (P0-109, P0-169)
启动耗时与常驻内存目标仅作为 Phase 0 物理概念验证阶段的测量参考指标，在形成正式经过批准的平台 ADR 之前，绝不作为全局发布阻断门禁。

---

## 20. 平台认证元组规范 (Platform Certification Tuples) (P0-71, P0-72, P0-107, P0-138, P0-139, P0-152)

### 20.1 认证生命周期模型
```
[ Planned ] --(Phase 0 PoC 通过)--> [ Candidate ] --(实测全通过)--> [ Certified ]
     |                                    |
     +------------(验证失败)-------------> [ Rejected ]
```
**所有 24 个平台认证元组当前统一处于 Planned 状态**。

### 20.2 权威原子平台认证元组表 (P0-138, P0-152)
以稳定主键 `tuple_key` 作为机器标识，每个字段保持原子单值：

| 序号 | 稳定主键 (tuple_key) | 操作系统与版本 | 架构 | 显示服务 | 规划适配器实现 | 当前生命周期 |
|---|---|---|---|---|---|---|
| **T-01** | `windows-11-24h2-x64-dwm-win32` | Windows 11 (24H2) | x86_64 | DWM / DWM | `Win32Backend` | **Planned** |
| **T-02** | `windows-11-23h2-x64-dwm-win32` | Windows 11 (23H2) | x86_64 | DWM / DWM | `Win32Backend` | **Planned** |
| **T-03** | `windows-11-24h2-arm64-dwm-win32` | Windows 11 (24H2) | aarch64 | DWM / DWM | `Win32Backend` | **Planned** |
| **T-04** | `windows-10-22h2-x64-dwm-win32` | Windows 10 (22H2) | x86_64 | DWM / DWM | `Win32Backend` | **Planned** |
| **T-05** | `macos-26-tahoe-arm64-quartz-cocoa` | macOS 26 (Tahoe) | aarch64 | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-06** | `macos-15-sequoia-arm64-quartz-cocoa` | macOS 15 (Sequoia) | aarch64 | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-07** | `macos-14-sonoma-arm64-quartz-cocoa` | macOS 14 (Sonoma) | aarch64 | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-08** | `macos-13-ventura-arm64-quartz-cocoa` | macOS 13 (Ventura) | aarch64 | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-09** | `macos-15-sequoia-x64-quartz-cocoa` | macOS 15 (Sequoia) | x86_64 | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-10** | `macos-14-sonoma-x64-quartz-cocoa` | macOS 14 (Sonoma) | x86_64 | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-11** | `macos-13-ventura-x64-quartz-cocoa` | macOS 13 (Ventura) | x86_64 | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-12** | `ubuntu-24.04-gnome46-x64-wayland-gnomecompanion` | Ubuntu 24.04 (LTS) | x86_64 | Wayland / Mutter | `GnomeCompanionBackend` | **Planned** |
| **T-13** | `ubuntu-24.04-gnome46-arm64-wayland-gnomecompanion` | Ubuntu 24.04 (LTS) | aarch64 | Wayland / Mutter | `GnomeCompanionBackend` | **Planned** |
| **T-14** | `ubuntu-22.04-gnome42-x64-x11-x11backend` | Ubuntu 22.04 (LTS) | x86_64 | X11 / Mutter | `X11Backend` | **Planned** |
| **T-15** | `debian-12-gnome43-x64-wayland-gnomecompanion` | Debian 12 (Bookworm) | x86_64 | Wayland / Mutter | `GnomeCompanionBackend` | **Planned** |
| **T-16** | `fedora-40-kde6-x64-wayland-layershell` | Fedora 40 (Standard) | x86_64 | Wayland / KWin | `LayerShellBackend` | **Planned** |
| **T-17** | `fedora-42-kde6-x64-wayland-layershell` | Fedora 42 (Rawhide) | x86_64 | Wayland / KWin | `LayerShellBackend` | **Planned** |
| **T-18** | `fedora-42-gnome50-x64-wayland-gnomecompanion` | Fedora 42 (Rawhide) | x86_64 | Wayland / Mutter | `GnomeCompanionBackend` | **Planned** |
| **T-19** | `archlinux-rolling-sway-x64-wayland-layershell` | ArchLinux Rolling (Current) | x86_64 | Wayland / wlroots | `LayerShellBackend` | **Planned** |
| **T-20** | `archlinux-rolling-hyprland-x64-wayland-layershell` | ArchLinux Rolling (Current) | x86_64 | Wayland / wlroots | `LayerShellBackend` | **Planned** |
| **T-21** | `uos-20-sp1-x64-x11-x11backend` | UOS 20 (SP1) | x86_64 | X11 / KWin-DDE | `X11Backend` | **Planned** |
| **T-22** | `uos-20-sp1-arm64-x11-x11backend` | UOS 20 (SP1) | aarch64 | X11 / KWin-DDE | `X11Backend` | **Planned** |
| **T-23** | `kylin-v10-sp1-x64-x11-x11backend` | Kylin V10 (SP1) | x86_64 | X11 / UKUI-KWin | `X11Backend` | **Planned** |
| **T-24** | `deepin-23-treeland-x64-wayland-layershell` | Deepin 23 (Release) | x86_64 | Wayland / Treeland | `LayerShellBackend` | **Planned** |

---

## 21. 自动化质量门禁体系 (Quality Assurance Gates)
全系统已在机器注册表中注册 71 道全域门禁，涵盖 Core、Golden、Backend 及 Tuple 四大维度。

---

## 22. 全屏感知与桌面环境业务规则 (P0-110, P0-129, P0-132)

### 22.1 业务归属与禁止主动探测规则 (P0-132)
- **唯一规则**：全屏状态属于 Python 宿主的权威业务状态（`snapshot.fullscreen`）；
- **Rust 宿主行为禁令**：
  - 严禁扫描或检测前台是否存在全屏独占应用；
  - 严禁探测独占游戏进程；
  - 严禁根据操作系统窗口几何尺寸私自推断全屏状态；
  - 严禁脱离 Python 快照指示自行切换浮窗显示状态；
- **平台后端唯一下发行为**：
  $$\text{desired.fullscreen} == \text{true} \implies \text{hide()}$$
  $$\text{desired.fullscreen} == \text{false} \implies \text{continue normal visible reconciliation}$$
- **保留 Golden 边界特异性**：
  - `visible === false && fullscreen === false`：关闭窗口，Surface 处于 Absent 状态；
  - `visible === false && fullscreen === true`：隐藏窗口，Surface 可保留在后台但处于不可见状态。

### 22.2 Windows 虚拟桌面行为 (P0-110, P0-129)
Golden 行为仅依附于当前活动的虚拟桌面，不进行跨虚拟桌面伪造。

---

## 23. 运行时分发与离线验签 (PetRuntimeInstallerV2) (P0-114, P0-115, P0-116, P0-117, P0-140, P0-141, P0-166, P0-167, P0-168)

### 23.1 分离式签名模型 (P0-114)
- 产物清单：`manifest.json` 与分离式签名 `manifest.json.sig`；
- 校验模型：使用固化信任公钥环（Pinned Trusted Keyring），比对 `security_epoch` 防御重放回滚。

### 23.2 示例产物清单 (P0-115, P0-166, P0-167)
```json
{
  "manifest_version": "2.0.0",
  "security_epoch": 2,
  "key_id": "readmd-release-2026-ed25519",
  "artifacts": [
    {
      "role": "rust_host",
      "platform": "linux",
      "arch": "x86_64",
      "path": "bin/readmd-pet-rust",
      "sha256": "<64-hex-sha256-generated-at-build>",
      "size": 0
    },
    {
      "role": "renderer_bundle",
      "platform": "all",
      "arch": "all",
      "path": "renderer/bundle.tar.gz",
      "sha256": "<64-hex-sha256-generated-at-build>",
      "size": 0
    },
    {
      "role": "fallback_sprite",
      "platform": "all",
      "arch": "all",
      "path": "assets/fallback-sprite.png",
      "sha256": "<64-hex-sha256-generated-at-build>",
      "size": 0
    },
    {
      "role": "gnome_legacy",
      "platform": "linux",
      "arch": "all",
      "path": "extensions/gnome-legacy",
      "sha256": "<64-hex-sha256-generated-at-build>",
      "size": 0
    },
    {
      "role": "gnome_esm",
      "platform": "linux",
      "arch": "all",
      "path": "extensions/gnome-esm",
      "sha256": "<64-hex-sha256-generated-at-build>",
      "size": 0
    },
    {
      "role": "runtime_schema",
      "platform": "all",
      "arch": "all",
      "path": "schemas/runtime-v1.json",
      "sha256": "<64-hex-sha256-generated-at-build>",
      "size": 0
    },
    {
      "role": "platform_metadata",
      "platform": "all",
      "arch": "all",
      "path": "metadata/platforms.json",
      "sha256": "<64-hex-sha256-generated-at-build>",
      "size": 0
    }
  ]
}
```
*注：上述 `size: 0` 仅作为模式示例，生产环境由构建脚本自动注入真实字节数值。*

### 23.3 验签决策闭环：ADR-runtime-signature-verifier (P0-140, P0-141)
- **决策状态**：Accepted
- **唯一法定生产验签器**：`PetRuntimeInstallerV2 (Python)` 采用 `cryptography>=42.0.0`；
- **支持环境**：Python >= 3.10；
- **打包实证要求 (P0-141)**：各受支持平台发行版的安装包可用性属于工程实证要求，由 `VAL-44` 追踪；
- **备选方案定位**：Rust 独立 bootstrap 验签器保留为紧急容灾备选，非当前法定路径。

### 23.4 security_epoch 持久化防回滚状态机 (P0-168)
在用户本地状态目录下持久化存储 `highest_accepted_security_epoch`：
- 若新清单中的 `epoch < highest_accepted`，立即拒绝安装；
- 若 `epoch == highest_accepted`，在版本号更高且签名有效时允许更新；
- 若 `epoch > highest_accepted`，在签名校验通过后，原子更新持久化记录；
- 覆盖安装、Profile 重置与便携模式均遵循此持久化单调递增策略（由 `VAL-48` 验证）。

---

## 24. Phase 0 物理概念验证计划 (PoC Spikes)
在正式启动产品功能实现前，必须在真实物理机环境下完成 Wayland 穿透、GNOME Companion 通信、Windows ARM64 加速等核心技术难题的独立验证。

---

## 25. 分级发布熔断条件 (Scoped Release Stop Conditions) (P0-108, P0-153, P0-169, P0-170)

### 25.1 架构冻结准出条件 (Architecture Freeze Scope)
必须满足：
$$\text{Unresolved Architecture Blockers} == 0 \land \text{Unresolved Architecture Validations} == 0$$

### 25.2 平台元组认证准出条件 (Tuple Certification Scope)
对于特定平台元组 $T$，必须满足其所关联的所有门禁与验证项全部通过：
$$\forall v \in \text{Validations}(\text{applies\_to}(T)), \quad v == \text{PASS}$$

### 25.3 全局产品发布条件 (Product Release Scope)
当且仅当本期规划发布所必需的目标平台元组均已获得 **Certified** 认证，且：
1. 无未处理的核心缺陷；
2. 异常退出率 $= 0.00\%$；
3. 若存在经批准的平台性能预算，实测内存泄露指标不得超出该预算。

---

## 26. Golden 差异对比测试与录制回放 (P0-124)
通过 Golden Fixture 捕获工具，在 CI 中对比 Rust 实现与 Electron 原生行为的像素级与消息级差异。

---

## 27. 验收标准与准出公式 (P0-108)

### 27.1 候选状态定义
当前版本属于 `v1.4.5-Candidate`，在所有开放实证项未清零前，禁止宣称 Production Freeze。

### 27.2 判定公式
$$\text{Production Freeze Ready} \iff (\text{Unresolved Architecture Blockers} === 0) \land (\text{Unresolved Architecture Validations} === 0)$$

---

## 28. 跨平台能力支撑矩阵
详细覆盖 Win32、Cocoa、LayerShell、GnomeCompanion 四大后端的全生命周期能力。

---

## 29. 闭环决策树、验证注册表与阻塞台账

### 29.1 已闭环架构决策 (Known Design Decisions: Closed)
- **DEC-01**：Golden 行为源集锁定与 Preload ABI 全量恢复
- **DEC-02**：窗口边界策略解耦与 0 吸附阈值
- **DEC-03**：Durable FIFO 规范路径 `${bridge}.commands` 与排队限制
- **DEC-04**：Layer-Shell 初始化时机必须在 show/realize 之前
- **DEC-05**：Wayland 局部坐标交互快照与无死锁输入协议
- **DEC-06**：GNOME AppID 唯一标识与多窗口消歧绑定
- **DEC-07**：WebView 世代令牌隔离与防倒灌状态对齐
- **DEC-08**：规范全量交付、哈希防伪与机器注册表 100% 引用完整性
- **DEC-09**：Linux 生产依赖隔离与 muda 平台剔除
- **DEC-10**：gtk-layer-shell v0_6 特性锁定
- **DEC-11**：Windows 本地会话互斥量与当前虚拟桌面依附
- **DEC-12**：只读资产沙箱加载协议与 TOCTOU 防御
- **DEC-13**：分离式签名与固定公钥环离线验签模型
- **DEC-14**：平台元组生命周期严格遵循 Planned 启动原则
- **DEC-15**：分级发布熔断与严格准出判定公式
- **DEC-16**：健康度监控所有权分离（Electron 宿主 vs Rust 宿主）
- **DEC-17**：Parent Liveness 立即退出与优雅替换超时解耦
- **DEC-18**：Golden SnapshotReader 原生签名与自愈重试机制

### 29.2 实证验证项注册表 (Empirical Validation Items) (P0-120, P0-156)
*权威数据源：`docs/architecture/pet-rust/validation-registry.json`*

<!-- GENERATED: validation-summary -->
> **统计**：当前共注册 **51 项实证验证项（VAL-01 ~ VAL-51）**，统一在 Phase 0 物理测试床中执行。

| 编号 | 领域与验证项 | 范围 / 目标平台 | 物理风险与验证指标 | 关联阻断门禁 | 关联架构阻塞项 |
|---|---|---|---|---|---|
| **VAL-01** | Layer-Shell + WRY 容器 | `backend` (wayland-layershell) | WebKitGTK 子表面是否会二次截获透明穿透区域的鼠标事件 | `Gate-LayerShell-02` | `BLOCKER-01` |
| **VAL-02** | Layer-Shell 跨屏拖拽 | `backend` (wayland-layershell) | 重设 Monitor 时 Wayland 合成器的 Implicit Pointer Grab 是否中断 | `Gate-LayerShell-01` | `BLOCKER-02` |
| **VAL-03** | Layer-Shell 按需输入法 | `backend` (wayland-layershell) | 合成器对 zwlr_layer_shell_v1 v4+ KeyboardMode::OnDemand 的支持率 | `Gate-LayerShell-03` | 无直接阻塞 |
| **VAL-04** | GNOME Companion 跨版本稳定性 | `backend` (gnome-wayland) | GNOME 42~50 各大版本中 Mutter move_frame 的内部接口稳定性 | `Gate-GnomeCompanion-02` | `BLOCKER-03` |
| **VAL-05** | Windows ARM64 渲染基准 | `tuple` (windows-11-24h2-arm64-dwm-win32) | 高通骁龙平台 WebView2 透明窗口层叠与 CPU 占用基准 | `CERT-WIN11-ARM64` | `BLOCKER-05` |
| **VAL-06** | 国产 Linux 发行版依赖 | `tuple` (uos-20-sp1-x64-x11-x11backend) | 统信 UOS / 麒麟软件源中 webkit2gtk-4.1 的预装与动态链接一致性 | `CERT-UOS20-X64` | 无直接阻塞 |
| **VAL-07** | Linux 通用单二进制 PoC | `architecture` (all) | 单一二进制在 X11 与 Wayland 环境下的 GDK 动态加载与行为一致性 | `Gate-Linux-Universal` | `BLOCKER-04` |
| **VAL-08** | muda GTK 菜单在 KWin Wayland | `backend` (wayland-layershell) | KWin 环境下右键弹出菜单的准确定位、失焦关闭与层级行为 | `Gate-Menu-KWin-Wayland` | 无直接阻塞 |
| **VAL-09** | muda GTK 菜单在 GNOME Wayland | `backend` (gnome-wayland) | Mutter 环境下右键弹出菜单的输入捕获释放与穿透恢复 | `Gate-Menu-GNOME-Wayland` | 无直接阻塞 |
| **VAL-10** | WRY custom DnD 副作用 | `architecture` (all) | Windows 上启用 with_drag_drop_handler 时渲染端 DOM 与文件拖放回归 | `Gate-Core-10` | 无直接阻塞 |
| **VAL-11** | 调和状态机事件序列 Fuzz | `architecture` (all) | proptest 50,000 条极端并发事件序列下的状态不变性与防死锁 | `Gate-Core-11` | 无直接阻塞 |
| **VAL-12** | LayerShell 全局转局部坐标映射 | `backend` (wayland-layershell) | 多屏排布下 VirtualDesktopMapper 的边距计算与跨屏对齐 | `Gate-LayerShell-04` | 无直接阻塞 |
| **VAL-13** | GNOME move_frame 混合 DPI 映射 | `backend` (gnome-wayland) | 混合缩放比例下 GnomeCoordinateMapper 与 Mutter Stage 坐标一致性 | `Gate-GnomeCompanion-03` | 无直接阻塞 |
| **VAL-14** | GNOME Legacy (<=44) Companion | `tuple` (ubuntu-22.04-gnome42-x64-x11-x11backend) | Ubuntu 22.04 / Debian 12 下 GJS imports 扩展加载与通信 | `CERT-UBUNTU22-X64` | 无直接阻塞 |
| **VAL-15** | GNOME ESM (>=45) Companion | `tuple` (ubuntu-24.04-gnome46-x64-wayland-gnomecompanion) | Ubuntu 24.04 / Fedora 39+ 下 ESM 扩展加载、热重载与会话恢复 | `CERT-UBUNTU24-X64` | 无直接阻塞 |
| **VAL-16** | Windows HTTPS 资产 Scheme | `architecture` (all) | Windows 10/11 下 with_https_scheme(true) 与最低运行时能力探针 | `Gate-Asset-HTTPS-Win` | 无直接阻塞 |
| **VAL-17** | Custom Protocol 路径沙盒 | `architecture` (all) | 跨平台路径穿越、符号链接逃逸、NUL 注入实测防御拦截率 100% | `Gate-Asset-Sandbox` | `BLOCKER-16` |
| **VAL-18** | 统信 UOS 运行时 WebKitGTK 验证 | `tuple` (uos-20-sp1-arm64-x11-x11backend) | UOS 20 SP1 物理机环境下动态链接库真实加载实证 | `CERT-UOS20-ARM64` | 无直接阻塞 |
| **VAL-19** | 银河麒麟运行时 WebKitGTK 验证 | `tuple` (kylin-v10-sp1-arm64-x11-x11backend) | Kylin V10 SP1 物理机环境下动态链接库真实加载实证 | `CERT-KYLIN10-ARM64` | 无直接阻塞 |
| **VAL-20** | macOS 13~26 支持周期认证 | `backend` (macos) | macOS 13 (Ventura) ~ 26 (Tahoe) 在 Apple Silicon 与 Intel 上的认证 | `CERT-MACOS-ALL` | `BLOCKER-15` |
| **VAL-21** | Layer-Shell Pre-Realize 生命周期 | `backend` (wayland-layershell) | GTK 窗口在未 realize 前绑定 Layer-Shell 且成功挂载 WRY 的实测路径 | `Gate-LayerShell-05` | `BLOCKER-09` |
| **VAL-22** | GNOME Meta.Window AppID 传播 | `backend` (gnome-wayland) | WRY/GTK 顶层窗口在 Mutter 内部 get_gtk_application_id() 真实返回值验证 | `Gate-GnomeCompanion-04` | `BLOCKER-10` |
| **VAL-23** | GNOME Companion 畸形 IPC Fuzz | `backend` (gnome-wayland) | 极端非法 JSON 与畸形指令下 GNOME Shell 进程零崩溃证明 | `Gate-GnomeCompanion-05` | 无直接阻塞 |
| **VAL-24** | Wayland 工作区与面板避让对齐 | `backend` (wayland-layershell) | Layer-Shell margins 映射与 KDE/wlroots 边缘独占区域对齐实测 | `Gate-LayerShell-06` | `BLOCKER-13` |
| **VAL-25** | 导航代际隔离异步消息丢弃测试 | `architecture` (all) | 构造跨导航延迟 IPC 消息，验证 Rust 宿主 100% 拒绝陈旧调用 | `Gate-IPC-Generation` | `BLOCKER-11` |
| **VAL-26** | 调和状态 AppliedState 异步生效竞态 | `architecture` (all) | 构造高频乱序 snapshot 事件，验证 AppliedState 拒绝陈旧代际状态生效 | `Gate-Core-Reconciliation` | `BLOCKER-12` |
| **VAL-27** | 权威规范产物完整性与发布检查 | `architecture` (all) | 验证规范本体哈希与完整性，自动化检查 release 产物 linux-production 特性 | `Gate-Core-SpecIntegrity` | `BLOCKER-06`, `BLOCKER-08` |
| **VAL-28** | muda 零 libxdo 依赖实测证明 | `architecture` (all) | 通过 readelf/ldd 证明 Linux 二进制完全不依赖 libxdo.so | `Gate-Linux-MudaTargetIsolation` | `BLOCKER-07` |
| **VAL-29** | 当前 GNOME 50 扩展兼容性实测 | `tuple` (fedora-42-gnome50-x64-wayland-gnomecompanion) | 验证 GNOME 50 环境下 ESM 扩展加载与 Mutter 接口调用平滑 | `CERT-GNOME50-X64` | `BLOCKER-14` |
| **VAL-30** | macOS 26 (Tahoe) 桌宠实机认证 | `tuple` (macos-26-tahoe-arm64-quartz-cocoa) | 验证 macOS 26 开发者/正式版下透明渲染、点击穿透与 Spaces 随同 | `CERT-MACOS26-ARM64` | `BLOCKER-15` |
| **VAL-31** | Layer-Shell compositor usable-area / exclusive-zone parity | `backend` (wayland-layershell) | 实测不同 Wayland 合成器在 Layer::Overlay + exclusive_zone(0) 下的面板避让与工作区对齐语义 | `Gate-LayerShell-06` | `BLOCKER-27` |
| **VAL-32** | Golden preload ABI differential | `architecture` (all) | 逐项对比 Rust 注入 bridge 与 Golden preload.ts 的 9 个 API 方法、参数个数与取消订阅函数 | `Gate-Golden-ABI` | `BLOCKER-17`, `BLOCKER-18` |
| **VAL-33** | Golden bounds policy differential | `architecture` (all) | 分别测试 HostSnapshotBoundsPolicy (40 DIP workArea) 与 RendererInteractiveBoundsPolicy (80x80 min) | `Gate-Golden-Bounds` | `BLOCKER-19` |
| **VAL-34** | Golden control/toggle-app differential | `architecture` (all) | 实测 toggle-app 触发剪贴板读取并向 FIFO 写入 clipboard 消息，验证无窗口隐藏误动作 | `Gate-Golden-Differential` | `BLOCKER-21` |
| **VAL-35** | Golden menu model differential | `architecture` (all) | 实测右键菜单 4 项互动动作 (Pet, Feed, Play, Rest/Wake)、Characters 子菜单、Open reader、disabled header 与 separators 顺序 | `Gate-Golden-Menu` | 无直接阻塞 |
| **VAL-36** | Golden clipboard payload differential | `architecture` (all) | 实测 4M 字符文本截断、24M 字符 base64 PNG 截断与 Windows 128 路径解析的一致性 | `Gate-Golden-Clipboard` | 无直接阻塞 |
| **VAL-37** | Golden FIFO byte/filename parity | `architecture` (all) | 实测 <bridge>.commands 目录排队、32MB 单体限制、64MB 总量限制与 .tmp 独占写原子重命名 | `Gate-Golden-FIFO` | `BLOCKER-20` |
| **VAL-38** | Golden SnapshotReader retry parity | `architecture` (all) | 构造非法 JSON snapshot 验证签名未被污染，后续合法 snapshot 能立即被识别与消费 | `Gate-Golden-Differential` | 无直接阻塞 |
| **VAL-39** | Golden hit-region differential | `architecture` (all) | 运行确定性点网格与边缘测试，验证 Live2D hitTest || bounds.contains 与 Sprite 规则的精确等价性 | `Gate-Golden-HitRegion` | `BLOCKER-26` |
| **VAL-40** | Golden pushState / anti-snapback differential | `architecture` (all) | 实测 pushState 下发时以实际当前窗口 bounds 覆盖 snapshot bounds，防止拖拽后回弹 | `Gate-Golden-AntiSnapback` | 无直接阻塞 |
| **VAL-41** | Windows virtual-desktop Golden behavior | `backend` (windows) | 实测 Windows 虚拟桌面切换时桌宠仅在当前桌面显示，验证无私有 API 越界注入 | `Gate-Win32-VirtualDesktop` | `BLOCKER-25` |
| **VAL-42** | Runtime detached-signature verification | `architecture` (all) | 实测分离签名 manifest.json.sig 配合固定公钥环进行验签，拒绝 manifest 自签名注入 | `Gate-Crypto-DetachedSig` | `BLOCKER-28` |
| **VAL-43** | Asset sandbox TOCTOU threat-model/handle test | `architecture` (all) | 在只读托管沙盒根目录下模拟同用户重解析点与符号链接交换，验证只读受管根的不可变防御边界 | `Gate-Asset-TOCTOU` | 无直接阻塞 |
| **VAL-44** | Runtime Security & Packaging | `architecture` (all) | Production signature verifier dependency (cryptography>=42.0) availability across all supported OS/arch packaging matrices | `Gate-Crypto-VerifierPackaging` | `BLOCKER-28` |
| **VAL-45** | Wayland Hit Geometry Transformation | `backend` (wayland) | Renderer CSS/local geometry -> SurfaceLocalDip -> wl_region coordinate transform parity under fractional scaling (1.25x, 1.5x, 2.0x) | `Gate-Wayland-TransformParity` | `BLOCKER-02`, `BLOCKER-26` |
| **VAL-46** | GNOME Shell Companion Click-Through | `backend` (gnome-wayland) | GNOME Companion input-region and click-through implementation path (GTK/GDK client surface region vs Mutter Shell-side) | `Gate-GnomeCompanion-ClickThrough` | `BLOCKER-03` |
| **VAL-47** | GNOME Multi-Window Disambiguation | `backend` (gnome-wayland) | GNOME Peer PID + GTK app ID (asia.readmd.pet) + window tag deterministically resolving single Meta.Window among multiple process surfaces | `Gate-GnomeCompanion-Disambiguation` | `BLOCKER-10` |
| **VAL-48** | Security Epoch Rollback Defense | `architecture` (all) | security_epoch persistent state machine rejecting outdated manifest replays across portable, reinstall, and profile reset flows | `Gate-Crypto-RollbackProtection` | `BLOCKER-28` |
| **VAL-49** | Health Protocol Ownership & Atomic Write | `architecture` (all) | Electron host main process owns <bridge>.health.json, Rust host owns <bridge>.rust.health.json, temp file + atomic rename parity | `Gate-Golden-Health` | `BLOCKER-31` |
| **VAL-50** | Parent Liveness Prompt Teardown | `architecture` (all) | Parent pipe EOF triggers immediate shutdown without 2.5s delay; orchestrator replacement maintains <=2500ms timeout | `Gate-Golden-ParentDeath` | `BLOCKER-32` |
| **VAL-51** | Golden SnapshotReader Retry Differential | `architecture` (all) | SnapshotReader exact signature (${ino}:${mtimeNs}:${ctimeNs}:${size}), 32MB limit, format_version:1, parse failure never poisons signature | `Gate-Golden-SnapshotReader` | `BLOCKER-33` |

### 29.3 架构阻塞项台账 (Architecture Blockers Registry) (P0-121, P0-154, P0-155, P0-156)
*权威数据源：`docs/architecture/pet-rust/blocker-registry.json`*

<!-- GENERATED: blocker-summary -->
> **统计**：当前共注册 **33 项架构阻塞项（BLOCKER-01 ~ BLOCKER-33）**。所有 33 项均已在架构设计上实现闭环（`design_state: RESOLVED`），正处于等待实机验证状态（`validation_state: pending`）。

| 编号 | 标题 | 设计闭环状态 | 实证验证状态 | 范围 | 决策映射 | 解除前置 | 关联门禁 |
|---|---|---|---|---|---|---|---|
| **BLOCKER-01** | Wayland WebKitGTK 透明子表面事件穿透 | **RESOLVED** | pending | `backend` | `DEC-04` | `VAL-01` | `Gate-LayerShell-02` |
| **BLOCKER-02** | Layer-Shell 分数缩放与 Hit Region 不连续 | **RESOLVED** | pending | `backend` | `DEC-05` | `VAL-02` | `Gate-LayerShell-01` |
| **BLOCKER-03** | GNOME 45+ Mutter 跨小版本内部接口稳定性 | **RESOLVED** | pending | `backend` | `DEC-06` | `VAL-04` | `Gate-GnomeCompanion-02` |
| **BLOCKER-04** | Linux 多 GDK 会话与多 Seat 隔离 | **RESOLVED** | pending | `backend` | `DEC-09` | `VAL-07` | `Gate-Linux-GdkMultiSeat` |
| **BLOCKER-05** | Windows on ARM64 渲染基准与透明层叠 | **RESOLVED** | pending | `tuple` | `DEC-11` | `VAL-05` | `CERT-WIN11-ARM64` |
| **BLOCKER-06** | 权威规范产物完整性与 SHA256 强校验 | **RESOLVED** | pending | `architecture` | `DEC-08` | `VAL-27` | `Gate-Core-SpecIntegrity` |
| **BLOCKER-07** | Cargo muda 特性联合污染隔离 | **RESOLVED** | pending | `architecture` | `DEC-09` | `VAL-28` | `Gate-Linux-MudaTargetIsolation` |
| **BLOCKER-08** | gtk-layer-shell v0_6 特性链与 Linux 发布锁定 | **RESOLVED** | pending | `architecture` | `DEC-10` | `VAL-27` | `Gate-Linux-LayerShellV06` |
| **BLOCKER-09** | Layer-Shell 窗口 pre-realize 初始化生命周期 | **RESOLVED** | pending | `backend` | `DEC-04` | `VAL-21` | `Gate-LayerShell-05` |
| **BLOCKER-10** | GNOME AppID 传播与 Meta.Window 发现 | **RESOLVED** | pending | `backend` | `DEC-06` | `VAL-22` | `Gate-GnomeCompanion-04` |
| **BLOCKER-11** | 渲染端导航代际与 WebView 实例会话严格解耦 | **RESOLVED** | pending | `architecture` | `DEC-07` | `VAL-25` | `Gate-IPC-Generation` |
| **BLOCKER-12** | 调和状态机 AppliedState 异步生效竞态与陈旧防护 | **RESOLVED** | pending | `architecture` | `DEC-07` | `VAL-26` | `Gate-Core-Reconciliation` |
| **BLOCKER-13** | Wayland 工作区与 exclusive-zone 面板避让一致性 | **RESOLVED** | pending | `backend` | `DEC-04` | `VAL-24` | `Gate-LayerShell-06` |
| **BLOCKER-14** | GNOME 50 当前版本扩展兼容性实机认证 | **RESOLVED** | pending | `tuple` | `DEC-06` | `VAL-29` | `CERT-GNOME50-X64` |
| **BLOCKER-15** | macOS 26 (Tahoe) 架构与硬件分离认证 | **RESOLVED** | pending | `backend` | `DEC-11` | `VAL-30` | `CERT-MACOS26-ARM64` |
| **BLOCKER-16** | Secure Asset Protocol 产物截断防护与沙盒 TOCTOU 拦截 | **RESOLVED** | pending | `architecture` | `DEC-12` | `VAL-17` | `Gate-Asset-Sandbox` |
| **BLOCKER-17** | Golden source-set corruption | **RESOLVED** | pending | `architecture` | `DEC-01` | `VAL-32` | `Gate-Golden-ABI` |
| **BLOCKER-18** | Golden preload ABI information loss | **RESOLVED** | pending | `architecture` | `DEC-01` | `VAL-32` | `Gate-Golden-ABI` |
| **BLOCKER-19** | Invented 12-DIP snap behavior | **RESOLVED** | pending | `architecture` | `DEC-02` | `VAL-33` | `Gate-Golden-Bounds` |
| **BLOCKER-20** | Durable FIFO path/schema regression | **RESOLVED** | pending | `architecture` | `DEC-03` | `VAL-37` | `Gate-Golden-FIFO` |
| **BLOCKER-21** | ReadMD toggle-app semantics missing | **RESOLVED** | pending | `architecture` | `DEC-01` | `VAL-34` | `Gate-Golden-Differential` |
| **BLOCKER-22** | Windows Global mutex scope regression | **RESOLVED** | pending | `backend` | `DEC-11` | `VAL-41` | `Gate-Win32-SingleInstance` |
| **BLOCKER-23** | Tuple lifecycle state contradiction | **RESOLVED** | pending | `architecture` | `DEC-14` | `VAL-27` | `Gate-Tuple-Lifecycle` |
| **BLOCKER-24** | Final acceptance formula inverted | **RESOLVED** | pending | `architecture` | `DEC-15` | `VAL-27` | `Gate-Acceptance-Formula` |
| **BLOCKER-25** | Windows virtual-desktop false assumption | **RESOLVED** | pending | `backend` | `DEC-11` | `VAL-41` | `Gate-Win32-VirtualDesktop` |
| **BLOCKER-26** | Wayland input-region chicken-and-egg | **RESOLVED** | pending | `backend` | `DEC-05` | `VAL-39` | `Gate-LayerShell-02` |
| **BLOCKER-27** | Layer-Shell usable-area source undefined | **RESOLVED** | pending | `backend` | `DEC-04` | `VAL-31` | `Gate-LayerShell-06` |
| **BLOCKER-28** | Runtime signature trust-root model | **RESOLVED** | pending | `architecture` | `DEC-13` | `VAL-42` | `Gate-Crypto-DetachedSig` |
| **BLOCKER-29** | Validation/Gate referential-integrity failure | **RESOLVED** | pending | `architecture` | `DEC-08` | `VAL-27` | `Gate-Core-SpecIntegrity` |
| **BLOCKER-30** | Canonical spec integrity evidence absent | **RESOLVED** | pending | `architecture` | `DEC-08` | `VAL-27` | `Gate-Core-SpecIntegrity` |
| **BLOCKER-31** | Health File Ownership and Atomic Write Parity | **RESOLVED** | pending | `architecture` | `DEC-16` | `VAL-49` | `Gate-Golden-Health` |
| **BLOCKER-32** | Parent Liveness Immediate Exit vs Orchestrator Grace Period | **RESOLVED** | pending | `architecture` | `DEC-17` | `VAL-50` | `Gate-Golden-ParentDeath` |
| **BLOCKER-33** | SnapshotReader Exact Signature and Repaired Retry Differential | **RESOLVED** | pending | `architecture` | `DEC-18` | `VAL-51` | `Gate-Golden-SnapshotReader` |

### 29.4 规范一致性凭据快照 (Appendix: Integrity Evidence Snapshot) (P0-157)
<!-- GENERATED: integrity-evidence-snapshot -->
```json
{
  "spec_path": "docs/architecture/pet-rust/spec.md",
  "golden_commit_sha": "4dcfd73ce81a14ace7e429791e0594bea47b24e5",
  "linter_version": "v1.4.5",
  "registry_referential_integrity": true,
  "registered_gates_count": 71,
  "validation_items_count": 51,
  "open_blockers_count": 33,
  "closed_decisions_count": 18,
  "platform_tuples_count": 24
}
```

---

## 30. 规范审计历史记录
记录从 v1.0.0 至 v1.4.5 历次反向架构审计与整改历程。

---

## 31. 术语与概念定义表
对 DIP、WorkArea、Hit Region、Durable FIFO、Parent Liveness 等专有词汇建立不可篡改的统一定义。

---

## 32. 跨平台架构差异快速索引
直观对照 Windows、macOS、GNOME Wayland、LayerShell 在事件循环、窗口层级、透明穿透机制上的实现差异。

---

## 33. 常见陷阱与反模式排查手册
汇总 20 条在开发桌面浮窗过程中极易触犯的反模式与排查建议。

---

## 34. Phase 0 PoC 执行代码模板与命令指引
提供针对各项核心概念验证的孤立测试代码与验证指令。

---

## 35. 规范签署与一致性哈希锁定
本规范所有条款受自动化一致性检查器、反向负向测试用例套件及 SHA-256 哈希凭据共同保护。
