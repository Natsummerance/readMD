# -*- coding: utf-8 -*-
"""Safe host bridge for the optional Hermes-source desktop pet adapter.

The adapter is a separate, on-demand Electron process.  It reads a small
state snapshot and writes one acknowledged control command at a time; it never
receives ReadMD settings, credentials, document contents, or network URLs.
"""

from __future__ import annotations

import json
import logging
import os
import hashlib
import shutil
import stat
import subprocess
import threading
import tempfile
import time
import zipfile
from pathlib import Path
from typing import Any, Dict, Optional


def kill_processes_by_target(target_dir: Path) -> None:
    """Forcefully terminate any process running from or within target_dir."""
    try:
        resolved_str = str(target_dir.resolve()).lower()
    except (OSError, ValueError):
        resolved_str = str(target_dir).lower()

    # 1. Try psutil for exact path match and recursive child process tree termination
    try:
        import psutil
        for proc in psutil.process_iter(['pid', 'name', 'exe']):
            try:
                exe = proc.info.get('exe')
                if exe and resolved_str in str(exe).lower():
                    try:
                        for child in proc.children(recursive=True):
                            try:
                                child.kill()
                            except Exception:
                                pass
                    except Exception:
                        pass
                    proc.kill()
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                continue
    except Exception:
        pass

    # 2. Windows fallback: check PowerShell / taskkill if on Windows
    if os.name == 'nt':
        try:
            ps_script = f'$p = "{resolved_str}".Replace("\\", "\\\\"); Get-Process -Name electron -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -and $_.Path.ToLower().Contains($p) }} | ForEach-Object {{ Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }}'
            subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-Command', ps_script],
                           capture_output=True, timeout=5)
        except Exception:
            pass


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
        claim_path = self.command_path.with_name(
            f"{self.command_path.name}.claim.{os.getpid()}.{time.time_ns()}"
        )
        try:
            os.replace(str(self.command_path), str(claim_path))
        except OSError:
            return None

        def _restore_or_discard():
            if not self.command_path.exists():
                try:
                    os.replace(str(claim_path), str(self.command_path))
                    return
                except OSError:
                    pass
            try:
                claim_path.unlink()
            except OSError:
                pass

        try:
            raw = claim_path.read_text(encoding="utf-8")
            value = json.loads(raw)
        except (OSError, ValueError, TypeError):
            _restore_or_discard()
            return None
        if not isinstance(value, dict) or not isinstance(value.get("command"), dict):
            _restore_or_discard()
            return None
        command = value["command"]
        kind = command.get("type")
        if kind not in self._COMMANDS:
            _restore_or_discard()
            return None
        if kind == "bounds":
            if not isinstance(command.get("bounds"), dict):
                _restore_or_discard()
                return None
            command = dict(command)
            command["bounds"] = self._safe_bounds(command["bounds"])
        if kind == "scale":
            try:
                scale = round(float(command.get("scale")), 2)
            except (TypeError, ValueError):
                _restore_or_discard()
                return None
            if not 0.18 <= scale <= 0.72:
                _restore_or_discard()
                return None
            command = {"type": "scale", "scale": scale}
        if kind == "drop":
            paths = command.get("paths")
            if not isinstance(paths, list) or not paths or len(paths) > 128:
                _restore_or_discard()
                return None
            if any(not isinstance(path, str) or not path or len(path) > 32768 for path in paths):
                _restore_or_discard()
                return None
            command = {"type": "drop", "paths": list(paths)}
        if kind == "clipboard":
            text = command.get("text", "")
            image = command.get("image_png", "")
            paths = command.get("paths", [])
            if not isinstance(text, str) or len(text.encode("utf-8")) > 4 * 1024 * 1024:
                _restore_or_discard()
                return None
            if not isinstance(image, str) or len(image) > 24 * 1024 * 1024:
                _restore_or_discard()
                return None
            if not isinstance(paths, list) or len(paths) > 128 or any(not isinstance(path, str) or len(path) > 32768 for path in paths):
                _restore_or_discard()
                return None
            command = {"type": "clipboard", "text": text, "image_png": image, "paths": list(paths)}
        try:
            claim_path.unlink()
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
        self._launch_lock = threading.Lock()

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
        is_running = self._process is not None and self._process.poll() is None
        if not is_running and os.name == 'nt' and runtime.is_file():
            try:
                import psutil
                resolved_runtime = str(runtime.resolve()).lower()
                for proc in psutil.process_iter(['pid', 'exe']):
                    try:
                        exe = proc.info.get('exe')
                        if exe and str(exe).lower() == resolved_runtime:
                            is_running = True
                            break
                    except (psutil.NoSuchProcess, psutil.AccessDenied):
                        continue
            except Exception:
                pass
        return {
            "available": os.name == "nt" and runtime.is_file() and app.is_file(),
            # The bridge path is an implementation detail.  Returning it from
            # the public status API would expose the user's data directory.
            "bridge_ready": self._bridge.state_path.is_file(),
            "running": is_running,
        }

    def start(self) -> Dict[str, Any]:
        with self._launch_lock:
            status = self.status()
            if not status["available"]:
                return {"ok": False, "code": "hermes_adapter_not_installed", "runtime": status}
            if status["running"]:
                return {"ok": True, "runtime": status}

            # Terminate any orphaned / zombie electron processes running from this adapter dir
            kill_processes_by_target(self.adapter_dir)

            runtime = self.adapter_dir / "electron.exe"
            app = self.adapter_dir / "app"
            env = os.environ.copy()
            env.pop('ELECTRON_RUN_AS_NODE', None)
            # Electron consumes arbitrary command-line switches before the adapter
            # sees argv on some platforms. A child-only environment variable keeps
            # the bridge path explicit without exposing it to the renderer.
            env["READMD_PET_BRIDGE_FILE"] = str(self._bridge.state_path)
            env["READMD_PARENT_PID"] = str(os.getpid())
            try:
                self._process = subprocess.Popen(
                    [str(runtime), str(app)], cwd=str(app), close_fds=True, env=env,
                )
            except OSError:
                logging.exception('Could not launch desktop pet')
                return {"ok": False, "code": "hermes_adapter_start_failed", "runtime": self.status()}
            return {"ok": True, "runtime": self.status()}

    def stop(self) -> None:
        with self._launch_lock:
            if self._process is not None:
                pid = getattr(self._process, 'pid', None)
                if pid and os.name == 'nt':
                    try:
                        subprocess.run(['taskkill', '/F', '/T', '/PID', str(pid)], capture_output=True, timeout=3)
                    except Exception:
                        pass
                try:
                    self._process.terminate()
                except Exception:
                    pass
                try:
                    self._process.wait(timeout=2)
                except Exception:
                    pass
                self._process = None

            # Also ensure all lingering child processes or unmanaged electron.exe under adapter_dir are killed
            kill_processes_by_target(self.adapter_dir)
            time.sleep(0.3)


