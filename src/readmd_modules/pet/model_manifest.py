# -*- coding: utf-8 -*-
"""Fail-closed validation for original, redistributable Live2D model bundles."""

from __future__ import annotations

import hashlib
import json
import os
import re
from pathlib import Path


MANIFEST_NAME = "readmd.live2d.json"
_ID = re.compile(r"^[a-z0-9][a-z0-9-]{1,62}$")
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_REQUIRED_RIGHTS = (
    "asset_origin",
    "redistribution_authorization",
    "cubism_publication_license",
)


def verify_model_bundle(root):
    """Return a public readiness report without loading any model or SDK."""
    root = Path(root).resolve()
    manifest_path = root / MANIFEST_NAME
    if not manifest_path.is_file():
        return _fail("model_manifest_missing")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return _fail("invalid_model_manifest")
    if not isinstance(manifest, dict) or manifest.get("format_version") != 1:
        return _fail("unsupported_manifest_format")
    if not _ID.fullmatch(str(manifest.get("id") or "")):
        return _fail("invalid_model_id")
    if manifest.get("renderer") != "cubism-web":
        return _fail("unsupported_model_renderer")
    if not str(manifest.get("author") or "").strip() or not str(manifest.get("license") or "").strip():
        return _fail("missing_model_provenance")
    rights = manifest.get("rights")
    if not isinstance(rights, dict) or any(not str(rights.get(key) or "").strip() for key in _REQUIRED_RIGHTS):
        return _fail("missing_model_rights_chain")
    if str(rights["cubism_publication_license"]).strip().lower() in {"pending", "pending-manual-review", "unknown", "none"}:
        return _fail("cubism_publication_license_pending")
    files = manifest.get("files")
    if not isinstance(files, dict) or not files:
        return _fail("missing_model_files")
    for relative, expected in files.items():
        if not isinstance(relative, str) or not _safe_relative(relative):
            return _fail("unsafe_asset_path")
        if not isinstance(expected, str) or not _DIGEST.fullmatch(expected.lower()):
            return _fail("invalid_asset_digest")
        candidate = (root / relative).resolve()
        if not _inside(root, candidate) or not candidate.is_file() or candidate.is_symlink():
            return _fail("missing_or_unsafe_asset")
        if _sha256(candidate) != expected.lower():
            return _fail("asset_digest_mismatch")
    return {
        "ready": True,
        "code": "ready_for_platform_probe",
        "model_id": manifest["id"],
        "renderer": manifest["renderer"],
        "license": manifest["license"],
        "asset_count": len(files),
    }


def _safe_relative(value):
    path = Path(value)
    return not path.is_absolute() and ".." not in path.parts and not any(part in {"", "."} for part in path.parts)


def _inside(root, candidate):
    try:
        candidate.relative_to(root)
        return True
    except ValueError:
        return False


def _sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _fail(code):
    return {"ready": False, "code": code}
