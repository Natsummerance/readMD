# -*- coding: utf-8 -*-
"""Contract tests for the ReadMD-owned shell around copied Hermes code."""

from __future__ import annotations

import json
import hashlib
import zipfile
from pathlib import Path

from src.readmd_modules.pet import HermesPetBridge, HermesPetLauncher, HermesPetPluginInstaller


def test_hermes_fallback_sheet_uses_its_actual_cell_geometry():
    """Do not regress to treating the complete 4x2 sheet as one huge pet."""
    source = Path('packages/readmd-hermes-pet-adapter/src/electron-main.ts').read_text(encoding='utf-8')
    assert 'frameH: 512' in source
    assert 'frameW: 384' in source
    assert 'framesPerState: 4' in source
    assert "stateRows: ['idle', 'wave']" in source
    assert 'frameH: 1024' not in source
    assert 'frameW: 1536' not in source


def test_optional_host_applies_a_bounded_native_window_opacity():
    source = Path('packages/readmd-hermes-pet-adapter/src/electron-main.ts').read_text(encoding='utf-8')
    assert 'function currentOpacity()' in source
    assert 'Math.max(0.35, Math.min(1, raw))' in source
    assert 'overlay.setOpacity(currentOpacity())' in source


def test_bridge_writes_only_narrow_display_state_and_sanitizes_bounds(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))
    payload = bridge.publish(
        {"visible": True, "state": "busy"},
        info={"enabled": True, "displayName": "ReadMD"},
        bounds={"x": 5.5, "y": -4, "width": 300, "height": 420},
    )
    on_disk = json.loads(bridge.state_path.read_text(encoding="utf-8"))

    assert payload == on_disk
    assert on_disk["format_version"] == 1
    assert on_disk["busy"] is True
    assert on_disk["activity"]["busy"] is True
    assert on_disk["bounds"] == {"x": 6, "y": -4, "width": 300, "height": 420}
    assert "credential" not in json.dumps(on_disk).lower()


def test_bridge_maps_readmd_work_states_to_the_copied_hermes_activity_contract(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))
    assert bridge.publish({"visible": True, "state": "success"})["activity"] == {
        "busy": False, "error": False, "justCompleted": True,
    }
    assert bridge.publish({"visible": True, "state": "error"})["activity"]["error"] is True


def test_bridge_accepts_only_known_control_commands(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))
    bridge.command_path.parent.mkdir(parents=True)
    bridge.command_path.write_text(json.dumps({"command": {"type": "bounds", "bounds": {
        "x": 999999, "y": -999999, "width": 1, "height": 999999}}}), encoding="utf-8")

    assert bridge.take_command() == {"type": "bounds", "bounds": {
        "x": 32768, "y": -32768, "width": 80, "height": 2048}}
    assert not bridge.command_path.exists()

    bridge.command_path.write_text(json.dumps({"command": {"type": "run-shell"}}), encoding="utf-8")
    assert bridge.take_command() is None
    assert bridge.command_path.exists()


def test_bridge_accepts_bounded_drop_paths_without_executing_them(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))
    bridge.command_path.parent.mkdir(parents=True)
    bridge.command_path.write_text(json.dumps({"command": {"type": "drop", "paths": ["C:/one.md", "C:/two.pdf"]}}), encoding="utf-8")

    assert bridge.take_command() == {"type": "drop", "paths": ["C:/one.md", "C:/two.pdf"]}


def test_bridge_accepts_the_copied_overlay_quick_menu_event(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))
    bridge.command_path.parent.mkdir(parents=True)
    bridge.command_path.write_text(json.dumps({"command": {"type": "open-menu"}}), encoding="utf-8")

    assert bridge.take_command() == {"type": "open-menu"}


def test_bridge_accepts_only_the_bounded_hermes_scale_range(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))
    bridge.command_path.parent.mkdir(parents=True)
    bridge.command_path.write_text(json.dumps({"command": {"type": "scale", "scale": 0.33}}), encoding="utf-8")
    assert bridge.take_command() == {"type": "scale", "scale": 0.33}

    bridge.command_path.write_text(json.dumps({"command": {"type": "scale", "scale": 99}}), encoding="utf-8")
    assert bridge.take_command() is None


