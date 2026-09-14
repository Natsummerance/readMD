# ReadMD Desktop Overlay v1.4.6 Evidence & Validation Closure Candidate

> **版本标识**：v1.4.6-Candidate
> **阶段说明**：Evidence & Validation Closure Candidate / Phase 0 Readiness Gate
> **架构冻结判定**：**NO — Production Architecture Freeze**（严禁进入正式生产代码实现）
> **Phase 1 Rust Host 生产实现状态**：**严格禁止**（在 Phase 0 物理硬件概念验证全数通过前严禁进入正式生产代码）
> **Phase 0 全量验证状态**：**BLOCKED**（允许 Phase 0 tooling/bootstrap、GoldenCapture、isolated backend spike 准备；但正式 PASS 需物理实机证据链）
> **前序版本溯源**：解决 v1.4.5 中遗留的 Artifact Provenance、Golden Build Closure、Validation Registry、Phase-0 Matrix 与 Evidence Classification 问题

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

### 1.1 黄金行为输入闭包 (Golden Behavioral Input Closure) (P0-172, P0-173, P0-174, P0-189)
本项目不再以固定主观的人工“核心文件数”定义基准，而是建立**黄金行为输入闭包 (Golden Behavioral Input Closure)**。
任何改变可观测桌面浮窗行为的源码、构建适配脚本、打包配置、运行时资产与依赖锁文件，均作为显式输入纳入机器注册表：
- **上游厂商仓库**：`https://github.com/NousResearch/hermes-agent`
- **上游固定提交 (Pinned Upstream Revision)**：`fb27614addac115d55299bc6538ae112fd01f688`（记录于 `third_party/hermes-agent-pet/UPSTREAM.md`，CI 实施强校验拦截）
- **黄金基准提交 (Golden Git Commit SHA)**：`47c40a38a60f27df369b60ed29dac4a4a037e3f6`
- **构建起源图谱 (Build Provenance Graph)**：`docs/architecture/pet-rust/golden-build-provenance.json`
- **机器可读契约源**：`docs/architecture/pet-rust/golden-contract.json`

