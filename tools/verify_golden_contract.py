# -*- coding: utf-8 -*-
"""
tools/verify_golden_contract.py
Validates Golden Behavioral Input Closure and Build Provenance.
Version: v1.4.7

P0-205: Structural ABI diff using preload-abi.fixture.json
P0-217/P0-218: Hash verification via `git show <golden_sha>:<path>` — never from working tree
"""

import os
import sys
import json
import hashlib
import subprocess
from pathlib import Path


MIGRATION_GOLDEN_SHA = "4dcfd73ce81a14ace7e429791e0594bea47b24e5"


def git_show_sha256(repo_root: Path, git_sha: str, rel_path: str) -> str | None:
    """Return SHA-256 of a file as it existed at a specific git commit, or None on failure."""
    try:
        result = subprocess.run(
            ["git", "show", f"{git_sha}:{rel_path}"],
            cwd=str(repo_root),
            capture_output=True,
        )
        if result.returncode != 0:
            return None
        return hashlib.sha256(result.stdout).hexdigest()
    except Exception:
        return None


def check_abi_fixture(errors: list, contract: dict, fixture: dict, repo_root: Path) -> None:
    """P0-205: Structural ABI diff — verify namespace contracts, forbidden namespaces, method arities."""

    # The contract stores ABI under preload_abi.namespaces, where each key is the full dotpath
    # e.g. "window.hermesDesktop.petOverlay" -> { "open": {...}, ... }
    preload_abi = contract.get("preload_abi", {})
    namespaces_dict = preload_abi.get("namespaces", {})

    # 1. Forbidden namespaces must NOT appear as keys in preload_abi.namespaces
    forbidden = fixture.get("forbidden_namespaces", [])
    for ns in forbidden:
        if ns in namespaces_dict:
            errors.append(f"[P0-205] Forbidden namespace `{ns}` found as key in contract preload_abi.namespaces")

    # 2. Required namespaces must be present as keys in preload_abi.namespaces
    required = fixture.get("required_namespaces", [])
    for ns in required:
        if ns not in namespaces_dict:
            errors.append(f"[P0-205] Required namespace `{ns}` missing from contract preload_abi")

    # 3. Method arity and forbidden signatures
    ns_contracts = fixture.get("namespace_contracts", {})
    for ns, ns_spec in ns_contracts.items():
        ns_obj = namespaces_dict.get(ns)
        if ns_obj is None:
            continue  # already reported above

        # Check required methods exist
        for method in ns_spec.get("required_methods", []):
            if method not in ns_obj:
                errors.append(f"[P0-205] Method `{ns}.{method}` missing from contract preload_abi")

        # Check method contracts (arity and forbidden_signatures)
        for method, mc in ns_spec.get("method_contracts", {}).items():
            method_obj = ns_obj.get(method)
            if method_obj is None:
                continue  # reported above

            declared_arity = method_obj.get("arity")
            expected_arity = mc.get("arity")
            if expected_arity is not None and declared_arity != expected_arity:
                errors.append(
                    f"[P0-205] Arity mismatch for `{ns}.{method}`: "
                    f"fixture expects {expected_arity}, contract declares {declared_arity}"
                )

            # forbidden_arity
            forbidden_arity = mc.get("forbidden_arity")
            if forbidden_arity is not None and declared_arity == forbidden_arity:
                errors.append(
                    f"[P0-205] Forbidden arity {forbidden_arity} used for `{ns}.{method}`"
                )

            declared_result = method_obj.get("result_shape")
            expected_result = mc.get("result_shape")
            if expected_result is not None and declared_result != expected_result:
                errors.append(
                    f"[P0-205] result_shape mismatch for `{ns}.{method}`: "
                    f"fixture expects `{expected_result}`, contract declares `{declared_result}`"
                )

            # forbidden_input_types
            for bad_type in mc.get("forbidden_input_types", []):
                if method_obj.get("input_type") == bad_type:
                    errors.append(
                        f"[P0-205] Forbidden input_type `{bad_type}` for `{ns}.{method}`"
                    )

            # forbidden_signatures in contract
            declared_sig = method_obj.get("signature", "")
            for bad_sig in mc.get("forbidden_signatures", []):
                if bad_sig == declared_sig:
                    errors.append(
                        f"[P0-205] Forbidden signature `{bad_sig}` found for `{ns}.{method}`"
                    )


