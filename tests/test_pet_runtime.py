# -*- coding: utf-8 -*-
"""Public-contract tests for the optional ReadMD desktop pet runtime."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from src.readmd_modules.pet import (
    PetBatchQueue,
    PetController,
    NativePetProbe,
    verify_model_bundle,
)


def test_pet_controller_defaults_off_and_obeys_motion_and_fullscreen():
    pet = PetController()

    assert pet.snapshot()["visible"] is False
    assert pet.enable()["state"] == "idle"
    assert pet.snapshot()["fps_cap"] == 6

    pet.handle_event("work_started")
    assert pet.snapshot()["fps_cap"] == 30
    assert pet.snapshot()["animation_enabled"] is True

    pet.set_reduced_motion(True)
    assert pet.snapshot()["fps_cap"] == 0
    assert pet.snapshot()["animation_enabled"] is False

    pet.set_fullscreen(True)
    assert pet.snapshot()["visible"] is False
    assert pet.snapshot()["enabled"] is True


def test_pet_batch_queue_groups_files_and_never_accepts_source_as_result(tmp_path):
    document = tmp_path / "note.md"
    document.write_text("# original", encoding="utf-8")
    image = tmp_path / "scan.png"
    image.write_bytes(b"png")

    queue = PetBatchQueue()
    tasks = queue.submit([str(document), str(image)])
    grouped = queue.grouped_snapshot()

    assert {task["kind"] for task in tasks} == {"markdown", "image"}
    assert set(grouped) == {"markdown", "image"}

    queue.start(tasks[0]["id"])
    rejected = queue.complete(tasks[0]["id"], output_path=str(document))
    assert rejected["status"] == "failed"
    assert rejected["code"] == "output_would_overwrite_source"
    assert document.read_text(encoding="utf-8") == "# original"


def test_model_bundle_requires_rights_chain_and_hashes(tmp_path):
    root = tmp_path / "original-model"
    root.mkdir()
    model = root / "character.model3.json"
    model.write_text('{"Version":3}', encoding="utf-8")
    motion = root / "idle.motion3.json"
    motion.write_text('{"Meta":{}}', encoding="utf-8")

    manifest = {
        "format_version": 1,
        "id": "readmd-original-character",
        "renderer": "cubism-web",
        "author": "ReadMD",
        "license": "LicenseRef-ReadMD-Original",
        "rights": {
            "asset_origin": "ReadMD original commissioned model",
            "redistribution_authorization": "internal record: asset-rights-001",
            "cubism_publication_license": "pending-manual-review",
        },
        "files": {
            "character.model3.json": hashlib.sha256(model.read_bytes()).hexdigest(),
            "idle.motion3.json": hashlib.sha256(motion.read_bytes()).hexdigest(),
        },
    }
    (root / "readmd.live2d.json").write_text(json.dumps(manifest), encoding="utf-8")

    verified = verify_model_bundle(root)
    assert verified["ready"] is False
    assert verified["code"] == "cubism_publication_license_pending"

    manifest["rights"]["cubism_publication_license"] = "approved-reference-2026-08-31"
    (root / "readmd.live2d.json").write_text(json.dumps(manifest), encoding="utf-8")
    verified = verify_model_bundle(root)
    assert verified["ready"] is True
    assert verified["model_id"] == "readmd-original-character"


def test_model_bundle_rejects_path_escape_and_digest_mismatch(tmp_path):
    root = tmp_path / "model"
    root.mkdir()
    manifest = {
        "format_version": 1,
        "id": "safe-model",
        "renderer": "cubism-web",
        "author": "ReadMD",
        "license": "LicenseRef-ReadMD-Original",
        "rights": {
            "asset_origin": "original",
            "redistribution_authorization": "record",
            "cubism_publication_license": "approved-record",
        },
        "files": {"../escape.moc3": "0" * 64},
    }
    (root / "readmd.live2d.json").write_text(json.dumps(manifest), encoding="utf-8")

    verified = verify_model_bundle(root)
    assert verified["ready"] is False
    assert verified["code"] == "unsafe_asset_path"


def test_native_probe_only_claims_capabilities_available_in_current_backend():
    class CompatibleWebview:
        @staticmethod
        def create_window(title, **kwargs):
            return {"title": title, **kwargs}

        @staticmethod
        def start(*args, **kwargs):
            return None

    report = NativePetProbe.probe_capabilities(CompatibleWebview, platform_name="win32")

    assert report["native_window"] is True
    assert report["transparent_window"] is True
    assert report["always_on_top"] is True
    assert report["click_through"] == "manual-verification-required"
    assert report["release_ready"] is False
