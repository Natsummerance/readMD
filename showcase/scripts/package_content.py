#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Package a QA-green showcase without flattening required subdirectories."""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
import zipfile
from pathlib import Path


REQUIRED_FILES = (
    "story.json",
    "title.txt",
    "body.txt",
    "topics.txt",
    "metadata.json",
    "composition.json",
    "variants.json",
    "qa.json",
    "copy-review.json",
    "pattern-audit.json",
    "dashboard-qa.json",
    "performance-report.json",
    "review-dashboard.html",
    "evidence/release-notes.md",
    "evidence/release.diff",
    "evidence/evidence-manifest.json",
    "raw/capture.json",
    "wechat/readmd-wechat.html",
    "wechat/wechat-qa.json",
)
EVIDENCE_ARTIFACTS = ("release-notes.md", "release.diff")
MANIFEST_SCHEMA_VERSION = 1


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def validate_release_evidence(package_dir: Path) -> None:
    """Verify that packaged claims point to immutable release snapshots."""
    package_dir = package_dir.resolve()
    manifest_path = package_dir / "evidence" / "evidence-manifest.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise ValueError(f"evidence manifest unreadable: {exc}") from exc

    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, dict) or set(artifacts) != set(EVIDENCE_ARTIFACTS):
        expected = ", ".join(EVIDENCE_ARTIFACTS)
        raise ValueError(f"evidence manifest must contain exactly: {expected}")

    for filename, artifact in artifacts.items():
        if not isinstance(artifact, dict) or artifact.get("path") != f"evidence/{filename}":
            raise ValueError(f"evidence path mismatch: {filename}")
        path = package_dir / "evidence" / filename
        if not path.is_file():
            raise ValueError(f"evidence snapshot missing: {filename}")
        payload = path.read_bytes()
        if len(payload) != int(artifact.get("bytes", -1)):
            raise ValueError(f"evidence byte count mismatch: {filename}")
        if _sha256(payload) != artifact.get("sha256"):
            raise ValueError(f"evidence sha256 mismatch: {filename}")

    story = json.loads((package_dir / "story.json").read_text(encoding="utf-8"))
    referenced = [
        source
        for claim in story.get("claims", [])
        for source in claim.get("sources", [])
        if str(source).startswith("evidence/")
    ]
    evidence_root = (package_dir / "evidence").resolve()
    missing = []
    for source in referenced:
        resolved = (package_dir / source).resolve()
        if not resolved.is_relative_to(evidence_root) or not resolved.is_file():
            missing.append(source)
    if missing:
        raise ValueError("packaged evidence references are missing: " + ", ".join(sorted(missing)))


def _relative_files(package_dir: Path) -> list[Path]:
    return sorted(
        (path for path in package_dir.rglob("*") if path.is_file() and not path.is_symlink()),
        key=lambda path: path.as_posix(),
    )


def verify_package_manifest(zip_path: Path) -> dict[str, object] | None:
    """Verify an adjacent transport manifest when the packaging step supplied one."""
    zip_path = Path(zip_path)
    manifest_path = Path(str(zip_path) + ".manifest.json")
    if not manifest_path.is_file():
        return None

    try:
        expected = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise ValueError(f"package manifest unreadable: {exc}") from exc

    if expected.get("schema_version") != MANIFEST_SCHEMA_VERSION:
        raise ValueError("package manifest schema_version is unsupported")
    if not isinstance(expected.get("files"), dict):
        raise ValueError("package manifest files must be an object")

    actual_digest = _sha256(zip_path.read_bytes())
    if actual_digest != expected.get("archive_sha256"):
        raise ValueError("package archive SHA-256 mismatch")

    with zipfile.ZipFile(zip_path) as archive:
        members = archive.infolist()
        actual_names = {member.filename for member in members}
        expected_names = set(expected["files"])
        missing = sorted(expected_names - actual_names)
        extra = sorted(actual_names - expected_names)
        if missing or extra:
            details = []
            if missing:
                details.append(f"missing {missing}")
            if extra:
                details.append(f"extra {extra}")
            raise ValueError("package manifest file set mismatch: " + "; ".join(details))
        for member in members:
            artifact = expected["files"].get(member.filename)
            payload = archive.read(member)
            if not isinstance(artifact, dict):
                raise ValueError(f"package manifest entry missing: {member.filename}")
            if len(payload) != int(artifact.get("bytes", -1)):
                raise ValueError(f"package byte count mismatch: {member.filename}")
            if _sha256(payload) != artifact.get("sha256"):
                raise ValueError(f"package SHA-256 mismatch: {member.filename}")
    return expected


def package_content(package_dir: Path, output: Path) -> dict[str, object]:
    package_dir = package_dir.resolve()
    if not package_dir.is_dir():
        raise ValueError(f"package directory does not exist: {package_dir}")

    missing = [name for name in REQUIRED_FILES if not (package_dir / name).is_file()]
    if missing:
        raise ValueError("required package files are missing: " + ", ".join(missing))

    qa = json.loads((package_dir / "qa.json").read_text(encoding="utf-8"))
    if qa.get("ok") is not True:
        raise ValueError("refusing to package red QA: " + "; ".join(map(str, qa.get("errors", []))))
    validate_release_evidence(package_dir)

    images = list((package_dir / "images").glob("*.jpg"))
    raw_shots = list((package_dir / "raw").glob("*.png"))
    if not images:
        raise ValueError("package has no composed JPG cards")
    if not raw_shots:
        raise ValueError("package has no authentic PNG evidence")

    files = _relative_files(package_dir)
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in files:
            relative = path.relative_to(package_dir).as_posix()
            archive.write(path, relative)

    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    manifest = {
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "archive_sha256": digest,
        "file_count": len(files),
        "files": {
            path.relative_to(package_dir).as_posix(): {
                "bytes": path.stat().st_size,
                "sha256": _sha256(path.read_bytes()),
            }
            for path in files
        },
    }
    manifest_path = Path(str(output) + ".manifest.json")
    manifest_payload = (json.dumps(manifest, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")
    manifest_path.write_bytes(manifest_payload)
    return {
        "schema_version": 1,
        "ok": True,
        "output": str(output.resolve()),
        "file_count": len(files),
        "image_count": len(images),
        "raw_shot_count": len(raw_shots),
        "sha256": digest,
        "manifest": str(manifest_path.resolve()),
        "manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        report = package_content(args.package, args.output)
    except Exception as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