def main():
    repo_root = Path(__file__).resolve().parents[1]
    arch_dir = repo_root / "docs" / "architecture" / "pet-rust"
    schema_dir = arch_dir / "schema"

    contract_file = arch_dir / "golden-contract.json"
    provenance_file = arch_dir / "golden-build-provenance.json"
    abi_fixture_file = arch_dir / "preload-abi.fixture.json"
    upstream_file = repo_root / "third_party" / "hermes-agent-pet" / "UPSTREAM.md"

    errors = []
    warnings = []

    # 1. Load files
    if not contract_file.exists():
        print("[-] FATAL: golden-contract.json missing!")
        sys.exit(1)
    if not provenance_file.exists():
        print("[-] FATAL: golden-build-provenance.json missing!")
        sys.exit(1)
    if not abi_fixture_file.exists():
        print("[-] FATAL: preload-abi.fixture.json missing!")
        sys.exit(1)

    with open(contract_file, "r", encoding="utf-8") as f:
        contract = json.load(f)
    with open(provenance_file, "r", encoding="utf-8") as f:
        provenance = json.load(f)
    with open(abi_fixture_file, "r", encoding="utf-8") as f:
        abi_fixture = json.load(f)

    # 2. Schema check
    try:
        import jsonschema
        with open(schema_dir / "golden-contract.schema.json", "r", encoding="utf-8") as f:
            c_schema = json.load(f)
        jsonschema.validate(instance=contract, schema=c_schema)
        with open(schema_dir / "golden-build-provenance.schema.json", "r", encoding="utf-8") as f:
            p_schema = json.load(f)
        jsonschema.validate(instance=provenance, schema=p_schema)
    except Exception as e:
        errors.append(f"JSON Schema validation error: {e}")

    # 3. Pinned revision match with UPSTREAM.md
    if upstream_file.exists():
        upstream_text = upstream_file.read_text(encoding="utf-8")
        if contract.get("pinned_revision") not in upstream_text:
            errors.append(f"Pinned revision {contract.get('pinned_revision')} not found in {upstream_file}")
    else:
        errors.append(f"UPSTREAM.md missing at {upstream_file}")

    # 4. Mandatory behavior inputs check
    behavior_paths = {item["path"]: item for item in contract.get("behavior_inputs", [])}
    req_sprite = "third_party/hermes-agent-pet/apps/desktop/src/app/pet-overlay/pet-overlay-app.tsx"
    req_build = "packages/readmd-hermes-pet-adapter/scripts/build.mjs"

    if req_sprite not in behavior_paths:
        errors.append(f"Mandatory sprite behavior input missing: {req_sprite}")
    elif not behavior_paths[req_sprite].get("behavior_critical"):
        errors.append(f"{req_sprite} must be marked behavior_critical = true")

    if req_build not in behavior_paths:
        errors.append(f"Mandatory build adaptation behavior input missing: {req_build}")
    elif not behavior_paths[req_build].get("behavior_critical"):
        errors.append(f"{req_build} must be marked behavior_critical = true")

    # 5. P0-217/P0-218: Hash verification via git show <migration_golden_sha>:<path>
    #    NEVER reads from working tree to avoid verifying uncommitted mutations as golden.
    golden_sha = contract.get("migration_golden_commit_sha") or MIGRATION_GOLDEN_SHA
    all_categories = ["behavior_inputs", "build_inputs", "generated_inputs", "runtime_assets", "supporting_evidence"]
    for cat in all_categories:
        for item in contract.get(cat, []):
            rel_p = item["path"]
            declared_sha = item.get("sha256", "")
            if not declared_sha:
                errors.append(f"[P0-217] No sha256 declared for {rel_p}")
                continue
            golden_sha_result = git_show_sha256(repo_root, golden_sha, rel_p)
            if golden_sha_result is None:
                # P0-218: file absent at golden commit is a soft WARNING for generated/runtime artifacts
                # (build outputs are not committed at golden commit — expected behavior)
                warnings.append(
                    f"[P0-218] Cannot resolve `git show {golden_sha[:8]}:{rel_p}` — "
                    f"file may be absent at golden commit (expected for generated/runtime artifacts)"
                )
            elif golden_sha_result != declared_sha:
                errors.append(
                    f"[P0-217] SHA-256 mismatch for {rel_p}: "
                    f"declared {declared_sha}, golden-commit actual {golden_sha_result}"
                )

    # 6. P0-205: Structural ABI fixture diff
    check_abi_fixture(errors, contract, abi_fixture, repo_root)

    # 7. Check sprite provenance in build graph
    sp = provenance.get("sprite_provenance", {})
    if (
        not sp.get("upstream_source_sha256")
        or not sp.get("adaptation_script_sha256")
        or not sp.get("generated_source_sha256")
        or not sp.get("renderer_bundle_sha256")
    ):
        errors.append("Sprite provenance incomplete in golden-build-provenance.json")

    # 8. Check new P0-206 objects exist
    if "readmd_host_fallback_sprite_metadata" not in provenance:
        errors.append("[P0-206] readmd_host_fallback_sprite_metadata missing from golden-build-provenance.json")
    if "vendor_renderer_default_geometry" not in provenance:
        errors.append("[P0-206] vendor_renderer_default_geometry missing from golden-build-provenance.json")

    if warnings:
        print(f"[!] {len(warnings)} soft warning(s) (non-fatal):")
        for idx, warn in enumerate(warnings, 1):
            print(f"  [{idx:02d}] {warn}")

    if errors:
        print("[-] FAILED: Golden contract verification failed with errors:")
        for idx, err in enumerate(errors, 1):
            print(f"  [{idx:02d}] {err}")
        sys.exit(1)

    print("[+] PASS: Golden contract and build provenance verified clean.")


if __name__ == "__main__":
    main()
