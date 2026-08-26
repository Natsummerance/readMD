#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Record operator approval after the rendered poster PDF has been reviewed."""
from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", type=Path, required=True)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--reviewer", default="Natsumer")
    args = parser.parse_args()

    request_path = args.request.resolve()
    root = (args.root or request_path.parent).resolve()
    request = json.loads(request_path.read_text(encoding="utf-8"))
    pdf_path = root / str(request["review_pdf"])
    checks = {
        "batch_sha256": sha256(root / request["batch"]),
        "review_pdf_sha256": sha256(pdf_path),
    }
    for key, actual in checks.items():
        if request.get(key) != actual:
            raise ValueError(f"review request {key} is stale: request={request.get(key)}, actual={actual}")
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
    output = request_path.with_name(
        request_path.name.replace(".approval-request.json", ".approved.json")
    )
    output.write_text(json.dumps(approval, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"ok": True, "approval": str(output)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
