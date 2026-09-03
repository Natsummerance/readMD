# -*- coding: utf-8 -*-
"""Safe host bridge for the optional Hermes-source desktop pet adapter.

The adapter is a separate, on-demand Electron process.  It reads a small
state snapshot and writes one acknowledged control command at a time; it never
receives ReadMD settings, credentials, document contents, or network URLs.
"""

from __future__ import annotations

import json
import os
import hashlib
import shutil
import subprocess
import threading
import tempfile
import time
import zipfile
from pathlib import Path
from typing import Any, Dict, Optional


class HermesPetBridge:
    """Versioned file bridge shared by ReadMD and the copied Hermes overlay."""

    FORMAT_VERSION = 1
    _COMMANDS = frozenset({"bounds", "clipboard", "drop", "open-app", "open-menu", "pop-in", "scale", "submit", "toggle-app"})

    def __init__(self, data_dir: str):
        root = Path(data_dir).resolve() / "pet"
        self._root = root
        self.state_path = root / "hermes-overlay-state.json"
        self.command_path = root / "hermes-overlay-state.json.command"
        self._lock = threading.Lock()

    def publish(self, runtime: Dict[str, Any], *, info: Optional[Dict[str, Any]] = None,
                activity: Optional[Dict[str, Any]] = None, bounds: Optional[Dict[str, Any]] = None,
                renderer: Optional[str] = None, fullscreen: Optional[bool] = None) -> Dict[str, Any]:
        """Atomically publish only the narrow display state required by Hermes."""
        runtime = runtime if isinstance(runtime, dict) else {}
        state = str(runtime.get("state") or "")
        derived_activity = {
            "busy": state == "busy",
            "error": state == "error",
            "justCompleted": state == "success",
        }
        payload = {
            "format_version": self.FORMAT_VERSION,
            "visible": bool(runtime.get("visible")),
            "info": dict(info or {}),
            "activity": {**derived_activity, **dict(activity or {})},
            "busy": state == "busy",
            "awaiting": False,
            "unread": False,
        }
        if isinstance(bounds, dict):
            payload["bounds"] = self._safe_bounds(bounds)
        if renderer is not None:
            payload["renderer"] = str(renderer)
        if fullscreen is not None:
            payload["fullscreen"] = bool(fullscreen)
        self._root.mkdir(parents=True, exist_ok=True)
        tmp = self.state_path.with_suffix(".tmp")
        encoded = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
        with self._lock:
            tmp.write_text(encoded, encoding="utf-8")
            os.replace(str(tmp), str(self.state_path))
        return payload

    def take_command(self) -> Optional[Dict[str, Any]]:
        """Return one validated command, deleting no state file on parse failure."""
        try:
            raw = self.command_path.read_text(encoding="utf-8")
            value = json.loads(raw)
        except (OSError, ValueError, TypeError):
            return None
        if not isinstance(value, dict) or not isinstance(value.get("command"), dict):
            return None
        command = value["command"]
        kind = command.get("type")
        if kind not in self._COMMANDS:
            return None
        if kind == "bounds":
            if not isinstance(command.get("bounds"), dict):
                return None
            command = dict(command)
            command["bounds"] = self._safe_bounds(command["bounds"])
        if kind == "scale":
            try:
                scale = round(float(command.get("scale")), 2)
            except (TypeError, ValueError):
                return None
            if not 0.18 <= scale <= 0.72:
                return None
            command = {"type": "scale", "scale": scale}
        if kind == "drop":
            paths = command.get("paths")
            if not isinstance(paths, list) or not paths or len(paths) > 128:
                return None
            if any(not isinstance(path, str) or not path or len(path) > 32768 for path in paths):
                return None
            command = {"type": "drop", "paths": list(paths)}
        if kind == "clipboard":
            text = command.get("text", "")
            image = command.get("image_png", "")
            paths = command.get("paths", [])
            if not isinstance(text, str) or len(text.encode("utf-8")) > 4 * 1024 * 1024:
                return None
            if not isinstance(image, str) or len(image) > 24 * 1024 * 1024:
                return None
            if not isinstance(paths, list) or len(paths) > 128 or any(not isinstance(path, str) or len(path) > 32768 for path in paths):
                return None
            command = {"type": "clipboard", "text": text, "image_png": image, "paths": list(paths)}
        try:
            self.command_path.unlink()
        except OSError:
            pass
        return command

    @staticmethod
    def _safe_bounds(value: Dict[str, Any]) -> Dict[str, int]:
        result = {}
        for key, low, high in (("x", -32768, 32768), ("y", -32768, 32768),
                               ("width", 80, 2048), ("height", 80, 2048)):
            try:
                number = int(round(float(value[key])))
            except (KeyError, TypeError, ValueError):
                raise ValueError("invalid_pet_bounds")
            result[key] = max(low, min(high, number))
        return result


