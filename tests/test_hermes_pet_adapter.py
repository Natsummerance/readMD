# -*- coding: utf-8 -*-
"""Contract tests for the ReadMD-owned shell around copied Hermes code."""

from __future__ import annotations

import json
import hashlib
import os
import re
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


def test_optional_host_loads_the_overlay_with_a_renderer_query():
    source = Path('packages/readmd-hermes-pet-adapter/src/electron-main.ts').read_text(encoding='utf-8')
    assert "loadFile(path.join(app.getAppPath(), 'renderer', 'index.html')" in source
    assert 'query: { renderer }' in source


def test_optional_host_reloads_in_place_when_the_renderer_preference_changes():
    source = Path('packages/readmd-hermes-pet-adapter/src/electron-main.ts').read_text(encoding='utf-8')
    assert 'lastRenderer' in source
    assert 'renderer !== lastRenderer' in source
    # A renderer switch only swaps the page in the existing window; the Electron
    # process and its IPC registrations must survive.
    assert 'app.relaunch' not in source
    assert 'app.quit()' not in source


def test_optional_host_hides_the_overlay_during_fullscreen_and_restores_afterwards():
    source = Path('packages/readmd-hermes-pet-adapter/src/electron-main.ts').read_text(encoding='utf-8')
    assert 'if (next.fullscreen === true) overlay.hide()' in source
    assert 'else overlay.showInactive()' in source


def test_overlay_entry_branches_to_the_live2d_stage_by_query_parameter():
    source = Path('packages/readmd-hermes-pet-adapter/src/renderer.tsx').read_text(encoding='utf-8')
    assert "location.search" in source
    assert "import('./live2d/stage')" in source
    assert "import('../.generated/overlay-root')" in source


def test_live2d_stage_boots_cubism_core_before_the_model_runtime():
    source = Path('packages/readmd-hermes-pet-adapter/src/live2d/stage.ts').read_text(encoding='utf-8')
    assert "'../vendor/live2dcubismcore.min.js'" in source
    assert 'Live2DCubismCore' in source
    assert "'../models/arch-chan/readmd.live2d.json'" in source


def test_live2d_stage_reuses_the_hermes_overlay_ipc_contract():
    source = Path('packages/readmd-hermes-pet-adapter/src/live2d/stage.ts').read_text(encoding='utf-8')
    assert 'hermesDesktop' in source
    assert 'petOverlay' in source
    assert "type: 'ready'" in source
    assert "type: 'open-menu'" in source
    assert "type: 'toggle-app'" in source
    assert 'setBounds' in source
    assert 'setIgnoreMouse' in source
    assert 'onState' in source
    assert 'info.scale' in source
    assert 'hitTest' in source
    assert 'expression' in source


def test_live2d_dependencies_stay_inside_the_optional_adapter_package():
    manifest = json.loads(Path('packages/readmd-hermes-pet-adapter/package.json').read_text(encoding='utf-8'))
    assert manifest['dependencies']['pixi.js'] == '^6.5.10'
    assert manifest['dependencies']['pixi-live2d-display'] == '^0.4.0'


def test_build_pins_the_cubism_core_download_with_a_verified_hash():
    source = Path('packages/readmd-hermes-pet-adapter/scripts/build.mjs').read_text(encoding='utf-8')
    assert 'ensureCubismCore' in source
    assert 'READMD_PET_CUBISM_CORE' in source
    assert 'cubism.live2d.com' in source
    assert 'live2dcubismcore.min.js' in source
    assert re.search(r"'[0-9a-f]{64}'", source)
    assert "'.cache'" in source
    assert "path.join(out, 'vendor')" in source


def test_staging_includes_the_arch_chan_model_bundle():
    source = Path('packages/readmd-hermes-pet-adapter/scripts/stage-plugin.mjs').read_text(encoding='utf-8')
    assert "path.resolve(root, '../../assets/pet/model')" in source
    assert "path.join(output, 'app', 'models', 'arch-chan')" in source
    assert re.search(r'(?<!path\.)\bresolve\(', source) is None


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


def test_bridge_publishes_optional_renderer_and_fullscreen_keys(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))

    payload = bridge.publish({"visible": True}, renderer="live2d", fullscreen=True)

    on_disk = json.loads(bridge.state_path.read_text(encoding="utf-8"))
    assert payload == on_disk
    assert payload["renderer"] == "live2d"
    assert payload["fullscreen"] is True
    assert payload["format_version"] == 1


