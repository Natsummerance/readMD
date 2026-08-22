#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Hard QA gate for an authentic ReadMD Xiaohongshu content package."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image


BANNED = ("公众号", "微信", "闲鱼", "咸鱼", "转卖", "出票", "转让", "售票", "二维码", "淘口令", "淘宝")
IMAGE_RE = re.compile(r"^xhs-(0[1-9])-[a-z0-9-]+\.jpg$")


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _resolve_evidence(path_value: str, package_dir: Path, repo_root: Path) -> Path | None:
    for base in (repo_root, package_dir):
        candidate = (base / path_value).resolve()
        if candidate.exists():
            return candidate
    return None


def _image_metrics(path: Path, screenshot_box: dict[str, float] | None = None) -> dict[str, Any]:
    with Image.open(path) as image:
        rgb = image.convert("RGB")
        arr = np.asarray(rgb, dtype=np.int16)
        gray = np.asarray(rgb.convert("L"), dtype=np.int16)
        corners = np.concatenate((arr[:12, :12].reshape(-1, 3), arr[:12, -12:].reshape(-1, 3), arr[-12:, :12].reshape(-1, 3), arr[-12:, -12:].reshape(-1, 3)))
        background = corners.mean(axis=0)
        distance = np.sqrt(np.sum((arr - background) ** 2, axis=2))
        # Dark app UI can sit close to the poster background, so structural edges count as content too.
        gradient_x = np.abs(np.diff(gray, axis=1, prepend=gray[:, :1]))
        gradient_y = np.abs(np.diff(gray, axis=0, prepend=gray[:1]))
        mask = (distance > 40) | (gradient_x > 30) | (gradient_y > 30)
        width, height = rgb.size
        if screenshot_box:
            left = max(0, int(screenshot_box.get("x", 0)))
            top = max(0, int(screenshot_box.get("y", 0)))
            right = min(width, int(left + screenshot_box.get("width", 0)))
            bottom = min(height, int(top + screenshot_box.get("height", 0)))
            if right > left and bottom > top:
                mask[top:bottom, left:right] = True
        row_counts = mask.sum(axis=1)
        col_counts = mask.sum(axis=0)
        meaningful_rows = row_counts >= max(3, width * 0.01)
        blank_runs: list[int] = []
        run = 0
        for meaningful in meaningful_rows:
            if meaningful:
                if run:
                    blank_runs.append(run)
                run = 0
            else:
                run += 1
        if run:
            blank_runs.append(run)
        content_rows = np.flatnonzero(meaningful_rows)
        top_gap = int(content_rows[0]) if len(content_rows) else height
        bottom_gap = int(height - 1 - content_rows[-1]) if len(content_rows) else height
        side_clear = float((mask[:, :8].mean() + mask[:, -8:].mean()) / 2)
        edge_clean = all(band.mean() < 0.02 for band in (mask[:4], mask[-4:], mask[:, :4], mask[:, -4:]))
        return {
            "size": rgb.size,
            "format": image.format,
            "coverage": float(mask.mean()),
            "blank_band": max(blank_runs, default=0),
            "top_gap": top_gap,
            "bottom_gap": bottom_gap,
            "side_clear": side_clear,
            "edge_clean": edge_clean,
        }


