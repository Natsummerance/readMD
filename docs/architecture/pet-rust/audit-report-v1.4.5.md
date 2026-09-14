# ReadMD Desktop Overlay v1.4.5 Semantic Closure Candidate 架构整改与终审报告

- **报告标识**：`REPORT-PET-OVERLAY-V145-SEMANTIC-CLOSURE`
- **目标规格**：`ReadMD Desktop Overlay Architecture Specification v1.4.5-Candidate`
- **版本阶段**：`Semantic Closure Candidate` (架构设计语义闭环候选)
- **基线版本**：`v1.4.4 Contract Restoration Candidate`
- **仓库地址**：`T:\Programming\Project\codex\creator\readmd` (`Natsummerance/readMD`)
- **Git Commit**：`f06074ef65b5c9f4afd1b7190fa0233932904cbf` (Branch: `main`)
- **评审结论**：**`NO — Production Architecture Freeze`**（严禁进入 Phase 1 正式 Rust Host 生产实现）

---

## 1. 交付物完整性与哈希基线

本轮整改已在规范主文件与 5 个机器可读注册表中实现完全收敛，所有文件均通过 SHA-256 强校验与跨表引用完整性验证。

| 规范文件 / 注册表 | 路径 | 关键指标 | SHA-256 校验和 |
| :--- | :--- | :--- | :--- |
| **主规范文件** | `docs/architecture/pet-rust/spec.md` | 948 行 / 60,486 字节 (NUL: 0) | `0ebd4a40dac8dbd6ad08c4270e8e38d9183dd94892f2e42265b8e3db93ad986e` |
| **Golden 契约清单** | `docs/architecture/pet-rust/golden-contract.json` | 6 核心文件 + 11 辅助文件 | `ea14639d312f3189a38a70479b0948af8734aca1d46fe04724e268ef8b0f072f` |
| **平台元组注册表** | `docs/architecture/pet-rust/tuple-registry.json` | 24 个单值原子元组 (T-01 ~ T-24) | `bd6a35abaec239f104abbdc7929076a066410dbe6bf04813bf8cc681c1de1cef` |
| **经验验证注册表** | `docs/architecture/pet-rust/validation-registry.json` | 51 个待测经验项 (VAL-01 ~ VAL-51) | `56d012e6454a6fa2fd9f75b1e14f2792e8d956191c7a4528da309814f821b97c` |
| **质量门禁注册表** | `docs/architecture/pet-rust/gate-registry.json` | 71 个全分级门禁 (GATE-01 ~ GATE-71) | `385b6dcbf2b86c1cdcb83f79f81786c81d0aad6dadc5773d6750e89ff04d11dc` |
| **阻塞项注册表** | `docs/architecture/pet-rust/blocker-registry.json` | 33 个设计已闭环项 (BLOCKER-01 ~ 33) | `7e8120e87f74b8ba96501de72d42d3cc80fedd86f13c24a3c096b2517e772242` |
| **完整性清单** | `docs/architecture/pet-rust/spec.integrity.json` | 自动化生成绑定清单 | `32595ee5a9ee005a14b1c5e343f61fd2e38c821b9ce350e3a1504b567fbc52f5` |

---

## 2. 自动化 Linter 与 33 组负向测试套件结果

执行自动化测试套件命令：
```bash
python tests/spec_linter/test_runner.py
```

### 2.1 正向规范校验
- **测试结果**：`PASS (Exit Code 0)`
- **校验内容**：规范文件章节序列、目录（TOC）双向映射、代码围栏闭合、NUL 字节拦截、版本声明唯一性、注册表引用完整性。