def test_bridge_omits_renderer_and_fullscreen_keys_when_not_supplied(tmp_path):
    bridge = HermesPetBridge(str(tmp_path))

    payload = bridge.publish({"visible": True, "state": "busy"})

    assert "renderer" not in payload
    assert "fullscreen" not in payload


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


def test_install_retries_swap_when_files_are_transiently_locked(tmp_path, monkeypatch):
    archive = tmp_path / "readmd-pet.zip"
    _write_plugin_archive(archive)
    installer = HermesPetPluginInstaller(str(tmp_path / "data"))
    monkeypatch.setattr(installer, "SWAP_ATTEMPTS", 2, raising=False)
    monkeypatch.setattr(installer, "SWAP_RETRY_DELAY", 0.0, raising=False)

    real_replace = os.replace
    swapped = []

    def flaky_replace(source, destination):
        swapped.append(destination)
        if Path(destination).name == "hermes-adapter" and len(swapped) == 1:
            raise PermissionError(5, "transient lock")
        return real_replace(source, destination)

    monkeypatch.setattr(os, "replace", flaky_replace)

    assert installer.install_archive(str(archive), confirm=True)["ok"] is True
    assert len([d for d in swapped if Path(d).name == "hermes-adapter"]) >= 2


def test_install_swap_outlasts_a_lock_window_longer_than_the_legacy_retry_budget(tmp_path, monkeypatch):
    # A real-world install hit a scanner window (~6s) longer than the legacy
    # 8 x 0.75s budget; the swap must keep retrying well past that window.
    archive = tmp_path / "readmd-pet.zip"
    _write_plugin_archive(archive)
    installer = HermesPetPluginInstaller(str(tmp_path / "data"))
    monkeypatch.setattr(installer, "SWAP_RETRY_DELAY", 0.0, raising=False)

    real_replace = os.replace
    locked = {"count": 0}

    def scanning_replace(source, destination):
        if Path(destination).name == "hermes-adapter" and locked["count"] < 12:
            locked["count"] += 1
            raise PermissionError(5, "scanner still holds the freshly written file")
        return real_replace(source, destination)

    monkeypatch.setattr(os, "replace", scanning_replace)

    assert installer.install_archive(str(archive), confirm=True)["ok"] is True
    assert locked["count"] == 12


def test_swap_retry_budget_covers_a_realistic_antivirus_scan_window():
    assert HermesPetPluginInstaller.SWAP_ATTEMPTS * HermesPetPluginInstaller.SWAP_RETRY_DELAY >= 30


def test_install_falls_back_to_copy_when_a_staged_file_stays_locked(tmp_path, monkeypatch):
    # A scanner can keep an open handle on a freshly written file for longer
    # than any reasonable retry budget, which keeps blocking the directory
    # rename itself. The verified bytes must still reach the target by being
    # copied to fresh paths, which no open handle can block.
    archive = tmp_path / "readmd-pet.zip"
    _write_plugin_archive(archive)
    installer = HermesPetPluginInstaller(str(tmp_path / "data"))
    monkeypatch.setattr(installer, "SWAP_RETRY_DELAY", 0.0, raising=False)

    real_replace = os.replace

    def replace_denied_for_staged_dir(source, destination):
        if Path(source).name == "adapter" and Path(destination).name == "hermes-adapter":
            raise PermissionError(5, "scanner keeps a staged file open")
        return real_replace(source, destination)

    monkeypatch.setattr(os, "replace", replace_denied_for_staged_dir)

    assert installer.install_archive(str(archive), confirm=True) == {"ok": True, "installed": True, "files": 5}
    assert (installer.target / "electron.exe").read_bytes() == b"runtime"
    assert (installer.target / "app" / "renderer" / "index.html").read_text(encoding="utf-8") == "<main></main>"
    assert not (installer.root / "hermes-adapter.previous").exists()


def test_install_sweeps_stale_temp_dirs_left_by_failed_installs(tmp_path):
    archive = tmp_path / "readmd-pet.zip"
    _write_plugin_archive(archive)
    installer = HermesPetPluginInstaller(str(tmp_path / "data"))
    garbage = installer.root / "readmd-pet-garbage"
    (garbage / "adapter" / "resources").mkdir(parents=True)
    (garbage / "adapter" / "resources" / "default_app.asar").write_bytes(b"stale")

    assert installer.install_archive(str(archive), confirm=True)["ok"] is True
    assert not garbage.exists()
