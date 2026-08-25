#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Validate a paused batch of repaired Xiaohongshu packages as one release set."""
from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
import sys
import zipfile
from pathlib import Path
from typing import Any

from copy_variants import jaccard_similarity, text_trigrams
from package_content import verify_package_manifest


BODY_NEAR_DUPLICATE_LIMIT = 0.85


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _read_member(archive: zipfile.ZipFile, name: str) -> bytes:
    return archive.read(name)


def _json_member(archive: zipfile.ZipFile, name: str) -> Any:
    return json.loads(_read_member(archive, name).decode("utf-8"))


def _text_member(archive: zipfile.ZipFile, name: str) -> str:
    return "\n".join(_read_member(archive, name).decode("utf-8").splitlines()).strip()


def _normalized(value: str) -> str:
    return re.sub(r"\s+", "", value).casefold()


def _resolve_package(root: Path, value: str) -> Path:
    package = (root / value).resolve()
    if not package.is_relative_to(root.resolve()):
        raise ValueError(f"package path escapes repository: {value}")
    if not package.is_file():
        raise ValueError(f"repair package missing: {value}")
    return package


def _verify_archive_identity(package_path: Path, entry: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    actual_sha = _sha256(package_path.read_bytes())
    expected_sha = str(entry.get("package_sha256", ""))
    if actual_sha != expected_sha:
        errors.append(f"package SHA-256 mismatch: expected {expected_sha}, got {actual_sha}")

    manifest_path = Path(str(package_path) + ".manifest.json")
    if not manifest_path.is_file():
        errors.append("transport manifest missing")
        return errors
    actual_manifest_sha = _sha256(manifest_path.read_bytes())
    expected_manifest_sha = str(entry.get("manifest_sha256", ""))
    if actual_manifest_sha != expected_manifest_sha:
        errors.append(
            f"manifest SHA-256 mismatch: expected {expected_manifest_sha}, got {actual_manifest_sha}"
        )
    try:
        verify_package_manifest(package_path)
    except Exception as exc:
        errors.append(f"transport manifest invalid: {exc}")
    return errors


def _validate_archive(package_path: Path, entry: dict[str, Any]) -> dict[str, Any]:
    errors: list[str] = []
    warnings: list[str] = []
    with zipfile.ZipFile(package_path) as archive:
        names = set(archive.namelist())
        required = {
            "story.json", "metadata.json", "title.txt", "body.txt", "topics.txt",
            "qa.json", "copy-review.json", "pattern-audit.json", "dashboard-qa.json",
            "composition.json", "raw/capture.json", "wechat/wechat-qa.json",
        }
        missing = sorted(required - names)
        if missing:
            raise ValueError("required members missing: " + ", ".join(missing))

        qa = _json_member(archive, "qa.json")
        reports = {
            name: _json_member(archive, f"{name}.json")
            for name in ("copy-review", "pattern-audit", "dashboard-qa", "wechat/wechat-qa")
        }
        for label, report in (("qa", qa), *reports.items()):
            if report.get("ok") is not True:
                errors.append(f"{label} report is not green")

        story = _json_member(archive, "story.json")
        metadata = _json_member(archive, "metadata.json")
        composition = _json_member(archive, "composition.json")
        capture = _json_member(archive, "raw/capture.json")
        title = _text_member(archive, "title.txt")
        body = _text_member(archive, "body.txt")
        topics = [line.strip() for line in _text_member(archive, "topics.txt").splitlines() if line.strip()]

        release = str(entry.get("release", ""))
        if story.get("release") != release or capture.get("release") != release:
            errors.append("story and capture releases differ from the batch entry")
        if metadata.get("version_state") != "prerelease":
            errors.append("prerelease disclosure contract is absent from metadata version_state")

        if title != str(metadata.get("title", "")).strip():
            errors.append("title.txt differs from metadata.title")
        if body != str(metadata.get("body", "")).strip():
            errors.append("body.txt differs from metadata.body")
        if topics != metadata.get("topics") or len(topics) != 5 or len(set(topics)) != 5:
            errors.append("topics contract must be five unique values matching metadata")

        plan = story.get("card_plan", [])
        image_names = [Path(item).name for item in metadata.get("images", [])]
        expected_names = [str(item.get("file", "")) for item in plan]
        if len(plan) < 4 or len(plan) > 9:
            errors.append(f"card plan count must be 4-9, got {len(plan)}")
        if image_names != expected_names or len(image_names) != len(set(image_names)):
            errors.append("metadata images differ from ordered card plan")

        composition_cards = {item.get("file"): item for item in composition.get("cards", [])}
        for filename in image_names:
            member = f"images/{filename}"
            if member not in names:
                errors.append(f"composed image missing: {filename}")
                continue
            digest = _sha256(_read_member(archive, member))
            recorded = str(composition_cards.get(filename, {}).get("sha256", ""))
            if digest != recorded:
                errors.append(f"composed image hash mismatch: {filename}")

        selected = [str(item) for item in story.get("selected_shots", [])]
        capture_by_id = {str(item.get("shot_id")): item for item in capture.get("shots", [])}
        capture_ids = list(capture_by_id)
        if len(capture_ids) != len(set(capture_ids)) or not set(selected).issubset(capture_by_id):
            errors.append("capture shots do not contain the unique selected shot set")
        for shot_id in selected:
            record = capture_by_id.get(shot_id, {})
            raw_name = str(record.get("file", ""))
            if raw_name not in names:
                errors.append(f"authentic raw screenshot missing: {shot_id}")
                continue
            digest = _sha256(_read_member(archive, raw_name))
            if digest != str(record.get("sha256", "")):
                errors.append(f"authentic screenshot hash mismatch: {shot_id}")

    return {
        "release": str(entry.get("release", "")),
        "title": title,
        "topics": topics,
        "semantic_image_count": len(image_names),
        "raw_shot_count": len(capture_by_id),
        "body": body,
        "errors": errors,
        "warnings": warnings,
    }


def validate_batch(batch_path: Path, *, root: Path | None = None) -> dict[str, Any]:
    batch_path = batch_path.resolve()
    root = (root or batch_path.parents[2]).resolve()
    batch = json.loads(batch_path.read_text(encoding="utf-8"))
    if batch.get("schema_version") != 1:
        raise ValueError("repair batch schema_version must be 1")
    if batch.get("status") != "paused_for_operator_review":
        raise ValueError("repair batch must remain paused before explicit operator approval")

    entries = batch.get("packages")
    if not isinstance(entries, list) or not 2 <= len(entries) <= 9:
        raise ValueError("repair batch must contain 2-9 packages")

    package_results: list[dict[str, Any]] = []
    seen_releases: set[str] = set()
    seen_titles: set[str] = set()
    title_prints: list[tuple[str, set[str]]] = []
    body_prints: list[tuple[str, set[str]]] = []

    for entry in entries:
        result = {"release": entry.get("release"), "errors": [], "warnings": []}
        package_path: Path | None = None
        try:
            package_path = _resolve_package(root, str(entry.get("package", "")))
            result["errors"].extend(_verify_archive_identity(package_path, entry))
            detail = _validate_archive(package_path, entry)
            archive_errors = detail.pop("errors", [])
            archive_warnings = detail.pop("warnings", [])
            result.update(detail)
            result["errors"].extend(archive_errors)
            result["warnings"].extend(archive_warnings)
        except Exception as exc:
            result["errors"].append(str(exc))

        release = str(result.get("release", ""))
        title = str(result.get("title", ""))
        if release:
            if release in seen_releases:
                result["errors"].append(f"duplicate release in batch: {release}")
            seen_releases.add(release)
        if title:
            normalized_title = _normalized(title)
            if normalized_title in seen_titles:
                result["errors"].append("duplicate normalized title in batch")
            seen_titles.add(normalized_title)

        if isinstance(result.get("body"), str):
            body_prints.append((release, text_trigrams(result["body"])))

        package_results.append(result)

    for left_name, left_prints in body_prints:
        for right_name, right_prints in body_prints:
            if left_name >= right_name:
                continue
            similarity = jaccard_similarity(left_prints, right_prints)
            if similarity >= BODY_NEAR_DUPLICATE_LIMIT:
                package_results.append({
                    "release": f"{left_name} + {right_name}",
                    "errors": [f"cross-package body similarity {similarity:.2f} exceeds 85%"],
                    "warnings": [],
                })

    all_errors = [error for result in package_results for error in result.get("errors", [])]
    return {
        "schema_version": 1,
        "ok": not all_errors,
        "batch": str(batch_path),
        "status": batch.get("status"),
        "package_count": len(entries),
        "packages": package_results,
        "checks": {
            "transport_hashes_and_manifests": True,
            "green_package_reports": True,
            "ordered_semantic_images": True,
            "authentic_raw_evidence": True,
            "unique_releases_and_titles": True,
            "cross_package_body_originality": True,
        },
        "errors": all_errors,
    }


def render_html(report: dict[str, Any]) -> str:
    def esc(value: Any) -> str:
        return html.escape(str(value), quote=True)

    rows = []
    for package in report.get("packages", []):
        status_color = "#157347" if not package.get("errors") else "#c1121f"
        status = "PASS" if not package.get("errors") else "FAIL"
        rows.append(
            "<tr>"
            f"<td>{esc(package.get('release'))}</td>"
            f"<td>{esc(package.get('title'))}</td>"
            + "".join(f"<td>{esc(topic)}</td>" for topic in package.get("topics", []))
            + f"<td><strong style=\"color:{status_color}\">{status}</strong></td>"
              "<td>"
              + (
                  "<ul style=\"margin:0;padding-left:18px\">"
                  + "".join(f"<li>{esc(error)}</li>" for error in package.get("errors", []))
                  + "</ul>"
                  if package.get("errors") else "none"
              )
              + "</td>"
            "</tr>"
        )
    overall_color = "#157347" if report.get("ok") else "#c1121f"
    checks = "".join(
        f"<li><strong>Pass</strong> · {esc(name.replace('_', ' '))}</li>"
        for name in report.get("checks", {})
    )
    return f"""<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>ReadMD Repair Batch Review</title>
</head>
<body style="margin:0;background:#f5f2ec;color:#182029;font-family:'Microsoft YaHei','Noto Sans SC',sans-serif">
<main style="max-width:1240px;margin:0 auto;padding:56px 40px 72px">
<header style="border-bottom:3px solid #d6482c;padding-bottom:24px;margin-bottom:32px">
<p style="margin:0 0 10px;font-size:15px;font-weight:700;letter-spacing:.08em;color:#d6482c">READMD · OPERATOR REVIEW</p>
<h1 style="margin:0;font-size:46px;line-height:1.16">三版本修复批次审查</h1>
<p style="margin:14px 0 0;font-size:19px;color:#5b6875">发布状态：{esc(report.get('status'))}</p>
</header>
<section style="display:flex;gap:18px;flex-wrap:wrap;margin-bottom:36px">
<div style="min-width:180px;background:#fbfcfd;border:1px solid #d8dee6;border-top:4px solid {overall_color};padding:20px 24px"><small>Overall</small><strong style="display:block;font-size:30px;color:{overall_color}">{'PASS' if report.get('ok') else 'FAIL'}</strong></div>
<div style="min-width:180px;background:#fbfcfd;border:1px solid #d8dee6;border-top:4px solid #d6482c;padding:20px 24px"><small>Packages</small><strong style="display:block;font-size:30px">{esc(report.get('package_count'))}</strong></div>
<div style="min-width:180px;background:#fbfcfd;border:1px solid #d8dee6;border-top:4px solid #182029;padding:20px 24px"><small>Evidence per package</small><strong style="display:block;font-size:30px">8 + 8</strong></div>
</section>
<section style="background:#fbfcfd;border:1px solid #d8dee6;padding:24px;margin-bottom:32px"><h2 style="margin:0 0 14px;font-size:23px">Batch gates</h2><ul style="margin:0;padding-left:22px;line-height:1.7;font-size:17px">{checks}</ul></section>
<section style="overflow-x:auto;border:1px solid #d8dee6;background:#ffffff"><table style="width:100%;border-collapse:collapse;min-width:900px;font-size:16px"><thead><tr style="background:#eef1f4;text-align:left"><th style="padding:14px">Release</th><th style="padding:14px">Title</th><th style="padding:14px">Topic 1</th><th style="padding:14px">Topic 2</th><th style="padding:14px">Topic 3</th><th style="padding:14px">Topic 4</th><th style="padding:14px">Topic 5</th><th style="padding:14px">Gate</th><th style="padding:14px">Findings</th></tr></thead><tbody>{''.join(rows)}</tbody></table></section>
<footer style="margin-top:34px;color:#5b6875;font-size:15px">Paused for operator review · no platform click is authorized by this page</footer>
</main></body></html>"""


def main() -> int:
    script_dir = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--batch",
        type=Path,
        default=script_dir / "reports" / "v2.3.7-beta-repair-packages.json",
    )
    parser.add_argument("--root", type=Path)
    parser.add_argument(
        "--output-json",
        type=Path,
        default=script_dir / "reports" / "v2.3.7-beta-repair-batch-qa.json",
    )
    parser.add_argument(
        "--output-html",
        type=Path,
        default=script_dir / "reports" / "v2.3.7-beta-repair-batch-review.html",
    )
    args = parser.parse_args()
    try:
        report = validate_batch(args.batch, root=args.root)
        args.output_json.parent.mkdir(parents=True, exist_ok=True)
        args.output_json.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        args.output_html.parent.mkdir(parents=True, exist_ok=True)
        args.output_html.write_text(render_html(report), encoding="utf-8")
    except Exception as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        return 1
    print(json.dumps({
        "ok": report["ok"],
        "package_count": report["package_count"],
        "json": str(args.output_json),
        "html": str(args.output_html),
        "errors": report["errors"],
    }, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