### 2.2 负向测试套件覆盖率（33 / 33 全部捕获）
33 组负向破坏性测试用例已全部验证，且全部返回预期非 0 退出码：
1. `missing_section.md` -> [PASS] 捕获缺少 Section 16
2. `truncated_spec.md` -> [PASS] 捕获奇数代码块未闭合截断
3. `stale_58_gates.md` -> [PASS] 捕获使用过期的 58 个门禁计数
4. `stale_set_skip_taskbar.md` -> [PASS] 捕获使用 GNOME 废弃 API `set_skip_taskbar`
5. `wrong_version.md` -> [PASS] 捕获未声明 v1.4.5 标题
6. `candidate_claims_certified.md` -> [PASS] 捕获元组未经验证非法标为 Certified
7. `duplicate_muda_defaults.md` -> [PASS] 捕获 Muda 跨平台重复定义依赖
8. `broken_toc.md` -> [PASS] 捕获目录与正文章节编号断链
9. `missing_asset_security.md` -> [PASS] 捕获缺少资产协议沙箱安全章节
10. `nul_byte_truncated.md` -> [PASS] 捕获二进制 NUL 字符截断隐患
11. `missing_golden_source.md` -> [PASS] 捕获缺少 Golden 行为源 `live2d-view.tsx`
12. `missing_preload_abi.md` -> [PASS] 捕获 Preload ABI 缺少 `dropFiles` 接口
13. `active_12_dip_snap.md` -> [PASS] 捕获臆造的 12 DIP 贴边吸附逻辑
14. `wrong_fifo_path.md` -> [PASS] 捕获 FIFO 命令路径脱离权威路径
15. `normal_ui_engine_leak.md` -> [PASS] 捕获普通用户界面泄露 Rust/Electron 切换
16. `global_mutex_scope.md` -> [PASS] 捕获 Windows 单实例互斥量使用全局 `Global\`
17. `premature_candidate_tuples.md` -> [PASS] 捕获未经验证元组提前设为 Candidate
18. `inverted_acceptance_formula.md` -> [PASS] 捕获反转验收公式未包含 Blockers === 0
19. `fake_sha_manifest.md` -> [PASS] 捕获示例清单伪造真实 SHA-256
20. `proactive_fullscreen_detection.md` -> [PASS] 捕获主机主动检测前台全屏窗口
21. `desired_state_missing_fullscreen.md` -> [PASS] 捕获 DesiredOverlayState 缺失 fullscreen
22. `desired_state_missing_opacity.md` -> [PASS] 捕获 DesiredOverlayState 缺失 opacity
23. `missing_orthogonal_state.md` -> [PASS] 捕获缺失正交运行时状态枚举 (Lifecycle/Surface/Input)
24. `parent_eof_arbitrary_delay.md` -> [PASS] 捕获将父死检测与 2.5s 重启宽限期混淆
25. `renderer_owned_health.md` -> [PASS] 捕获 Renderer 进程非法拥有健康文件
26. `non_atomic_platform_tuple.md` -> [PASS] 捕获平台元组包含复合多版本字段
27. `unresolved_signature_decision.md` -> [PASS] 捕获签名方案保留“或/均可”多选一未决状态
28. `missing_snapshot_reader_section.md` -> [PASS] 捕获缺失 Section 10.2 快照读取规范
29. `fifo_missing_exact_envelope.md` -> [PASS] 捕获 FIFO 命令缺少 `created_at` 封装
30. `five_actions_context_menu.md` -> [PASS] 捕获右键菜单误写为 5 个常驻动作
31. `release_provisional_leak_gate.md` -> [PASS] 捕获内存泄漏门禁硬编码暂定 0.05 上限
32. `sprite_vague_hit_semantics.md` -> [PASS] 捕获 Sprite 命中检测使用模糊未验证描述
33. `interaction_snapshot_global_dip.md` -> [PASS] 捕获交互区域快照错误使用全局坐标

---

## 3. P0 架构整改全量闭环审计（P0-132 ~ P0-170）

### 3.1 状态模型、调谐与生命周期（Reconciliation & State Models）
1. **[P0-132] 彻底删除主动全屏检测，确立 Python 权威驱动**
   - 彻底删除主机探测前台全屏独占窗口代码与表述。
   - `fullscreen` 权威状态完全由 Python 业务快照下发。
   - 调谐规则严格遵循 Golden：`desired.fullscreen == true => overlay.hide()`; `desired.fullscreen == false => overlay.showInactive()`。
   - 边缘条件机械对齐：`visible=false && fullscreen=false` 注销 Surface；`visible=false && fullscreen=true` 维持实例但在视觉上隐藏。
2. **[P0-133] 恢复 DesiredOverlayState 与 AppliedOverlayState 完整字段**
   - 补齐 `fullscreen: bool` 与 `opacity: f64`。
   - 渲染器字段恢复强类型枚举：`enum RendererKind { Sprite, Live2D }`，严禁使用裸字符串。
3. **[P0-134] 恢复正交运行时状态枚举与调谐仲裁优先级**
   - 恢复三大正交状态：`HostLifecycle` (Booting, Probing, Running, Suspended, Degraded, ShuttingDown)、`SurfaceState` (Absent, Creating, Loading, Ready, Recovering)、`InputState` (PassThrough, Interactive, Dragging, MenuOpen, TextInput)。
   - 裁决优先级冻结：`ShuttingDown > Suspended > (visible=false && !fullscreen) > fullscreen=true > Recovering/Loading > Ready`。
   - 拖拽（Dragging）过程中遭遇全屏或隐藏突发事件，强制先执行 `AbortDrag` 释放光标，再执行状态转换。
4. **[P0-135] 彻底解耦 Liveness Pipe EOF 与 2.5s 替换宽限期**
   - Rust 主机检测到父进程管道 EOF 即刻执行 `ParentDeathShutdown`（立即退出，上限 500ms watchdog 清理资源）。
   - 2.5s 仅为 Python 编排器在重启引擎前等待旧进程退出的最长宽限时间，两者语义与实现彻底分立。
5. **[P0-146] 消除 pushState 命名歧义**
   - 规范区分两组状态通道：
     - `petOverlay.pushState`：Renderer -> Host 的单向控制通知。
     - `host_publish_state()`：Host -> Renderer 的调谐状态广播。

### 3.2 通信契约、健康机制与文件系统（IPC & Health Protocol）
6. **[P0-136 & P0-137] 终结健康文件所有权混淆与 Payload 混乱**
   - `<bridge>.health.json`：Electron 宿主主进程通过 `reportHealth(...)` 独占写入。
   - `<bridge>.rust.health.json`：Rust 宿主专属独占写入。
   - Renderer 沙箱无写盘权，仅通过 IPC 上报 `renderer-ready` 与 `renderer-failed` 控制事件。
   - 明确分离 Legacy 与 Rust 健康载荷 Schema，严禁复用污染。
7. **[P0-142 & P0-143] 独立新增 §10.2 Golden SnapshotReader 契约**
   - 文件大小硬限制 <= 32MB，超限立即抛出异常。
   - 缓存特征签名机制：`${stat.ino}:${stat.mtimeNs}:${stat.ctimeNs}:${stat.size}`。
   - 明确规范要求 `format_version == 1`；JSON 解析错误或校验失败时严禁更新缓存签名，确保修正后可立即热重试。
   - 将 Windows 平台文件唯一标识差异列为经验验证项 `VAL-51`。
8. **[P0-144 & P0-145] 恢复 FIFO 精确封包与队列容量前置检查**
   - 消息封包格式：`JSON.stringify({ command, created_at: Date.now() })`。
   - 前置容量检查：`queued.length >= 128` 时抛出 `pet_command_queue_full` 异常拒绝，队列内允许 0..127 个既有文件。
9. **[P0-149 & P0-150] 严格对齐 Renderer 崩溃恢复时序与退避阶梯**
   - 捕获 `render-process-gone` 时先向宿主上报 `pet_renderer_crashed` 失败状态。
   - 滑动窗口统计过去 60 秒内崩溃次数，若 `>= 3` 次则触发熔断；否则按阶梯延迟重载：`500ms * count`（500ms、1000ms、1500ms，第 4 次熔断）。
10. **[P0-151] 健康文件原子化写入契约**
    - 所有健康状态更新强制执行“写入 `<target>.tmp` -> 原子重命名覆盖”，避免并发半写入冲突。

### 3.3 交互细节、右键菜单与命中检测（UI & Hit Testing）
11. **[P0-147 & P0-148] 恢复右键菜单 4 个交互动作模型**
    - 菜单动作严格限定为 4 项：Pet, Feed, Play, Rest/Wake。
    - 根据 `life.resting` 状态动态互斥切换文案与冷却检测键：`cooldowns['rest']` / `cooldowns['wake']`。
    - 顶层包含角色切换子菜单、打开主阅读器入口、禁用标题行与分隔线。
12. **[P0-160] 机械复刻 Sprite 像素级透明命中检测**
    - 从 Golden 源码 `pet-overlay-app.tsx` 提取：非 canvas 交互性 DOM 元素直接信任 DOM 命中测试；canvas 元素采样 2D 像素 alpha >= 16 (`ALPHA_HIT_THRESHOLD`)，上下文读取异常时安全回退返回 `true`。

### 3.4 平台认证矩阵与 Wayland 几何（Platform Matrix & Geometry）
13. **[P0-138 & P0-139] 平台元组 100% 原子化与主键定义**
    - 彻底拆分复合字段（消除 `/`, `~`, `+`, `40/42`, `46/50` 等组合声明），保证各字段均为单值枚举。
    - 引入全局稳定主键 `tuple_key`（如 `linux-ubuntu_24_04-gnome_wayland-x86_64`）。
    - 24 个平台元组生命周期统一置为 `Planned`，认证状态统一置为 `Pending VAL`。
14. **[P0-152] 补齐 T-24 (Deepin Treeland) 缺失后端**
    - 补齐计划后端为 `LayerShellBackend`，状态标定为 `Pending VAL`。
15. **[P0-161 & P0-162] Wayland 几何坐标系类型安全定义**
    - 严格隔离坐标类型：`CssPxPoint`, `CssPxRect`, `SurfaceLocalDipRect`, `BridgeGlobalDipRect`, `OutputLocalDipRect`, `PhysicalPxRect`。
    - 交互区域快照 `InteractionRegionSnapshot` 明确采用 Surface 局部坐标系 `SurfaceLocalDipRect`。
16. **[P0-163] LayerShell 独占区候选行为列为经验项**
    - `exclusive_zone(0)` 标记为候选行为，列入 `VAL-31` 物理验证。
17. **[P0-164 & P0-165] GNOME 穿透与多窗口消歧机制确立**
    - GNOME 点击穿透实现路径标定为 `VAL-46`。
    - 窗口消歧通过组合 PID + GTK Application ID + 窗口自定义 Tag 标记，标定为 `VAL-47`。

### 3.5 运行时安全与打包机制（Security & Verification）
18. **[P0-140 & P0-141] 运行时签名校验器单一决策闭环**
    - 正式决策：选定唯一生产方案为 Python 侧 `PetRuntimeInstallerV2`（基于 `cryptography>=42.0.0`，要求 Python >= 3.10）。
    - 离线分发打包可用性作为经验项 `VAL-44`，Rust 校验器降级为离线/应急备用。
19. **[P0-166 & P0-167] 规范资产清单结构示例**
    - 明确规范中的 `size: 0` 仅为示例格式，生产签名清单由 CI 流水线注入真实 SHA-256 与文件大小。
20. **[P0-168] 防回滚机制与状态机明确**
    - 定义 `security_epoch` 持久化防回滚状态机，作为经验项 `VAL-48` 验证。

### 3.6 门禁、阻塞项与发布准则（Gates, Blockers & Criteria）
21. **[P0-153 & P0-170] 严格分层发布停止条件与解耦暂定门禁**
    - 严格划定三层停止条件：
      1. **Architecture Freeze**：所有 Architecture-Scoped Blockers == 0 且 架构经验项闭环。
      2. **Tuple Certification**：各平台对应 `VAL-xx` 测试全数通过 (PASS)。
      3. **Product Release**：发版所需元组完成认证。
    - 72 小时内存泄漏门禁解耦：0.05 MiB/h 标记为暂定上限，实际以物理测试基线动态校准。
22. **[P0-154 & P0-155] 阻塞项注册表 Schema 升级与全量设计闭环**
    - 升级 Schema 增加 `design_state: "resolved"`、`validation_state: "pending"`、`decision_id`、`blocking_until`、`scope`、`applies_to`。
    - 全部 33 个架构阻塞项（BLOCKER-01 ~ BLOCKER-33）在设计语义层面均已闭环，无二义性或多选一保留。
23. **[P0-156 & P0-158] 自动生成注释与统计数据一致性**
    - 规范内统计表头增加 `<!-- GENERATED: ... -->` 标记，由 linter 确保文本叙述与 JSON 数据 100% 吻合。
24. **[P0-157] 附录 29.4 生成真实完整性快照**
    - 附录 29.4 成功绑定真实 SHA-256、Git Commit 与各表项计数。

---

## 4. Phase 0 物理硬件验证执行规划

当前架构规格已达 `v1.4.5-Candidate`（语义闭环候选），所有 33 个阻塞项均处于 `design_state: "resolved"` 且 `validation_state: "pending"` 状态。进入 Phase 0 后，必须在真实物理机/虚拟机环境中完成以下硬件 Spike，方可逐步将验证状态置为 `PASS`：

| 验证编号 | 目标平台 / 环境 | 核心验证内容 | 验证目标产物 |
| :--- | :--- | :--- | :--- |
| **VAL-01 / 06 / 30** | Linux Wayland (Ubuntu 24.04 / GNOME 46) | WebKitGTK 透明无边框窗口渲染 + `zwlr_layer_shell_v1` 透明区域点击穿透动态更新 | 独立 Spike 可执行文件与录屏日志 |
| **VAL-46 / 47** | GNOME Shell 42 ~ 47 (X11 / Wayland) | GNOME Companion 扩展与 Rust 宿主 Unix Domain Socket 状态同步与多窗口消歧 | GJS 扩展源码与 Socket 压测脚本 |
| **VAL-11 / 14 / 49** | Windows 10/11 (x86_64 & ARM64) | WebView2 透明窗口 + DWM 混合层点击穿透与 CPU 占用基线 | Windows 自动化穿透测试套件 |
| **VAL-20 / 30** | macOS 13 ~ 15 (Apple Silicon & Intel) | Cocoa WKWebView 多 Spaces 漫游透明悬浮窗口 + 点击穿透响应 | macOS 独立 PoC 工程与活动监视器基线 |
| **VAL-44 / 48** | 跨平台离线安装包 | Python `cryptography` 打包体积与 `security_epoch` 防回滚持久化 | 安装器单体安装包与回滚攻击测试用例 |

---

## 5. 架构冻结判定确认

```text
================================================================================
                    READMD ARCHITECTURAL GATE STATUS
================================================================================
Current Candidate Version : v1.4.5-Candidate
Milestone Stage           : Semantic Closure Candidate
Phase 0 PoC Verification  : PENDING (51 Validation Items Pending Physical Run)
Phase 1 Production Rust   : FORBIDDEN (No implementation code until Phase 0 PASS)
Production Freeze Verdict : NO — Production Architecture Freeze
================================================================================
```
