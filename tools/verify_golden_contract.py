# -*- coding: utf-8 -*-
"""
tools/verify_golden_contract.py
Validates Golden Behavioral Input Closure and Build Provenance.
Version: v1.4.6
"""

import os
import sys
import json
import hashlib
from pathlib import Path

def main():
    repo_root = Path(__file__).resolve().parents[1]
    arch_dir = repo_root / "docs" / "architecture" / "pet-rust"
    schema_dir = arch_dir / "schema"

    contract_file = arch_dir / "golden-contract.json"
    provenance_file = arch_dir / "golden-build-provenance.json"
    upstream_file = repo_root / "third_party" / "hermes-agent-pet" / "UPSTREAM.md"

    errors = []

    # 1. Load files
    if not contract_file.exists():
        print("[-] FATAL: golden-contract.json missing!")
        sys.exit(1)
    if not provenance_file.exists():
        print("[-] FATAL: golden-build-provenance.json missing!")
        sys.exit(1)

    with open(contract_file, "r", encoding="utf-8") as f:
        contract = json.load(f)
    with open(provenance_file, "r", encoding="utf-8") as f:
        provenance = json.load(f)

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

    # 5. Disk file hash verification for all inputs
    all_categories = ["behavior_inputs", "build_inputs", "generated_inputs", "runtime_assets", "supporting_evidence"]
    for cat in all_categories:
        for item in contract.get(cat, []):
            rel_p = item["path"]
            full_p = repo_root / rel_p.replace("/", os.sep)
            if not full_p.exists():
                errors.append(f"Declared golden file missing on disk: {rel_p}")
                continue
            with open(full_p, "rb") as fp:
                content = fp.read()
            actual_sha = hashlib.sha256(content).hexdigest()
            if actual_sha != item["sha256"]:
                errors.append(f"SHA-256 mismatch for {rel_p}: declared {item['sha256']}, actual {actual_sha}")

    # 6. Check sprite provenance in build graph
    sp = provenance.get("sprite_provenance", {})
    if not sp.get("upstream_source_sha256") or not sp.get("adaptation_script_sha256") or not sp.get("generated_source_sha256") or not sp.get("renderer_bundle_sha256"):
        errors.append("Sprite provenance incomplete in golden-build-provenance.json")

    if errors:
        print("[-] FAILED: Golden contract verification failed with errors:")
        for idx, err in enumerate(errors, 1):
            print(f"  [{idx:02d}] {err}")
        sys.exit(1)

    print("[+] PASS: Golden contract and build provenance verified clean.")

if __name__ == "__main__":
    main()