def validate_package(package_dir: Path, *, repo_root: Path | None = None) -> list[str]:
    package_dir = package_dir.resolve()
    repo_root = (repo_root or Path(__file__).resolve().parents[2]).resolve()
    errors: list[str] = []

    try:
        story = _load_json(package_dir / "story.json")
        capture = _load_json(package_dir / "raw" / "capture.json")
        metadata = _load_json(package_dir / "metadata.json")
        composition = _load_json(package_dir / "composition.json")
    except Exception as exc:
        return [f"package JSON unreadable: {exc}"]

    copy_review_path = package_dir / "copy-review.json"
    if copy_review_path.exists():
        try:
            copy_review = _load_json(copy_review_path)
            if copy_review.get("ok") is not True:
                errors.append("semantic alignment gate failed: " + "; ".join(map(str, copy_review.get("hard_failures", []))))
        except Exception as exc:
            errors.append(f"copy-review.json unreadable: {exc}")

    wechat_qa_path = package_dir / "wechat" / "wechat-qa.json"
    if wechat_qa_path.exists():
        try:
            wechat_qa = _load_json(wechat_qa_path)
            if wechat_qa.get("ok") is not True:
                errors.append("WeChat adapter gate failed: " + "; ".join(map(str, wechat_qa.get("errors", []))))
        except Exception as exc:
            errors.append(f"wechat-qa.json unreadable: {exc}")

    variants_path = package_dir / "variants.json"
    if variants_path.exists():
        try:
            variants = _load_json(variants_path)
            if variants.get("ok") is not True or not variants.get("chosen_strategy"):
                errors.append("variant selection report is incomplete")
        except Exception as exc:
            errors.append(f"variants.json unreadable: {exc}")

    dashboard_qa_path = package_dir / "dashboard-qa.json"
    if dashboard_qa_path.exists():
        try:
            dashboard = _load_json(dashboard_qa_path)
            if dashboard.get("ok") is not True:
                errors.append("review dashboard gate failed: " + "; ".join(map(str, dashboard.get("errors", []))))
        except Exception as exc:
            errors.append(f"dashboard-qa.json unreadable: {exc}")

    if story.get("schema_version") != 1 or capture.get("schema_version") != 1:
        errors.append("story/capture schema_version must be 1")
    release = story.get("release")
    version_state = story.get("version_state")
    if not release or version_state not in {"release", "prerelease"}:
        errors.append("release/version_state invalid")

    capture_by_id = {shot.get("shot_id"): shot for shot in capture.get("shots", [])}
    selected = story.get("selected_shots", [])
    if not selected or selected[0] != "overview.reader":
        errors.append("selected_shots must begin with overview.reader")
    if len(selected) < 2 or len(selected) > 7:
        errors.append("selected raw shots must support a 4-9 card package")
    for shot_id in selected:
        captured = capture_by_id.get(shot_id)
        if not captured:
            errors.append(f"shot missing from capture.json: {shot_id}")
            continue
        source = package_dir / captured.get("file", "")
        if not source.is_file():
            errors.append(f"captured PNG missing: {shot_id}")
            continue
        digest = hashlib.sha256(source.read_bytes()).digest()
        expected = str(captured.get("sha256", ""))
        if digest.hex() != expected:
            errors.append(f"SHA-256 mismatch: {shot_id}")
        story_shot = next((item for item in story.get("shots", []) if item.get("id") == shot_id), {})
        if story_shot.get("sha256") and story_shot["sha256"] != digest.hex():
            errors.append(f"SHA-256 mismatch: {shot_id}")

    claims = story.get("claims", [])
    if not claims:
        errors.append("story has no claims")
    known_shot_ids = set(selected)
    for claim in claims:
        if not claim.get("id") or not claim.get("user_value"):
            errors.append(f"claim incomplete: {claim.get('id', '<missing>')}")
        sources = claim.get("sources", [])
        if not sources:
            errors.append(f"claim has no evidence: {claim.get('id', '<missing>')}")
        for source in sources:
            if _resolve_evidence(source, package_dir, repo_root) is None:
                errors.append(f"evidence does not exist: {source}")
        unknown = set(claim.get("shot_ids", [])) - known_shot_ids
        if unknown:
            errors.append(f"claim references uncaptured shots: {sorted(unknown)}")

    cards = story.get("card_plan", [])
    if not 4 <= len(cards) <= 9:
        errors.append("card_plan must contain 4-9 cards")
    elif cards[1].get("role") != "pure_ui_hero" or cards[1].get("shot_id") != "overview.reader":
        errors.append("card 2 must be the pure UI overview.reader hero")
    names = [card.get("file", "") for card in cards]
    if len(names) != len(set(names)) or any(not IMAGE_RE.match(name) for name in names):
        errors.append("card filenames must be unique semantic xhs-NN-slug.jpg values")
    if composition.get("overflow_errors"):
        errors.append("composition DOM overflow errors: " + "; ".join(map(str, composition["overflow_errors"])))
    design_audit = composition.get("design_audit", {})
    for key in ("contrast_errors", "small_text", "images_failed"):
        if design_audit.get(key):
            errors.append(f"composition design audit {key}: " + "; ".join(map(str, design_audit[key])))

    title = str(metadata.get("title", "")).strip()
    body = str(metadata.get("body", "")).strip()
    topics = metadata.get("topics", [])
    images = metadata.get("images", [])
    if not title or len(title) > 20:
        errors.append("title must be 1-20 characters")
    if not 600 <= len(body) <= 900:
        errors.append(f"body must be 600-900 characters, got {len(body)}")
    if re.search(r"https?://|www\.", body, re.I):
        errors.append("body contains a URL")
    if banned := [word for word in BANNED if word.lower() in (title + body).lower()]:
        errors.append("banned words: " + ", ".join(banned))
    if not isinstance(topics, list) or len(topics) != 5 or any(not str(topic).strip() or topic.startswith("#") for topic in topics):
        errors.append("topics must be five non-empty values without #")
    if not isinstance(images, list) or not 4 <= len(images) <= 9:
        errors.append("metadata.images must contain 4-9 paths")
        images = []
    if metadata.get("version_state") != version_state:
        errors.append("metadata.version_state differs from story")
    if not metadata.get("source_urls"):
        errors.append("metadata.source_urls is empty")
    if version_state == "prerelease":
        if "正式发布" in body or "正式版" in body:
            errors.append("prerelease uses formal-release wording")
        if not ("预览版" in body or "更新线" in body or "beta" in body.lower()):
            errors.append("prerelease wording missing")
    elif "正式发布" not in body and "正式版" not in body:
        errors.append("release should state formal availability")

    hashes: set[str] = set()
    metrics_by_name: dict[str, dict[str, Any]] = {}
    composition_by_name = {item.get("file"): item for item in composition.get("cards", [])}
    for index, raw_path in enumerate(images):
        path = Path(raw_path)
        expected_name = names[index] if index < len(names) else ""
        if path.name != expected_name:
            errors.append(f"image order/name mismatch at {index + 1}: {path.name} != {expected_name}")
        if not path.is_file():
            errors.append(f"image missing: {path}")
            continue
        try:
            metrics = _image_metrics(path, composition_by_name.get(path.name, {}).get("screenshot_box"))
        except Exception as exc:
            errors.append(f"image unreadable: {path.name}: {exc}")
            continue
        metrics_by_name[path.name] = metrics
        if metrics["size"] != (1080, 1440) or metrics["format"] != "JPEG":
            errors.append(f"image must be 1080x1440 JPEG: {path.name}")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest in hashes:
            errors.append(f"duplicate image hash: {path.name}")
        hashes.add(digest)
        if not metrics["edge_clean"]:
            errors.append(f"image edge is not clean: {path.name}")
        if metrics["coverage"] < 0.22:
            errors.append(f"image coverage below 22%: {path.name} ({metrics['coverage']:.2%})")
        if metrics["blank_band"] > 120:
            errors.append(f"blank band exceeds 120px: {path.name} ({metrics['blank_band']}px)")
        if metrics["top_gap"] > 220:
            errors.append(f"top gap exceeds 220px: {path.name} ({metrics['top_gap']}px)")
        if metrics["bottom_gap"] > 90:
            errors.append(f"bottom gap exceeds 90px: {path.name} ({metrics['bottom_gap']}px)")
        if metrics["side_clear"] >= 0.01:
            errors.append(f"content bleeds into side margin: {path.name}")

    for card in cards:
        name = card.get("file")
        metric = metrics_by_name.get(name)
        minimum = float(card.get("ui_min_ratio", 0))
        composed = composition_by_name.get(name)
        if metric and composed is None:
            errors.append(f"composition ratio missing: {name}")
        elif metric and minimum > 0 and float(composed.get("ui_area_ratio", 0)) + 0.01 < minimum:
            errors.append(f"UI area below contract: {name} ({composed.get('ui_area_ratio', 0):.2%} < {minimum:.2%})")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    parser.add_argument("--repo-root", type=Path)
    args = parser.parse_args()
    errors = validate_package(args.package, repo_root=args.repo_root)
    report = {"ok": not errors, "errors": errors}
    (args.package / "qa.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
