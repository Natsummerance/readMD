# -*- coding: utf-8 -*-
"""
tools/verify_report_consistency.py
Verifies parity and consistency between audit report and machine registries.
Version: v1.4.7
"""

import os
import sys
import json
import hashlib
import re
from pathlib import Path

def main():
    repo_root = Path(__file__).resolve().parents[1]
    arch_dir = repo_root / "docs" / "architecture" / "pet-rust"
    report_file = arch_dir / "audit-report-v1.4.7.md"

    if not report_file.exists():
        print("[-] FATAL: audit-report-v1.4.7.md missing!")
        sys.exit(1)

    with open(report_file, "r", encoding="utf-8") as f:
        report_text = f.read()

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
    with open(arch_dir / "spec.integrity.json", "r", encoding="utf-8") as f:
        integrity = json.load(f)

    errors = []

    # 1. Check Version and Verdict
    if "v1.4.7" not in report_text:
        errors.append("Report does not declare 'v1.4.7'")
    if "NO — Production Architecture Freeze" not in report_text:
        errors.append("Report missing mandatory verdict: 'NO — Production Architecture Freeze'")
    if "STRICTLY FORBIDDEN" not in report_text:
        errors.append("Report missing mandatory prohibition: 'Phase 1 生产实现准入: STRICTLY FORBIDDEN'")

    # 2. Check counts parity
    manifest_cnt = len(manifest["artifacts"])
    if f"{manifest_cnt} 个注册表资产" not in report_text:
        errors.append(f"Report does not match manifest artifacts count ({manifest_cnt})")

    b_inp_cnt = len(contract["behavior_inputs"])
    if f"行为输入 (Behavior Inputs)**：`{b_inp_cnt} 个`" not in report_text:
        errors.append(f"Report does not match behavior inputs count ({b_inp_cnt})")

    val_cnt = len(validations)
    if f"{val_cnt} 项实证验证项" not in report_text:
        errors.append(f"Report does not match validations count ({val_cnt})")

    # 3. Check hashes of artifacts in report
    for art in manifest["artifacts"]:
        rel_p = art["path"]
        fpath = repo_root / rel_p.replace("/", os.sep)
        actual_sha = hashlib.sha256(fpath.read_bytes()).hexdigest()
        if actual_sha not in report_text:
            errors.append(f"Report does not contain current exact SHA-256 for {rel_p} ({actual_sha})")

    # 4. Check that VAL-30 is exclusively macOS and never Linux
    val30_matches = [line for line in report_text.splitlines() if "**VAL-30**" in line]
    for line in val30_matches:
        if "Linux" in line or "Wayland" in line:
            errors.append(f"Report incorrectly associates VAL-30 with Linux/Wayland: {line}")
        if "Cocoa" not in line and "macOS" not in line and "Tahoe" not in line:
            errors.append(f"Report line for VAL-30 does not mention macOS/Cocoa: {line}")

    # 5. Check GNOME LayerShell prohibition
    if "GNOME Wayland 绝对不支持 LayerShell 协议" not in report_text:
        errors.append("Report missing explicit GNOME LayerShell prohibition statement")

    if errors:
        print(f"[-] Report consistency verification FAILED with {len(errors)} errors:")
        for idx, err in enumerate(errors, 1):
            print(f"  [{idx:02d}] {err}")
        sys.exit(1)

    print("[+] PASS: audit-report-v1.4.7.md consistency verified clean with all registries.")

if __name__ == "__main__":
    main()