<!-- GENERATED: golden-source-table -->
| 分类 | 路径 | 职责角色 | 文件大小 | 精确 SHA-256 哈希 | 行为关键 |
|---|---|---|---|---|---|
| **行为输入** | `third_party/hermes-agent-pet/apps/desktop/src/app/pet-overlay/pet-overlay-app.tsx` | Upstream Sprite click-through, double-click, and event handling | 6,185 B | `9b2c3d492d0f2adaaa4e2b7ac536e988dea1908208ee7a321fa23203b2c7357d` | **是** |
| **行为输入** | `packages/readmd-hermes-pet-adapter/scripts/build.mjs` | Build-time adaptation script patching single-click to open-menu and generating bundle | 4,661 B | `59fe50c2d31841aadef9013d2b47b107f49dac62434cebb5d3106bb97ca51db2` | **是** |
| **行为输入** | `packages/readmd-hermes-pet-adapter/src/electron-main.ts` | Main window lifecycle, context menu model, tray and clipboard capture | 14,929 B | `0a1b6473d155f8121d77d1463316a7968b0d973f76bb6080f4abb58de65a269d` | **是** |
| **行为输入** | `packages/readmd-hermes-pet-adapter/src/preload.ts` | Preload ABI context exposure (window.__HERMES_PET__) | 1,883 B | `fafeb3c1e5241efe3c25646f4ec1cb818ca46a17e375f85e3e16710963df1179` | **是** |
| **行为输入** | `packages/readmd-hermes-pet-adapter/src/bridge-transport.ts` | Durable FIFO queue and SnapshotReader atomic reader | 2,276 B | `7055deed1d644687fe8fc1a3adff39fba85185644903e334be6b502397100723` | **是** |
| **行为输入** | `packages/readmd-hermes-pet-adapter/src/renderer.tsx` | Frontend React mounting, state dispatch, and error boundary | 2,002 B | `5bbbd06c222c572d75b68b10bb09e910a5e02e6f1e89475a9811d0934dccaa36` | **是** |
| **行为输入** | `packages/readmd-hermes-pet-adapter/src/live2d/stage.ts` | Live2D stage hit test and bounds checking | 19,525 B | `bec994ed0a299fd7f05156f54cef6fa06da750f96f6f931a547313bd3e64522a` | **是** |
| **行为输入** | `third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc.ts` | Upstream pet IPC protocol and window click-through control | 5,824 B | `5c99fce416fece34d0fb66fdb662af0fb0169b9c4e8aae71977f9a46ac171d8d` | **是** |
| **行为输入** | `src/readmd_modules/pet/hermes_adapter.py` | Python host orchestrator, lifecycle management, and FIFO response | 33,603 B | `2a2f09188d3f9f6f52ac9a0a0571d3a94eaf2e385949184ef24058f3ec5b03ee` | **是** |
| **构建输入** | `packages/readmd-hermes-pet-adapter/vite.config.mjs` | Vite bundling configuration for renderer | 1,358 B | `8a5ed317a59a567fa8660e887d3b4e6067375583f85cb6072f7090cfae7c17db` | **是** |
| **构建输入** | `packages/readmd-hermes-pet-adapter/package.json` | Dependencies, build scripts, and package metadata | 709 B | `ee63a91062219ea13672d4440246745f5eb821573a3007b30a7385e857780600` | **是** |
| **构建输入** | `packages/readmd-hermes-pet-adapter/package-lock.json` | Dependency lockfile ensuring reproducible toolchain | 120,446 B | `c354caaed6f277fbddad21b6e27d6e2de961f6cefa9a397f3e213a9ecf3efb4f` | 否 |
| **生成产物** | `packages/readmd-hermes-pet-adapter/dist/electron-main.cjs` | Production bundled Electron main host | 20,120 B | `b5747182b5e883e2e89aad869affbd3a7d3a7b8f70642a4997d757adb569f921` | **是** |
| **生成产物** | `packages/readmd-hermes-pet-adapter/dist/preload.cjs` | Production bundled Preload script | 3,014 B | `50d95c7d2b62f3ba3198597c0a73cb53d4fa6c89d36534d59268a3a0fb8c08e0` | **是** |
| **生成产物** | `packages/readmd-hermes-pet-adapter/dist/renderer/index.html` | Production bundled Renderer HTML entry | 536 B | `d9f1da3457bac312a2790d270f6052de02c87290a19d623ee5d23bfb378ccb9d` | **是** |
| **运行资产** | `packages/readmd-hermes-pet-adapter/assets/hermes-sprite.png` | Default Hermes character sprite sheet | 180,556 B | `a5661b457de00b9a57570effcb7a3ecb8f6cb960b48c6633987a32542f2f58e0` | **是** |
| **运行资产** | `packages/readmd-hermes-pet-adapter/assets/mochi-sprite.png` | Mochi character sprite sheet | 171,542 B | `6e03b6065b5790b9ec860f13edcf930c902930456e5be5915b264eb225c68c01` | 否 |
| **运行资产** | `packages/readmd-hermes-pet-adapter/assets/moss-sprite.png` | Moss character sprite sheet | 165,431 B | `088f67906646a79d1bf8232d8bce324d5b000185ba52dcf12853c9ccd4a99af6` | 否 |
| **运行资产** | `packages/readmd-hermes-pet-adapter/assets/amber-sprite.png` | Amber character sprite sheet | 174,921 B | `fe9a84570db99898b20ac40e31012228b4959c84b636c166447a15f5cfa26bab` | 否 |
| **运行资产** | `packages/readmd-hermes-pet-adapter/dist/models/arch-chan/arch chan model0.model3.json` | Arch-Chan Live2D model definition | 689 B | `5d1a05c2eadba5296d2e26f67da5369191a9ca0d124a5e9fcbb142dff578679c` | **是** |
| **辅助证据** | `packages/readmd-hermes-pet-adapter/src/pet-life.ts` | Pet life companion state machine and attribute constants | 12,123 B | `21ef9bf62592d4d00a3b99cd3fd6f50944245fb26ac1f51cd3f3ad2db20d1dd5` | 否 |
| **辅助证据** | `third_party/hermes-agent-pet/UPSTREAM.md` | Upstream provenance documentation and revision record | 966 B | `35076f31d0ea6d70df494d6b2fab258d6348a24706e707c373629fea75d51110` | 否 |

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
2. **Sprite 判定合约 (`third_party/.../pet-overlay-app.tsx` L132-L165)** (P0-160, P0-173, P0-188)：
   - 使用 `document.elementFromPoint(x, y)` 检测拾取目标；
   - 若拾取目标不在宠物根容器 `petRef` 内，判定为透明穿透区域（返回 `false`）；
   - 若拾取目标为非 Canvas 交互 DOM 元素（如对话气泡 `PetBubble`、未读邮件图标 `Mail`、弹出式输入框 `composer`），直接信任 DOM 命中测试（返回 `true`）；
   - 若拾取目标为 `HTMLCanvasElement`，则获取 2D 上下文并在对应纹理坐标处进行像素采样：
     $$\text{Solid Pixel} \iff \text{ctx.getImageData}(px, py, 1, 1).\text{data}[3] \ge 16 \quad (\text{ALPHA\_HIT\_THRESHOLD} = 16)$$
   - 若 Canvas 受到污染（Tainted）或读取抛出异常，执行安全打开策略（fail-open，返回 `true`），确保桌宠依然可被鼠标抓取；
   - **构建起源绑定 (P0-188)**：Sprite 原生源码经由 `packages/readmd-hermes-pet-adapter/scripts/build.mjs` 进行单次点击行为适配（单次点击触发 `open-menu`，双击保持 `toggle-app`），构建产物哈希与原始源码哈希在 `golden-build-provenance.json` 中完整绑定。

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

