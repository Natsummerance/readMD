# ReadMD Desktop Overlay v1.4.7 Reproducible Evidence Candidate 架构整改与终审报告

- **报告标识**：`REPORT-PET-OVERLAY-V147-EVIDENCE-CLOSURE`
- **目标规格**：`ReadMD Desktop Overlay Architecture Specification v1.4.7-Candidate`
- **版本阶段**：`Reproducible Evidence Candidate` (可复现证据候选)
- **基线版本**：`v1.4.6 Evidence & Validation Closure Candidate`
- **代码仓库**：`Natsummerance/readMD`
- **Git 分支**：`main`
- **报告所描述的提交 (parent commit SHA)**：`a90a3fd87dbfcc745b82dbdff037e9cba936cac9`
  _（此报告描述其生成时所在的提交，而非报告文件本身最终落入的提交）_
- **远端提交 SHA**：`a90a3fd87dbfcc745b82dbdff037e9cba936cac9`
- **工作树状态**：`DIRTY (tracked files modified)`
- **Git HEAD 出处状态**：`DIRTY_HEAD_MATCHES_REMOTE`
- **制品出处状态 (Artifact Provenance State)**：`UNVERIFIED DIRTY WORKTREE — REMOTE_VERIFIED blocked until clean commit is pushed`
- **架构冻结结论**：**`NO — Production Architecture Freeze`**
- **Phase 1 生产实现准入**：**`STRICTLY FORBIDDEN`**（严禁编写正式 Rust Host 生产代码）
- **Phase 0 全量验证启动**：**`BLOCKED`**（允许 Tooling / GoldenCapture / isolated backend spike 准备；实机验证待硬件证据链输入）

---

## 1. 代码仓库出处与远端可复现性凭据 (P0-171, Gate-Evidence-RemoteProvenance)

```json
{
  "repository": "Natsummerance/readMD",
  "branch": "main",
  "local_commit_sha": "a90a3fd87dbfcc745b82dbdff037e9cba936cac9",
  "remote_commit_sha": "a90a3fd87dbfcc745b82dbdff037e9cba936cac9",
  "working_tree_clean": false,
  "git_head_provenance_state": "DIRTY_HEAD_MATCHES_REMOTE",
  "artifact_provenance_state": "UNCOMMITTED_EVIDENCE",
  "provenance_state": "UNCOMMITTED_EVIDENCE"
}
```

> [!NOTE]
> 当前处于 `LOCAL_ONLY` 模式，所有指标属于本地工作树证据 (local working tree evidence only)。推送至远端 GitHub 分支后将自动跃迁至 `REMOTE_VERIFIED`。

---

## 2. 机器注册表清单与外部完整性校验基线 (P0-178, P0-191, P0-192)

通过 `registry-manifest.json` 机器自动枚举所有架构资产（共 **9 个注册表资产**），消除人工硬编码数量漂移。

