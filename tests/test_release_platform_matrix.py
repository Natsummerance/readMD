# -*- coding: utf-8 -*-
import json
from pathlib import Path


def test_formal_platform_matrix_is_explicit_and_harmonyos_is_out_of_scope():
    path = Path(__file__).parents[1] / "release" / "platform-matrix.json"
    matrix = json.loads(path.read_text(encoding="utf-8"))
    ids = {item["id"] for item in matrix["formal_support"]}
    assert {"windows-x64", "windows-arm64", "macos-x64", "macos-arm64", "linux-x64", "linux-arm64"} <= ids
    assert "HarmonyOS/OpenHarmony" in matrix["out_of_scope"]
    assert matrix["evidence_policy"]["release_blocked_until_complete"] is True
    assert all(item.get("native_backend") for item in matrix["formal_support"])
    assert matrix["feature_matrix"]["no_implicit_empty_cells"] is True
    assert matrix["feature_matrix"]["default_status"] == "pending-native-evidence"


def test_matrix_covers_every_declared_feature():
    path = Path(__file__).parents[1] / "release" / "platform-matrix.json"
    matrix = json.loads(path.read_text(encoding="utf-8"))
    required = {"open", "edit", "save", "preview", "convert", "export", "ocr", "ai", "skills", "vsix", "mcp"}
    assert required <= set(matrix["features"])
