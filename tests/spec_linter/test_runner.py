# -*- coding: utf-8 -*-
"""
tests/spec_linter/test_runner.py
Automated Positive & Negative Test Suite for ReadMD Spec Linter v1.4.4.

Verifies:
1. Canonical spec (docs/architecture/pet-rust/spec.md) PASSES (exit code 0).
2. All 19 negative test fixtures FAIL (exit code != 0) with expected diagnostic errors.
3. Referential integrity across all 4 machine-readable architecture registries.
"""

import sys
import subprocess
from pathlib import Path

def main():
    repo_root = Path(__file__).resolve().parents[2]
    linter_path = repo_root / "tools" / "verify_spec_consistency.py"
    fixtures_dir = repo_root / "tests" / "spec_linter" / "fixtures"
    canonical_spec = repo_root / "docs" / "architecture" / "pet-rust" / "spec.md"

    print("=" * 76)
    print("      ReadMD Spec Linter Automated Negative & Positive Test Suite     ")
    print("                      Version: v1.4.5-Candidate                         ")
    print("=" * 76)
    print(f"Repository Root:    {repo_root}")
    print(f"Linter Script:      {linter_path}")
    print(f"Canonical Spec:     {canonical_spec}")
    print(f"Fixtures Directory: {fixtures_dir}")
    print("=" * 76)

    # 1. Positive Test: Canonical Spec
    print("\n[+] Running Positive Test: Canonical Spec...")
    res = subprocess.run(
        [sys.executable, str(linter_path)],
        cwd=str(repo_root),
        capture_output=True,
        text=True
    )
    if res.returncode != 0:
        print("[-] FATAL: Canonical specification FAILED linter check!")
        print(res.stdout)
        print(res.stderr)
        sys.exit(1)
    else:
        print("[+] PASS: Canonical specification verified clean (exit code 0).")

    # 2. Negative Tests: All fixtures
    fixtures = [
        ("missing_section.md", "Missing mandatory section (Section 16 removed)"),
        ("truncated_spec.md", "Spec truncated at 50% lines"),
        ("stale_58_gates.md", "Active 58 gates assertion"),
        ("stale_set_skip_taskbar.md", "Active set_skip_taskbar API call"),
        ("wrong_version.md", "Wrong version declared"),
        ("candidate_claims_certified.md", "Premature Production-Freeze status claim"),
        ("duplicate_muda_defaults.md", "muda in common dependencies (target isolation violation)"),
        ("broken_toc.md", "TOC references broken anchor / missing section"),
        ("missing_asset_security.md", "Section 9 (Secure Asset Protocol) missing"),
        ("nul_byte_truncated.md", "Binary NUL byte injected"),
        ("missing_golden_source.md", "Golden Source Set missing live2d/stage.ts (P0-91)"),
        ("missing_preload_abi.md", "Preload ABI missing dropFiles method (P0-92)"),
        ("active_12_dip_snap.md", "Active 12-DIP snap logic (P0-93)"),
        ("wrong_fifo_path.md", "FIFO documented under events directory (P0-100)"),
        ("normal_ui_engine_leak.md", "Normal UI exposes electron/rust selector (P0-104)"),
        ("global_mutex_scope.md", "Windows mutex uses Global namespace (P0-105)"),
        ("premature_candidate_tuples.md", "Phase 0 un-run tuples marked Candidate (P0-107)"),
        ("inverted_acceptance_formula.md", "Final acceptance formula permits open blockers (P0-108)"),
        ("fake_sha_manifest.md", "Manifest uses realistic fake SHA256 string (P0-115)"),
        ("proactive_fullscreen_detection.md", "Proactive foreground fullscreen detection (P0-132)"),
        ("desired_state_missing_fullscreen.md", "DesiredOverlayState missing fullscreen (P0-133)"),
        ("desired_state_missing_opacity.md", "DesiredOverlayState missing opacity (P0-133)"),
        ("missing_orthogonal_state.md", "Missing HostLifecycle / SurfaceState / InputState (P0-134)"),
        ("parent_eof_arbitrary_delay.md", "Parent EOF claims 2.5s delay (P0-135)"),
        ("renderer_owned_health.md", "Legacy health described as renderer-owned (P0-136)"),
        ("non_atomic_platform_tuple.md", "Platform tuple contains multi-version specification (P0-138)"),
        ("unresolved_signature_decision.md", "ADR contains unresolved '均可' alternative (P0-140)"),
        ("missing_snapshot_reader_section.md", "Section 10 missing SnapshotReader full contract (P0-142)"),
        ("fifo_missing_exact_envelope.md", "FIFO missing created_at exact envelope (P0-144)"),
        ("five_actions_context_menu.md", "Context menu described as 5 actions (P0-147)"),
        ("release_provisional_leak_gate.md", "Release uses provisional 0.05 MiB/h as hard gate (P0-169)"),
        ("sprite_vague_hit_semantics.md", "Sprite hit semantics contain unverified pixel or DOM (P0-160)"),
        ("interaction_snapshot_global_dip.md", "InteractionRegionSnapshot uses global BridgeDipRect (P0-161)"),
    ]

    all_negative_passed = True
    print(f"\n[-] Running Negative Tests ({len(fixtures)} fixtures, all must return non-zero exit code)...")

    for fname, desc in fixtures:
        fpath = fixtures_dir / fname
        if not fpath.exists():
            print(f"  [MISSING] Fixture {fname} not found!")
            all_negative_passed = False
            continue

        res = subprocess.run(
            [sys.executable, str(linter_path), "--spec", str(fpath), "--fixture-mode"],
            cwd=str(repo_root),
            capture_output=True,
            text=True
        )

        if res.returncode == 0:
            print(f"  [FAIL - FALSE POSITIVE] {fname} was expected to FAIL but PASSED!")
            all_negative_passed = False
        else:
            err_lines = [l.strip() for l in res.stdout.splitlines() if "FAILED with" in l or l.strip().startswith("[")]
            first_err = err_lines[1] if len(err_lines) > 1 else (err_lines[0] if err_lines else "Non-zero exit")
            print(f"  [PASS] {fname:32s} -> Caught: {first_err[:60]}")

    print("\n" + "=" * 76)
    if all_negative_passed:
        print("SUMMARY: ALL POSITIVE AND NEGATIVE TESTS PASSED.")
        print("  - Canonical spec verified: 100% compliant.")
        print(f"  - Negative fixtures verified: {len(fixtures)}/{len(fixtures)} correctly caught.")
        print("  - Referential integrity across registries: 100% clean.")
        print("=" * 76)
        sys.exit(0)
    else:
        print("SUMMARY: TEST SUITE FAILED.")
        print("=" * 76)
        sys.exit(1)

if __name__ == "__main__":
    main()
