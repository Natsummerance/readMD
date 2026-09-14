# -*- coding: utf-8 -*-
"""
tools/verify_spec_consistency.py
Authoritative Linter and Architectural Consistency Verifier for ReadMD Desktop Overlay Specification.
Version: v1.4.6 (Evidence & Validation Closure Candidate)
"""

import os
import sys
import re
import json
import hashlib
import argparse
import subprocess
from pathlib import Path

def get_git_commit(repo_root):
    try:
        res = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=str(repo_root),
            capture_output=True,
            text=True,
            check=True
        )
        return res.stdout.strip()
    except Exception:
        return "unknown_git_commit"

def verify_registries_integrity(arch_dir):
    errors = []
    gate_file = arch_dir / "gate-registry.json"
    val_file = arch_dir / "validation-registry.json"
    blocker_file = arch_dir / "blocker-registry.json"
    golden_file = arch_dir / "golden-contract.json"
    tuple_file = arch_dir / "tuple-registry.json"
    provenance_file = arch_dir / "golden-build-provenance.json"
    manifest_file = arch_dir / "registry-manifest.json"
    integrity_file = arch_dir / "spec.integrity.json"

    for f, name in [
        (gate_file, "gate-registry.json"),
        (val_file, "validation-registry.json"),
        (blocker_file, "blocker-registry.json"),
        (golden_file, "golden-contract.json"),
        (tuple_file, "tuple-registry.json"),
        (provenance_file, "golden-build-provenance.json"),
        (manifest_file, "registry-manifest.json"),
        (integrity_file, "spec.integrity.json")
    ]:
        if not f.exists():
            errors.append(f"Machine registry file missing: {name}")
            return False, errors, 0, 0, 0, 0, 0

    try:
        gates = json.loads(gate_file.read_text(encoding="utf-8"))
        validations = json.loads(val_file.read_text(encoding="utf-8"))
        blockers = json.loads(blocker_file.read_text(encoding="utf-8"))
        golden = json.loads(golden_file.read_text(encoding="utf-8"))
        tuples = json.loads(tuple_file.read_text(encoding="utf-8"))
        provenance = json.loads(provenance_file.read_text(encoding="utf-8"))
        manifest = json.loads(manifest_file.read_text(encoding="utf-8"))
        integrity = json.loads(integrity_file.read_text(encoding="utf-8"))
    except Exception as e:
        errors.append(f"JSON parsing error in registry files: {e}")
        return False, errors, 0, 0, 0, 0, 0

    gate_ids = {g["id"] for g in gates}
    val_ids = {v["id"] for v in validations}
    blocker_ids = {b["id"] for b in blockers}
    tuple_keys = {t["tuple_key"] for t in tuples}

    # Referential checks: Validations -> Gates & Blockers
    for v in validations:
        bg = v.get("blocking_gate")
        if bg and bg not in gate_ids:
            errors.append(f"Referential error: Validation {v['id']} references unknown gate '{bg}'")
        for b in v.get("associated_blockers", []):
            if b not in blocker_ids:
                errors.append(f"Referential error: Validation {v['id']} references unknown blocker '{b}'")
        for tkey in v.get("tuple_keys", []):
            if tkey not in tuple_keys:
                errors.append(f"Referential error: Validation {v['id']} references unknown tuple '{tkey}'")

    # Referential checks: Blockers -> Gates & Validations
    for b in blockers:
        for g in b.get("gate_ids", []):
            if g not in gate_ids:
                errors.append(f"Referential error: Blocker {b['id']} references unknown gate '{g}'")
        bu = b.get("blocking_until")
        if bu and bu not in val_ids:
            errors.append(f"Referential error: Blocker {b['id']} blocking_until references unknown validation '{bu}'")
        if b.get("design_state") == "open":
            errors.append(f"Blocker {b['id']} has design_state 'open'; all architecture design decisions must be resolved in v1.4.6")
        if b.get("validation_state") != "pending":
            errors.append(f"Blocker {b['id']} validation_state must remain 'pending' before physical hardware spikes")

    # Check behavior inputs in golden contract (P0-172, P0-173, P0-174)
    behavior_inputs = {item["path"]: item for item in golden.get("behavior_inputs", [])}
    req_sprite = "third_party/hermes-agent-pet/apps/desktop/src/app/pet-overlay/pet-overlay-app.tsx"
    req_build = "packages/readmd-hermes-pet-adapter/scripts/build.mjs"

    if req_sprite not in behavior_inputs:
        errors.append(f"Golden Contract missing essential sprite behavior input: {req_sprite}")
    elif not behavior_inputs[req_sprite].get("behavior_critical"):
        errors.append(f"{req_sprite} must have behavior_critical = true")

    if req_build not in behavior_inputs:
        errors.append(f"Golden Contract missing build-time adaptation script: {req_build}")
    elif not behavior_inputs[req_build].get("behavior_critical"):
        errors.append(f"{req_build} must have behavior_critical = true")

    # Check tuples atomicity and GNOME backend prohibition (P0-179)
    for t in tuples:
        tkey = t.get("tuple_key", "")
        if "/" in tkey or "~" in tkey or "+" in tkey:
            errors.append(f"Platform Tuple key '{tkey}' is non-atomic (contains forbidden delimiter)")
        for field in ["os", "os_version", "arch", "display_protocol", "compositor", "planned_backend", "lifecycle"]:
            val_str = str(t.get(field, ""))
            if "/" in val_str or "~" in val_str:
                errors.append(f"Platform Tuple {tkey} field '{field}'='{val_str}' contains multiple version values")
        if t.get("desktop_environment") == "GNOME" and t.get("planned_backend") == "LayerShellBackend":
            errors.append(f"Platform Tuple {tkey} assigns LayerShellBackend to GNOME (physically unsupported)")

    # Integrity file must not self-reference (P0-192)
    if "spec.integrity.json" in integrity.get("artifacts", {}):
        errors.append("spec.integrity.json contains recursive self-reference in artifacts dict")

    return len(errors) == 0, errors, len(gates), len(validations), len(blockers), len(tuples), len(manifest["artifacts"])

