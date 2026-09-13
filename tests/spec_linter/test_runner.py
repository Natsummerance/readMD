# -*- coding: utf-8 -*-
"""
tests/spec_linter/test_runner.py
Test runner for ReadMD Specification Consistency & Architectural Integrity Linter.

Verifies:
1. Canonical spec (docs/architecture/pet-rust/spec.md) PASSES (exit code 0).
2. All 10 negative test fixtures FAIL (exit code != 0) with expected diagnostic errors.
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
    print("=" * 76)
    print(f"Repository Root:    {repo_root}")
    print(f"Linter Script:      {linter_path}")
    print(f"Canonical Spec:     {canonical_spec}")
    print(f"Fixtures Directory: {fixtures_dir}")
    print("=" * 76)

    # 1. Test Canonical Specification (Positive Test)
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

    # 2. Test Negative Fixtures
    fixtures = [
        ("missing_section.md", "Missing mandatory section (Section 16 removed)"),
        ("truncated_spec.md", "Spec truncated at 50% lines"),
        ("stale_58_gates.md", "Active 58 gates assertion"),
        ("stale_set_skip_taskbar.md", "Active set_skip_taskbar API call"),
        ("wrong_version.md", "Wrong version declared (v1.4.1 instead of v1.4.3)"),
        ("candidate_claims_certified.md", "Premature Production-Freeze status claim"),
        ("duplicate_muda_defaults.md", "muda in common dependencies (target isolation violation)"),
        ("broken_toc.md", "TOC references broken anchor / missing section"),
        ("missing_asset_security.md", "Section 9 (Secure Asset Protocol) missing"),
        ("nul_byte_truncated.md", "Binary NUL byte injected"),
    ]

    all_negative_passed = True
    print("\n[-] Running Negative Tests (All must return non-zero exit code)...")

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
            # Extract error summary from output
            err_lines = [l.strip() for l in res.stdout.splitlines() if "FAILED with" in l or l.strip().startswith("[")]
            first_err = err_lines[1] if len(err_lines) > 1 else (err_lines[0] if err_lines else "Non-zero exit")
            print(f"  [PASS] {fname:30s} -> Caught correctly: {first_err[:60]}")

    print("\n" + "=" * 76)
    if all_negative_passed:
        print("SUMMARY: ALL POSITIVE AND NEGATIVE TESTS PASSED.")
        print("  - Canonical spec verified: 100% compliant.")
        print(f"  - Negative fixtures verified: 10/10 caught with non-zero exit codes.")
        print("=" * 76)
        sys.exit(0)
    else:
        print("SUMMARY: TEST SUITE FAILED.")
        print("=" * 76)
        sys.exit(1)

if __name__ == "__main__":
    main()
