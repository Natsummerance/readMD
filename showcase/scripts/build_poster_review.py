#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Build a reviewer PDF and immutable approval request for poster batches."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import zipfile
from datetime import datetime, timezone
from pathlib import Path

from PIL import Image


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch", type=Path, required=True)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--dpi", type=int, default=150)
    args = parser.parse_args()

    batch_path = args.batch.resolve()
    root = (args.root or batch_path.parent).resolve()
    batch = json.loads(batch_path.read_text(encoding="utf-8"))
    output = (args.output or batch_path.parent / "poster-review.pdf").resolve()
    packages: list[dict] = []
    pages: list[Image.Image] = []

    for entry in batch.get("packages", []):
        package_path = (root / str(entry["package"])).resolve()
        if not package_path.is_relative_to(root):
            raise ValueError(f"package escapes root: {package_path}")
        with zipfile.ZipFile(package_path) as archive:
            metadata = json.loads(archive.read("metadata.json"))
            story = json.loads(archive.read("story.json"))
            composition = json.loads(archive.read("composition.json"))
            by_name = {item["file"]: item for item in composition["cards"]}
            images = []
            for name in metadata["images"]:
                filename = Path(name).name
                payload = archive.read(f"images/{filename}")
                image = Image.open(io.BytesIO(payload)).convert("RGB")
                if image.size != (1080, 1440):
                    raise ValueError(f"{package_path.name}/{filename} must be 1080x1440")
                images.append(image)
                pages.append(image)
        packages.append({
            "release": story["release"],
            "title": metadata["title"],
            "poster_style": composition["poster_style"],
            "card_count": len(images),
            "package": package_path.name,
            "package_sha256": sha256(package_path),
            "cards": [
                {
                    "file": filename,
                    "role": by_name[filename]["role"],
                    "sha256": by_name[filename]["sha256"],
                }
                for filename in (Path(path).name for path in metadata["images"])
            ],
        })

    if not pages:
        raise ValueError("batch has no poster pages")
    pages[0].save(
        output,
        format="PDF",
        save_all=True,
        append_images=pages[1:],
        resolution=args.dpi,
        quality=95,
    )
    request = {
        "schema_version": 1,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "status": "pending_operator_review",
        "batch": batch_path.name,
        "batch_sha256": sha256(batch_path),
        "review_pdf": output.name,
        "review_pdf_sha256": sha256(output),
        "page_count": len(pages),
        "packages": packages,
    }
    request_path = output.with_suffix(".approval-request.json")
    request_path.write_text(json.dumps(request, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"ok": True, "pdf": str(output), "request": str(request_path), "pages": len(pages)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
