# -*- coding: utf-8 -*-
"""
tools/verify_registry_integrity.py
Validates JSON Schemas and Referential Integrity across all architecture registries.
Version: v1.4.6
"""

import os
import sys
import json
from pathlib import Path

def main():
    repo_root = Path(__file__).resolve().parents[1]
    arch_dir = repo_root / "docs" / "architecture" / "pet-rust"
    schema_dir = arch_dir / "schema"

    registry_files = [
        ("golden-contract.json", "golden-contract.schema.json"),
        ("golden-build-provenance.json", "golden-build-provenance.schema.json"),
        ("tuple-registry.json", "tuple-registry.schema.json"),
        ("validation-registry.json", "validation-registry.schema.json"),
        ("gate-registry.json", "gate-registry.schema.json"),
        ("blocker-registry.json", "blocker-registry.schema.json"),
        ("registry-manifest.json", "registry-manifest.schema.json"),
        ("spec.integrity.json", "spec-integrity.schema.json")
    ]

    errors = []

    # 1. Check file existence and JSON Schema validity
    try:
        import jsonschema
    except ImportError:
        print("[-] FATAL: jsonschema package not installed!")
        sys.exit(1)

    registries = {}
    for data_name, schema_name in registry_files:
        d_path = arch_dir / data_name
        s_path = schema_dir / schema_name
        if not d_path.exists():
            errors.append(f"Missing registry file: {data_name}")
            continue
        if not s_path.exists():
            errors.append(f"Missing schema file: {schema_name}")
            continue
        with open(d_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        with open(s_path, "r", encoding="utf-8") as f:
            schema = json.load(f)
        try:
            jsonschema.validate(instance=data, schema=schema)
            registries[data_name] = data
        except Exception as e:
            errors.append(f"Schema validation error in {data_name} ({schema_name}): {e}")

    if errors:
        print("[-] FAILED Schema validation:")
        for e in errors:
            print(f"  - {e}")
        sys.exit(1)

    # 2. Referential Integrity
    gates = registries.get("gate-registry.json", [])
    validations = registries.get("validation-registry.json", [])
    blockers = registries.get("blocker-registry.json", [])
    tuples = registries.get("tuple-registry.json", [])
    integrity = registries.get("spec.integrity.json", {})

    gate_ids = {g["id"] for g in gates}
    val_ids = {v["id"] for v in validations}
    blocker_ids = {b["id"] for b in blockers}
    tuple_keys = {t["tuple_key"] for t in tuples}

    # 2.1 Check validation referential integrity
    for v in validations:
        vid = v.get("id")
        gate = v.get("blocking_gate")
        if gate and gate not in gate_ids:
            errors.append(f"Validation {vid} references non-existent gate: {gate}")
        for tkey in v.get("tuple_keys", []):
            if tkey not in tuple_keys:
                errors.append(f"Validation {vid} references non-existent tuple_key: {tkey}")
        for bid in v.get("associated_blockers", []):
            if bid not in blocker_ids:
                errors.append(f"Validation {vid} references non-existent blocker: {bid}")

    # 2.2 Check blocker referential integrity
    for b in blockers:
        bid = b.get("id")
        b_until = b.get("blocking_until")
        if b_until and b_until not in val_ids:
            errors.append(f"Blocker {bid} references non-existent validation: {b_until}")
        for gid in b.get("gate_ids", []):
            if gid not in gate_ids:
                errors.append(f"Blocker {bid} references non-existent gate: {gid}")
        if b.get("design_state") != "resolved":
            errors.append(f"Blocker {bid} design_state is not 'resolved'!")
        if b.get("validation_state") != "pending":
            errors.append(f"Blocker {bid} validation_state must be 'pending' prior to physical spikes!")

    # 2.3 Check tuple constraints
    for t in tuples:
        tid = t.get("display_id")
        de = t.get("desktop_environment", "")
        backend = t.get("planned_backend", "")
        if de == "GNOME" and backend == "LayerShellBackend":
            errors.append(f"Tuple {tid} assigns LayerShellBackend to GNOME, which is physically unsupported!")

    # 2.4 Integrity file must not self-reference
    art_hashes = integrity.get("artifacts", {})
    if "spec.integrity.json" in art_hashes:
        errors.append("spec.integrity.json contains recursive self-reference in artifacts dict!")

    if errors:
        print("[-] FAILED Referential integrity checks:")
        for idx, err in enumerate(errors, 1):
            print(f"  [{idx:02d}] {err}")
        sys.exit(1)

    print(f"[+] PASS: All {len(registry_files)} machine registries schema and referential integrity verified clean.")

if __name__ == "__main__":
    main()
