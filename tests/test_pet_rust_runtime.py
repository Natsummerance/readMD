"""Production Rust runtime installer/orchestrator contract tests."""

from __future__ import annotations

import hashlib
import json
import zipfile
from pathlib import Path

from src.readmd_modules.pet.runtime import (
    PetRuntimeOrchestrator,
    RustPetRuntimeInstaller,
)


def _manifest_tree(root: Path) -> Path:
    root.mkdir()
    exe = root / "readmd-pet-rust.exe"
    index = root / "renderer" / "index.html"
    index.parent.mkdir()
    exe.write_bytes(b"native-host")
    index.write_text("<!doctype html>", encoding="utf-8")
    artifacts = []
    for path, role in ((exe, "executable"), (index, "renderer")):
        artifacts.append(
            {
                "path": path.relative_to(root).as_posix(),
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "size": path.stat().st_size,
                "role": role,
            }
        )
    (root / "runtime-manifest.json").write_text(
        json.dumps(
            {
                "runtime": "readmd-pet-rust",
                "version": "1.0.0",
                "protocol_version": 1,
                "platform": RustPetRuntimeInstaller._platform(),
                "arch": RustPetRuntimeInstaller._arch(),
                "artifacts": artifacts,
            }
        ),
        encoding="utf-8",
    )
    return root


def test_rust_installer_promotes_verified_runtime_to_fixed_target(tmp_path):
    source = _manifest_tree(tmp_path / "source")
    installer = RustPetRuntimeInstaller(str(tmp_path / "plugins"))

    result = installer.install_directory(str(source), confirm=True)

    assert result["ok"] is True
    assert installer.target == tmp_path / "plugins" / "pet" / "readmd-rust-host"
    assert installer.available() is True
    assert (installer.target / "readmd-pet-rust.exe").read_bytes() == b"native-host"


def test_rust_installer_rejects_hash_mismatch_without_promotion(tmp_path):
    source = _manifest_tree(tmp_path / "source")
    (source / "readmd-pet-rust.exe").write_bytes(b"tampered")
    installer = RustPetRuntimeInstaller(str(tmp_path / "plugins"))

    result = installer.install_directory(str(source), confirm=True)

    assert result["ok"] is False
    assert result["code"] == "rust_runtime_hash_mismatch"
    assert not installer.target.exists()


def test_rust_archive_preserves_manifest_fingerprint(tmp_path):
    source = _manifest_tree(tmp_path / "source")
    archive = tmp_path / "runtime.zip"
    manifest_bytes = (source / "runtime-manifest.json").read_bytes()
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for path in sorted(item for item in source.rglob("*") if item.is_file()):
            bundle.write(path, path.relative_to(source).as_posix())

    installer = RustPetRuntimeInstaller(str(tmp_path / "plugins"))
    result = installer.install_archive(str(archive), confirm=True)

    assert result["ok"] is True
    assert installer.get_installed_manifest_hash() == hashlib.sha256(manifest_bytes).hexdigest()


class _Backend:
    def __init__(self, available=True, start_ok=True):
        self.available = available
        self.start_ok = start_ok
        self.started = False
        self.stopped = False

    def status(self):
        return {"available": self.available, "running": self.started, "health": {}}

    def start(self):
        self.started = self.start_ok
        return {"ok": self.start_ok, "code": "start_failed" if not self.start_ok else ""}

    def stop(self):
        self.stopped = True
        self.started = False


def _orchestrator(rust, electron, mode="auto"):
    value = object.__new__(PetRuntimeOrchestrator)
    value.mode = mode
    value.rust = rust
    value.electron = electron
    value.active_backend = ""
    value.diagnostic = ""
    return value


def test_orchestrator_prefers_rust_and_keeps_electron_idle():
    rust = _Backend()
    electron = _Backend()
    orchestrator = _orchestrator(rust, electron)

    result = orchestrator.start()

    assert result["ok"] is True
    assert orchestrator.active_backend == "rust"
    assert rust.started is True
    assert electron.started is False


def test_orchestrator_falls_back_to_electron_after_rust_start_failure():
    rust = _Backend(start_ok=False)
    electron = _Backend()
    orchestrator = _orchestrator(rust, electron)

    result = orchestrator.start()

    assert result["ok"] is True
    assert orchestrator.active_backend == "electron"
    assert rust.stopped is True
    assert electron.started is True
