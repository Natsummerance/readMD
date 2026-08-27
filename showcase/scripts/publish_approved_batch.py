#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""One-click publish exactly the posters approved through the reviewed PDF."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path
from datetime import datetime, timezone

import validate_repair_batch
from content_memory import load_records
from watch_and_publish import process_package


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_approval_or_request(args, root: Path, batch_path: Path) -> tuple[dict, Path, bool]:
    if args.approval:
        approval_path = args.approval.resolve()
        return json.loads(approval_path.read_text(encoding="utf-8")), approval_path

    request_path = args.approval_request.resolve()
    request = json.loads(request_path.read_text(encoding="utf-8"))
    checks = {
        "batch_sha256": sha256(batch_path),
        "review_pdf_sha256": sha256(root / request["review_pdf"]),
    }
    for key, actual in checks.items():
        if request.get(key) != actual:
            raise ValueError(f"approval request {key} is stale: request={request.get(key)}, actual={actual}")
    created = True
    approval = {
        "schema_version": 1,
        "approved": True,
        "approved_at": datetime.now(timezone.utc).isoformat(),
        "reviewer": args.reviewer,
        "batch": request["batch"],
        **checks,
        "review_pdf": request["review_pdf"],
        "package_hashes": {
            item["release"]: item["package_sha256"]
            for item in request.get("packages", [])
        },
    }
    approval_path = request_path.with_name(
        request_path.name.replace(".approval-request.json", ".approved.json")
    )
    if not args.validate_only:
        approval_path.write_text(json.dumps(approval, ensure_ascii=False, indent=2), encoding="utf-8")
    else:
        created = False
    return approval, approval_path, created


def validate_approved_batch(
    *,
    approval: dict,
    batch_path: Path,
    root: Path,
    ledger: Path,
) -> tuple[list[str], list[dict], list[str]]:
    errors: list[str] = []
    if approval.get("schema_version") != 1 or approval.get("approved") is not True:
        errors.append("poster approval is missing approved=true")
    pdf_name = approval.get("review_pdf")
    pdf_path = root / str(pdf_name)
    expected = {
        "batch_sha256": sha256(batch_path),
        "review_pdf_sha256": sha256(pdf_path),
    }
    for key, value in expected.items():
        if approval.get(key) != value:
            errors.append(f"approval {key} does not match current artifacts")
    batch = json.loads(batch_path.read_text(encoding="utf-8"))
    approved_hashes = approval.get("package_hashes", {})
    for entry in batch.get("packages", []):
        if approved_hashes.get(entry["release"]) != entry["package_sha256"]:
            errors.append(f"approved package hash changed: {entry['release']}")
    if errors:
        return errors, [], []

    report = validate_repair_batch.validate_batch(batch_path, root=root)
    if not report.get("ok"):
        errors.append("approved batch QA failed: " + "; ".join(report.get("errors", [])))
        return errors, [], []

    records = load_records(ledger)
    published_releases = {str(record.get("release", "")) for record in records}
    entries = batch.get("packages", [])
    pending = [entry for entry in entries if entry["release"] not in published_releases]
    skipped = [entry["release"] for entry in entries if entry["release"] in published_releases]
    return errors, pending, skipped


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch", type=Path, required=True)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--approval", type=Path)
    group.add_argument("--approval-request", type=Path)
    parser.add_argument("--reviewer", default=os.getenv("READMD_REVIEWER", "maintainer"))
    parser.add_argument("--root", type=Path)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--ledger", type=Path, required=True)
    parser.add_argument("--publisher", default=os.getenv("XHS_PUBLISHER_SCRIPT", "tools/xhs_publish.py"))
    parser.add_argument("--publisher-proxy", default="http://127.0.0.1:3456")
    parser.add_argument("--max-attempts", type=int, choices={1, 2, 3}, default=2)
    parser.add_argument("--validate-only", action="store_true", help="verify PDF, hashes, QA, and ledger state without creating an approval or publishing")
    args = parser.parse_args()

    root = (args.root or args.batch.parent).resolve()
    batch_path = args.batch.resolve()
    approval, _, created = read_approval_or_request(args, root, batch_path)
    errors, pending, skipped = validate_approved_batch(
        approval=approval,
        batch_path=batch_path,
        root=root,
        ledger=args.ledger,
    )
    action = "validate-approved-batch" if args.validate_only else "publish-approved-batch"
    print(json.dumps({
        "ok": not errors,
        "action": action,
        "errors": errors,
        "pending": [entry["release"] for entry in pending],
        "skipped": skipped,
        "approval_created": created,
    }, ensure_ascii=False))
    if errors:
        raise ValueError("; ".join(errors))
    if args.validate_only:
        return 0

    for index, entry in enumerate(pending):
        watch_dir = args.work_dir / f"{index:02d}-{entry['release']}"
        watch_dir.mkdir(parents=True, exist_ok=True)
        source = root / str(entry["package"])
        target = watch_dir / source.name
        shutil.copyfile(source, target)
        shutil.copyfile(Path(str(source) + ".manifest.json"), Path(str(target) + ".manifest.json"))
        published = process_package(
            target,
            args.work_dir / "extract",
            args.state,
            Path(args.publisher),
            args.max_attempts,
            False,
            ledger_path=args.ledger,
            reuse_edge=index > 0,
            publisher_proxy=args.publisher_proxy,
        )
        if not published:
            state = json.loads(args.state.read_text(encoding="utf-8"))
            failed = [item for item in state.get("packages", {}).values() if item.get("release") == entry["release"]]
            raise RuntimeError(f"one-click publish stopped at {entry['release']}: {failed[-1].get('error') if failed else 'unknown error'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
