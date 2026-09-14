# ReadMD Desktop Overlay（桌面桌宠）全平台重构执行规格
## —— v1.4.4 Contract Restoration Candidate (Golden Contract & Pure Architecture)

> **版本**：v1.4.4-Candidate (Contract Restoration Candidate)  
> **状态说明**：本版本为 Contract Restoration Candidate，绝非 Production Architecture Freeze。所有平台认证元组（Platform Tuples T-01 ~ T-23）状态严格保持为 **Planned**（等待 Phase 0 PoC 实测证据产生后方可进入 Candidate）。  
> **核心使命**：彻底恢复被前序版本压缩与虚构的真实 Golden Contract，机器级绑定 7 大核心源码及 SHA-256，确立机器可读注册表，消除一切推测性 API 假设与虚构吸附逻辑。  
> **唯一黄金参考源（Golden Reference）**：当前生产 Windows Electron 完整实现 (`packages/readmd-hermes-pet-adapter`)、第三方底层 IPC 原型 (`third_party/hermes-agent-pet`) 与 Python 宿主控制层 (`src/readmd_modules/pet/hermes_adapter.py`)。  
> **重构四大核心原则**：  
> 1. **全平台真机原生支持**：严禁通过将不支持的平台简单降级为 In-App 网页阅读器内部元素来伪装支持。为 Windows、macOS、Linux X11/XWayland、KDE/wlroots 原生 Wayland、GNOME 原生 Wayland 提供明确、可验证的原生能力路径。  
> 2. **真实生态事实为唯一标准**：严禁虚构任何底层 API 与 crate 特性；统一锁定 **Rust 1.85.0**、`wry 0.57.0` (开启 `os-webview`)、`tao 0.37.0`、`muda 0.19.3` (按目标隔离，Linux 禁用默认 `libxdo`)、`gtk-layer-shell 0.8.2` (显式激活 `v0_6`)。  
> 3. **不可篡改的黄金行为基准（Golden Contract Immutable）**：重构目标是 Rust 宿主 100% 行为等价还原现有 Electron 表现，严禁为适应 Rust 实现而反向修改现有 Python 契约与 Renderer 假设。  
> 4. **统一平台后端族（Platform Backend Family）**：单一 Tao 无法应对全平台差异，明确 `OverlayWindowBackend` 统一抽象，各平台提供 Win32、Cocoa、X11、Layer-Shell 及 GNOME Companion 专属后端实现。

---