def test_launcher_is_fail_closed_without_bundled_runtime(tmp_path):
    bridge = HermesPetBridge(str(tmp_path / "data"))
    launcher = HermesPetLauncher(str(tmp_path / "app"), bridge)

    assert launcher.status()["available"] is False
    assert launcher.start()["code"] == "hermes_adapter_not_installed"


def test_launcher_passes_bridge_path_only_to_the_child_environment(monkeypatch, tmp_path):
    bridge = HermesPetBridge(str(tmp_path / "data"))
    launcher = HermesPetLauncher(str(tmp_path / "app"), bridge)
    launcher.adapter_dir.mkdir(parents=True)
    (launcher.adapter_dir / "electron.exe").write_bytes(b"runtime")
    (launcher.adapter_dir / "app").mkdir()
    (launcher.adapter_dir / "app" / "package.json").write_text("{}", encoding="utf-8")
    captured = {}

    class Process:
        def poll(self):
            return None

    def fake_popen(args, **kwargs):
        captured["args"] = args
        captured["env"] = kwargs["env"]
        return Process()

    monkeypatch.setattr("src.readmd_modules.pet.hermes_adapter.subprocess.Popen", fake_popen)

    assert launcher.start()["ok"] is True
    assert captured["args"] == [str(launcher.adapter_dir / "electron.exe"), str(launcher.adapter_dir / "app")]
    assert captured["env"]["READMD_PET_BRIDGE_FILE"] == str(bridge.state_path)


def test_launcher_uses_an_external_adapter_dir_without_bloating_the_app(tmp_path):
    bridge = HermesPetBridge(str(tmp_path / "data"))
    external = tmp_path / "data" / "pet" / "hermes-adapter"
    launcher = HermesPetLauncher(str(tmp_path / "app"), bridge, adapter_dir=str(external))

    assert launcher.adapter_dir == external.resolve()
    assert launcher.status()["available"] is False


def _write_plugin_archive(path, *, corrupt=False):
    files = {
        "electron.exe": b"runtime",
        "app/package.json": b'{}',
        "app/electron-main.cjs": b"main",
        "app/preload.cjs": b"preload",
        "app/renderer/index.html": b"<main></main>",
    }
    manifest = {
        "format_version": 1,
        "id": "readmd-hermes-pet",
        "files": [
            {"path": name, "sha256": ("0" * 64 if corrupt and name == "electron.exe" else hashlib.sha256(data).hexdigest())}
            for name, data in files.items()
        ],
    }
    with zipfile.ZipFile(path, "w") as archive:
        for name, data in files.items():
            archive.writestr(name, data)
        archive.writestr("readmd-pet-plugin.json", json.dumps(manifest))


def test_plugin_installer_verifies_manifest_before_installing_to_user_data(tmp_path):
    archive = tmp_path / "readmd-pet.zip"
    _write_plugin_archive(archive)
    installer = HermesPetPluginInstaller(str(tmp_path / "data"))

    assert installer.install_archive(str(archive), confirm=False)["code"] == "pet_install_confirmation_required"
    result = installer.install_archive(str(archive), confirm=True)

    assert result == {"ok": True, "installed": True, "files": 5}
    assert (installer.target / "electron.exe").read_bytes() == b"runtime"
    assert (installer.target / "app" / "package.json").read_text(encoding="utf-8") == "{}"


def test_plugin_installer_rejects_a_bad_manifest_without_creating_runtime(tmp_path):
    archive = tmp_path / "readmd-pet-corrupt.zip"
    _write_plugin_archive(archive, corrupt=True)
    installer = HermesPetPluginInstaller(str(tmp_path / "data"))

    assert installer.install_archive(str(archive), confirm=True)["code"] == "pet_plugin_hash_mismatch"
    assert not installer.target.exists()


def test_plugin_update_replaces_only_the_verified_optional_runtime(tmp_path):
    first = tmp_path / "readmd-pet-one.zip"
    second = tmp_path / "readmd-pet-two.zip"
    _write_plugin_archive(first)
    _write_plugin_archive(second)
    installer = HermesPetPluginInstaller(str(tmp_path / "data"))

    assert installer.install_archive(str(first), confirm=True)["ok"] is True
    (installer.target / "app" / "package.json").write_text('{"version":"old"}', encoding="utf-8")
    assert installer.install_archive(str(second), confirm=True)["ok"] is True

    assert (installer.target / "app" / "package.json").read_text(encoding="utf-8") == "{}"
    assert not (installer.root / "hermes-adapter.previous").exists()