| 资产角色 | 规范路径 | 行数 | 字节数 | SHA-256 校验和 |
| :--- | :--- | :--- | :--- | :--- |
| **`golden_contract`** | [`docs/architecture/pet-rust/golden-contract.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/golden-contract.json) | 347 | 14,987 B | `a316a1fa97c86fb6b50c394c7f51b67c78a072d819540b3e400f990af004925c` |
| **`golden_build_provenance`** | [`docs/architecture/pet-rust/golden-build-provenance.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/golden-build-provenance.json) | 228 | 8,349 B | `8735b71b7475e9699093a42b1ef1a01ba8260d318a9c90c1632468e7c12404e0` |
| **`tuple_registry`** | [`docs/architecture/pet-rust/tuple-registry.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/tuple-registry.json) | 490 | 15,304 B | `b37b992d7fca571976586aa98ad7ae0a10ceded31f7545e89c504276c00412c5` |
| **`validation_registry`** | [`docs/architecture/pet-rust/validation-registry.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/validation-registry.json) | 1032 | 33,886 B | `c988996b59fbd59834e6220f91fa0ff293a3c644285d22dd4c27afb7da705746` |
| **`gate_registry`** | [`docs/architecture/pet-rust/gate-registry.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/gate-registry.json) | 839 | 18,688 B | `85aaa3d637de38009085d4ec1b8f6a1fb7eb7c0c9f797cd2cb7031394c75f019` |
| **`blocker_registry`** | [`docs/architecture/pet-rust/blocker-registry.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/blocker-registry.json) | 664 | 19,267 B | `7e8120e87f74b8ba96501de72d42d3cc80fedd86f13c24a3c096b2517e772242` |
| **`spec_integrity`** | [`docs/architecture/pet-rust/spec.integrity.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/spec.integrity.json) | 27 | 1,307 B | `b19d153948ef9169fdeb60fd183b7277e6b43064ca029d6b1e1f22a336c45fd8` |
| **`preload_abi_fixture`** | [`docs/architecture/pet-rust/preload-abi.fixture.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/preload-abi.fixture.json) | 96 | 2,602 B | `bc0e613f6c3b8e6fe3f2896a866a4a1218194e47e8e486c728ee5092b1b7a7ff` |
| **`pet_architecture_attestation`** | [`docs/architecture/pet-rust/pet-architecture-attestation.json`](file:///T:/Programming/Project/codex/creator/readmd/docs/architecture/pet-rust/pet-architecture-attestation.json) | 21 | 1,215 B | `bd3139e842a6423a37a581239c64e511bbe604e13f4faa33ea75317ffc034384` |

> **完整性解耦原则 (P0-192)**：`spec.integrity.json` 记录所有其他资产的哈希，其自身的哈希由报告生成器在写入后外部独立计算，杜绝自引悖论。

---

## 3. 黄金行为输入闭包与构建起源图谱 (P0-172, P0-173, P0-174, P0-175, P0-188, P0-189)

- **上游厂商**：`hermes-agent-pet` (`https://github.com/NousResearch/hermes-agent`)
- **上游固定提交 (Pinned Upstream Revision)**：`fb27614addac115d55299bc6538ae112fd01f688`（记录于 `third_party/hermes-agent-pet/UPSTREAM.md`）
- **黄金基准提交 (Migration Golden Commit SHA)**：`4dcfd73ce81a14ace7e429791e0594bea47b24e5`

### 3.1 闭包输入统计 (Behavioral Input Closure Counts)

- **行为输入 (Behavior Inputs)**：`9 个`（包含 `pet-overlay-app.tsx` 与 `build.mjs`，均为 `behavior_critical = true`）
- **构建输入 (Build Inputs)**：`3 个`
- **生成输入 (Generated Inputs)**：`3 个`
- **运行资产 (Runtime Assets)**：`5 个`
- **辅助证据 (Supporting Evidence)**：`2 个`

### 3.2 Sprite 真实构建起源哈希绑定 (P0-188)

```json
{
  "upstream_source_sha256": "9b2c3d492d0f2adaaa4e2b7ac536e988dea1908208ee7a321fa23203b2c7357d",
  "adaptation_script_sha256": "59fe50c2d31841aadef9013d2b47b107f49dac62434cebb5d3106bb97ca51db2",
  "generated_source_sha256": "ea266476b462fa8e6e6cd3aa1a3dbce96cad2718ebdb4e541aa8cc5eb9d0a3a5",
  "renderer_bundle_sha256": "d9f1da3457bac312a2790d270f6052de02c87290a19d623ee5d23bfb378ccb9d"
}
```

---

## 4. Phase 0 代表环境矩阵与全量认证矩阵解耦 (P0-182, P0-198)

严格划分最小代表性环境与全量认证矩阵：

| 代表编号 | 操作系统与版本 | 桌面环境 / 架构 | 绑定后端 | 代表性目标 |
| :--- | :--- | :--- | :--- | :--- |
| **T-01** | Windows 11 (24H2) | DWM / x86_64 | `Win32Backend` | Phase 0 代表性 Spike 试验环境 |
| **T-03** | Windows 11 (24H2) | DWM / aarch64 | `Win32Backend` | Phase 0 代表性 Spike 试验环境 |
| **T-05** | macOS 26 (Tahoe) | Aqua / aarch64 | `CocoaBackend` | Phase 0 代表性 Spike 试验环境 |
| **T-12** | Ubuntu 24.04 (LTS) | GNOME / x86_64 | `GnomeCompanionBackend` | Phase 0 代表性 Spike 试验环境 |
| **T-14** | Ubuntu 22.04 (LTS) | GNOME / x86_64 | `X11Backend` | Phase 0 代表性 Spike 试验环境 |
| **T-16** | Fedora 40 (Standard) | KDE / x86_64 | `LayerShellBackend` | Phase 0 代表性 Spike 试验环境 |
| **T-18** | Fedora 42 (Standard) | GNOME / x86_64 | `GnomeCompanionBackend` | Phase 0 代表性 Spike 试验环境 |
| **T-19** | ArchLinux Rolling (Current) | Sway / x86_64 | `LayerShellBackend` | Phase 0 代表性 Spike 试验环境 |
| **T-22** | UOS 20 (SP1) | DDE / aarch64 | `X11Backend` | Phase 0 代表性 Spike 试验环境 |
| **T-25** | Fedora 44 (Workstation) | GNOME / x86_64 | `GnomeCompanionBackend` | Phase 0 代表性 Spike 试验环境 |
| **T-26** | Fedora 44 (KDE Plasma Desktop) | KDE / x86_64 | `LayerShellBackend` | Phase 0 代表性 Spike 试验环境 |
| **T-27** | Windows 11 (25H2) | DWM / x86_64 | `Win32Backend` | Phase 0 代表性 Spike 试验环境 |
| **T-28** | Windows 11 (26H1) | DWM / aarch64 | `Win32Backend` | Phase 0 代表性 Spike 试验环境 |

> [!IMPORTANT]
> **GNOME 约束 (P0-179)**：GNOME Wayland 绝对不支持 LayerShell 协议，严禁将 LayerShellBackend 绑定至 GNOME。LayerShellBackend 仅在 KDE/Sway/Hyprland 代表环境中验证；GNOME Wayland 专属验证 GnomeCompanionBackend。

---

## 5. 实证验证项与证据等级 (P0-180, P0-181, P0-185, P0-195)

当前注册 **57 项实证验证项（VAL-01 ~ VAL-57）**，直接由 `validation-registry.json` 导出。

| 编号 | 领域与验证项 | 范围 | 绑定后端 | 证据等级 (Evidence Class) | 关联门禁 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **VAL-01** | Layer-Shell + WRY 容器 | `backend` | `LayerShellBackend` | `physical` | `Gate-LayerShell-02` |
| **VAL-02** | Layer-Shell 跨屏拖拽 | `backend` | `LayerShellBackend` | `physical` | `Gate-LayerShell-01` |
| **VAL-03** | Layer-Shell 按需输入法 | `backend` | `LayerShellBackend` | `physical` | `Gate-LayerShell-03` |
| **VAL-04** | GNOME Companion 跨版本稳定性 | `backend` | `GnomeCompanionBackend` | `physical` | `Gate-GnomeCompanion-02` |
| **VAL-05** | Windows ARM64 渲染基准 | `tuple` | `Win32Backend` | `physical_gpu_required` | `CERT-WIN11-ARM64` |
| **VAL-06** | 国产 Linux 发行版依赖 | `tuple` | `All` | `integration` | `CERT-UOS20-X64` |
| **VAL-07** | Linux 通用单二进制 PoC | `architecture` | `All` | `integration` | `Gate-Linux-Universal` |
| **VAL-08** | muda GTK 菜单在 KWin Wayland | `backend` | `LayerShellBackend` | `physical` | `Gate-Menu-KWin-Wayland` |
| **VAL-09** | muda GTK 菜单在 GNOME Wayland | `backend` | `GnomeCompanionBackend` | `physical` | `Gate-Menu-GNOME-Wayland` |
| **VAL-10** | WRY custom DnD 副作用 | `architecture` | `All` | `integration` | `Gate-Core-10` |
| **VAL-11** | 调和状态机事件序列 Fuzz | `architecture` | `All` | `integration` | `Gate-Core-11` |
| **VAL-12** | LayerShell 全局转局部坐标映射 | `backend` | `LayerShellBackend` | `physical` | `Gate-LayerShell-04` |
| **VAL-13** | GNOME move_frame 混合 DPI 映射 | `backend` | `GnomeCompanionBackend` | `physical` | `Gate-GnomeCompanion-03` |
| **VAL-14** | GNOME Legacy (<=44) Companion | `tuple` | `GnomeCompanionBackend` | `physical` | `CERT-UBUNTU22-X64` |
| **VAL-15** | GNOME ESM (>=45) Companion | `tuple` | `GnomeCompanionBackend` | `physical` | `CERT-UBUNTU24-X64` |
| **VAL-16** | Windows HTTPS 资产 Scheme | `architecture` | `Win32Backend` | `physical` | `Gate-Asset-HTTPS-Win` |
| **VAL-17** | Custom Protocol 路径沙盒 | `architecture` | `All` | `integration` | `Gate-Asset-Sandbox` |
| **VAL-18** | 统信 UOS 运行时 WebKitGTK 验证 | `tuple` | `All` | `integration` | `CERT-UOS20-ARM64` |
| **VAL-19** | 银河麒麟运行时 WebKitGTK 验证 | `tuple` | `All` | `integration` | `CERT-KYLIN10-ARM64` |
| **VAL-20** | macOS 13~26 支持周期认证 | `backend` | `CocoaBackend` | `physical` | `CERT-MACOS-ALL` |
| **VAL-21** | Layer-Shell Pre-Realize 生命周期 | `backend` | `LayerShellBackend` | `physical` | `Gate-LayerShell-05` |
| **VAL-22** | GNOME Meta.Window AppID 传播 | `backend` | `GnomeCompanionBackend` | `physical` | `Gate-GnomeCompanion-04` |
| **VAL-23** | GNOME Companion 畸形 IPC Fuzz | `backend` | `GnomeCompanionBackend` | `integration` | `Gate-GnomeCompanion-05` |
| **VAL-24** | Wayland 工作区与面板避让对齐 | `backend` | `LayerShellBackend` | `physical` | `Gate-LayerShell-06` |
| **VAL-25** | 导航代际隔离异步消息丢弃测试 | `architecture` | `All` | `integration` | `Gate-IPC-Generation` |
| **VAL-26** | 调和状态 AppliedState 异步生效竞态 | `architecture` | `All` | `integration` | `Gate-Core-Reconciliation` |
| **VAL-27** | 权威规范产物完整性与发布检查 | `architecture` | `All` | `integration` | `Gate-Core-SpecIntegrity` |
| **VAL-28** | muda 零 libxdo 依赖实测证明 | `architecture` | `All` | `integration` | `Gate-Linux-MudaTargetIsolation` |
| **VAL-29** | 当前 GNOME 50 扩展兼容性实测 | `tuple` | `GnomeCompanionBackend` | `physical` | `CERT-GNOME50-X64` |
| **VAL-30** | macOS 26 (Tahoe) 桌宠实机认证 | `tuple` | `CocoaBackend` | `physical` | `CERT-MACOS26-ARM64` |
| **VAL-31** | Layer-Shell compositor usable-area / exclusive-zone parity | `backend` | `LayerShellBackend` | `physical` | `Gate-LayerShell-06` |
| **VAL-32** | Golden preload ABI differential | `architecture` | `All` | `integration` | `Gate-Golden-ABI` |
| **VAL-33** | Golden bounds policy differential | `architecture` | `All` | `integration` | `Gate-Golden-Bounds` |
| **VAL-34** | Golden control/toggle-app differential | `architecture` | `All` | `integration` | `Gate-Golden-Differential` |
| **VAL-35** | Golden menu model differential | `architecture` | `All` | `integration` | `Gate-Golden-Menu` |
| **VAL-36** | Golden clipboard payload differential | `architecture` | `All` | `integration` | `Gate-Golden-Clipboard` |
| **VAL-37** | Golden FIFO byte/filename parity | `architecture` | `All` | `integration` | `Gate-Golden-FIFO` |
| **VAL-38** | Golden SnapshotReader retry parity | `architecture` | `All` | `integration` | `Gate-Golden-Differential` |
| **VAL-39** | Golden hit-region differential | `architecture` | `All` | `integration` | `Gate-Golden-HitRegion` |
| **VAL-40** | Golden pushState / anti-snapback differential | `architecture` | `All` | `integration` | `Gate-Golden-AntiSnapback` |
| **VAL-41** | Windows virtual-desktop Golden behavior | `backend` | `Win32Backend` | `physical` | `Gate-Win32-VirtualDesktop` |
| **VAL-42** | Runtime detached-signature verification | `architecture` | `All` | `integration` | `Gate-Crypto-DetachedSig` |
| **VAL-43** | Asset sandbox TOCTOU threat-model/handle test | `architecture` | `All` | `integration` | `Gate-Asset-TOCTOU` |
| **VAL-44** | Runtime Security & Packaging | `architecture` | `All` | `integration` | `Gate-Crypto-VerifierPackaging` |
| **VAL-45** | Wayland Hit Geometry Transformation | `backend` | `LayerShellBackend` | `physical` | `Gate-Wayland-TransformParity` |
| **VAL-46** | GNOME Shell Companion Click-Through | `backend` | `GnomeCompanionBackend` | `physical` | `Gate-GnomeCompanion-ClickThrough` |
| **VAL-47** | GNOME Multi-Window Disambiguation | `backend` | `GnomeCompanionBackend` | `integration` | `Gate-GnomeCompanion-Disambiguation` |
| **VAL-48** | Security Epoch Rollback Defense | `architecture` | `All` | `integration` | `Gate-Crypto-RollbackProtection` |
| **VAL-49** | Health Protocol Ownership & Atomic Write | `architecture` | `All` | `integration` | `Gate-Golden-Health` |
| **VAL-50** | Parent Liveness Prompt Teardown | `architecture` | `All` | `integration` | `Gate-Golden-ParentDeath` |
| **VAL-51** | Golden SnapshotReader Retry Differential | `architecture` | `All` | `integration` | `Gate-Golden-SnapshotReader` |
| **VAL-52** | Golden build provenance closure | `golden` | `All` | `integration` | `Gate-Golden-BuildProvenance` |
| **VAL-53** | Generated renderer bundle provenance | `golden` | `All` | `integration` | `Gate-Golden-BuildProvenance` |
| **VAL-54** | Report <-> Registry consistency | `core` | `All` | `unit` | `Gate-Report-Consistency` |
| **VAL-55** | Remote repository provenance reproducibility | `evidence` | `All` | `integration` | `Gate-Evidence-RemoteProvenance` |
| **VAL-56** | Phase0 plan registry generation correctness | `core` | `All` | `unit` | `Gate-Core-SpecIntegrity` |
| **VAL-57** | Validation evidence-class enforcement | `evidence` | `All` | `unit` | `Gate-Validation-EvidenceClass` |

> **证据等级铁律 (P0-185)**：标记为 `physical` 或 `physical_gpu_required` 的验证项，**虚拟机 (VM) 证据绝不可替代物理机实测**。

---

## 6. P0-171 至 P0-198 架构与证据链整改全量闭环审计

### 6.1 代码出处与仓库复现性

- **[P0-171] Git Commit 真实可复现性**：确立 `git rev-parse HEAD == remote GitHub commit` 验证通道，未推送时如实标记 `LOCAL_ONLY`，增加 `Gate-Evidence-RemoteProvenance`。
- **[P0-193] 报告生成器作为单一事实源**：新建 `tools/generate_pet_arch_report.py`，所有哈希、表项计数、映射关系均由机器提取渲染，杜绝人工手写漂移。
- **[P0-194] 报告一致性自检器**：新建 `tools/verify_report_consistency.py`，双向核验报告与注册表的一致性。

### 6.2 黄金行为输入闭包与构建图谱

- **[P0-172] 消除硬编码核心文件计数**：以黄金行为输入闭包取代“6/7 核心”概念，输入分类涵盖行为输入、构建输入、生成产物、运行时资产与辅助证据。
- **[P0-173] 纳入 Sprite 真实行为源**：上游 `pet-overlay-app.tsx` 升级为核心 `behavior_inputs`（标记 `behavior_critical: true`），彻底清除过期的 `live2d-view.tsx` 虚构路径。
- **[P0-174] 纳入 `build.mjs` 适配脚本**：生产单次点击行为来自构建期代码替换，`build.mjs` 标记为 `behavior_critical: true`。
- **[P0-175] 建立 Golden 构建起源图谱**：交付 `golden-build-provenance.json`，完整映射上游源码 -> 适配脚本 -> 编译产物 -> 运行时行为节点与边。
- **[P0-176] GoldenCapture 面向实际构建产物**：记录 `build_input_hash`、`renderer_bundle_hash`、`preload_bundle_hash` 与 `electron_main_bundle_hash`。
- **[P0-188] Sprite 行为与构建起源哈希绑定**：记录上游源码、适配脚本、生成中间源码与渲染包的四元哈希。
- **[P0-189] Pinned Hermes Revision 纳入机器契约**：绑定厂商固定提交 `fb27614addac115d55299bc6538ae112fd01f688`，CI 不一致即阻断。

### 6.3 注册表规范化与 Schema 验证

- **[P0-178] 注册表资产清单解耦**：交付 `registry-manifest.json`，以机器清单描述 7 大架构资产文件。
- **[P0-190] 语义化 Gate ID 与显示序号分离**：门禁保持 `Gate-Core-SpecIntegrity` 等语义标识为主键，增加 `display_order` 排序字段。
- **[P0-191] 全量 JSON Schema 强校验**：建立 `docs/architecture/pet-rust/schema/` 目录并交付全部注册表的 Schema 校验器。
- **[P0-192] 消除完整性文件自证悖论**：`spec.integrity.json` 排除自身哈希，自身校验由外部报告计算。

### 6.4 平台环境与 Phase 0 代表性规划

- **[P0-179] GNOME 绝对禁止 LayerShellBackend**：明确 Mutter 协议物理限制，GNOME 专属使用 GnomeCompanionBackend，LayerShellBackend 仅限 KDE/wlroots。
- **[P0-180 & P0-181] Validation ID 全局单义与 Phase 0 计划全自动生成**：修正 `VAL-30` 仅指代 macOS 26 Tahoe，消除跨平台重用，Phase 0 计划由注册表 100% 渲染。
- **[P0-182 & P0-198] Phase 0 代表矩阵与全量认证解耦**：确立 9 大 Phase 0 代表性环境（涵盖 Win11 x64/ARM64、macOS 26、KDE、Sway、GNOME 46/50、X11、UOS/Kylin）。
- **[P0-183] GNOME 50 进入当前验证计划**：代表性家族涵盖 GNOME 42 (Legacy)、46 (Early ESM)、50 (Current ESM)。
- **[P0-184] macOS 26 Tahoe 纳入当前验证计划**：覆盖 Apple Silicon 最新主版本。
- **[P0-185] 证据等级分类体系**：建立 `unit` / `integration` / `vm` / `physical` / `physical_gpu_required` 分级体系。

### 6.5 运行时行为与自愈协议澄清

- **[P0-186] 渲染层崩溃自愈精确伪代码**：冻结 `recoveries.filter(t => now - t < 60_000)` 顺序，前 3 次分别延时 500/1000/1500ms，第 4 次立即触发熔断。
- **[P0-187] 崩溃健康度报告主体澄清**：界定为宿主直接观测事件（Host-Observed Failure），Electron 主进程自身调用 `reportHealth`，Rust 宿主直接写入健康文件。
- **[P0-196] Phase 0 Spike 试验代码与生产实现隔离**：Spike 代码严格限定在 `experiments/pet-rust/`，严禁提前混入生产源码树。
- **[P0-197] 签名校验器边界澄清**：选定 Python `PetRuntimeInstallerV2` 为唯一生产实现，Rust 校验器定位为非生产应急备用原型。
- **[P0-195] Phase 0 状态只允许实测证据驱动**：禁止手动修改验证状态，PASS 必须附带真实环境指纹与日志证据。

---

## 7. 架构测试与工具集验证结果汇总

```text
================================================================================
                   READMD SPEC LINTER & VERIFICATION SUMMARY                    
================================================================================
Canonical Spec Verification    : PASS (0 errors, 1074 lines, NUL=0)
Golden Contract & Provenance   : PASS (tools/verify_golden_contract.py clean)
Registry Schemas & Integrity   : PASS (tools/verify_registry_integrity.py clean)
Report & Registry Parity       : PASS (tools/verify_report_consistency.py clean)
Negative & Mutation Test Suite : PASS (33 static + programmatic mutations rejected)
================================================================================
```

---

## 8. 架构最终判定结论

```text
================================================================================
                    READMD ARCHITECTURAL GATE STATUS                            
================================================================================
Current Version           : v1.4.7-Candidate
Milestone Stage           : Reproducible Evidence Candidate
Architecture Design State : 33/33 blockers resolved
Empirical Validations     : 0/57 empirical validations passed
Artifact Provenance State : UNCOMMITTED_EVIDENCE
Phase 0 Readiness         : BLOCKED_BY_EVIDENCE — artifacts are not REMOTE_VERIFIED with a clean worktree. Push a clean commit where all declared artifacts are committed before Phase 0 evidence can be validated remotely.
Phase 0 Full Validation   : BLOCKED (Awaiting physical hardware test execution)
Phase 1 Production Rust   : STRICTLY FORBIDDEN (No production implementation code)
Production Freeze Verdict : NO — Production Architecture Freeze
================================================================================
```
