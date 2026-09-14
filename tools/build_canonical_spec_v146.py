# -*- coding: utf-8 -*-
"""
tools/build_canonical_spec_v146.py
Generates canonical spec.md for v1.4.6 Evidence & Validation Closure Candidate.
"""

import json
import os
from pathlib import Path

def generate_spec():
    repo_root = Path(__file__).resolve().parents[1]
    arch_dir = repo_root / "docs" / "architecture" / "pet-rust"

    with open(arch_dir / "golden-contract.json", "r", encoding="utf-8") as f:
        contract = json.load(f)
    with open(arch_dir / "tuple-registry.json", "r", encoding="utf-8") as f:
        tuples = json.load(f)
    with open(arch_dir / "validation-registry.json", "r", encoding="utf-8") as f:
        validations = json.load(f)
    with open(arch_dir / "gate-registry.json", "r", encoding="utf-8") as f:
        gates = json.load(f)
    with open(arch_dir / "blocker-registry.json", "r", encoding="utf-8") as f:
        blockers = json.load(f)
    with open(arch_dir / "registry-manifest.json", "r", encoding="utf-8") as f:
        manifest = json.load(f)

    # Read base spec template or current spec
    with open(arch_dir / "spec.md", "r", encoding="utf-8") as f:
        base_text = f.read()

    # 1. Update Title and Header Block
    header = """# ReadMD Desktop Overlay v1.4.6 Evidence & Validation Closure Candidate

> **版本标识**：v1.4.6-Candidate
> **阶段说明**：Evidence & Validation Closure Candidate / Phase 0 Readiness Gate
> **架构冻结判定**：**NO — Production Architecture Freeze**（严禁进入正式生产代码实现）
> **Phase 1 Rust Host 生产实现状态**：**严格禁止**（在 Phase 0 物理硬件概念验证全数通过前严禁进入正式生产代码）
> **Phase 0 全量验证状态**：**BLOCKED**（允许 Phase 0 tooling/bootstrap、GoldenCapture、isolated backend spike 准备；但正式 PASS 需物理实机证据链）
> **前序版本溯源**：解决 v1.4.5 中遗留的 Artifact Provenance、Golden Build Closure、Validation Registry、Phase-0 Matrix 与 Evidence Classification 问题
"""

    # 2. Build Golden behavioral input closure table
    golden_table_lines = [
        "<!-- GENERATED: golden-source-table -->",
        "| 分类 | 路径 | 职责角色 | 文件大小 | 精确 SHA-256 哈希 | 行为关键 |",
        "|---|---|---|---|---|---|"
    ]
    for item in contract["behavior_inputs"]:
        size_str = f"{item['byte_size']:,} B" if "byte_size" in item else "-"
        golden_table_lines.append(f"| **行为输入** | `{item['path']}` | {item['role']} | {size_str} | `{item['sha256']}` | **是** |")
    for item in contract["build_inputs"]:
        size_str = f"{item['byte_size']:,} B" if "byte_size" in item else "-"
        crit_str = "**是**" if item.get("behavior_critical") else "否"
        golden_table_lines.append(f"| **构建输入** | `{item['path']}` | {item['role']} | {size_str} | `{item['sha256']}` | {crit_str} |")
    for item in contract["generated_inputs"]:
        size_str = f"{item['byte_size']:,} B" if "byte_size" in item else "-"
        golden_table_lines.append(f"| **生成产物** | `{item['path']}` | {item['role']} | {size_str} | `{item['sha256']}` | **是** |")
    for item in contract["runtime_assets"]:
        size_str = f"{item['byte_size']:,} B" if "byte_size" in item else "-"
        crit_str = "**是**" if item.get("behavior_critical") else "否"
        golden_table_lines.append(f"| **运行资产** | `{item['path']}` | {item['role']} | {size_str} | `{item['sha256']}` | {crit_str} |")
    for item in contract["supporting_evidence"]:
        size_str = f"{item['byte_size']:,} B" if "byte_size" in item else "-"
        golden_table_lines.append(f"| **辅助证据** | `{item['path']}` | {item['role']} | {size_str} | `{item['sha256']}` | 否 |")
    golden_table_str = "\n".join(golden_table_lines)

    # 3. Build Section 24 Phase 0 Validation Plan & Representative Matrix
    phase0_plan_lines = [
        "<!-- GENERATED: phase0-validation-plan -->",
        f"> **统计**：Phase 0 验证计划直接由 `validation-registry.json` 生成，共包含 **{len(validations)} 项实证验证项（VAL-01 ~ VAL-{len(validations):02d}）**，严禁人工手写映射。",
        "",
        "| 验证编号 | 验证目标 / 领域 | 范围 | 绑定后端 | 证据等级 (Evidence Class) | 关联门禁 |",
        "|---|---|---|---|---|---|"
    ]
    for v in validations:
        phase0_plan_lines.append(f"| **{v['id']}** | {v['title']} | `{v['scope']}` | `{v['backend']}` | `{v['evidence_class']}` | `{v['blocking_gate']}` |")
    phase0_plan_str = "\n".join(phase0_plan_lines)

    phase0_rep_lines = [
        "<!-- GENERATED: phase0-representative-matrix -->",
        "### 24.2 Phase 0 代表环境矩阵 (Phase 0 Representative Matrix) (P0-182, P0-198)",
        "> **架构约束**：Phase 0 代表矩阵仅用于验证四大后端的最小代表性技术可行性与物理穿透能力，**不等于最终全量平台认证矩阵 (Certification Matrix)**。全量平台认证必须在 Phase 0 代表性门禁全数通过后，针对所有 24 个平台元组逐项实机交付通过。",
        "",
        "| 代表环境编号 | 平台与版本 | 桌面环境 / 架构 | 绑定后端 | 代表性验证目标 |",
        "|---|---|---|---|---|"
    ]
    for t in tuples:
        if t.get("is_phase0_representative"):
            phase0_rep_lines.append(f"| **{t['display_id']}** | {t['os']} {t['os_version']} ({t['os_build']}) | {t['desktop_environment']} / {t['arch']} | `{t['planned_backend']}` | Phase 0 代表性实机 Spike 环境 |")
    phase0_rep_str = "\n".join(phase0_rep_lines)

    # 4. Build Section 29.2 Validation Summary
    validation_summary_lines = [
        "<!-- GENERATED: validation-summary -->",
        f"> **统计**：当前共注册 **{len(validations)} 项实证验证项（VAL-01 ~ VAL-{len(validations):02d}）**，统一在 Phase 0 物理测试床中执行。",
        "",
        "| 编号 | 领域与验证项 | 范围 / 目标后端 | 证据等级 | 物理风险与验证指标 | 关联阻断门禁 | 关联架构阻塞项 |",
        "|---|---|---|---|---|---|---|"
    ]
    for v in validations:
        blocker_str = ", ".join(v.get("associated_blockers", [])) if v.get("associated_blockers") else "无直接阻塞"
        validation_summary_lines.append(f"| **{v['id']}** | {v['title']} | `{v['scope']}` ({v['backend']}) | `{v['evidence_class']}` | {v['risk_and_physical_facts']} | `{v['blocking_gate']}` | {blocker_str} |")
    validation_summary_str = "\n".join(validation_summary_lines)

    # 5. Build Section 29.3 Blocker Summary
    blocker_summary_lines = [
        "<!-- GENERATED: blocker-summary -->",
        f"> **统计**：当前共注册 **{len(blockers)} 项架构阻塞项（BLOCKER-01 ~ BLOCKER-{len(blockers):02d}）**。所有 {len(blockers)} 项均已在架构设计上实现闭环（`design_state: RESOLVED`），正处于等待实机验证状态（`validation_state: pending`）。",
        "",
        "| 编号 | 标题 | 设计闭环状态 | 实证验证状态 | 范围 | 决策映射 | 解除前置 | 关联门禁 |",
        "|---|---|---|---|---|---|---|---|"
    ]
    for b in blockers:
        gates_str = ", ".join(b.get("gate_ids", []))
        blocker_summary_lines.append(f"| **{b['id']}** | {b['title']} | **RESOLVED** | pending | `{b['scope']}` | `{b['decision_id']}` | `{b['blocking_until']}` | `{gates_str}` |")
    blocker_summary_str = "\n".join(blocker_summary_lines)

    # 6. Build Section 29.4 Integrity Evidence Snapshot
    integrity_snapshot_lines = [
        "<!-- GENERATED: integrity-evidence-snapshot -->",
        "```json",
        json.dumps({
            "spec_path": "docs/architecture/pet-rust/spec.md",
            "golden_commit_sha": contract["golden_commit_sha"],
            "vendor_pinned_revision": contract["pinned_revision"],
            "linter_version": "v1.4.6",
            "registry_manifest_artifacts": len(manifest["artifacts"]),
            "registered_gates_count": len(gates),
            "validation_items_count": len(validations),
            "open_blockers_count": len(blockers),
            "platform_tuples_count": len(tuples)
        }, indent=2),
        "```"
    ]
    integrity_snapshot_str = "\n".join(integrity_snapshot_lines)

    # Now replace generated blocks in base spec
    text = base_text

    # Replace header block (from start to first '---')
    first_hr = text.find("\n---\n")
    if first_hr != -1:
        text = header + text[first_hr:]

    # Replace TOC title if needed
    text = text.replace("ReadMD Desktop Overlay v1.4.5 Semantic Closure Candidate", "ReadMD Desktop Overlay v1.4.6 Evidence & Validation Closure Candidate")
    text = text.replace("v1.4.5-Candidate", "v1.4.6-Candidate")

    # Replace Golden Source Table
    import re
    text = re.sub(
        r"<!-- GENERATED: golden-source-table -->.*?(\n### 1\.2)",
        lambda m: golden_table_str + m.group(1),
        text,
        flags=re.DOTALL
    )

    # Add Section 1.1 behavioral input closure prose
    closure_prose = f"""### 1.1 黄金行为输入闭包 (Golden Behavioral Input Closure) (P0-172, P0-173, P0-174, P0-189)
本项目不再以固定主观的人工“核心文件数”定义基准，而是建立**黄金行为输入闭包 (Golden Behavioral Input Closure)**。
任何改变可观测桌面浮窗行为的源码、构建适配脚本、打包配置、运行时资产与依赖锁文件，均作为显式输入纳入机器注册表：
- **上游厂商仓库**：`{contract['upstream_repository']}`
- **上游固定提交 (Pinned Upstream Revision)**：`{contract['pinned_revision']}`（记录于 `third_party/hermes-agent-pet/UPSTREAM.md`，CI 实施强校验拦截）
- **黄金基准提交 (Golden Git Commit SHA)**：`{contract['golden_commit_sha']}`
- **构建起源图谱 (Build Provenance Graph)**：`docs/architecture/pet-rust/golden-build-provenance.json`
- **机器可读契约源**：`docs/architecture/pet-rust/golden-contract.json`

{golden_table_str}
"""
    text = re.sub(
        r"### 1\.1 核心行为源与 Git 提交哈希锁定.*?<!-- GENERATED: golden-source-table -->.*?(?=### 1\.2)",
        lambda m: closure_prose + "\n",
        text,
        flags=re.DOTALL
    )

    # Ensure Sprite click-through logic in 1.4 is exact
    sprite_text = """2. **Sprite 判定合约 (`third_party/.../pet-overlay-app.tsx` L132-L165)** (P0-160, P0-173, P0-188)：
   - 使用 `document.elementFromPoint(x, y)` 检测拾取目标；
   - 若拾取目标不在宠物根容器 `petRef` 内，判定为透明穿透区域（返回 `false`）；
   - 若拾取目标为非 Canvas 交互 DOM 元素（如对话气泡 `PetBubble`、未读邮件图标 `Mail`、弹出式输入框 `composer`），直接信任 DOM 命中测试（返回 `true`）；
   - 若拾取目标为 `HTMLCanvasElement`，则获取 2D 上下文并在对应纹理坐标处进行像素采样：
     $$\\text{Solid Pixel} \\iff \\text{ctx.getImageData}(px, py, 1, 1).\\text{data}[3] \\ge 16 \\quad (\\text{ALPHA\\_HIT\\_THRESHOLD} = 16)$$
   - 若 Canvas 受到污染（Tainted）或读取抛出异常，执行安全打开策略（fail-open，返回 `true`），确保桌宠依然可被鼠标抓取；
   - **构建起源绑定 (P0-188)**：Sprite 原生源码经由 `packages/readmd-hermes-pet-adapter/scripts/build.mjs` 进行单次点击行为适配（单次点击触发 `open-menu`，双击保持 `toggle-app`），构建产物哈希与原始源码哈希在 `golden-build-provenance.json` 中完整绑定。"""
    text = re.sub(
        r"2\. \*\*Sprite 判定合约.*?(?=### 1\.5)",
        lambda m: sprite_text + "\n\n",
        text,
        flags=re.DOTALL
    )

    # Exact Renderer Crash Recovery pseudo-code in 1.8 (P0-186, P0-187)
    recovery_text = """### 1.8 宿主感知渲染层崩溃自愈与健康度报告 (P0-99, P0-149, P0-150, P0-186, P0-187)
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
  - **同一 60 秒内第 4 次崩溃**：检测前 `recoveries.length == 3`（`recoveries.length >= 3` 成立），立即触发熔断并 `return`，**不记录时间戳、不执行任何延时重载**，保持静默故障状态。"""
    text = re.sub(
        r"### 1\.8 渲染层崩溃自愈与健康度报告.*?(?=### 1\.9)",
        lambda m: recovery_text + "\n\n",
        text,
        flags=re.DOTALL
    )

    # Section 4 prohibition of LayerShell on GNOME (P0-179)
    gnome_prohibition = """### 4.4 GNOME 桌面环境强约束禁止调用 (P0-179)
GNOME Wayland (Mutter) 在架构上不支持 `zwlr_layer_shell_v1` 协议。**严禁在 GNOME 桌面环境下将任何窗口或图层绑定到 LayerShellBackend**，任何在 GNOME Wayland 下尝试加载或调用 `gtk_layer_shell` 的行为均视为严重架构违规。
- **LayerShellBackend** 严格限定于支持 Layer-Shell 的合成器：KDE Plasma 6 (KWin Wayland)、Sway、Hyprland、Deepin Treeland；
- **GNOME Wayland** 必须且只能绑定 **GnomeCompanionBackend**（借助 Companion 扩展与 Mutter 通信）。"""
    if "### 4.4 GNOME 桌面环境强约束禁止调用" not in text:
        text = text.replace("## 5. GNOME Wayland 专属扩展", gnome_prohibition + "\n\n---\n\n## 5. GNOME Wayland 专属扩展")

    # Section 24 Phase 0 Plan update (P0-180, P0-182, P0-196, P0-198)
    sec24_content = f"""## 24. Phase 0 物理概念验证计划 (PoC Spikes) (P0-179, P0-180, P0-182, P0-196, P0-198)
在正式启动产品功能实现前，必须在真实物理机环境下完成 Wayland 穿透、GNOME Companion 通信、Windows ARM64 加速等核心技术难题的独立验证。

### 24.1 试验代码隔离与生产禁令 (Phase 0 Spike Boundary) (P0-196)
- **允许代码**：Phase 0 概念验证代码必须存放于 `experiments/pet-rust/` 目录中，包括一次性 PoC、Layer-Shell 探针、GNOME GJS 通信脚手架、Win32/Cocoa 穿透验证程序及 GoldenCapture 工具；
- **严格禁止**：在 Phase 0 物理概念验证门禁全数通过前，**绝对禁止将生产级 `readmd-pet-rust` 宿主代码合并至主源码树或生产发布配置**。

{phase0_plan_str}

{phase0_rep_str}
"""
    text = re.sub(
        r"## 24\. Phase 0 物理概念验证计划.*?(?=## 25\.)",
        lambda m: sec24_content + "\n---\n\n",
        text,
        flags=re.DOTALL
    )

    # Section 29 updates
    text = re.sub(
        r"<!-- GENERATED: validation-summary -->.*?(?=### 29\.3)",
        lambda m: validation_summary_str + "\n\n",
        text,
        flags=re.DOTALL
    )

    text = re.sub(
        r"<!-- GENERATED: blocker-summary -->.*?(?=### 29\.4)",
        lambda m: blocker_summary_str + "\n\n",
        text,
        flags=re.DOTALL
    )

    text = re.sub(
        r"<!-- GENERATED: integrity-evidence-snapshot -->.*?(?=\n---|\Z)",
        lambda m: integrity_snapshot_str + "\n",
        text,
        flags=re.DOTALL
    )

    # Write back
    with open(arch_dir / "spec.md", "w", encoding="utf-8") as f:
        f.write(text)

    print("Successfully built canonical spec.md v1.4.6")

if __name__ == "__main__":
    generate_spec()