class HermesPetLauncher:
    """Starts only a bundled adapter runtime; never shells out to arbitrary paths."""

    def __init__(self, app_dir: str, bridge: HermesPetBridge, adapter_dir: Optional[str] = None):
        self._app_dir = Path(app_dir).resolve()
        self._bridge = bridge
        self._external_adapter_dir = Path(adapter_dir).resolve() if adapter_dir else None
        self._process: Optional[subprocess.Popen] = None

    @property
    def adapter_dir(self) -> Path:
        # An installed external package wins. Keeping its Electron runtime in
        # user data prevents the lightweight reader package from inheriting it.
        if self._external_adapter_dir is not None:
            return self._external_adapter_dir
        return self._app_dir / "assets" / "pet" / "hermes-adapter"

    def status(self) -> Dict[str, Any]:
        runtime = self.adapter_dir / "electron.exe"
        app = self.adapter_dir / "app" / "package.json"
        return {
            "available": os.name == "nt" and runtime.is_file() and app.is_file(),
            # The bridge path is an implementation detail.  Returning it from
            # the public status API would expose the user's data directory.
            "bridge_ready": self._bridge.state_path.is_file(),
            "running": self._process is not None and self._process.poll() is None,
        }

    def start(self) -> Dict[str, Any]:
        status = self.status()
        if not status["available"]:
            return {"ok": False, "code": "hermes_adapter_not_installed", "runtime": status}
        if status["running"]:
            return {"ok": True, "runtime": status}
        runtime = self.adapter_dir / "electron.exe"
        app = self.adapter_dir / "app"
        env = os.environ.copy()
        # Electron consumes arbitrary command-line switches before the adapter
        # sees argv on some platforms. A child-only environment variable keeps
        # the bridge path explicit without exposing it to the renderer.
        env["READMD_PET_BRIDGE_FILE"] = str(self._bridge.state_path)
        self._process = subprocess.Popen(
            [str(runtime), str(app)], cwd=str(app), close_fds=True, env=env,
        )
        return {"ok": True, "runtime": self.status()}

    def stop(self) -> None:
        if self._process is not None and self._process.poll() is None:
            self._process.terminate()
        self._process = None


