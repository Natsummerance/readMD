"""Programmatic pixel QA for authentic showcase captures."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def audit(showcase_dir: Path) -> dict:
    capture_path = showcase_dir / "raw" / "capture.json"
    capture = json.loads(capture_path.read_text(encoding="utf-8"))
    scale = int(capture["config"]["scale"])
    expected_width = int(capture["config"]["viewport"]["width"]) * scale
    expected_height = int(capture["config"]["viewport"]["height"]) * scale
    records = []

    for shot in capture["shots"]:
        image_path = showcase_dir / shot["file"]
        with Image.open(image_path) as source:
            image = source.convert("RGB")
        pixels = np.asarray(image, dtype=np.float32)
        gray = np.dot(pixels[..., :3], [0.299, 0.587, 0.114])
        edge_density = float(
            (
                np.abs(np.diff(gray, axis=1)).mean()
                + np.abs(np.diff(gray, axis=0)).mean()
            )
            / 2
        )
        corner = np.median(pixels[:12, :12].reshape(-1, 3), axis=0)
        content_fraction = float(
            (np.sqrt(((pixels - corner) ** 2).sum(axis=2)) > 18).mean()
        )
        downsampled = np.asarray(
            image.resize((max(1, image.width // 4), max(1, image.height // 4))),
            dtype=np.uint8,
        ).reshape(-1, 3)
        records.append(
            {
                "shot_id": shot["shot_id"],
                "file": shot["file"],
                "bytes": image_path.stat().st_size,
                "sha256": shot["sha256"],
                "width": image.width,
                "height": image.height,
                "expected_width": expected_width,
                "expected_height": expected_height,
                "aspect_ratio": round(image.width / image.height, 4),
                "megapixels": round(image.width * image.height / 1_000_000, 2),
                "luminance_mean": round(float(gray.mean()), 2),
                "luminance_std": round(float(gray.std()), 2),
                "edge_density": round(edge_density, 3),
                "content_fraction": round(content_fraction, 4),
                "unique_colors_downsampled": int(np.unique(downsampled, axis=0).shape[0]),
            }
        )

    failures = []
    if len({record["sha256"] for record in records}) != len(records):
        failures.append("one or more screenshot hashes are duplicated")
    for record in records:
        if record["width"] != expected_width or record["height"] != expected_height:
            failures.append(f"{record['shot_id']} has unexpected dimensions")
        if record["content_fraction"] < 0.02:
            failures.append(f"{record['shot_id']} looks blank")
        if record["edge_density"] < 0.22:
            failures.append(f"{record['shot_id']} has too little UI structure")
        if record["luminance_std"] < 8:
            failures.append(f"{record['shot_id']} has too little tonal range")
        if record["unique_colors_downsampled"] < 1000:
            failures.append(f"{record['shot_id']} has too little color detail")

    return {
        "schema_version": 1,
        "capture_release": capture["release"],
        "captured_at": capture["captured_at"],
        "expected_dimensions": {"width": expected_width, "height": expected_height},
        "thresholds": {
            "minimum_content_fraction": 0.02,
            "minimum_edge_density": 0.22,
            "minimum_luminance_std": 8,
            "minimum_unique_colors_downsampled": 1000,
        },
        "shots": records,
        "passed": not failures,
        "failures": failures,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--showcase-dir", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args()
    report = audit(args.showcase_dir)
    output = args.output or args.showcase_dir / "reports" / "pixel_audit.json"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
