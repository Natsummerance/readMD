# -*- coding: utf-8 -*-
"""The desktop bridge must expose safe pet readiness and batch seams."""

import readmd


def test_api_refuses_to_enable_pet_without_a_verified_original_model(monkeypatch):
    monkeypatch.setattr(readmd, "verify_model_bundle", lambda _path: {
        "ready": False,
        "code": "model_manifest_missing",
    })
    api = readmd.Api()

    result = api.configure_pet({"enabled": True})

    assert result == {"ok": False, "code": "model_manifest_missing"}
    assert api.get_pet_runtime_status()["enabled"] is False


def test_api_exposes_non_destructive_pet_batch_queue(tmp_path):
    source = tmp_path / "note.md"
    source.write_text("unchanged", encoding="utf-8")
    api = readmd.Api()

    result = api.enqueue_pet_files([str(source)])

    assert result["ok"] is True
    assert result["tasks"][0]["source_path"] == str(source)
    assert source.read_text(encoding="utf-8") == "unchanged"