def run_checks(spec_path, repo_root, is_fixture=False):
    errors = []

    spec_path = spec_path.resolve()
    repo_root = repo_root.resolve()
    arch_dir = repo_root / "docs" / "architecture" / "pet-rust"

    if not is_fixture:
        path_str = str(spec_path).lower()
        if ".gemini" in path_str or "scratch" in path_str or "brain" in path_str:
            errors.append(f"FATAL: Spec path {spec_path} is in unauthorized private/scratch directory!")
            return False, errors

        try:
            rel_path = spec_path.relative_to(repo_root)
            if str(rel_path).replace("\\", "/") != "docs/architecture/pet-rust/spec.md":
                errors.append(f"FATAL: Canonical spec must be at docs/architecture/pet-rust/spec.md, got {rel_path}")
        except ValueError:
            errors.append(f"FATAL: Spec path {spec_path} is outside repository root {repo_root}")

    if not spec_path.exists():
        errors.append(f"Specification file does not exist: {spec_path}")
        return False, errors

    data = spec_path.read_bytes()
    nul_count = data.count(b'\x00')
    if nul_count > 0:
        errors.append(f"File contains {nul_count} binary NUL bytes (truncation hazard)")

    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as e:
        errors.append(f"File cannot be decoded as valid UTF-8: {e}")
        return False, errors

    lines = text.splitlines()
    line_count = len(lines)

    # 1. Structural checks
    if text.count("```") % 2 != 0:
        errors.append("Unclosed code fence detected (count of ``` is odd)")

    if line_count < 500:
        errors.append(f"File too short ({line_count} lines), expected full specification (>=500 lines)")

    # Mandatory sections 0 to 35
    for i in range(36):
        pattern = rf'^##\s+{i}\.'
        if not re.search(pattern, text, re.MULTILINE):
            errors.append(f"Missing mandatory Section {i}")

    # 2. TOC checks
    toc_start = text.find("## 目录")
    if toc_start == -1:
        errors.append("Missing '## 目录' Table of Contents")
    else:
        toc_end = text.find("## 0.")
        if toc_end == -1:
            errors.append("Malformed document structure: Section 0 not found after TOC")
        else:
            toc_text = text[toc_start:toc_end]
            for i in range(36):
                if f"[{i}." not in toc_text:
                    errors.append(f"TOC does not reference Section {i}")

    # 3. Version checks (v1.4.6)
    if "v1.4.6" not in lines[0]:
        errors.append("Header does not declare 'v1.4.6'")
    if "> **版本标识**：v1.4.6-Candidate" not in text and "> **版本**：v1.4.6-Candidate" not in text:
        errors.append("Metadata block does not declare 'v1.4.6-Candidate'")

    # Check status line for forbidden premature freeze
    for line in lines[:25]:
        if line.startswith("> **版本**：") or line.startswith("> **状态说明**："):
            if "Production-Freeze" in line or ("Production Architecture Freeze" in line and "绝非" not in line and "not" not in line.lower() and "no" not in line.lower()):
                errors.append(f"Premature status declared in metadata block: {line.strip()}")

    # 4. Golden Behavioral Input Closure checks (P0-172, P0-173, P0-174)
    if "live2d/stage.ts" not in text:
        errors.append("Golden Source Set missing required behavior source 'live2d/stage.ts'")
    if "pet-overlay-app.tsx" not in text:
        errors.append("Golden Source Set missing required sprite behavior source 'pet-overlay-app.tsx'")
    if "build.mjs" not in text:
        errors.append("Golden Source Set missing required build adaptation source 'build.mjs'")

    # Ban hardcoded magic source count in prose (P0-172)
    for line in lines[:100]:
        if ("6 核心" in line or "7 核心" in line or "11 个辅助" in line) and "废弃" not in line and "历史" not in line:
            errors.append(f"Prose contains banned hardcoded golden source magic count: {line.strip()}")

    # 5. Preload ABI checks (P0-92)
    s1_idx = text.find("## 1.")
    s2_idx = text.find("## 2.")
    sec1 = text[s1_idx:s2_idx] if s1_idx != -1 and s2_idx != -1 else ""

    required_abi_methods = [
        "open(", "close()", "setBounds(", "setIgnoreMouse(", "setFocusable(",
        "pushState(", "control(", "onState(", "onControl(", "dropFiles("
    ]
    for method in required_abi_methods:
        if method not in sec1:
            errors.append(f"Preload ABI missing required method definition: {method}")

    # 6. Invented 12-DIP snap check (P0-93)
    for line in lines:
        if "吸附检测距离固定为 12 DIP" in line or ("12 DIP" in line and "吸附" in line and not any(k in line for k in ["剔除", "删除", "虚构", "伪设定", "banned"])):
            errors.append(f"Active usage of fabricated '12 DIP snap/吸附阈值' logic: {line.strip()}")

    # 7. FIFO Path check (P0-100, P0-144, P0-145)
    for line in lines:
        if "<runtime_dir>/events/" in line and not any(k in line for k in ["严禁", "禁止", "虚构", "banned"]):
            errors.append(f"FIFO documented under incorrect path: {line.strip()}")
    if "${bridge}.commands" not in text:
        errors.append("FIFO missing authoritative directory definition '${bridge}.commands'")

    # Section 10 checks (P0-144, P0-145, P0-142)
    s10_idx = text.find("## 10.")
    s11_idx = text.find("## 11.")
    sec10 = text[s10_idx:s11_idx] if s10_idx != -1 and s11_idx != -1 else ""
    if "created_at: Date.now()" not in sec10:
        errors.append("Section 10 missing exact FIFO command envelope with 'created_at: Date.now()'")
    if "10.2" not in sec10 or "SnapshotReader" not in sec10:
        errors.append("Section 10 missing dedicated Section 10.2 'Golden SnapshotReader Contract'")
    if "stat.ino" not in sec10 or "stat.mtimeNs" not in sec10:
        errors.append("Section 10.2 SnapshotReader missing Node.js Golden signature formula '${stat.ino}:${stat.mtimeNs}:${stat.ctimeNs}:${stat.size}'")

    # 8. User UI check (P0-104)
    sec2 = text[s2_idx:text.find("## 3.")] if s2_idx != -1 else ""
    if 'desktop_pet_engine:' in sec2 and ('"electron"' in sec2 or '"rust"' in sec2):
        errors.append("Normal user UI incorrectly exposes 'electron' / 'rust' engine selection")

    # 9. Mutex check (P0-105)
    sec14 = text[text.find("## 14."):text.find("## 15.")] if text.find("## 14.") != -1 else ""
    for line in sec14.splitlines():
        if "Global\\" in line and not any(k in line for k in ["严禁", "禁止", "banned", "forbidden"]):
            errors.append(f"Windows single instance mutex uses forbidden Global namespace: {line.strip()}")

    # 10. Platform Tuples Lifecycle & Atomicity check (P0-107, P0-138, P0-139, P0-152)
    sec20 = text[text.find("## 20."):text.find("## 21.")] if text.find("## 20.") != -1 else ""
    tuple_rows = [l for l in sec20.splitlines() if re.search(r'\|\s*\*\*T-\d+\*\*', l)]
    for row in tuple_rows:
        if "**Candidate**" in row or "**Certified**" in row:
            errors.append(f"Platform Tuple in Section 20 prematurely marked as Candidate/Certified: {row.strip()}")
        cols = [c.strip() for c in row.split("|")[1:-1]]
        if len(cols) >= 4:
            if "40/42" in cols[2] or "46/50" in cols[2] or "GNOME 46/50" in row:
                errors.append(f"Platform Tuple contains multi-version specification '{row.strip()}'")

    # 11. Final Acceptance Formula check (P0-108, P0-153)
    sec27 = text[text.find("## 27."):text.find("## 28.")] if text.find("## 27.") != -1 else ""
    if "Unresolved Architecture Blockers === 0" not in sec27 and "Unresolved Architecture Blockers} === 0" not in sec27:
        errors.append("Final acceptance formula does not enforce 'Unresolved Architecture Blockers === 0'")

    # 11b. Manifest security check (P0-114, P0-115)
    sec23 = text[text.find("## 23."):text.find("## 24.")] if text.find("## 23.") != -1 else ""
    if "e3b0c44298fc1c149afbf4c8996fb924" in sec23:
        errors.append("Manifest example uses production-looking fake SHA256 string instead of obvious placeholder")

    # 12. Fullscreen Proactive Detection ban check (P0-132)
    for idx, line in enumerate(lines):
        for banned_phrase in ["检测到前台存在全屏", "detect foreground fullscreen", "scan fullscreen window", "exclusive application detection"]:
            if banned_phrase in line:
                if not any(k in line for k in ["严禁", "禁止", "不探测", "非 Golden", "banned", "never", "must not"]):
                    errors.append(f"Active usage of banned proactive fullscreen detection logic at line {idx+1}: {line.strip()}")

    # 13. Reconciliation Fullness check (P0-133, P0-134)
    s16_idx = text.find("## 16.")
    s17_idx = text.find("## 17.")
    sec16 = text[s16_idx:s17_idx] if s16_idx != -1 and s17_idx != -1 else ""
    if "fullscreen: bool" not in sec16:
        errors.append("Section 16 DesiredOverlayState missing 'fullscreen: bool'")
    if "opacity: f64" not in sec16:
        errors.append("Section 16 DesiredOverlayState or AppliedOverlayState missing 'opacity: f64'")
    if "RendererKind" not in sec16:
        errors.append("Section 16 missing RendererKind enum definition (Sprite, Live2D)")
    if "pub enum HostLifecycle {" not in sec16 or "pub enum SurfaceState {" not in sec16 or "pub enum InputState {" not in sec16:
        errors.append("Section 16 missing Orthogonal Runtime State enums (HostLifecycle, SurfaceState, InputState)")

    # 14. Parent Liveness timing check (P0-135)
    s15_idx = text.find("## 15.")
    sec15 = text[s15_idx:s16_idx] if s15_idx != -1 and s16_idx != -1 else ""
    if "触发 2.5 秒倒计时安全退出" in sec15 or ("2.5 秒" in sec15 and "ParentDeath" in sec15 and not any(k in sec15 for k in ["严禁", "非 Golden", "解耦", "绝非"])):
        errors.append("Parent Liveness incorrectly conflated with 2.5s delay on pipe EOF")

    # 15. Health Ownership check (P0-136, P0-137)
    s17_idx = text.find("## 17.")
    s18_idx = text.find("## 18.")
    sec17 = text[s17_idx:s18_idx] if s17_idx != -1 and s18_idx != -1 else ""
    if "由前端渲染层定时回写" in sec17 or "由渲染层定期写" in sec17:
        errors.append("Legacy health file incorrectly documented as owned/written by renderer")

    # 16. Context Menu actions count check (P0-147)
    if "右键菜单 5 项动作" in text:
        errors.append("Context menu incorrectly described as '5 项动作' instead of 4 interaction actions + submenu + reader")

    # 17. Release Stop 0.05 MiB/h hard gate check (P0-169)
    s25_idx = text.find("## 25.")
    s26_idx = text.find("## 26.")
    sec25 = text[s25_idx:s26_idx] if s25_idx != -1 and s26_idx != -1 else ""
    if "> 0.05" in sec25 and "provisional" not in sec25 and "预算" not in sec25:
        errors.append("Section 25 incorrectly uses provisional 0.05 MiB/h as hard release gate")

    # 18. Sprite Hit Semantics check (P0-160)
    if "有效可视像素或 DOM 矩形区域" in text:
        errors.append("Sprite hit semantics contain vague unverified '有效可视像素或 DOM 矩形区域'")

    # 19. Wayland Input Region coordinate space check (P0-161)
    s6_idx = text.find("## 6.")
    s7_idx = text.find("## 7.")
    sec6 = text[s6_idx:s7_idx] if s6_idx != -1 and s7_idx != -1 else ""
    if "Vec<BridgeDipRect>" in sec6:
        errors.append("InteractionRegionSnapshot incorrectly uses global BridgeDipRect instead of SurfaceLocalDipRect")

    # 20. Signature Verifier ADR check (P0-140, P0-197)
    if "均可" in sec23 and "ADR" in sec23:
        errors.append("ADR-runtime-signature-verifier contains unresolved '均可' instead of choosing single production verifier")
    if "fallback production verifier" in text.lower():
        errors.append("Rust verifier incorrectly described as fallback production verifier; must be non-production contingency prototype")

    # 21. GNOME Layer-Shell backend prohibition check (P0-179)
    sec4 = text[text.find("## 4."):text.find("## 5.")] if text.find("## 4.") != -1 else ""
    if "不支持 `zwlr_layer_shell_v1`" not in sec4 and "不支持 zwlr_layer_shell_v1" not in sec4:
        errors.append("Section 4 missing explicit prohibition of LayerShellBackend on GNOME Wayland")

    # 22. Renderer crash recovery exact boundary & logic (P0-186, P0-187)
    if "recoveries.length >= 3" not in sec1 or "delay = 500 * recoveries.length" not in sec1:
        errors.append("Section 1.8 missing exact Golden renderer recovery logic and backoff delay formula")

    # 23. Cargo Section 18 checks
    s18 = text.find("## 18.")
    s19 = text.find("## 19.")
    if s18 == -1 or s19 == -1:
        errors.append("Section 18 (Cargo Specification) missing or unclosed")
    else:
        sec18 = text[s18:s19]
        if "[dependencies]" in sec18 and "[target" in sec18:
            common_deps = sec18.split("[dependencies]")[1].split("[target")[0]
            if "muda =" in common_deps:
                errors.append("Cargo check failed: muda is present in common [dependencies] (muda feature contamination)")
        if "v0_6" not in sec18:
            errors.append("Cargo check failed: gtk-layer-shell missing required feature 'v0_6'")
        if "os-webview" not in sec18:
            errors.append("Cargo check failed: wry missing required feature 'os-webview'")
        if "linux-production" not in sec18:
            errors.append("Cargo check failed: linux-production feature profile missing")

    # 24. Stale symbols checks
    s31_idx = text.find("## 31.")
    main_spec = text[:s31_idx] if s31_idx != -1 else text
    for line in main_spec.splitlines():
        if "set_skip_taskbar" in line:
            if not any(k in line for k in ["剔除", "虚构", "禁止", "废弃", "replaced", "removed"]):
                errors.append(f"Active usage of stale GNOME API 'set_skip_taskbar': {line.strip()}")
        if "58 道" in line or "58 gates" in line.lower():
            if not any(k in line for k in ["废弃", "历史", "旧", "replaced", "removed"]):
                errors.append(f"Active usage of legacy hardcoded '58 道' gates count: {line.strip()}")
        if "<=64" in line or "<= 64" in line:
            if not any(k in line for k in ["剔除", "删除", "废弃", "历史", "removed"]):
                errors.append(f"Lossy hit-region approximation <=64 found in main spec: {line.strip()}")
        if "WebViewBuilder::new(&window)" in line:
            if not any(k in line for k in ["严禁", "已弃用", "已废弃", "deprecated", "forbidden"]):
                errors.append(f"Active usage of invalid WRY API 'WebViewBuilder::new(&window)': {line.strip()}")

    # 25. Machine Registries Referential Integrity
    if not is_fixture and arch_dir.exists():
        reg_ok, reg_errs, g_cnt, v_cnt, b_cnt, t_cnt, m_cnt = verify_registries_integrity(arch_dir)
        if not reg_ok:
            errors.extend(reg_errs)

        # Verify dynamic summaries in spec prose match computed registry counts
        if f"{v_cnt} 项实证验证项" not in text:
            errors.append(f"Spec prose does not reflect computed validation count ({v_cnt} 项)")
        if f"{b_cnt} 项架构阻塞项" not in text:
            errors.append(f"Spec prose does not reflect computed blocker count ({b_cnt} 项)")

    return len(errors) == 0, errors

def main():
    parser = argparse.ArgumentParser(description="Verify ReadMD Desktop Overlay Specification Consistency.")
    parser.add_argument("--spec", type=str, default="docs/architecture/pet-rust/spec.md", help="Path to spec file")
    parser.add_argument("--fixture-mode", action="store_true", help="Run in test fixture mode (skips repo root checks)")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    spec_path = Path(args.spec)
    if not spec_path.is_absolute():
        spec_path = repo_root / spec_path

    ok, errors = run_checks(spec_path, repo_root, is_fixture=args.fixture_mode)
    if not ok:
        print(f"[-] Specification verification FAILED with {len(errors)} errors:")
        for idx, err in enumerate(errors, 1):
            print(f"  [{idx:02d}] {err}")
        sys.exit(1)
    else:
        print(f"[+] Specification verified clean: {spec_path}")
        sys.exit(0)

if __name__ == "__main__":
    main()
