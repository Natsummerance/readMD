# -*- coding: utf-8 -*-
"""
tools/update_fixtures_v146.py
Regenerates all 33 negative test fixtures based on canonical v1.4.6 specification.
"""

import os
import re
from pathlib import Path

def main():
    repo_root = Path(__file__).resolve().parents[1]
    arch_dir = repo_root / "docs" / "architecture" / "pet-rust"
    fixtures_dir = repo_root / "tests" / "spec_linter" / "fixtures"

    canonical_text = (arch_dir / "spec.md").read_text(encoding="utf-8")

    mutations = {
        "missing_section.md": lambda t: re.sub(r'## 16\..*?(?=## 17\.)', '', t, flags=re.DOTALL),
        "truncated_spec.md": lambda t: t[:len(t)//2] + "\n```rust\nlet broken = true;\n",
        "stale_58_gates.md": lambda t: t.replace("## 21. 自动化质量门禁体系", "## 21. 自动化质量门禁体系\n\n全系统由 58 道质量门禁严格控制。"),
        "stale_set_skip_taskbar.md": lambda t: t.replace("- **去除废弃 API**：剔除已废弃的 `set_skip_taskbar`，统一调用 `metaWindow.hide_from_window_list()`。", "- **窗口属性控制**：统一调用 `metaWindow.set_skip_taskbar(true)` 隐藏任务栏图标。"),
        "wrong_version.md": lambda t: t.replace("v1.4.6", "v1.4.0"),
        "candidate_claims_certified.md": lambda t: t.replace("> **版本标识**：v1.4.6-Candidate", "> **版本**：Production-Freeze"),
        "duplicate_muda_defaults.md": lambda t: t.replace("[dependencies]\n", '[dependencies]\nmuda = "0.15"\n'),
        "broken_toc.md": lambda t: t.replace("- [8. ReadMD Pet IPC v1", "- [broken. ReadMD Pet IPC v1"),
        "missing_asset_security.md": lambda t: re.sub(r'## 9\..*?(?=## 10\.)', '', t, flags=re.DOTALL),
        "nul_byte_truncated.md": lambda t: t[:200] + "\x00" + t[200:],
        "missing_golden_source.md": lambda t: t.replace("pet-overlay-app.tsx", "arbitrary-unknown-pet.tsx"),
        "missing_preload_abi.md": lambda t: t.replace("dropFiles(files: string[]): void;", "// dropFiles removed"),
        "active_12_dip_snap.md": lambda t: t.replace("## 2. 桌面级 Shell 保真度契约", "## 2. 桌面级 Shell 保真度契约\n\n吸附检测距离固定为 12 DIP。"),
        "wrong_fifo_path.md": lambda t: t.replace("${bridge}.commands", "<runtime_dir>/events/"),
        "normal_ui_engine_leak.md": lambda t: t.replace("### 2.1 普通用户交互隔离", '### 2.1 普通用户交互隔离\n\n用户可通过设置项 desktop_pet_engine: "rust" 自由切换。'),
        "global_mutex_scope.md": lambda t: t.replace("Local\\ReadMDPetOverlay", "Global\\ReadMDPetOverlay"),
        "premature_candidate_tuples.md": lambda t: t.replace("| **T-01** | `windows-11-24h2-x64-dwm-win32` | Windows 11 (24H2) | x86_64 | DWM / DWM | `Win32Backend` | **Planned** |", "| **T-01** | `windows-11-24h2-x64-dwm-win32` | Windows 11 (24H2) | x86_64 | DWM / DWM | `Win32Backend` | **Candidate** |"),
        "inverted_acceptance_formula.md": lambda t: t.replace(r"\text{Unresolved Architecture Blockers} === 0", r"\text{Unresolved Architecture Blockers} !== 0"),
        "fake_sha_manifest.md": lambda t: t.replace("<64-hex-sha256-generated-at-build>", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
        "proactive_fullscreen_detection.md": lambda t: t.replace("### 1.10 Bridge 轮询时序", "### 1.10 Bridge 轮询时序\n\n若检测到前台存在全屏独占应用，则自动执行 hide()。"),
        "desired_state_missing_fullscreen.md": lambda t: t.replace("pub fullscreen: bool,", "// fullscreen removed"),
        "desired_state_missing_opacity.md": lambda t: t.replace("pub opacity: f64,", "// opacity removed"),
        "missing_orthogonal_state.md": lambda t: t.replace("pub enum HostLifecycle {", "// pub enum HostLifecycle removed {"),
        "parent_eof_arbitrary_delay.md": lambda t: t.replace("立即向主线程事件循环发送 `ShutdownParentGone` 事件", "触发 2.5 秒倒计时安全退出"),
        "renderer_owned_health.md": lambda t: t.replace("唯一所有者为 Electron 宿主主进程", "由前端渲染层定时回写更新健康文件"),
        "non_atomic_platform_tuple.md": lambda t: t.replace("Ubuntu 24.04 (LTS)", "Ubuntu 40/42 (LTS)"),
        "unresolved_signature_decision.md": lambda t: t.replace("### 23.3 验签决策闭环：ADR-runtime-signature-verifier (P0-140, P0-141)", "### 23.3 验签决策闭环：ADR-runtime-signature-verifier (P0-140, P0-141)\n- **决策**：方案 A 与方案 B 均可"),
        "missing_snapshot_reader_section.md": lambda t: re.sub(r'### 10\.2 Golden SnapshotReader 契约.*?(?=## 11\.)', '', t, flags=re.DOTALL),
        "fifo_missing_exact_envelope.md": lambda t: t.replace("created_at: Date.now()", "// created_at removed"),
        "five_actions_context_menu.md": lambda t: t.replace("4 项互动动作", "右键菜单 5 项动作"),
        "release_provisional_leak_gate.md": lambda t: t.replace("3. 若存在经批准的平台性能预算，实测内存泄露指标不得超出该预算。", "3. 内存泄露增长率不得 > 0.05 MiB/h，否则阻断发布。"),
        "sprite_vague_hit_semantics.md": lambda t: t.replace("## 1. Golden Contract 权威行为源与基准契约", "## 1. Golden Contract 权威行为源与基准契约\n\n有效可视像素或 DOM 矩形区域\n"),
        "interaction_snapshot_global_dip.md": lambda t: t.replace("pub rects: Vec<SurfaceLocalDipRect>", "pub rects: Vec<BridgeDipRect>")
    }

    for fname, func in mutations.items():
        fpath = fixtures_dir / fname
        mutated_text = func(canonical_text)
        with open(fpath, "w", encoding="utf-8") as f:
            f.write(mutated_text)
        print(f"Regenerated fixture: {fname}")

if __name__ == "__main__":
    main()