## 目录
1. [0. 现实生态事实与架构原则](#0-现实生态事实与架构原则)
2. [1. Golden Contract 权威行为源与基准契约](#1-golden-contract-权威行为源与基准契约)
3. [2. 引擎编排器 (Engine Orchestrator) 与产品交互模型](#2-引擎编排器-engine-orchestrator-与产品交互模型)
4. [3. 统一平台后端族 Platform Backend Family 架构](#3-统一平台后端族-platform-backend-family-架构)
5. [4. Native Wayland Layer-Shell 后端 (KDE / wlroots)](#4-native-wayland-layer-shell-后端-kde--wlroots)
6. [5. GNOME Wayland 专属后端：ReadMD GNOME Shell Companion](#5-gnome-wayland-专属后端readmd-gnome-shell-companion)
7. [6. Wayland Input Model 主动推送几何模型](#6-wayland-input-model-主动推送几何模型)
8. [7. 黄金交互区 Golden Hit Region 原则与交互边界](#7-黄金交互区-golden-hit-region-原则与交互边界)
9. [8. ReadMD Pet IPC v1：会话解耦与 WRY 官方 API 对齐](#8-readmd-pet-ipc-v1会话解耦与-wry-官方-api-对齐)
10. [9. 完整安全资产协议 (Secure Asset Protocol)](#9-完整安全资产协议-secure-asset-protocol)
11. [10. 权威 Durable FIFO 字节级协议规范](#10-权威-durable-fifo-字节级协议规范)
12. [11. Native Drag & Drop：WRY 0.57 官方路径重构解耦](#11-native-drag--dropwry-057-官方路径重构解耦)
13. [12. CSP 策略与资产沙盒边界](#12-csp-策略与资产沙盒边界)
14. [13. 跨平台安全 ClipboardService 规范](#13-跨平台安全-clipboardservice-规范)
15. [14. 实例 Scope 与用户 Profile 隔离](#14-实例-scope-与用户-profile-隔离)
16. [15. Parent Liveness Pipe 继承模型与深度验证](#15-parent-liveness-pipe-继承模型与深度验证)
17. [16. 权威状态调和模型 (Reconciliation State Model)](#16-权威状态调和模型-reconciliation-state-model)
18. [17. Renderer 后台探测路径与健康所有权隔离](#17-renderer-后台探测路径与健康所有权隔离)
19. [18. 权威代码与构建规范 (Authoritative Cargo Specification)](#18-权威代码与构建规范-authoritative-cargo-specification)
20. [19. 性能协议：阶段性时间与内存预算](#19-性能协议阶段性时间与内存预算)
21. [20. 平台认证元组规范 (Platform Certification Tuples)](#20-平台认证元组规范-platform-certification-tuples)
22. [21. 动态门禁架构模型 (Dynamic Gate Composition Model)](#21-动态门禁架构模型-dynamic-gate-composition-model)
23. [22. Fullscreen 与虚拟桌面跨平台规范](#22-fullscreen-与虚拟桌面跨平台规范)
24. [23. 运行时分发与权威 A/B 升级 (PetRuntimeInstallerV2)](#23-运行时分发与权威-ab-升级-petruntimeinstallerv2)
25. [24. 供应链安全、签名与权限边界](#24-供应链安全签名与权限边界)
26. [25. Release Stop Conditions 严格红线](#25-release-stop-conditions-严格红线)
27. [26. Codex 实施执行协议与 Golden 差异测试套件](#26-codex-实施执行协议与-golden-差异测试套件)
28. [27. 重构验收判断标准](#27-重构验收判断标准)
29. [28. Platform Capability Matrix 综合平台能力矩阵](#28-platform-capability-matrix-综合平台能力矩阵)
30. [29. 架构决策与实机实证项注册表 (Empirical Validation Register)](#29-架构决策与实机实证项注册表-empirical-validation-register)
31. [30. Real Hardware Certification Matrix 实机硬件认证矩阵](#30-real-hardware-certification-matrix-实机硬件认证矩阵)
32. [31. Change Log (v1.3.0 -> v1.4.0 演进)](#31-change-log-v130---v140-演进)
33. [32. Change Log (v1.4.0 -> v1.4.1 Freeze Candidate 演进)](#32-change-log-v140---v141-freeze-candidate-演进)
34. [33. Change Log (v1.4.1 -> v1.4.2 Freeze Candidate 演进)](#33-change-log-v141---v142-freeze-candidate-演进)
35. [34. Change Log (v1.4.2 -> v1.4.3 Architecture Freeze Candidate 演进)](#34-change-log-v142---v143-architecture-freeze-candidate-演进)
36. [35. Change Log (v1.4.3 -> v1.4.4 Contract Restoration Candidate 演进)](#35-change-log-v143---v144-contract-restoration-candidate-演进)
37. [附录 A：验证关键系统底层实现](#附录-a验证关键系统底层实现)
38. [附录 B：Phase 0 实施执行清单与 ADR-0001 草案](#附录-bphase-0-实施执行清单与-adr-0001-草案)

---

## 0. 现实生态事实与架构原则

### 0.1 核心依赖与 MSRV 事实约束 (P0-55, P0-56, P0-57, P0-58)
1. **Rust 编译工具链基准**：MSRV 锁定为 **Rust 1.85.0**（2021 Edition）。
2. **WRY 0.57.0 约束**：
   - 必须显式激活 `features = ["os-webview"]`。
   - 严禁在 new 中传入窗口指针引用（如已废弃的 `new(&window)` 虚构模式），统一采用 `WebViewBuilder::new()` 链式构建后调用 `.build(&window)`。
   - 自定义拖放事件枚举 `DragDropEvent` 包含 `Enter`、`Over`、`Drop`、`Leave` 四种变体，**严禁使用虚构的 `Hover`**。
3. **muda 0.19.3 特性联合污染隔离 (P0-55)**：
   - muda 0.19.3 默认开启 `libxdo` 特性，在 Linux 上会导致动态链接 `libxdo.so.3`。
   - **架构强制约束**：禁止在通用 `[dependencies]` 中声明 `muda`；必须按目标平台在 `[target.'cfg(...)'.dependencies]` 中分别声明。Linux 目标必须显式声明 `muda = { version = "=0.19.3", default-features = false, features = ["gtk"] }`。
4. **gtk-layer-shell 0.8.2 特性链锁定 (P0-57)**：
   - `is_supported()` 需要 feature `v0_5`，`protocol_version()` 需要 feature `v0_6`。
   - Linux 构建必须配置：`gtk-layer-shell = { version = "=0.8.2", default-features = false, features = ["v0_6"], optional = true }`。
   - 生产发布特性统一定义：`linux-production = ["wayland-layer-shell", "gnome-companion"]`。
5. **macOS objc2 生态版本统一 (P0-56)**：
   - 锁定 `objc2 = "0.6"`, `objc2-app-kit = "0.3"`, `objc2-foundation = "0.3"`，彻底根除老旧 `objc 0.2` 的符号与运行时冲突。

---
## 1. Golden Contract 权威行为源与基准契约

### 1.1 黄金行为源集合与 Git 提交哈希绑定 (P0-91, P0-103)
本重构规格唯一承认的黄金行为基准（Golden Behavior Set）由且仅由以下 7 个核心源码文件构成。规格已与代码库当前提交建立不可篡改的机器级绑定：
- **Golden Git Commit SHA**：`4dcfd73ce81a14ace7e429791e0594bea47b24e5`
- **机器可读契约定义**：`docs/architecture/pet-rust/golden-contract.json`

| 序号 | 核心行为源码文件路径 | 承担的核心契约职责 | 文件大小 | 精确 SHA-256 哈希 |
|---|---|---|---|---|
| **1** | `packages/readmd-hermes-pet-adapter/src/electron-main.ts` | 宿主生命周期、clampBounds 计算、右键菜单模型、恢复基线、剪贴板转发 | 14,929 B | `0a1b6473d155f8121d77d1463316a7968b0d973f76bb6080f4abb58de65a269d` |
| **2** | `packages/readmd-hermes-pet-adapter/src/preload.ts` | 隔离上下文桥接、hermesDesktop 与 readmdPet 命名空间导出及方法签名 | 1,883 B | `fafeb3c1e5241efe3c25646f4ec1cb818ca46a17e375f85e3e16710963df1179` |
| **3** | `packages/readmd-hermes-pet-adapter/src/bridge-transport.ts` | 原子快照读取器 (SnapshotReader)、有界持久化 FIFO 命令发布器 | 2,276 B | `7055deed1d644687fe8fc1a3adff39fba85185644903e334be6b502397100723` |
| **4** | `packages/readmd-hermes-pet-adapter/src/renderer.tsx` | 渲染入口挂载、舞台切换 (live2d vs sprite)、就绪与失败状态上报 | 2,002 B | `5bbbd06c222c572d75b68b10bb09e910a5e02e6f1e89475a9811d0934dccaa36` |
| **5** | `packages/readmd-hermes-pet-adapter/src/live2d/stage.ts` | Live2D Pixi 舞台、hitTest 与 bounds 命中测试、动效循环 | 19,525 B | `bec994ed0a299fd7f05156f54cef6fa06da750f96f6f931a547313bd3e64522a` |
| **6** | `third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc.ts` | 原始 IPC 处理器（open、set-bounds 带临时 resizable 切换、ignore-mouse） | 5,824 B | `5c99fce416fece34d0fb66fdb662af0fb0169b9c4e8aae71977f9a46ac171d8d` |
| **7** | `src/readmd_modules/pet/hermes_adapter.py` | Python 控制层适配器、桥接状态写入、FIFO 命令队列消费、健康观察 | 33,603 B | `2a2f09188d3f9f6f52ac9a0a0571d3a94eaf2e385949184ef24058f3ec5b03ee` |

**支持性证据文件（Supporting Evidence，不作为独立行为判定源）**：
- `packages/readmd-hermes-pet-adapter/package.json` (709 B, `ee63a91062219ea13672d4440246745f5eb821573a3007b30a7385e857780600`)
- `packages/readmd-hermes-pet-adapter/src/pet-life.ts` (12,123 B, `21ef9bf62592d4d00a3b99cd3fd6f50944245fb26ac1f51cd3f3ad2db20d1dd5`)

**黄金契约变更管理与重录流程 (P0-103)**：
区分“迁移基准（Migration Golden Baseline）”与“当前产品源码（Current Product Source）”。若未来 Electron 代码发生合法变更导致上述文件哈希改变，CI 迁移门禁将拦截报警，但绝不影响主程序常规构建。解除条件为触发权威重录流程：人工与 API 联合审阅变更差异 $	o$ 重新生成 `golden-contract.json` $	o$ 更新 Rust 差异测试 Fixture $	o$ 运行差异对比门禁 $	o$ 签署并提交新的 `golden_commit_sha`。

### 1.2 权威 Preload ABI 规范 (P0-92)
渲染端通过 `contextBridge` 访问的接口必须 100% 逐字义还原，严禁使用摘要式模糊定义。Rust 注入的兼容层脚本必须提供完全一致的调用签名：

```typescript
interface Window {
  hermesDesktop: {
    petOverlay: {
      // 异步调用：调用宿主打开悬浮窗，返回操作结果及解析后的屏幕坐标
      open(request?: { bounds?: unknown; screen?: boolean }): Promise<{ ok: boolean; bounds?: unknown }>;

      // 异步调用：关闭悬浮窗
      close(): Promise<{ ok: boolean }>;

      // 同步调用：通知宿主设置窗口物理边界
      setBounds(bounds: { x: number; y: number; width: number; height: number }): void;

      // 同步调用：切换鼠标事件穿透状态
      setIgnoreMouse(ignore: boolean): void;

      // 同步调用：切换窗口是否可获取焦点
      setFocusable(focusable: boolean): void;

      // 同步调用：渲染端向宿主回推自身状态
      pushState(payload: unknown): void;

      // 同步调用：控制信令派发（必须为单个 payload 对象，严禁改为多参数签名）
      control(payload: { type: string; [key: string]: unknown }): void;

      // 事件监听：监听宿主推送的状态，必须返回取消订阅函数 () => void
      onState(callback: (payload: unknown) => void): () => void;

      // 事件监听：监听宿主推送的控制信令，必须返回取消订阅函数 () => void
      onControl(callback: (payload: unknown) => void): () => void;
    };
  };

  readmdPet: {
    // 同步调用：从原生拖放事件注入文件路径（最多 128 个文件）
    dropFiles(files: File[]): void;
  };
}
```

**关键 ABI 语义红线**：
1. `control(payload)` 必须严格接收单一 payload 对象，严禁设计为 `control(action, payload)`。
2. `onState` 和 `onControl` 必须返回一个无参取消订阅函数 `() => void`。
3. 同步触发方法（`setBounds`, `setIgnoreMouse`, `setFocusable`, `pushState`, `control`, `dropFiles`）与 Promise 异步方法（`open`, `close`）严禁混淆互换。

### 1.3 窗口尺寸策略解耦：彻底删除虚构吸附 (P0-93, P0-94)
**严正声明**：生产 Electron 源码中**根本不存在任何拖拽自动贴边吸附机制（彻底剔除历史版本出现的 12 DIP 离手吸附伪设定）**。前序版本出现的“12 DIP 离手吸附”系编造内容，必须从规范中彻底剔除！

真实 Golden 实现包含两个完全解耦的边界策略：
1. **宿主快照边界策略 (HostSnapshotBoundsPolicy)**：
   - 源码来源：`packages/readmd-hermes-pet-adapter/src/electron-main.ts` 中的 `clampBounds` 函数。
   - 宽度范围：`Math.max(240, Math.min(640, Math.round(Number(input.width) || 300)))`。
   - 高度范围：`Math.max(300, Math.min(720, Math.round(Number(input.height) || 420)))`。
   - 默认坐标：`x = 72`, `y = 72`。
   - **工作区合法性约束**：遍历系统所有显示器，窗口矩形必须在至少一个 `display.workArea` 内满足边缘重叠 $\ge 40	ext{ DIP}$：
     ```typescript
     x + width >= dx + 40 &&
     x <= dx + dw - 40 &&
     y + height >= dy + 40 &&
     y <= dy + dh - 40
     ```
   - **离屏退让机制 (Offscreen Fallback)**：若完全脱离所有工作区，移动至主显示器右下角安全区域：
     ```typescript
     primary = screen.getPrimaryDisplay().workArea;
     x = primary.x + Math.max(12, primary.width - width - 24);
     y = primary.y + Math.max(12, primary.height - height - 24);
     ```
     *注：公式中的 12 与 24 是主显示器边界的安全留白边距（Margin），绝对不是拖拽吸附检测阈值！*
2. **渲染端交互式边界策略 (RendererInteractiveBoundsPolicy)**：
   - 源码来源：`third_party/hermes-agent-pet/apps/desktop/electron/pet-overlay-ipc.ts` 中的 `set-bounds` 处理器。
   - 限制下限：`width >= 80`, `height >= 80`。
   - **尺寸切换原子锁**：当请求尺寸与当前窗口不一致时，必须执行临时可调整开关切换：
     ```typescript
     const resizing = width !== curW || height !== curH;
     if (resizing && !win.isResizable()) win.setResizable(true);
     win.setBounds({ x: Math.round(bounds.x), y: Math.round(bounds.y), width, height });
     if (resizing) win.setResizable(false);
     ```

### 1.4 Golden Hit Region 真实判定与测试模型 (P0-95, P0-112)
彻底废除“交互多边形”等简化假设，严格还原 Live2D 与 Sprite 的真实判定逻辑：
1. **Live2D 判定契约 (stage.ts L128-L129)**：
   ```typescript
   const areas = typeof model.hitTest === 'function' ? model.hitTest(x, y) : [];
   return (Array.isArray(areas) && areas.length > 0) || Boolean(model.getBounds?.().contains(x, y));
   ```
   **核心逻辑**：只要满足 `model.hitTest(x, y)` 非空 **或者** `model.getBounds().contains(x, y)` 为真，即判定为命中！
   由于 `getBounds()` 覆盖了整个可视轮廓，绝大部分有效交互均被外接矩形覆盖。Phase 0 必须输出 `Live2D Golden Region Complexity Report`，决定底层最佳表达形式。
2. **Sprite 判定契约**：
   依据渲染端 DOM 矩形或 Canvas 激活区域计算，光标悬浮在精灵图有效帧区域内时开启鼠标捕获。
3. **确定性测试网格 (Deterministic Fixture Grid)**：
   在 `golden/hit-region/` 下建立包含内部网格点、精确轮廓边界点、外部非命中点的机器验证集。

### 1.5 ReadMD 适配层拓扑特有的 `toggle-app` 语义 (P0-96)
在 vendor Hermes 原生实现中，`toggle-app` 信令用于切换主窗口显示与最小化。但在 ReadMD 生产适配层中，注册 IPC 时显式传入了 `getMainWindow: () => null`，并独立拦截了 `toggle-app` 信令：
```typescript
else if (type === 'toggle-app') writeCommand(clipboardCommand());
```
**权威规范冻结**：
- 当渲染端派发 `control({ type: "toggle-app" })` 时，Rust 宿主必须读取系统当前剪贴板，并将提取的数据打包写入持久化 FIFO 命令队列：
  ```json
  {
    "type": "clipboard",
    "text": "...",
    "image_png": "...",
    "paths": []
  }
  ```
- **架构红线**：严禁将 `toggle-app` 误实现为最小化或显示宿主窗口！本次重构的黄金基准是 ReadMD 外部适配层拓扑，而非 vendor 独立拓扑。

### 1.6 右键上下文菜单 Golden Contract (P0-97)
右键菜单必须精确保持现有生产顺序与使能条件，严禁擅自增删菜单项：
1. **Header 状态展示**：`Level N · Energy N · Mood N`（固定 `enabled = false`）。
2. **Separator 分隔线**。
3. **互动动作序列**：
   - `Pet`（摸摸）：无冷却时可用。
   - `Feed`（喂食）：无冷却时可用。
   - `Play`（玩耍）：**仅当 `energy >= 10` 且无冷却时可用**。
   - `Rest`（休息）：激活状态显示；若桌宠正处于休息状态，则文字自动变为 `Wake up`（唤醒）；无冷却时可用。
4. **Separator 分隔线**。
5. **角色列表 (Characters)**：单选菜单列表（Radio items），最多展示 128 个角色，选中项与当前装载角色严格对齐。
6. **Separator 分隔线**。
7. **`Open reader`**：触发打开主阅读器窗口。
*红线：严禁在未经过产品 ADR 审批前加入 "Hide pet"、"Quit"、"Settings" 等非黄金契约项。*

### 1.7 剪贴板读取 Golden 边界与容量上限 (P0-98)
当触发剪贴板抓取（如 `toggle-app` 或拖入文本）时，必须严格遵守以下截断边界：
- **文本内容 (text)**：`clipboard.readText().slice(0, 4 * 1024 * 1024)`，最大截取 4M 字符。
- **图像内容 (image_png)**：将原生图像转为 PNG 并编码为 Base64 字符串，截取最大 **24 * 1024 * 1024 字符**（*注：这是 Base64 字符串长度切片，绝非 24MB 裸二进制字节！*）。
- **文件路径 (paths)**：
  - Windows：读取 `FileNameW` 缓冲区，按 UTF-16LE 解码，使用 `\0` 拆分并过滤空值，最多截取 128 个路径。
  - macOS / Linux：通过对应的原生 URI / 文件剪贴板协议读取，过滤后最多保留 128 个路径。
- **下发信令结构**：
  ```json
  {
    "type": "clipboard",
    "text": "...",
    "image_png": "...",
    "paths": ["..."]
  }
  ```

### 1.8 渲染端异常恢复黄金基线 (Renderer Recovery Baseline, P0-99)
生产 Electron 针对渲染进程崩溃（`render-process-gone`）的恢复基线如下：
1. 若正在主动关闭或崩溃原因为正常退出（`clean-exit`），则直接忽略。
2. 记录崩溃时刻至恢复窗口数组，过滤保留过去 60 秒内的记录。
3. **熔断阈值**：若 60 秒内崩溃恢复次数已达到 **3 次**，彻底放弃重启，向健康文件写入 `pet_renderer_crashed` 失败标记。
4. **渐进退避**：若未达熔断阈值，安排延迟重载当前页面，延迟时间为 `500ms * recoveries.length`。
*Rust 架构可以在此基线之上实现更先进的断路器（Circuit Breaker），但其向 Python 暴露的可观察行为必须完全兼容。*

### 1.9 Fallback 精灵图权威元数据 (P0-125)
当无网络或模型加载失败时，内置备用精灵图的渲染参数必须严格一致：
```json
{
  "frameH": 512,
  "frameW": 384,
  "framesPerState": 4,
  "mime": "image/png",
  "scale": 0.33,
  "spritesheetRevision": "hermes-fallback-a5661b457de00b9a",
  "stateRows": ["idle", "wave"]
}
```

### 1.10 Bridge 轮询时序与 `pushState` 防回弹语义 (P0-126, P0-127, P0-128)
1. **权威轮询判定时序**：
   `读取 snapshot.json` $	o$ `检查父进程存活` $	o$ `规范化状态` $	o$ `若 visible === false 且 fullscreen !== true 则关闭窗口并退出` $	o$ `解析目标 renderer` $	o$ `若窗口不存在则打开` $	o$ `比对 lastHostBounds 应用新 bounds` $	o$ `若 fullscreen === true 则 hide 否则 showInactive` $	o$ `若 renderer 变更则 reload 页面并返回` $	o$ `执行 pushState`。
2. **`pushState` 防回弹关键设计 (Anti-Snapback)**：
   在向渲染端推送 `pushState(payload)` 前，宿主必须调用原生 `overlay.getBounds()` 获取当前物理窗口的实际坐标，并强制覆盖 payload 中的 bounds 字段。这样拖拽松手后，渲染端收到的永远是当前已经物理生效的坐标，彻底消除坐标回跳 Bug。
3. **全屏状态与可见性边缘判定 (P0-128)**：
   当 `visible === false` 且 `fullscreen === true` 时，窗口执行隐藏（`hide`）保持后台实例，绝不销毁重建。

---
## 2. 引擎编排器 (Engine Orchestrator) 与产品交互模型

### 2.1 面向普通用户的极简交互模型 (P0-104)
在 ReadMD 主程序设置面板中，面向普通终端用户的配置界面必须保持绝对简洁，彻底隐藏底层技术实现：
- **普通用户界面选项**：
  ```text
  桌面桌宠运行位置：
  ○ 阅读器内 (In-App)
  ○ 独立桌面 (Standalone Desktop)
  ```
- **底层引擎选择逻辑**：
  - 内部配置项固定为 `pet.runtime.engine = "auto"`。
  - 严禁在普通用户界面展示任何有关 "Rust"、"Electron"、"WRY"、"Layer-Shell" 或 "GNOME Companion" 的单选项！
  - 仅在主程序进入“开发者模式（Developer Mode）”或“故障诊断（Diagnostics）”时，才提供调试下拉菜单：
    `[ Auto (推荐) | Rust (Candidate) | Electron (Legacy) | Force In-App ]`。

### 2.2 本地优先的诊断事件原则 (P1-131)
- 当发生引擎降级、异常崩溃或健康报警时，编排器默认**仅记录本地诊断日志文件与诊断事件（Local Diagnostics Events）**。
- 严禁借由 Rust 引擎重构引入任何隐式的未经授权的网络遥测（Telemetry）上报。所有网络级数据同步必须严格继承主程序已有的隐私协议与用户显式授权。

### 2.3 确定性子进程生命周期管理 (P0-64)
- 严禁编排器使用 `kill_processes_by_target` 遍历全系统同名进程。
- 必须通过启动时记录的 Exact PID，在 Windows 上绑定 JobObject，在 Linux 上使用 `pidfd` 进行精确生命周期管理。

---
## 3. 统一平台后端族 Platform Backend Family 架构

各平台窗口服务器的底层拓扑差异极大，架构定义 `OverlayWindowBackend` 统一接口：
- **Win32Backend**：Windows 10/11，DirectComposition / DWM 透明穿透分层。
- **CocoaBackend**：macOS 13~26，NSWindow CollectionBehavior，Spaces 随同与无标题栏穿透。
- **X11Backend**：Linux X11 / XWayland，XShape / XFixes 输入穿透与 EWMH 状态。
- **LayerShellBackend**：KDE Plasma / wlroots 原生 Wayland，`zwlr_layer_shell_v1` Overlay 层级。
- **GnomeCompanionBackend**：GNOME 42~50 原生 Wayland，专用 GNOME Shell Companion 协同扩展。

主线程独占 GUI 事件循环，后端 Trait 彻底剔除 `Send + Sync` 标记，由后台 Tokio 线程池通过 mpsc 信道向主线程下发更新指令。

---

## 4. Native Wayland Layer-Shell 后端 (KDE / wlroots)

### 4.1 Route A 初始化生命周期 (P0-59)
GTK Window 必须在调用 `widget.show()` 或 `widget.realize()` **之前**绑定 Layer-Shell，否则会触发底层断言崩溃：
```rust
let window = gtk::Window::new(gtk::WindowType::Toplevel);
gtk_layer_shell::init_for_window(&window);
gtk_layer_shell::set_layer(&window, gtk_layer_shell::Layer::Overlay);
gtk_layer_shell::set_namespace(&window, "readmd-pet");
gtk_layer_shell::set_keyboard_mode(&window, gtk_layer_shell::KeyboardMode::None);
// 随后再将 WRY 的 WebKitWebView 容器挂载到 GTK 容器中并 show_all
```

### 4.2 能力探测冻结 (P0-60)
`is_supported()` 与 `protocol_version()` 仅在启动 probe 阶段执行一次，探测结果冻结在不可变 `LayerShellCapabilities` 结构体中，严禁在渲染循环中频繁跨 IPC 查询。

### 4.3 合成器避让与独占区语义 (P0-113)
Wayland 缺乏全局 `Display.workArea`。使用 `Layer::Overlay` 配合 `exclusive_zone(0)`，声明桌宠期望被顶层面板避让，结合 margins 映射实现边缘防遮挡。增加实机验证项 `VAL-31`。

---

## 5. GNOME Wayland 专属后端：ReadMD GNOME Shell Companion

### 5.1 架构分工与 AppID 传播 (P0-65, P0-66, P0-80, P0-81)
- GNOME Wayland 严禁 layer-shell 协议。桌宠采用标准 Wayland 顶层窗口，通过专用 Companion 扩展在 Mutter 内部将窗口强制置顶、穿透并隐藏任务栏。
- **AppID 唯一标识**：`PET_OVERLAY_APP_ID = "asia.readmd.pet"`。
- **单窗口精确匹配**：扩展通过连接 Unix Domain Socket 的 Peer PID，在 Mutter 窗口树中精确匹配 `metaWindow.get_pid() === clientPid`，杜绝误操作同名窗口。
- **真实 Mutter API**：彻底剔除虚构的 `set_skip_taskbar`，统一调用 `metaWindow.hide_from_window_list()`。

### 5.2 双产物分发与扩展生命周期 (P0-67, P0-68, P0-69)
- GNOME 42~44（Ubuntu 22.04 / Debian 12）：分发 Legacy GJS imports 产物。
- GNOME 45~50（Ubuntu 24.04 / Fedora 40+）：分发 ESM 模块标准产物。
- 扩展安装时显式提示用户登出当前会话以加载扩展。扩展内部注入异步崩溃防护 (`VAL-23`)，确保畸形 IPC 不影响 GNOME Shell 稳定性。

---

## 6. Wayland Input Model 主动推送几何模型 (P0-111)

### 6.1 解决 Chicken-and-Egg 死锁
若在 Wayland 下当光标进入模型轮廓时才开启 input region，则在进入之前 client 由于 input region 为空根本无法接收到 pointer enter/motion 事件。
**主动推送模型**：
渲染端模型或几何状态变更 $	o$ 生成 `InteractionRegionSnapshot { generation: u64, rects: Vec<BridgeDipRect> }` $	o$ Rust 宿主转换为表面局部物理坐标 $	o$ 调用 `wl_surface.set_input_region(region)` $	o$ `wl_surface.commit()`。
交互区域由几何变更主动维护，彻底消除指针死锁。

---

## 7. 黄金交互区 Golden Hit Region 原则与交互边界 (P0-95, P0-112)

彻底删除历史版本中限制在 64 个矩形以内的有损近似描述，以真实渲染器命中为唯一准绳。
- **Live2D**：`model.hitTest(x, y).length > 0 || model.getBounds().contains(x, y)`。
- **Sprite**：有效可视像素或 DOM 矩形区域。
- 坐标系统使用严格强类型：`BridgeDipRect`、`OutputLocalDipRect` 与 `OutputPlacement`。

---

## 8. ReadMD Pet IPC v1：会话解耦与 WRY 官方 API 对齐 (P0-73, P0-74, P0-88, P0-89)

### 8.1 会话解耦模型
- `webview_session_id`：表示宿主进程创建该 WebView 实例的全局唯一 UUID。
- `navigation_generation`：原子递增计数器，页面每次发生导航或重载时递增。
跨代纪异步 IPC 消息持有已失效的 generation token 时，宿主坚决予以丢弃，消除陈旧回调覆盖。

### 8.2 IIFE 包装防语法错误 (P0-73)
初始化脚本必须使用立即执行函数（IIFE）包装，防止顶层 `return` 导致 `SyntaxError: Illegal return statement`：
```javascript
(() => {
  if (window.top !== window.self) {
    console.warn("[ReadMD] Blocked petOverlay initialization in non-top frame");
    return;
  }
  // 注入 window.hermesDesktop 与 window.readmdPet
})();
```

---

## 9. 完整安全资产协议 (Secure Asset Protocol) (P0-86, P0-87, P0-118, P0-119)

### 9.1 平台专属 Origin
- macOS / Linux：`readmd-pet://localhost/`
- Windows (WebView2)：`http://readmd-pet.localhost/`（或 `https://readmd-pet.localhost/` 配合安全探针）。

### 9.2 严格路径沙盒与 TOCTOU 防御 (P0-118)
- 资产统一部署在只读托管沙盒根目录（Staged Readonly Managed Root）。
- 防御 `..`、NUL 字符 `0x00`、盘符穿透与 UNC 路径。
- Unix 采用目录相对描述符结合 `O_NOFOLLOW` 打开；Windows 结合 Final Opened Handle Path 校验，防御同用户软链接交换攻击。

### 9.3 生产 DevTools 与功能权限锁定 (P0-119)
- 生产构建强制关闭 DevTools（`with_devtools(false)`），仅在诊断模式显式开启。
- 彻底禁止外部导航、新窗口弹出、摄像头、麦克风、地理定位、系统通知与用户扩展的可执行脚本。

---

## 10. 权威 Durable FIFO 字节级协议规范 (P0-100)

- **唯一权威目录路径**：`<runtime_dir>/events/`。
- **容量与排队限制**：单文件体积 $\le 32	ext{ MiB}$；队列文件总数 $\le 128$ 个；队列总未消费容量 $\le 64	ext{ MiB}$。
- **文件名格式**：`${wall_clock_ms.padStart(16,'0')}-${sequence.padStart(8,'0')}-${UUID}.json`。
- **原子写入**：写入 `<target>.tmp`（flag `wx` 独占创建） $	o$ 原子 `rename` 至 `<target>.json`。

---

## 11. Native Drag & Drop：WRY 0.57 官方路径重构解耦 (P0-102)

- 接收拖放文件后，提取路径数组校验（每个路径必须为非空字符串，长度 $\le 32768$，总数 $\le 128$）。
- 通过 FIFO 发布标准命令：
  ```json
  {
    "type": "drop",
    "paths": ["/path/to/file1.pdf", "/path/to/file2.epub"]
  }
  ```

---

## 12. CSP 策略与资产沙盒边界
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

## 13. 跨平台安全 ClipboardService 规范 (P0-98)
统一遵照 §1.7 限制，按平台原生 API 安全抓取文本（截取 4M 字符）、PNG Base64 编码（截取 24M 字符）与文件路径列表（最多 128 项）。

---

## 14. 实例 Scope 与用户 Profile 隔离 (P0-105)
- Windows 互斥体**严禁使用全局会话命名空间（禁止使用 Global 作用域）**。
- 采用局部会话与用户隔离命名空间：`Local\ReadMDPetOverlay_<UserSIDHash>_<DataDirHash>`。
- 同一用户同一 Profile 单实例互斥；不同用户会话（Fast User Switching）或不同 DataDir 允许独立并存。

---

## 15. Parent Liveness Pipe 继承模型与深度验证 (P0-106)
- Python 父进程创建管道并持有写端（WRITE end）。
- Rust 子进程仅继承读端（READ end）；Python 在子进程启动后立即关闭自身持有的读端副本。
- Rust 绝不持有写端，由 Rust 衍生的 WebView 子进程也不可能继承写端。
- 父进程异常退出 $	o$ 内核自动回收写端句柄 $	o$ Rust 读端收到 EOF $	o$ 触发 2.5 秒倒计时优雅退场。

---

## 16. 权威状态调和模型 (Reconciliation State Model) (P0-75, P0-76)

```rust
pub struct DesiredOverlayState {
    pub visible: bool,
    pub bounds: BridgeDipRect,
    pub renderer: String,
    pub snapshot_revision: u64,
}

pub struct AppliedOverlayState {
    pub applied_visible: bool,
    pub applied_bounds: BridgeDipRect,
    pub applied_renderer: String,
    pub applied_generation: u64,
}
```
调和循环（`reconcile`）比对 Desired 与 Applied 状态，异步操作携带 generation token，生效后更新 AppliedState。晚到的过时代际更新坚决丢弃。

---

## 17. Renderer 后台探测路径与健康所有权隔离 (P0-85)
- **Renderer 观察健康**：`<bridge>.health.json`，由渲染端定期写入自身视角。
- **Rust 宿主主权健康**：`<bridge>.rust.health.json`，由 Rust 宿主独占写入自身主权健康，两者互不污染。

---

## 18. 权威代码与构建规范 (Authoritative Cargo Specification) (P0-55, P0-56, P0-57, P0-58)

### 18.1 全平台统一定义：Cargo.toml
```toml
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

## 19. 性能协议：阶段性时间与内存预算 (P0-109)

*特别说明：以下指标为开发阶段探索性占位预算（Provisional measurement placeholders），绝非冻结的发布门禁。最终性能门禁由 Phase 0 在真实物理机上对比 Electron 与 Rust 实测输出后经 ADR 确立。*
- 冷启动时间：参考目标 P95 $\le 250	ext{ ms}$。
- 常驻私有内存：Windows 参考 $\le 55	ext{ MB}$，macOS 参考 $\le 60	ext{ MB}$，Linux 参考 $\le 75	ext{ MB}$。
- 72 小时内存泄漏率：参考斜率 $\le 0.05	ext{ MiB/h}$。

---

## 20. 平台认证元组规范 (Platform Certification Tuples) (P0-107)

### 20.1 平台认证生命周期
```
[ Planned ] --(Phase 0 PoC 通过)--> [ Candidate ] --(实机门禁全通)--> [ Certified ]
     |                                    |
     +------------(验证失败)-------------> [ Rejected ]
```
**声明**：由于 Phase 0 PoC 尚未正式执行，**当前所有 23 个平台元组状态统一保持为 Planned**。

### 20.2 标准化一元化平台元组表 (P0-71, P0-72, P0-107)
| 元组编号 | 操作系统与版本 | 硬件架构 | 显示服务器 | 平台后端实现 | 当前生命周期状态 |
|---|---|---|---|---|---|
| **T-01** | Windows 11 24H2 | x86_64 | Desktop Window Manager | `Win32Backend` | **Planned** |
| **T-02** | Windows 11 23H2 | x86_64 | Desktop Window Manager | `Win32Backend` | **Planned** |
| **T-03** | Windows 11 24H2 | aarch64 (Snapdragon X) | Desktop Window Manager | `Win32Backend` | **Planned** |
| **T-04** | Windows 10 22H2 (Build 19045+) | x86_64 | Desktop Window Manager | `Win32Backend` | **Planned** |
| **T-05** | macOS 26 (Tahoe) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-06** | macOS 15 (Sequoia) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-07** | macOS 14 (Sonoma) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-08** | macOS 13 (Ventura) | Apple Silicon | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-09** | macOS 15 (Sequoia) | x86_64 (支持机型) | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-10** | macOS 14 (Sonoma) | x86_64 (支持机型) | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-11** | macOS 13 (Ventura) | x86_64 (支持机型) | Quartz / WindowServer | `CocoaBackend` | **Planned** |
| **T-12** | Ubuntu 24.04 LTS (GNOME 46/50) | x86_64 | Native Wayland | `GnomeCompanionBackend` (esm) | **Planned** |
| **T-13** | Ubuntu 24.04 LTS (GNOME 46/50) | aarch64 | Native Wayland | `GnomeCompanionBackend` (esm) | **Planned** |
| **T-14** | Ubuntu 22.04 LTS (GNOME 42) | x86_64 | X11 | `X11Backend` | **Planned** |
| **T-15** | Debian 12 (GNOME 43) | x86_64 | Wayland | `GnomeCompanionBackend` (legacy) | **Planned** |
| **T-16** | Fedora 40/42 (KDE Plasma 6) | x86_64 | Native Wayland | `LayerShellBackend` (KWin) | **Planned** |
| **T-17** | Arch Linux (Sway) | x86_64 | Native Wayland | `LayerShellBackend` (wlroots) | **Planned** |
| **T-18** | Arch Linux (Hyprland) | x86_64 | Native Wayland | `LayerShellBackend` (wlroots) | **Planned** |
| **T-19** | 统信 UOS 20 SP1 | x86_64 | X11 | `X11Backend` (WebKitGTK 4.1) | **Planned** |
| **T-20** | 统信 UOS 20 SP1 | aarch64 | X11 | `X11Backend` (WebKitGTK 4.1) | **Planned** |
| **T-21** | 银河麒麟 V10 SP1 | x86_64 | X11 | `X11Backend` | **Planned** |
| **T-22** | 银河麒麟 V10 SP1 | aarch64 | X11 | `X11Backend` | **Planned** |
| **T-23** | 深度 Deepin 23 | x86_64 | Treeland / Wayland | `LayerShellBackend` / Candidate | **Planned** |

---

## 21. 动态门禁架构模型 (Dynamic Gate Composition Model) (P0-122)

机器注册定义见 `docs/architecture/pet-rust/gate-registry.json`（共注册 63 个严格门禁）：
1. **核心通用门禁 (Core Gates)**：
   `Gate-Core-SpecIntegrity`, `Gate-Core-AssetSecurity`, `Gate-Core-GUIThreadOwnership`, `Gate-Core-Reconciliation`, `Gate-Core-01` ~ `Gate-Core-11`, `Gate-IPC-Generation`, `Gate-IPC-01`, `Gate-Acceptance-Formula`, `Gate-Crypto-DetachedSig`, `Gate-Asset-TOCTOU`, `Gate-Tuple-Lifecycle`。
2. **黄金契约对比门禁 (Golden Parity Gates)**：
   `Gate-Golden-Differential`, `Gate-Golden-ABI`, `Gate-Golden-Bounds`, `Gate-Golden-FIFO`, `Gate-Golden-Clipboard`, `Gate-Golden-Menu`, `Gate-Golden-HitRegion`, `Gate-Golden-AntiSnapback`。
3. **后端专属门禁 (Backend Gates)**：
   - LayerShell: `Gate-LayerShell-01` ~ `Gate-LayerShell-06`。
   - GnomeCompanion: `Gate-GnomeCompanion-01` ~ `Gate-GnomeCompanion-05`。
   - LinuxUniversal: `Gate-Linux-Universal`, `Gate-Linux-Binary`, `Gate-Linux-MudaTargetIsolation`, `Gate-Linux-LayerShellV06`, `Gate-Linux-GdkMultiSeat`。
   - Menu & Desktop: `Gate-Menu-KWin-Wayland`, `Gate-Menu-GNOME-Wayland`, `Gate-Menu-Dependency`, `Gate-Win32-SingleInstance`, `Gate-Win32-VirtualDesktop`, `Gate-Cocoa-Thread`, `Gate-Asset-HTTPS-Win`, `Gate-Asset-Sandbox`。
4. **元组专属硬件门禁 (Tuple Certification Gates)**：
   `CERT-WIN11-ARM64`, `CERT-UOS20-X64`, `CERT-UOS20-ARM64`, `CERT-KYLIN10-ARM64`, `CERT-UBUNTU22-X64`, `CERT-UBUNTU24-X64`, `CERT-MACOS-ALL`, `CERT-GNOME50-X64`, `CERT-MACOS26-ARM64`, `CERT-FEDORA42-X64`。

---

## 22. Fullscreen 与虚拟桌面跨平台规范 (P0-110, P0-129)

### 22.1 Windows 虚拟桌面事实契约
- Windows 原生模型中，顶层窗口默认属于创建它时的当前虚拟桌面。切换虚拟桌面时，系统自动隐藏非当前桌面的顶层窗口。
- **Golden 表现基准**：桌宠仅依附于当前虚拟桌面，不跨桌面常驻。
- **能力矩阵明确声明**：Windows 跨虚拟桌面支持不属于本次重构迁移范围。若未来需要桌面常驻，需另开专项 Product Enhancement ADR，严禁使用未公开的私有 COM 接口进行越界操作。

### 22.2 全屏避让
检测到前台存在全屏独占应用时，桌宠窗口自动下发 `hide` 进入后台待命，全屏退出后恢复 `showInactive`。

---

## 23. 运行时分发与权威 A/B 升级 (PetRuntimeInstallerV2) (P0-114, P0-115, P0-116, P0-117)

### 23.1 密码学分离签名信任模型 (P0-114)
Manifest 文件本身严禁包含自身的公钥。采用分离式签名验证机制：
- 发行产物：`manifest.json` 与其分离式签名 `manifest.json.sig`。
- 校验模型：校验器内置固定的受信任公钥环（Pinned Trusted Keyring），根据 manifest 内声明的 `key_id` 选择对应公钥验签，强制比对 `security_epoch` 防御重放回滚攻击。

### 23.2 示例值占位符净化 (P0-115)
Manifest 示例值中彻底清除所有看起来真实的假 SHA256 字符串，统一使用明显占位符：
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
      "size": 18452000
    },
    {
      "role": "gnome_companion_esm",
      "platform": "linux",
      "arch": "all",
      "path": "extensions/gnome-esm",
      "sha256": "${ESM_COMPANION_SHA256}",
      "size": 42100
    }
  ]
}
```

### 23.3 签名校验器技术选型 (ADR-runtime-signature-verifier, P0-116)
由 Python `PetRuntimeInstallerV2` 负责验签时，依赖标准 `cryptography>=42.0` 库（国内发行版源完整提供预编译 Wheel）；或在无 Python 依赖场景下由独立的微型 Rust bootstrap verifier 二进制完成。

---

## 24. 供应链安全、签名与权限边界
所有发布二进制均在隔离 CI/CD 容器中构建，产出对应平台的 SBOM 清单，严格执行 Windows Authenticode 签名与 macOS Notarization 公证。

---

## 25. Release Stop Conditions 严格红线 (P0-108)

任何发布构建若出现以下任意一项，立即熔断发布流程：
1. 未解决的架构阻塞项（Open Architecture Blockers） $> 0$。
2. 未解决的必需实机实证项（Open Empirical Validation Items） $> 0$。
3. 任何核心门禁（Core Gates）、后端门禁（Backend Gates）或元组门禁（Tuple Gates）未通过或处于 Flaky 状态。
4. 任何 Golden 差异对比测试（Golden Differential Tests）出现未通过项。
5. 异常退出子进程残留率 $> 0.00\%$。
6. 72 小时常驻内存泄漏斜率 $> 0.05	ext{ MiB/h}$。

---

## 26. Codex 实施执行协议与 Golden 差异测试套件 (P0-124)

在 Phase 0 阶段，首先构建 `GoldenCapture` 工具，驱动当前生产 Electron 录制以下完整的 Golden Fixtures：
- `golden/fixtures/preload-abi.fixture.json`
- `golden/fixtures/bounds-policy.fixture.json`
- `golden/fixtures/toggle-app.fixture.json`
- `golden/fixtures/context-menu.fixture.json`
- `golden/fixtures/clipboard.fixture.json`
- `golden/fixtures/fifo-commands.fixture.json`
- `golden/fixtures/snapshot-reader.fixture.json`
- `golden/fixtures/hit-region-live2d.fixture.json`
- `golden/fixtures/hit-region-sprite.fixture.json`
- `golden/fixtures/anti-snapback.fixture.json`

Rust 候选实现必须通过 `GoldenDifferentialHarness` 逐一回放上述输入，验证产生的窗口动作、IPC 调用、FIFO 文件写入与可观察状态 100% 等价。

---

## 27. 重构验收判断标准 (P0-108)

### 27.1 当前版本候选状态 (Candidate Status)
```text
Tracked Open Empirical Validation Items = 43 (VAL-01 ~ VAL-43)
Tracked Open Architecture Blockers      = 30 (BLOCKER-01 ~ BLOCKER-30)
Platform Tuples Status                  = 23 Planned / 0 Candidate / 0 Certified
Document Verdict                        = NO — Contract Restoration Candidate
```

### 27.2 最终生产冻结验收标准 (Final Release Acceptance Criteria)
```text
Unresolved Architecture Blockers        === 0
Unresolved Empirical Validation Items   === 0
Required Compile Gates                  === PASS
Required PoC Gates                      === PASS
Golden Differential Parity Gates        === PASS
```

---

## 28. Platform Capability Matrix 综合平台能力矩阵 (P0-110, P0-129)

| 平台环境 | 窗口层级实现路径 | 输入穿透穿透机制 | 跨虚拟桌面常驻 (Cross-Workspace) | 菜单与托盘支持 |
|---|---|---|---|---|
| **Windows 10/11** | DWM Topmost / ToolWindow | Win32 Transparent | **Golden: 当前桌面显示；跨桌面需专属 ADR** | muda Win32 原生菜单 |
| **macOS (Cocoa)** | NSWindow Floating Level | Cocoa IgnoresMouseEvents | 支持（可配置 NSWindowCollectionBehavior） | muda Cocoa 原生菜单 |
| **Linux (KDE/wlroots)** | Layer-Shell (Overlay 层) | Cairo Surface Input Shape | 天然多工作区固定呈现 | muda GTK 菜单 |
| **Linux (GNOME Shell)** | 顶层窗口 + Shell Companion | Mutter Input Shape 穿透 | 由 Shell Companion 锁定置顶 | muda GTK 菜单 |
| **Linux (X11 通用)** | EWMH DOCK / Topmost | XFixes / XShape 穿透 | EWMH Sticky 状态支持 | muda GTK 菜单 |

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
- **DEC-11**：确立 7 大核心 Golden 行为源及 Git Commit SHA256 机器绑定 (P0-91)。
- **DEC-12**：确立 HostSnapshotBounds 与 RendererInteractiveBounds 解耦模型，彻底剔除虚构吸附 (P0-93, P0-94)。
- **DEC-13**：确立 A/B 升级分离签名信任根与受信任公钥环模型 (P0-114)。
- **DEC-14**：确立 Durable FIFO 权威目录 `${bridge}.commands` 与原子重命名 (P0-100)。
- **DEC-15**：确立 Wayland 几何主动推送模型，根除 Chicken-and-egg 光标死锁 (P0-111)。

### 29.2 开放实机实证事项注册表 (Empirical Validation Items: 43 Items Open, P0-120)
*机器可读权威源：`docs/architecture/pet-rust/validation-registry.json`*

| 编号 | 实证领域 | 核心风险与需物理验证事实 | 阻塞门禁 |
|---|---|---|---|
| **VAL-01** | Layer-Shell + WRY 容器 | WebKitGTK 子表面是否会二次截获透明穿透区域的鼠标事件 | `Gate-LayerShell-02` |
| **VAL-02** | Layer-Shell 跨屏拖拽 | 重设 Monitor 时 Wayland 合成器的 Implicit Pointer Grab 是否中断 | `Gate-LayerShell-01` |
| **VAL-03** | Layer-Shell 按需输入法 | 合成器对 `zwlr_layer_shell_v1` v4+ `KeyboardMode::OnDemand` 的支持率 | `Gate-LayerShell-03` |
| **VAL-04** | GNOME Companion 跨版本稳定性 | GNOME 42~50 各大版本中 Mutter `move_frame` 的内部接口稳定性 | `Gate-GnomeCompanion-02` |
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
| **VAL-20** | macOS 13~26 支持周期认证 | macOS 13 (Ventura) ~ 26 (Tahoe) 在 Apple Silicon 与 Intel 上的认证 | `CERT-MACOS-ALL` |
| **VAL-21** | Layer-Shell Pre-Realize 生命周期 | GTK 窗口在未 realize 前绑定 Layer-Shell 且成功挂载 WRY 的实测路径 | `Gate-LayerShell-05` |
| **VAL-22** | GNOME Meta.Window AppID 传播 | WRY/GTK 顶层窗口在 Mutter 内部 `get_gtk_application_id()` 真实返回值验证 | `Gate-GnomeCompanion-04` |
| **VAL-23** | GNOME Companion 畸形 IPC Fuzz | 极端非法 JSON 与畸形指令下 GNOME Shell 进程零崩溃证明 | `Gate-GnomeCompanion-05` |
| **VAL-24** | Wayland 工作区与面板避让对齐 | Layer-Shell margins 映射与 KDE/wlroots 边缘独占区域对齐实测 | `Gate-LayerShell-06` |
| **VAL-25** | 导航代际隔离异步消息丢弃测试 | 构造跨导航延迟 IPC 消息，验证 Rust 宿主 100% 拒绝陈旧调用 | `Gate-IPC-Generation` |
| **VAL-26** | 调和状态 AppliedState 异步生效竞态 | 构造高频乱序 snapshot 事件，验证 AppliedState 拒绝陈旧代际状态生效 | `Gate-Core-Reconciliation` |
| **VAL-27** | 权威规范产物完整性与发布检查 | 验证规范本体哈希与完整性，自动化检查 release 产物 linux-production 特性 | `Gate-Core-SpecIntegrity` |
| **VAL-28** | muda 零 libxdo 依赖实测证明 | 通过 readelf/ldd 证明 Linux 二进制完全不依赖 `libxdo.so` | `Gate-Linux-MudaTargetIsolation` |
| **VAL-29** | 当前 GNOME 50 扩展兼容性实测 | 验证 GNOME 50 环境下 ESM 扩展加载与 Mutter 接口调用平滑 | `CERT-GNOME50-X64` |
| **VAL-30** | macOS 26 (Tahoe) 桌宠实机认证 | 验证 macOS 26 开发者/正式版下透明渲染、点击穿透与 Spaces 随同 | `CERT-MACOS26-ARM64` |
| **VAL-31** | Layer-Shell compositor usable-area / exclusive-zone parity | 实测不同 Wayland 合成器在 Layer::Overlay + exclusive_zone(0) 下的面板避让与工作区对齐语义 | `Gate-LayerShell-06` |
| **VAL-32** | Golden preload ABI differential | 逐项对比 Rust 注入 bridge 与 Golden preload.ts 的 9 个 API 方法、参数个数与取消订阅函数 | `Gate-Golden-ABI` |
| **VAL-33** | Golden bounds policy differential | 分别测试 HostSnapshotBoundsPolicy (40 DIP workArea) 与 RendererInteractiveBoundsPolicy (80x80 min) | `Gate-Golden-Bounds` |
| **VAL-34** | Golden control/toggle-app differential | 实测 toggle-app 触发剪贴板读取并向 FIFO 写入 clipboard 消息，验证无窗口隐藏误动作 | `Gate-Golden-Differential` |
| **VAL-35** | Golden menu model differential | 实测右键菜单 5 项动作顺序、header 禁用、Play 能量阈值 (>=10) 与最大 128 角色单选列表 | `Gate-Golden-Menu` |
| **VAL-36** | Golden clipboard payload differential | 实测 4M 字符文本截断、24M 字符 base64 PNG 截断与 Windows 128 路径解析的一致性 | `Gate-Golden-Clipboard` |
| **VAL-37** | Golden FIFO byte/filename parity | 实测 <bridge>.commands 目录排队、32MB 单体限制、64MB 总量限制与 .tmp 独占写原子重命名 | `Gate-Golden-FIFO` |
| **VAL-38** | Golden SnapshotReader retry parity | 构造非法 JSON snapshot 验证签名未被污染，后续合法 snapshot 能立即被识别与消费 | `Gate-Golden-Differential` |
| **VAL-39** | Golden hit-region differential | 运行确定性点网格与边缘测试，验证 Live2D hitTest \|\| bounds.contains 与 Sprite 规则的精确等价性 | `Gate-Golden-HitRegion` |
| **VAL-40** | Golden pushState / anti-snapback differential | 实测 pushState 下发时以实际当前窗口 bounds 覆盖 snapshot bounds，防止拖拽后回弹 | `Gate-Golden-AntiSnapback` |
| **VAL-41** | Windows virtual-desktop Golden behavior | 实测 Windows 虚拟桌面切换时桌宠仅在当前桌面显示，验证无私有 API 越界注入 | `Gate-Win32-VirtualDesktop` |
| **VAL-42** | Runtime detached-signature verification | 实测分离签名 manifest.json.sig 配合固定公钥环进行验签，拒绝 manifest 自签名注入 | `Gate-Crypto-DetachedSig` |
| **VAL-43** | Asset sandbox TOCTOU threat-model/handle test | 在只读托管沙盒根目录下模拟同用户重解析点与符号链接交换，验证只读受管根的不可变防御边界 | `Gate-Asset-TOCTOU` |

### 29.3 开放架构阻塞项注册表 (Open Architecture Blockers: 30 Items Open, P0-121)
*机器可读权威源：`docs/architecture/pet-rust/blocker-registry.json`*

| 编号 | 阻塞领域与项 | 核心架构风险与解除条件 | 关联门禁 / 验证项 | 当前状态 |
|---|---|---|---|---|
| **BLOCKER-01** | Wayland WebKitGTK 透明子表面事件穿透 | WebKitGTK 子表面在 Wayland 下可能二次拦截透明区域鼠标事件；需验证 cairo input region 穿透 | `Gate-LayerShell-02` / `VAL-01` | **OPEN** |
| **BLOCKER-02** | Layer-Shell 分数缩放与 Hit Region 不连续 | 合成器在分数缩放（如 125%、150%）下对子表面 input shape 的舍入截断；需实机测试无死区 | `Gate-LayerShell-01` / `VAL-02` | **OPEN** |
| **BLOCKER-03** | GNOME 45+ Mutter 跨小版本内部接口稳定性 | Mutter 内部 move_frame、get_gtk_application_id 在 45~50 跨版本 API 变动；需 ESM 扩展隔离 | `Gate-GnomeCompanion-02` / `VAL-04` | **OPEN** |
| **BLOCKER-04** | Linux 多 GDK 会话与多 Seat 隔离 | 相同用户或多显示服务器下单二进制 GDK 静态上下文污染；需验证独立 DISPLAY/WAYLAND_DISPLAY 隔离 | `Gate-Linux-GdkMultiSeat` / `VAL-07` | **OPEN** |
| **BLOCKER-05** | Windows on ARM64 渲染基准与透明层叠 | 高通骁龙平台 WebView2 在 DWM 透明窗口下的 GPU 加速层叠与 CPU 占用基准；需实机达标 | `CERT-WIN11-ARM64` / `VAL-05` | **OPEN** |
| **BLOCKER-06** | 权威规范产物完整性与 SHA256 强校验 | 规范交付链截断、TOC 锚点悬空或哈希不匹配；需通过 Spec Linter 完整性自动化校验 | `Gate-Core-SpecIntegrity` / `VAL-27` | **OPEN** |
| **BLOCKER-07** | Cargo muda 特性联合污染隔离 | 通用依赖引入 muda 导致 Linux 错误激活默认 libxdo 特性；必须按 target 隔离依赖 | `Gate-Linux-MudaTargetIsolation` / `VAL-28` | **OPEN** |
| **BLOCKER-08** | gtk-layer-shell v0_6 特性链与 Linux 发布锁定 | 调用 protocol_version() 必须显式激活 v0_6；发布配置必须激活 linux-production | `Gate-Linux-LayerShellV06` / `VAL-27` | **OPEN** |
| **BLOCKER-09** | Layer-Shell 窗口 pre-realize 初始化生命周期 | GTK 窗口在 realize 之后调用 layer-shell 初始化会触发底层断言 panic；必须在 show/realize 前绑定 | `Gate-LayerShell-05` / `VAL-21` | **OPEN** |
| **BLOCKER-10** | GNOME AppID 传播与 Meta.Window 发现 | WRY/GTK 窗口创建时必须传播 asia.readmd.pet，使 Mutter get_gtk_application_id() 稳定识别 | `Gate-GnomeCompanion-04` / `VAL-22` | **OPEN** |
| **BLOCKER-11** | 渲染端导航代际与 WebView 实例会话严格解耦 | 跨导航异步回调必须持有独立 generation token；防止页面重载后陈旧 IPC 调用执行 | `Gate-IPC-Generation` / `VAL-25` | **OPEN** |
| **BLOCKER-12** | 调和状态机 AppliedState 异步生效竞态与陈旧防护 | 异步更新生效前必须记录 applied_generation，忽略晚到的过时 snapshot | `Gate-Core-Reconciliation` / `VAL-26` | **OPEN** |
| **BLOCKER-13** | Wayland 工作区与 exclusive-zone 面板避让一致性 | Layer-Shell margins 必须与各合成器 panels / dock 边距动态对齐，防止桌宠遮挡面板 | `Gate-LayerShell-06` / `VAL-24` | **OPEN** |
| **BLOCKER-14** | GNOME 50 当前版本扩展兼容性实机认证 | GNOME 50 新架构规范与扩展接口兼容性验证，确保扩展零崩溃 | `CERT-GNOME50-X64` / `VAL-29` | **OPEN** |
| **BLOCKER-15** | macOS 26 (Tahoe) 架构与硬件分离认证 | macOS 26 上 Spaces 随同、透明点击穿透与 ARM64/Intel 硬件分离实机认证 | `CERT-MACOS26-ARM64` / `VAL-20, VAL-30` | **OPEN** |
| **BLOCKER-16** | Secure Asset Protocol 产物截断防护与沙盒 TOCTOU 拦截 | 静态资产协议支持严格路径沙盒、零 NUL 注入、双重扩展名拦截与符号链接 TOCTOU 防御 | `Gate-Asset-Sandbox` / `VAL-17` | **OPEN** |
| **BLOCKER-17** | Golden source-set corruption | 核心行为源遗漏 live2d/stage.ts 与 pet-overlay-ipc.ts；需绑定 7 大核心源码及 SHA256 | `Gate-Golden-ABI` / `VAL-32` | **OPEN** |
| **BLOCKER-18** | Golden preload ABI information loss | Preload ABI 沦为自然语言摘要导致方法签名失真；需完整冻结 TypeScript 接口定义 | `Gate-Golden-ABI` / `VAL-32` | **OPEN** |
| **BLOCKER-19** | Invented 12-DIP snap behavior | 将工作区退让边距误当成桌宠吸附阈值；需彻底从规范删除虚构吸附逻辑并恢复真实 clamp | `Gate-Golden-Bounds` / `VAL-33` | **OPEN** |
| **BLOCKER-20** | Durable FIFO path/schema regression | FIFO 路径误写为 events 目录；需恢复为 <bridge>.commands 目录及原子重命名策略 | `Gate-Golden-FIFO` / `VAL-37` | **OPEN** |
| **BLOCKER-21** | ReadMD toggle-app semantics missing | 误用 vendor 主窗口最小化逻辑；需恢复 ReadMD 适配层独有的剪贴板读取与命令派发语义 | `Gate-Golden-Differential` / `VAL-34` | **OPEN** |
| **BLOCKER-22** | Windows Global mutex scope regression | 单实例互斥体误用 Global 命名空间跨越所有用户会话；需调整为基于用户 SID 和配置隔离的 Local 互斥体 | `Gate-Win32-SingleInstance` | **OPEN** |
| **BLOCKER-23** | Tuple lifecycle state contradiction | 在未运行 Phase 0 PoC 的情况下将平台元组标为 Candidate；需将所有平台元组重置为 Planned | `Gate-Tuple-Lifecycle` | **OPEN** |
| **BLOCKER-24** | Final acceptance formula inverted | 最终验收公式将候选期未解决阻塞项误当成验收通过条件；需修正为未解决阻塞项等于 0 | `Gate-Acceptance-Formula` | **OPEN** |
| **BLOCKER-25** | Windows virtual-desktop false assumption | 虚构 Windows 全虚拟桌面穿透特性；需明确 Golden 行为仅依附于当前虚拟桌面 | `Gate-Win32-VirtualDesktop` / `VAL-41` | **OPEN** |
| **BLOCKER-26** | Wayland input-region chicken-and-egg | 光标进入才开启 input region 的被动模型在 Wayland 下无法接收 enter 事件；需改为主动 snapshot 推送模型 | `Gate-LayerShell-02` / `VAL-39` | **OPEN** |
| **BLOCKER-27** | Layer-Shell usable-area source undefined | 假定 Wayland 客户端原生具备 Electron 风格全局工作区；需明确 exclusive-zone(0) 的合成器避让语义 | `Gate-LayerShell-06` / `VAL-31` | **OPEN** |
| **BLOCKER-28** | Runtime signature trust-root model | Manifest 内部包含自身公钥导致自签名安全失效；需改为分离式签名 manifest.json.sig 与内置受信任密钥环 | `Gate-Crypto-DetachedSig` / `VAL-42` | **OPEN** |
| **BLOCKER-29** | Validation/Gate referential-integrity failure | 手工维护的表格存在交叉引用错位与悬空 ID；需建立机器注册表与自动化引用完整性检验 | `Gate-Core-SpecIntegrity` | **OPEN** |
| **BLOCKER-30** | Canonical spec integrity evidence absent | 规范完整性结论缺乏机器背书；需生成 spec.integrity.json 并将 exact SHA256 与 git commit 绑定 | `Gate-Core-SpecIntegrity` | **OPEN** |

---

## 30. Real Hardware Certification Matrix 实机硬件认证矩阵

实机门禁矩阵：
1. **Windows 11 on ARM (高通骁龙 X Elite / Surface Pro 11)**：aarch64 原生透明渲染与 WebView2 性能基准。
2. **macOS Apple Silicon (M1/M2/M3/M4)**：Spaces 随同与暗色模式平滑切换。
3. **KDE Plasma 6 on Wayland (AMD/Intel GPU)**：`gtk-layer-shell` 边距与 120Hz 刷新同步。
4. **GNOME 46 / 50 on Wayland (Ubuntu 24.04 / Fedora 40+)**：Companion 扩展安装与 Mutter 移动控制。
5. **统信 UOS 20 SP1 & 银河麒麟 V10 SP1**：国产 Linux 发行版环境下的单透明窗体交互。

---

## 31. Change Log (v1.3.0 -> v1.4.0 演进)
- **[P0] 架构重塑**：单一 Tao 重构为包含 5 大后端的 **`Platform Backend Family`**。
- **[P0] 原生 Wayland 双轨制**：KDE/wlroots 走 `LayerShellBackend`；GNOME 走 `ReadMD GNOME Shell Companion`。
- **[P0] 输入模型穿透**：Native Wayland 废除全屏透明层，改用标准 **Surface Input Region**。
- **[P0] 依赖统一**：统一为 **Rust 1.85.0**、`wry 0.57.0`、`tao 0.37.0`、`muda 0.19.3`、`gtk-layer-shell 0.8.2`。
- **[P0] 协议安全加固**：CSPRNG 令牌认证，双向 JSON 序列化杜绝 XSS，FIFO 排队。
- **[P1] 门禁体系演进**：引入动态门禁架构模型。

---

## 32. Change Log (v1.4.0 -> v1.4.1 Freeze Candidate 演进)
- **[P0-01] Cargo Feature Matrix 真实重构**：消除盲目 `default-features = false`，按 Target 显式配置。
- **[P0-02] 严格主线程 GUI 边界**：从 `OverlayWindowBackend` 移除 `Send + Sync`。
- **[P0-03 & P0-04] gtk-layer-shell 架构澄清**：明确单一二进制发布模式下 KDE/wlroots 与 GNOME 的隔离。
- **[P0-07 & P0-08] GNOME Shell Companion 双产物机制**：ESM 与 Legacy 拆分，确立 GJS 边界。
- **[P0-09] 坐标系强类型化**：建立 `BridgeDipRect`、`OutputLocalDipRect` 与 `OutputPlacement`。
- **[P0-10 & P0-11] 依赖事实对齐**：WRY 0.57 DnD API 对齐，muda 0.19.3 Linux 剥离 libxdo。
- **[P0-12 & P0-13] 状态机与资产协议**：确立调和状态机与自定义 Scheme 路径沙盒。
- **[P0-14] 支持周期对齐**：macOS 基线对齐至 macOS 13+。
- **[P0-15] 经验实证事项**：建立包含 VAL-01 至 VAL-20 的追踪表。

---

## 33. Change Log (v1.4.1 -> v1.4.2 Freeze Candidate 演进)
- **[P0-24] Cargo 根配置彻底统一**：Section 18 补全 `os-webview` 与 Linux 特性。
- **[P0-25] WRY 0.57 API 纠正**：修正 `WebViewBuilder::new()` 与 `DragDropEvent`。
- **[P0-26] 状态机命名统一**：确立权威期望驱动调和状态机 (Reconciliation Model)。
- **[P0-27] 资产协议完整恢复**：完全恢复 Secure Asset Protocol §9。
- **[P0-28] 门禁计数模型**：废除硬编码 58 道门禁，采用动态门禁模型。
- **[P0-29] 历史残留彻底清除**：清除 `set_skip_taskbar` 与历史路径。
- **[P0-30] macOS 支持矩阵对齐**：明确 macOS 13 (Ventura) 为最低基线。

---

## 34. Change Log (v1.4.2 -> v1.4.3 Architecture Freeze Candidate 演进)
- **[P0-52 & P0-53] 官方仓库规范权威落盘与 Linter 重构**：规范唯一路径建立在 `docs/architecture/pet-rust/spec.md`，构建全量校验脚本。
- **[P0-54] 校验器负例自测套件**：建立 `tests/spec_linter/` 与 10 组损坏 fixture。
- **[P0-55 ~ P0-58] muda 目标隔离与编译特性补全**：按 target 隔离 muda，补全 `gtk-layer-shell` 的 `v0_6` 特性，定义 `linux-production`。
- **[P0-59 ~ P0-62] Layer-Shell Pre-Realize 与 Wayland 失败闭环**：确立 Route A 生命周期，Wayland 探测失败安全闭环。
- **[P0-63 & P0-64] 进程所有权与主线程保护**：禁止 `#[tokio::main]` 抢占主线程，精确 PID 进程管理。
- **[P0-65 ~ P0-68] GNOME Companion 安全硬化**：锁定 `asia.readmd.pet` AppID，Peer PID 窗口匹配，异步崩溃隔离。
- **[P0-69 ~ P0-72] 矩阵现实对齐与标准化元组**：覆盖 GNOME 50 与 macOS 26，重构 T-01 至 T-23 一元化表格。
- **[P0-73 ~ P0-76] 脚本 IIFE 包装与调和状态解耦**：注入脚本 IIFE 隔离防语法错误，分离 `webview_session_id` 与 `navigation_generation`。
- **[P0-77 ~ P0-86] 区域与资产协议完整性**：删除 `<=64` 有损近似，双健康文件隔离。

---

## 35. Change Log (v1.4.3 -> v1.4.4 Contract Restoration Candidate 演进)
- **[P0-91] 权威黄金行为源集合机器级锁定**：
  绑定 7 大核心行为源码及 SHA-256（`electron-main.ts`, `preload.ts`, `bridge-transport.ts`, `renderer.tsx`, `live2d/stage.ts`, `pet-overlay-ipc.ts`, `hermes_adapter.py`），Git Commit 锁定为 `4dcfd73ce81a14ace7e429791e0594bea47b24e5`。建立 `golden-contract.json`。
- **[P0-92] 完整 Preload ABI 接口定义恢复**：
  彻底废弃自然语言摘要，完整冻结 `Window.hermesDesktop.petOverlay` 与 `Window.readmdPet` 的 TypeScript 接口定义。特别锁定 `control(payload)` 单参数签名与 `onState`/`onControl` 返回取消订阅函数 `() => void` 的强契约。
- **[P0-93 & P0-94] 彻底剔除虚构吸附，解耦两套 Bounds 策略**：
  从规范全文彻底删除虚构的 12 DIP 离手吸附伪设定。明确 12/24 仅为主显示器离屏退让边距。解耦并形式化定义 `HostSnapshotBoundsPolicy` (40 DIP workArea 重叠) 与 `RendererInteractiveBoundsPolicy` (80x80 min 与临时 resizable 开关)。
- **[P0-95 & P0-112] 还原 Live2D 与 Sprite 真实命中测试规则**：
  逐字义还原 `stage.ts` 中的 `model.hitTest(x, y).length > 0 || model.getBounds().contains(x, y)` 判定准则。输出确定性测试网格，废除主观多边形臆断。
- **[P0-96] 恢复外部 ReadMD 适配层特有的 `toggle-app` 语义**：
  明确 ReadMD 拓扑覆盖了 vendor 原生主窗口切换逻辑，确立 `toggle-app` $	o$ 读取系统剪贴板 $	o$ 写入 FIFO 队列的标准语义，严禁误实现为最小化窗口。
- **[P0-97] 完整恢复右键上下文菜单黄金契约**：
  冻结 Header 禁用状态展示、Pet/Feed/Play/Rest 动作顺序、Play 能量门槛 (>=10)、Rest/Wake up 动态文字及最多 128 个角色 Radio 单选项。
- **[P0-98] 完整恢复剪贴板容量上限基准**：
  锁定文本 4M 字符截断、PNG Base64 字符串 24M 字符截断与 Windows 128 路径解析。
- **[P0-99] 还原渲染端恢复黄金基线**：
  还原 60 秒内 3 次熔断、`500ms * count` 阶梯退避重载的基线行为。
- **[P0-100] 修正 Durable FIFO 权威目录路径**：
  彻底废除 `<runtime_dir>/events/` 虚构路径，恢复权威目录 `${bridge}.commands`、32MB 单体与 64MB 总量限制，以及 `.tmp` 独占创建原子重命名策略。
- **[P0-101] 恢复快照读取器 (SnapshotReader) 精确重试行为**：
  冻结 Unix 与 Windows 复合文件签名，明确 JSON 解析失败绝不污染签名缓存，保障下次轮询可无缝重试。
- **[P0-102] 恢复 Native Drop 路径边界契约**：
  校验数组类型、单路径字符串长度 $\le 32768$、路径总数 $\le 128$。
- **[P0-104] 普通用户设置界面消除技术泄露**：
  普通用户 UI 仅暴露“阅读器内”与“独立桌面”选项，彻底隐藏 Electron 与 Rust 单选项。
- **[P0-105] Windows 单实例互斥体命名空间纠偏**：
  删除 `Global\`，改为基于用户 SID 和配置哈希隔离的 `Local\ReadMDPetOverlay_<UserSIDHash>_<DataDirHash>`，支持快速用户切换。
- **[P0-106] 进程存活管道精准所有权界定**：
  明确 Python 持有写端、Rust 仅继承读端，Python 退出内核自动关闭写句柄触发 Rust EOF。
- **[P0-107] 平台认证元组生命周期重置为 Planned**：
  在 Phase 0 PoC 未物理通过前，T-01 至 T-23 状态严格重置并保持为 **Planned**。
- **[P0-108] 最终验收公式逻辑纠偏**：
  彻底消除将候选期开放阻塞项误写为验收条件的错误，修正为未解决阻塞项等于 0、实证项等于 0。
- **[P0-109] 性能硬编码降级为阶段性占位预算**：
  将未经验证的时间与内存硬编码标记为开发占位预算，不作为发布门禁。
- **[P0-110 & P0-129] 纠正 Windows 虚拟桌面假设**：
  明确 Golden 行为仅依附于当前虚拟桌面，跨虚拟桌面常驻不属于迁移基准范围。
- **[P0-111] 建立 Wayland 几何主动推送模型**：
  由几何变更主动推送 `InteractionRegionSnapshot`，根除光标进入死锁。
- **[P0-113] Layer-Shell 工作区与面板避让机制明确**：
  阐明 `exclusive_zone(0)` 合成器避让语义，新增 `VAL-31`。
- **[P0-114 ~ P0-117] A/B 升级密码学签名模型重塑**：
  改为分离式签名 `manifest.json.sig` 与内置固定公钥环；净化假 SHA256 字符串；确立 Python `cryptography` 验签决策；补全运行时产物清单。
- **[P0-118 & P0-119] 沙盒 TOCTOU 边界与 DevTools 生产禁用**：
  确立只读受管根边界与生产构建禁用 DevTools 策略。
- **[P0-120 ~ P0-123] 机器级注册表与规范完整性自证**：
  建立 `validation-registry.json` (43 项), `blocker-registry.json` (30 项), `gate-registry.json` (63 项), `spec.integrity.json`，实现 100% 引用完整性。
- **[P0-124 ~ P0-128] 恢复 Fallback 精灵图、时序与 `pushState` 防回弹**：
  冻结精灵图元数据、轮询时序、原生 bounds 覆盖防回弹与全屏隐藏边缘分支。
- **[P1-131] 本地优先诊断原则**：
  明确引擎降级与异常仅记录本地诊断事件，严禁未授权网络遥测。

---

## 附录 A：验证关键系统底层实现
- **Windows DWM / JobObject / Local Mutex**：提供透明穿透与单实例保障。
- **macOS Cocoa NSWindow**：提供无边框全透明悬浮与 Spaces 随同。
- **Linux GDK / WebKitGTK / Layer-Shell / GNOME GJS**：保障单一通用二进制与 Wayland 双轨原生协同。

---

## 附录 B：Phase 0 实施执行清单与 ADR-0001 草案
在正式进入 Phase 1 实现前，Phase 0 必须严格执行以下任务：
1. 运行 `GoldenCapture` 录制生成 10 组黄金对比测试 Fixture。
2. 逐一运行 `VAL-01` 至 `VAL-43` 实机探测脚本。
3. 在物理机上验证解决 `BLOCKER-01` 至 `BLOCKER-30`，将对应平台元组由 Planned 晋升为 Candidate。
4. 实测产出性能基准报告，经 ADR 确立各平台专属性能门禁。