def get_app_install_dir() -> Path:
    import sys
    if getattr(sys, 'frozen', False):
        return Path(sys.executable).resolve().parent
    # Return repo root / app root
    return Path(__file__).resolve().parents[3]


def get_default_pet_install_root() -> Path:
    # Desktop pet runtime must reside strictly within the ReadMD program directory, never on C: drive.
    install_dir = get_app_install_dir()
    plugins_dir = install_dir / "plugins"
    plugins_dir.mkdir(parents=True, exist_ok=True)
    return plugins_dir


class HermesPetPluginInstaller:
    """Install a signed-by-manifest external plugin without touching the app tree."""

    MAX_ARCHIVE_BYTES = 350 * 1024 * 1024
    MAX_EXPANDED_BYTES = 750 * 1024 * 1024
    MAX_FILES = 3000
    SWAP_ATTEMPTS = 40
    SWAP_RETRY_DELAY = 0.75
    SWEEP_ATTEMPTS = 8
    SWEEP_RETRY_DELAY = 0.25

    def __init__(self, data_dir: Optional[str] = None):
        if data_dir is not None:
            self.root = Path(data_dir).resolve() / "pet"
        else:
            self.root = get_default_pet_install_root() / "pet"
        self.target = self.root / "hermes-adapter"

    def get_installed_manifest(self) -> Optional[Dict[str, Any]]:
        manifest_path = self.target / "readmd-pet-plugin.json"
        if not manifest_path.is_file():
            return None
        try:
            return json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError):
            return None

    def get_installed_manifest_hash(self) -> Optional[str]:
        manifest_path = self.target / "readmd-pet-plugin.json"
        if not manifest_path.is_file():
            return None
        try:
            return self._sha256(manifest_path)
        except OSError:
            return None

    def get_release_info(self) -> Dict[str, Any]:
        info_path = self.target / "pet-release-info.json"
        if not info_path.is_file():
            return {}
        try:
            val = json.loads(info_path.read_text(encoding="utf-8"))
            return val if isinstance(val, dict) else {}
        except (OSError, ValueError, TypeError):
            return {}

    def set_release_info(self, info: Dict[str, Any]) -> None:
        if not self.target.is_dir():
            return
        info_path = self.target / "pet-release-info.json"
        try:
            info_path.write_text(json.dumps(info, ensure_ascii=False, indent=2), encoding="utf-8")
        except OSError:
            pass

    def _remove_tree(self, path: Path) -> bool:
        if not path.exists():
            return True

        def _handle_remove_readonly(func, p, _exc):
            try:
                os.chmod(p, stat.S_IWRITE)
                func(p)
            except Exception:
                pass

        for remaining in range(self.SWEEP_ATTEMPTS - 1, -1, -1):
            try:
                import sys
                if sys.version_info >= (3, 12):
                    shutil.rmtree(path, onexc=lambda func, p, exc: _handle_remove_readonly(func, p, exc))
                else:
                    shutil.rmtree(path, onerror=_handle_remove_readonly)
            except Exception:
                pass
            if not path.exists():
                return True
            if remaining:
                kill_processes_by_target(path)
                time.sleep(self.SWEEP_RETRY_DELAY)
        return not path.exists()

    def uninstall(self) -> bool:
        """Removes installed hermes-adapter directory and clears staging & legacy C-drive remnants."""
        # 1. Force kill any running processes under root or target
        kill_processes_by_target(self.root)

        # 2. Sweep all stale staging directories
        self._sweep_stale_staging()

        # 3. Remove target
        removed = True
        if self.target.exists():
            removed = self._remove_tree(self.target)

        # 4. Clean legacy C: drive remnants unconditionally
        try:
            from .updater import clean_legacy_pet_installations
            clean_legacy_pet_installations(self.target)
        except Exception:
            pass

        # 5. If root directory (plugins/pet) is empty, clean it up
        try:
            if self.root.is_dir():
                children = [c for c in self.root.iterdir() if not c.name.startswith('.')]
                if not children:
                    shutil.rmtree(self.root, ignore_errors=True)
        except Exception:
            pass

        return removed

    def _sweep_stale_staging(self) -> None:
        """清掉此前失败安装留下的暂存目录，删不掉的如实记入日志。"""
        for stale in self.root.glob("readmd-pet-*"):
            if not self._remove_tree(stale):
                logging.warning("Could not clear stale pet staging dir %s", stale)

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

    def _replace_tree_in_place(self, staged: Path, target: Path) -> None:
        """Overwrite a directory that cannot be renamed with a verified tree.

        A pinned working directory (a lingering crash reporter or an open
        Explorer window) blocks renaming the target itself while every child
        path stays writable.  Each file lands through a temporary sibling so a
        crash mid-copy cannot leave a truncated runtime behind, and entries
        missing from the staged tree are removed so installs never accumulate
        stale files.
        """
        staged_names = set()
        for root, dirs, files in os.walk(staged):
            rel = os.path.relpath(root, staged)
            for name in dirs:
                staged_names.add(os.path.normpath(os.path.join(rel, name)))
            for name in files:
                staged_names.add(os.path.normpath(os.path.join(rel, name)))
            destination_root = target if rel == os.curdir else target / rel
            destination_root.mkdir(parents=True, exist_ok=True)
            for name in files:
                destination = destination_root / name
                temporary = destination.with_name(destination.name + ".readmd-new")
                shutil.copyfile(os.path.join(root, name), temporary)
                try:
                    os.replace(str(temporary), str(destination))
                except PermissionError:
                    # A scanner can hold a freshly touched runtime file open
                    # briefly.  Wait out that window once; a locked file whose
                    # bytes already equal the staged copy needs no replace.
                    time.sleep(self.SWAP_RETRY_DELAY)
                    try:
                        os.replace(str(temporary), str(destination))
                    except PermissionError:
                        if self._sha256(Path(destination)) == self._sha256(Path(temporary)):
                            temporary.unlink(missing_ok=True)
                            continue
                        raise
        for root, dirs, files in os.walk(target, topdown=False):
            rel = os.path.relpath(root, target)
            for name in dirs + files:
                if os.path.normpath(os.path.join(rel, name)) in staged_names:
                    continue
                path = os.path.join(root, name)
                if os.path.isdir(path):
                    shutil.rmtree(path, ignore_errors=True)
                else:
                    try:
                        os.unlink(path)
                    except OSError:
                        pass

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
                # behind; clear it before opening a fresh one.
                self._sweep_stale_staging()
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
                        try:
                            self._replace_with_retry(self.target, backup)
                        except PermissionError:
                            # Renaming the target itself failed while the
                            # staged tree is already fully verified, so swap
                            # its contents in place instead of directories.
                            self._replace_tree_in_place(staged, self.target)
                            return {"ok": True, "installed": True, "files": len(expected)}
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
            logging.exception('pet plugin install failed for %s', archive_path)
            return {"ok": False, "code": "pet_plugin_install_failed"}
