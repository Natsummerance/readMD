# -*- coding: utf-8 -*-
"""Production desktop-pet runtime selection and Rust package installation.

The public boundary is :class:`PetRuntimeOrchestrator`. It keeps the legacy
Electron launcher available as a compatibility backend while making the native
``readmd-pet-rust`` host the first choice for independent desktop mode.
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
import shutil
import stat
import subprocess
import tempfile
import time
import zipfile
from pathlib import Path
from typing import Any, Dict, Iterable, Optional, Protocol

from .hermes_adapter import HermesPetBridge, HermesPetLauncher, kill_processes_by_target


class PetRuntimeBackend(Protocol):
    def status(self) -> Dict[str, Any]: ...
    def start(self) -> Dict[str, Any]: ...
    def stop(self) -> None: ...


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


class RustPetRuntimeInstaller:
    """Install a verified Rust runtime below ``<ReadMD>/plugins/pet``."""

    MAX_ARCHIVE_BYTES = 750 * 1024 * 1024
    MAX_EXPANDED_BYTES = 1_500 * 1024 * 1024
    MAX_FILES = 10_000
    SWAP_ATTEMPTS = 40
    SWAP_DELAY = 0.25
    ALLOWED_METADATA = frozenset({"runtime-manifest.json", "runtime-release-info.json"})

    def __init__(self, data_dir: Optional[str] = None):
        # ``get_default_pet_install_root`` already points at ``<ReadMD>/plugins``
        # while unit callers commonly pass a temporary data directory. Accept
        # both forms without ever creating the accidental ``plugins/plugins``
        # tree that made the packaged runtime invisible to the app.
        if data_dir:
            root = Path(data_dir).resolve()
            self.root = root if root.name.lower() == "pet" else root / "pet"
        else:
            self.root = Path(__file__).resolve().parents[3] / "plugins" / "pet"
        self.target = self.root / "readmd-rust-host"

    @property
    def manifest_path(self) -> Path:
        return self.target / "runtime-manifest.json"

    @property
    def binary_path(self) -> Path:
        return self.target / ("readmd-pet-rust.exe" if os.name == "nt" else "readmd-pet-rust")

    @property
    def renderer_path(self) -> Path:
        return self.target / "renderer"

    def get_installed_manifest(self) -> Optional[Dict[str, Any]]:
        try:
            value = json.loads(self.manifest_path.read_text(encoding="utf-8"))
            return value if isinstance(value, dict) else None
        except (OSError, ValueError, TypeError):
            return None

    def get_installed_manifest_hash(self) -> Optional[str]:
        try:
            return _sha256(self.manifest_path)
        except OSError:
            return None

    def get_release_info(self) -> Dict[str, Any]:
        path = self.target / "runtime-release-info.json"
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
            return value if isinstance(value, dict) else {}
        except (OSError, ValueError, TypeError):
            return {}

    def set_release_info(self, info: Dict[str, Any]) -> None:
        if self.target.is_dir():
            try:
                (self.target / "runtime-release-info.json").write_text(json.dumps(info, ensure_ascii=False, indent=2), encoding="utf-8")
            except OSError:
                logging.debug("could not write Rust pet release info", exc_info=True)

    @staticmethod
    def _safe_name(name: str) -> bool:
        path = Path(name)
        return bool(name and not path.is_absolute() and ".." not in path.parts and "\\" not in name)

    @staticmethod
    def _platform() -> str:
        if os.name == "nt":
            return "windows"
        if sys_platform := os.sys.platform:
            if sys_platform == "darwin":
                return "macos"
            if sys_platform.startswith("linux"):
                return "linux"
        return "unknown"

    @staticmethod
    def _arch() -> str:
        machine = os.environ.get("PROCESSOR_ARCHITECTURE") or (
            os.uname().machine if hasattr(os, "uname") else "x86_64"
        )
        machine = str(machine).lower()
        if machine in {"amd64", "x86_64", "x64"}:
            return "x86_64"
        if machine in {"arm64", "aarch64"}:
            return "aarch64"
        return machine

    @classmethod
    def _manifest_expected(cls, manifest: Any) -> tuple[Optional[Dict[str, Dict[str, Any]]], Optional[str]]:
        if not isinstance(manifest, dict) or manifest.get("runtime") != "readmd-pet-rust":
            return None, "invalid_rust_runtime_manifest"
        try:
            protocol = int(manifest.get("protocol_version", manifest.get("protocol", -1)))
        except (TypeError, ValueError, OverflowError):
            return None, "invalid_rust_runtime_manifest"
        if protocol != 1:
            return None, "invalid_rust_runtime_manifest"
        platform = str(manifest.get("platform") or "")
        if platform not in {cls._platform(), "any"}:
            return None, "rust_runtime_platform_mismatch"
        arch = str(manifest.get("arch") or "")
        if arch not in {cls._arch(), "any"}:
            return None, "rust_runtime_arch_mismatch"
        artifacts = manifest.get("artifacts")
        if not isinstance(artifacts, list) or not artifacts:
            return None, "invalid_rust_runtime_manifest"
        expected: Dict[str, Dict[str, Any]] = {}
        for item in artifacts:
            if not isinstance(item, dict):
                return None, "invalid_rust_runtime_manifest"
            path = str(item.get("path") or "")
            digest = str(item.get("sha256") or "").lower()
            try:
                size = int(item.get("size"))
            except (TypeError, ValueError):
                return None, "invalid_rust_runtime_manifest"
            if not cls._safe_name(path) or path in expected or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest) or size < 0:
                return None, "invalid_rust_runtime_manifest"
            expected[path] = {"sha256": digest, "size": size, "role": str(item.get("role") or "asset")}
        executable = [p for p, item in expected.items() if item["role"] == "executable"]
        if not executable or not any(p.replace("\\", "/").endswith("readmd-pet-rust.exe") or p.replace("\\", "/").endswith("readmd-pet-rust") for p in executable):
            return None, "rust_runtime_executable_missing"
        if "renderer/index.html" not in expected:
            return None, "rust_runtime_renderer_missing"
        return expected, None

    @classmethod
    def _verify_tree(cls, source: Path, manifest: Any) -> tuple[Optional[Dict[str, Dict[str, Any]]], Optional[str]]:
        expected, error = cls._manifest_expected(manifest)
        if error:
            return None, error
        assert expected is not None
        actual: set[str] = set()
        total = 0
        for root, dirs, files in os.walk(source, followlinks=False):
            root_path = Path(root)
            for name in dirs:
                if (root_path / name).is_symlink():
                    return None, "unsafe_rust_runtime_path"
            for name in files:
                path = root_path / name
                if path.is_symlink():
                    return None, "unsafe_rust_runtime_path"
                relative = path.relative_to(source).as_posix()
                if not cls._safe_name(relative):
                    return None, "unsafe_rust_runtime_path"
                actual.add(relative)
                total += path.stat().st_size
        if len(actual) > cls.MAX_FILES or total > cls.MAX_EXPANDED_BYTES:
            return None, "rust_runtime_too_large"
        if not set(expected).issubset(actual):
            return None, "rust_runtime_file_missing"
        mutable_prefixes = tuple(
            f"{relative}.WebView2/"
            for relative, item in expected.items()
            if item.get("role") == "executable"
        )
        extras = {
            relative
            for relative in actual - set(expected) - cls.ALLOWED_METADATA
            if not any(relative.startswith(prefix) for prefix in mutable_prefixes)
        }
        # Every shipped file is represented by the manifest. This catches a
        # corrupt or tampered executable before it can ever be spawned.
        if extras:
            return None, "rust_runtime_unlisted_file"
        for relative, item in expected.items():
            path = source / relative
            if path.stat().st_size != item["size"] or _sha256(path) != item["sha256"]:
                return None, "rust_runtime_hash_mismatch"
        return expected, None

    def _publish(self, staged: Path) -> Dict[str, Any]:
        self.root.mkdir(parents=True, exist_ok=True)
        backup = self.root / "readmd-rust-host.previous"
        if backup.exists():
            shutil.rmtree(backup, ignore_errors=True)
        if self.target.exists():
            for _ in range(self.SWAP_ATTEMPTS):
                try:
                    os.replace(self.target, backup)
                    break
                except PermissionError:
                    time.sleep(self.SWAP_DELAY)
            else:
                raise PermissionError("rust_runtime_target_locked")
        try:
            for _ in range(self.SWAP_ATTEMPTS):
                try:
                    os.replace(staged, self.target)
                    break
                except PermissionError:
                    time.sleep(self.SWAP_DELAY)
            else:
                raise PermissionError("rust_runtime_stage_locked")
        except Exception:
            if backup.exists() and not self.target.exists():
                os.replace(backup, self.target)
            raise
        if backup.exists():
            shutil.rmtree(backup, ignore_errors=True)
        return {"ok": True, "installed": True, "install_path": str(self.target)}

    def install_directory(self, directory_path: str, *, confirm: bool = False) -> Dict[str, Any]:
        if not confirm:
            return {"ok": False, "code": "pet_install_confirmation_required"}
        raw_source = Path(directory_path)
        if raw_source.is_symlink():
            return {"ok": False, "code": "unsafe_rust_runtime_path"}
        source = raw_source.resolve()
        if not source.is_dir():
            return {"ok": False, "code": "invalid_rust_runtime_directory"}
        try:
            manifest_path = source / "runtime-manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            expected, error = self._verify_tree(source, manifest)
            if error:
                return {"ok": False, "code": error}
            assert expected is not None
            self.root.mkdir(parents=True, exist_ok=True)
            with tempfile.TemporaryDirectory(prefix="readmd-rust-", dir=str(self.root)) as temp:
                staged = Path(temp) / "host"
                staged.mkdir()
                for relative in expected:
                    destination = staged / relative
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source / relative, destination)
                shutil.copyfile(manifest_path, staged / "runtime-manifest.json")
                return self._publish(staged)
        except (OSError, ValueError, TypeError, KeyError):
            logging.exception("Rust pet runtime directory install failed")
            return {"ok": False, "code": "rust_runtime_install_failed"}

    def install_archive(self, archive_path: str, *, confirm: bool = False) -> Dict[str, Any]:
        if not confirm:
            return {"ok": False, "code": "pet_install_confirmation_required"}
        archive = Path(archive_path).resolve()
        if not archive.is_file() or archive.suffix.lower() != ".zip":
            return {"ok": False, "code": "invalid_rust_runtime_archive"}
        try:
            if archive.stat().st_size > self.MAX_ARCHIVE_BYTES:
                return {"ok": False, "code": "rust_runtime_archive_too_large"}
            with zipfile.ZipFile(archive) as bundle:
                entries = [item for item in bundle.infolist() if not item.is_dir()]
                if not entries or len(entries) > self.MAX_FILES:
                    return {"ok": False, "code": "invalid_rust_runtime_contents"}
                if any(not self._safe_name(item.filename) or ((item.external_attr >> 16) & 0o170000) == 0o120000 for item in entries):
                    return {"ok": False, "code": "unsafe_rust_runtime_path"}
                manifest_item = next((item for item in entries if item.filename == "runtime-manifest.json"), None)
                if manifest_item is None:
                    return {"ok": False, "code": "rust_runtime_manifest_missing"}
                # Preserve the manifest bytes exactly as shipped.  The updater
                # fingerprints the archive's manifest, so re-serializing JSON
                # here would make an installed package look stale forever.
                manifest_bytes = bundle.read(manifest_item)
                manifest = json.loads(manifest_bytes.decode("utf-8"))
                expected, error = self._manifest_expected(manifest)
                if error:
                    return {"ok": False, "code": error}
                assert expected is not None
                names = {item.filename for item in entries}
                if len(names) != len(entries) or not set(expected).issubset(names) or names - set(expected) - self.ALLOWED_METADATA:
                    return {"ok": False, "code": "rust_runtime_unlisted_file"}
                if sum(item.file_size for item in entries) > self.MAX_EXPANDED_BYTES:
                    return {"ok": False, "code": "rust_runtime_too_large"}
                self.root.mkdir(parents=True, exist_ok=True)
                with tempfile.TemporaryDirectory(prefix="readmd-rust-", dir=str(self.root)) as temp:
                    staged = Path(temp) / "host"
                    staged.mkdir()
                    for relative, item in expected.items():
                        destination = staged / relative
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        with bundle.open(relative) as source, destination.open("wb") as target:
                            shutil.copyfileobj(source, target, length=1024 * 1024)
                        if destination.stat().st_size != item["size"] or _sha256(destination) != item["sha256"]:
                            return {"ok": False, "code": "rust_runtime_hash_mismatch"}
                    (staged / "runtime-manifest.json").write_bytes(manifest_bytes)
                    return self._publish(staged)
        except (OSError, ValueError, TypeError, KeyError, zipfile.BadZipFile, UnicodeError):
            logging.exception("Rust pet runtime archive install failed")
            return {"ok": False, "code": "rust_runtime_install_failed"}

    def available(self) -> bool:
        manifest = self.get_installed_manifest()
        if not manifest or not self.binary_path.is_file() or not (self.renderer_path / "index.html").is_file():
            return False
        _, error = self._verify_tree(self.target, manifest)
        return error is None

    def uninstall(self) -> bool:
        kill_processes_by_target(self.target)
        try:
            if self.target.exists():
                shutil.rmtree(self.target)
            return not self.target.exists()
        except OSError:
            return False


class RustPetRuntime:
    """Starts one exact Rust executable and owns the parent's EOF pipe."""

    def __init__(self, app_dir: str, bridge: HermesPetBridge, runtime_dir: str, renderer: str = "hermes-sprite"):
        self._app_dir = Path(app_dir).resolve()
        self._bridge = bridge
        self.runtime_dir = Path(runtime_dir).resolve()
        self.renderer = renderer
        self._process: Optional[subprocess.Popen] = None
        self._parent_read: Optional[int] = None
        self._parent_write: Optional[int] = None
        self._lock = __import__("threading").RLock()
        self._diagnostic = ""

    @property
    def binary_path(self) -> Path:
        return self.runtime_dir / ("readmd-pet-rust.exe" if os.name == "nt" else "readmd-pet-rust")

    @property
    def manifest_path(self) -> Path:
        return self.runtime_dir / "runtime-manifest.json"

    @property
    def renderer_root(self) -> Path:
        return self.runtime_dir / "renderer"

    def set_renderer(self, renderer: str) -> None:
        if renderer in {"hermes-sprite", "live2d"}:
            self.renderer = renderer

    def _health(self) -> Dict[str, Any]:
        path = Path(str(self._bridge.state_path) + ".rust.health.json")
        try:
            if path.stat().st_size > 32 * 1024:
                return {}
            value = json.loads(path.read_text(encoding="utf-8"))
            return value if isinstance(value, dict) and value.get("engine") == "rust" else {}
        except (OSError, ValueError, TypeError):
            return {}

    def _verified_install(self) -> bool:
        """Verify the installed tree before exposing it as launchable."""
        try:
            manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        except (OSError, ValueError, TypeError):
            return False
        if not isinstance(manifest, dict):
            return False
        if not manifest or not self.binary_path.is_file() or not (self.renderer_root / "index.html").is_file():
            return False
        try:
            _, error = RustPetRuntimeInstaller._verify_tree(self.runtime_dir, manifest)
        except (OSError, ValueError, TypeError, KeyError):
            return False
        return error is None

    def _is_running_exact(self) -> bool:
        if self._process is not None and self._process.poll() is None:
            return True
        if os.name == "nt" and self.binary_path.is_file():
            try:
                import psutil
                wanted = os.path.normcase(os.path.realpath(str(self.binary_path)))
                return any(os.path.normcase(os.path.realpath(proc.info.get("exe") or "")) == wanted for proc in psutil.process_iter(["exe"]))
            except Exception:
                return False
        return False

    def status(self) -> Dict[str, Any]:
        health = self._health()
        running = self._is_running_exact()
        return {"available": self._verified_install(), "running": running, "health": health if running else {}, "runtime": "rust", "executable": str(self.binary_path), "diagnostic": self._diagnostic}

    @staticmethod
    def _prepare_parent_pipe() -> tuple[int, int, Optional[int]]:
        read_fd, write_fd = os.pipe()
        try:
            os.set_inheritable(read_fd, True)
        except OSError:
            pass
        handle: Optional[int] = read_fd
        if os.name == "nt":
            import msvcrt
            handle = int(msvcrt.get_osfhandle(read_fd))
            try:
                os.set_handle_inheritable(handle, True)
            except OSError:
                pass
        return read_fd, write_fd, handle

    def _wait_health(self, timeout: float = 15.0) -> bool:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self._process is not None and self._process.poll() is not None:
                self._diagnostic = f"rust_exit_{self._process.returncode}"
                return False
            health = self._health()
            owned_pid = getattr(self._process, "pid", None)
            # A previous Rust process can leave a healthy health file behind
            # for a few milliseconds while Windows releases WebView2.  Only
            # accept a report written by the exact child we just spawned.
            if (
                health
                and health.get("state") == "ready"
                and (owned_pid is None or health.get("pid") == owned_pid)
            ):
                return True
            if (
                health
                and (owned_pid is None or health.get("pid") in {None, owned_pid})
                and health.get("state") in {"degraded", "stopped"}
            ):
                self._diagnostic = "rust_health_%s_%s" % (
                    health.get("state"), health.get("code") or "unknown"
                )
                return False
            time.sleep(0.05)
        self._diagnostic = "rust_health_timeout"
        return False

    def start(self) -> Dict[str, Any]:
        with self._lock:
            current = self.status()
            if not current["available"]:
                return {"ok": False, "code": "rust_runtime_not_installed", "runtime": current}
            if current["running"]:
                return {"ok": True, "runtime": current}
            kill_processes_by_target(self.runtime_dir)
            self.runtime_dir.mkdir(parents=True, exist_ok=True)
            try:
                Path(str(self._bridge.state_path) + ".rust.health.json").unlink()
            except FileNotFoundError:
                pass
            except OSError:
                # The child still owns the authoritative write path; a stale
                # report cannot be accepted because _wait_health checks pid.
                logging.debug("could not clear stale Rust pet health file", exc_info=True)
            read_fd, write_fd, inherited = self._prepare_parent_pipe()
            env = os.environ.copy()
            env.update({"READMD_PET_BRIDGE_FILE": str(self._bridge.state_path), "READMD_PARENT_PID": str(os.getpid()), "READMD_PET_RENDERER": self.renderer, "READMD_PET_RUNTIME_DIR": str(self.runtime_dir), "READMD_PET_RENDERER_ROOT": str(self.renderer_root), "READMD_DATA_DIR": str(self._bridge._root.parent)})
            # WebView2 creates a mutable user-data directory on first launch.
            # Keep it beside the bridge state rather than inside the verified
            # runtime package, otherwise its cache files look like unlisted
            # tampering on the next availability check.
            env["WEBVIEW2_USER_DATA_FOLDER"] = str(self._bridge._root / "webview2")
            if inherited is not None:
                env["READMD_PARENT_PIPE_HANDLE"] = str(inherited)
            startupinfo = None
            creationflags = 0
            kwargs: Dict[str, Any] = {"cwd": str(self.runtime_dir), "env": env, "close_fds": True}
            if os.name == "nt":
                if hasattr(subprocess, "STARTUPINFO"):
                    startupinfo = subprocess.STARTUPINFO()
                    startupinfo.dwFlags |= getattr(subprocess, "STARTF_USESHOWWINDOW", 1)
                    startupinfo.wShowWindow = 0
                    if inherited is not None:
                        try:
                            startupinfo.lpAttributeList = {"handle_list": [inherited]}
                        except Exception:
                            pass
                creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0x08000000)
                kwargs.update(startupinfo=startupinfo, creationflags=creationflags)
            else:
                kwargs["pass_fds"] = (read_fd,)
            try:
                self._process = subprocess.Popen([str(self.binary_path)], **kwargs)
            except OSError as error:
                self._diagnostic = f"rust_spawn_failed:{error.__class__.__name__}"
                try:
                    os.close(read_fd)
                    os.close(write_fd)
                except OSError:
                    pass
                return {"ok": False, "code": "rust_runtime_start_failed", "runtime": self.status()}
            try:
                os.close(read_fd)
            except OSError:
                pass
            self._parent_read = None
            self._parent_write = write_fd
            if not self._wait_health(float(os.environ.get("READMD_PET_HEALTH_TIMEOUT", "15"))):
                self.stop()
                return {"ok": False, "code": "rust_runtime_health_failed", "runtime": self.status(), "diagnostic": self._diagnostic}
            return {"ok": True, "runtime": self.status()}

    def stop(self) -> None:
        with self._lock:
            if self._parent_write is not None:
                try:
                    os.close(self._parent_write)
                except OSError:
                    pass
                self._parent_write = None
            process = self._process
            self._process = None
            if process is None:
                # A previous ReadMD instance may have exited before its
                # Python object observed the child. Keep stop idempotent while
                # still removing that exact managed runtime process.
                kill_processes_by_target(self.runtime_dir)
                return
            pid = getattr(process, "pid", None)
            if pid and os.name == "nt":
                try:
                    subprocess.run(["taskkill", "/F", "/T", "/PID", str(pid)], capture_output=True, timeout=3, creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0x08000000))
                except Exception:
                    pass
            try:
                process.terminate()
                process.wait(timeout=2)
            except Exception:
                try:
                    process.kill()
                except Exception:
                    pass