class HermesPetPluginInstaller:
    """Install a signed-by-manifest external plugin without touching the app tree."""

    MAX_ARCHIVE_BYTES = 350 * 1024 * 1024
    MAX_EXPANDED_BYTES = 750 * 1024 * 1024
    MAX_FILES = 3000
    SWAP_ATTEMPTS = 40
    SWAP_RETRY_DELAY = 0.75

    def __init__(self, data_dir: str):
        self.root = Path(data_dir).resolve() / "pet"
        self.target = self.root / "hermes-adapter"

    def _replace_with_retry(self, source: Path, destination: Path) -> None:
        # Windows real-time scanners keep freshly written executables open for
        # a scan window (observed ~6s) that blocks the rename; the budget must
        # comfortably outlast that window, hence ~30s of retries.
        for remaining in range(self.SWAP_ATTEMPTS - 1, -1, -1):
            try:
                os.replace(str(source), str(destination))
                return
            except PermissionError as error:
                if remaining == 0:
                    raise
                time.sleep(self.SWAP_RETRY_DELAY)

    @staticmethod
    def _is_safe_name(name: str) -> bool:
        value = Path(name)
        return bool(name and not value.is_absolute() and ".." not in value.parts and "\\" not in name)

    @staticmethod
    def _sha256(path: Path) -> str:
        digest = hashlib.sha256()
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest()

    def install_archive(self, archive_path: str, *, confirm: bool = False) -> Dict[str, Any]:
        if not confirm:
            return {"ok": False, "code": "pet_install_confirmation_required"}
        archive = Path(archive_path).resolve()
        if not archive.is_file() or archive.suffix.lower() != ".zip":
            return {"ok": False, "code": "invalid_pet_plugin_archive"}
        if archive.stat().st_size > self.MAX_ARCHIVE_BYTES:
            return {"ok": False, "code": "pet_plugin_archive_too_large"}
        try:
            with zipfile.ZipFile(archive) as bundle:
                entries = bundle.infolist()
                if not entries or len(entries) > self.MAX_FILES:
                    return {"ok": False, "code": "invalid_pet_plugin_contents"}
                if any(
                    not self._is_safe_name(item.filename)
                    or ((item.external_attr >> 16) & 0o170000) == 0o120000
                    for item in entries
                ):
                    return {"ok": False, "code": "unsafe_pet_plugin_path"}
                if sum(item.file_size for item in entries) > self.MAX_EXPANDED_BYTES:
                    return {"ok": False, "code": "pet_plugin_expanded_too_large"}
                manifest_item = next((item for item in entries if item.filename == "readmd-pet-plugin.json"), None)
                if manifest_item is None:
                    return {"ok": False, "code": "pet_plugin_manifest_missing"}
                manifest = json.loads(bundle.read(manifest_item).decode("utf-8"))
                if not isinstance(manifest, dict) or manifest.get("id") != "readmd-hermes-pet" or manifest.get("format_version") != 1:
                    return {"ok": False, "code": "invalid_pet_plugin_manifest"}
                listed = manifest.get("files")
                if not isinstance(listed, list) or not listed:
                    return {"ok": False, "code": "invalid_pet_plugin_manifest"}
                expected = {}
                for item in listed:
                    if not isinstance(item, dict) or not self._is_safe_name(str(item.get("path") or "")):
                        return {"ok": False, "code": "invalid_pet_plugin_manifest"}
                    digest = str(item.get("sha256") or "").lower()
                    if len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
                        return {"ok": False, "code": "invalid_pet_plugin_manifest"}
                    expected[str(item["path"])] = digest
                required = {"electron.exe", "app/package.json", "app/electron-main.cjs", "app/preload.cjs"}
                if not required.issubset(expected):
                    return {"ok": False, "code": "pet_plugin_required_file_missing"}
                names = {item.filename for item in entries if not item.is_dir()}
                if not set(expected).issubset(names):
                    return {"ok": False, "code": "pet_plugin_file_missing"}
                self.root.mkdir(parents=True, exist_ok=True)
                # A previously failed install can leave a locked staging dir
                # behind; clear it best-effort before opening a fresh one.
                for stale in self.root.glob("readmd-pet-*"):
                    shutil.rmtree(stale, ignore_errors=True)
                with tempfile.TemporaryDirectory(prefix="readmd-pet-", dir=str(self.root), ignore_cleanup_errors=True) as temporary:
                    staged = Path(temporary) / "adapter"
                    staged.mkdir()
                    for name in expected:
                        destination = staged / name
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        with bundle.open(name) as source, destination.open("wb") as target:
                            target.write(source.read())
                        if self._sha256(destination) != expected[name]:
                            return {"ok": False, "code": "pet_plugin_hash_mismatch"}
                    # A complete manifest means no unlisted executables can be
                    # smuggled into the runtime. The installation is a replace
                    # operation only after every listed file verified.
                    backup = self.root / "hermes-adapter.previous"
                    # The optional Electron runtime is intentionally large.
                    # Unlike a user-authored Skill, it has no user-editable
                    # state and therefore gains nothing from an implicit full
                    # duplicate. Remove a stale, never-launched rollback only
                    # after the new archive has passed every verification.
                    if backup.exists():
                        shutil.rmtree(backup)
                    if self.target.exists():
                        self._replace_with_retry(self.target, backup)
                    try:
                        try:
                            self._replace_with_retry(staged, self.target)
                        except PermissionError:
                            # A scanner can keep an open handle on a freshly
                            # written file inside the staged tree for longer
                            # than any retry budget, which keeps blocking the
                            # directory rename itself. Copying to fresh target
                            # paths is unaffected by such handles and the bytes
                            # were already verified above.
                            try:
                                shutil.copytree(staged, self.target)
                            except OSError:
                                shutil.rmtree(self.target, ignore_errors=True)
                                raise
                    except OSError:
                        # Preserve the last working adapter if the publish
                        # itself fails; no partially extracted runtime remains.
                        if backup.exists() and not self.target.exists():
                            self._replace_with_retry(backup, self.target)
                        raise
                    if backup.exists():
                        shutil.rmtree(backup)
            return {"ok": True, "installed": True, "files": len(expected)}
        except (OSError, ValueError, zipfile.BadZipFile, UnicodeError):
            return {"ok": False, "code": "pet_plugin_install_failed"}