### 1.8 宿主感知渲染层崩溃自愈与健康度报告 (P0-99, P0-149, P0-150, P0-186, P0-187)
严格还原 `electron-main.ts` 第 165-172 行原生自愈时序。**主体责任界定 (P0-187)**：渲染进程崩溃属于**宿主进程直接观测事件 (Host-Observed Failure)**。在 Electron 中由主进程监听 `render-process-gone` 并由主进程自身执行 `reportHealth(...)`；在 Rust 宿主中由主事件循环监听 WebView 进程终止事件并向 `<bridge>.rust.health.json` 写入故障报告，渲染层沙箱绝无直接写盘权限。

```typescript
// 冻结原生 Golden 自愈逻辑 (P0-186)
recoveries = recoveries.filter(t => now - t < 60_000)
reportHealth("failed", renderer, "pet_renderer_crashed")

if (recoveries.length >= 3) {
    return // 过去 60 秒内已有 3 次自愈记录，触发熔断，停止重载
}

recoveries.push(now)
delay = 500 * recoveries.length
scheduleReload(delay)
```

- **边界判定与执行阶梯矩阵 (P0-186)**：
  - **第 1 次崩溃**：检测前 `recoveries.length == 0`（未达熔断阈值 3），记录当前时间戳（长度变为 1），延时 `500 * 1 = 500ms` 执行重载；
  - **第 2 次崩溃**：检测前 `recoveries.length == 1`（未达阈值 3），记录当前时间戳（长度变为 2），延时 `500 * 2 = 1000ms` 执行重载；
  - **第 3 次崩溃**：检测前 `recoveries.length == 2`（未达阈值 3），记录当前时间戳（长度变为 3），延时 `500 * 3 = 1500ms` 执行重载；
  - **同一 60 秒内第 4 次崩溃**：检测前 `recoveries.length == 3`（`recoveries.length >= 3` 成立），立即触发熔断并 `return`，**不记录时间戳、不执行任何延时重载**，保持静默故障状态。

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

### 4.4 GNOME 桌面环境强约束禁止调用 (P0-179)
GNOME Wayland (Mutter) 在架构上不支持 `zwlr_layer_shell_v1` 协议。**严禁在 GNOME 桌面环境下将任何窗口或图层绑定到 LayerShellBackend**，任何在 GNOME Wayland 下尝试加载或调用 `gtk_layer_shell` 的行为均视为严重架构违规。
- **LayerShellBackend** 严格限定于支持 Layer-Shell 的合成器：KDE Plasma 6 (KWin Wayland)、Sway、Hyprland、Deepin Treeland；
- **GNOME Wayland** 必须且只能绑定 **GnomeCompanionBackend**（借助 Companion 扩展与 Mutter 通信）。

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

### 10.2 Golden SnapshotReader 契约 (P0-142, P0-143)
还原 `bridge-transport.ts` 第 31-53 行原生实现规范：
- **最大快照限制**：`MAX_BYTES = 32 * 1024 * 1024`（32 MiB）；
- **Golden 签名格式**：
  `${stat.ino}:${stat.mtimeNs}:${stat.ctimeNs}:${stat.size}`
- **Windows 等价实现说明 (P0-143)**：Node.js Golden 在各平台统一通过 `fs.promises.stat(..., {bigint: true})` 暴露上述属性。Rust Windows 实现若采用 `volume_serial:file_index:last_write_time:size`，属于等价平台实现，必须通过 `VAL-51` 差异测试证明行为完全一致。
- **执行流程与自愈重试**：
  1. 若当前正在读取中（`reading === true`），立即返回 `undefined`；
  2. 读取文件元数据，若非正规文件或体积大于 32 MiB，抛出 `invalid_pet_snapshot`；
  3. 计算当前签名，若与上次成功签名 `this.signature` 相同，立即返回 `undefined`；
  4. 读取 UTF-8 文本并执行 `JSON.parse`；
  5. 校验快照结构：必须为非 null 对象、非数组，且满足 `format_version === 1`；
  6. **关键规则**：只有在解析与校验完全成功后，才更新 `this.signature = signature` 并返回快照对象；
  7. **任何读取、解析或校验失败均不得更新签名缓存**，确保快照文件被修复后可在下一个轮询时隙立即重试。

---

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
```rust
let broken = true;