class ElectronPetRuntime(HermesPetLauncher):
    """Named compatibility backend for the legacy Electron sidecar."""


class PetRuntimeOrchestrator:
    """Select Rust first, then Electron only for an explicit fallback case."""

    def __init__(self, app_dir: str, bridge: HermesPetBridge, rust_dir: str, electron_dir: str, mode: Optional[str] = None):
        self.mode = (mode or os.environ.get("READMD_PET_RUNTIME_MODE", "auto")).strip().lower()
        if self.mode not in {"auto", "rust", "electron", "in_app"}:
            self.mode = "auto"
        self.rust = RustPetRuntime(app_dir, bridge, rust_dir)
        self.electron = ElectronPetRuntime(app_dir, bridge, adapter_dir=electron_dir)
        self.active_backend = ""
        self.diagnostic = ""

    def set_renderer(self, renderer: str) -> None:
        self.rust.set_renderer(renderer)

    def status(self) -> Dict[str, Any]:
        rust = self.rust.status()
        electron = self.electron.status()
        active = self.rust if self.active_backend == "rust" else self.electron if self.active_backend == "electron" else None
        running = bool(active and active.status().get("running"))
        if self.mode == "in_app":
            available = True
        elif self.mode == "rust":
            available = rust.get("available", False)
        elif self.mode == "electron":
            available = electron.get("available", False)
        else:
            available = bool(rust.get("available") or electron.get("available"))
        health = active.status().get("health", {}) if active and running else {}
        return {"available": available, "running": running, "backend": self.active_backend or None, "runtime": "rust" if self.active_backend == "rust" else "electron" if self.active_backend == "electron" else None, "health": health, "rust": rust, "electron": electron, "diagnostic": self.diagnostic}

    def start(self) -> Dict[str, Any]:
        if self.mode == "in_app":
            self.diagnostic = "in_app_mode"
            return {"ok": True, "runtime": self.status()}
        candidates = [("rust", self.rust), ("electron", self.electron)] if self.mode == "auto" else [(self.mode, self.rust if self.mode == "rust" else self.electron)]
        failures = []
        for name, backend in candidates:
            if not backend.status().get("available"):
                failures.append(f"{name}:unavailable")
                continue
            result = backend.start()
            if result.get("ok"):
                self.active_backend = name
                self.diagnostic = ""
                return {"ok": True, "runtime": self.status()}
            failures.append(f"{name}:{result.get('code', 'start_failed')}")
            backend.stop()
            if self.mode != "auto":
                break
        self.diagnostic = ";".join(failures) or "no_runtime_available"
        # Preserve the historical API error for callers that only know about
        # the Electron adapter; the structured diagnostic still identifies all
        # Rust/Electron candidates and is returned to new callers.
        code = "hermes_adapter_not_installed" if failures and all(item.endswith(":unavailable") for item in failures) else "pet_runtime_start_failed"
        return {"ok": False, "code": code, "diagnostic": self.diagnostic, "runtime": self.status()}

    def stop(self) -> None:
        self.rust.stop()
        self.electron.stop()
        self.active_backend = ""


__all__ = ["PetRuntimeBackend", "RustPetRuntimeInstaller", "RustPetRuntime", "ElectronPetRuntime", "PetRuntimeOrchestrator"]
