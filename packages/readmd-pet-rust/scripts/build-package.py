#!/usr/bin/env python3
"""Build the native ReadMD pet runtime package.

The package is intentionally independent from the Electron adapter. It copies
the already-built production renderer and model assets, records every byte in
``runtime-manifest.json``, and emits a deterministic ZIP consumed by the
managed Rust installer.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def package_version(crate: Path) -> str:
    match = re.search(r'^version\s*=\s*"([^"]+)"', (crate / "Cargo.toml").read_text(encoding="utf-8"), re.MULTILINE)
    return match.group(1) if match else "0.0.0"


def copy_tree(source: Path, destination: Path) -> None:
    if not source.is_dir():
        raise SystemExit(f"required runtime asset directory is missing: {source}")
    shutil.copytree(source, destination, dirs_exist_ok=True)


def target_for(platform: str, arch: str) -> str | None:
    if platform == "windows" and arch == "x86_64":
        return "x86_64-pc-windows-msvc"
    if platform == "windows" and arch == "aarch64":
        return "aarch64-pc-windows-msvc"
    if platform == "macos" and arch == "x86_64":
        return "x86_64-apple-darwin"
    if platform == "macos" and arch == "aarch64":
        return "aarch64-apple-darwin"
    if platform == "linux" and arch == "x86_64":
        return "x86_64-unknown-linux-gnu"
    if platform == "linux" and arch == "aarch64":
        return "aarch64-unknown-linux-gnu"
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--platform", choices=("windows", "macos", "linux"), default=None)
    parser.add_argument("--arch", choices=("x86_64", "aarch64"), default=None)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args()

    repo = Path(__file__).resolve().parents[3]
    crate = repo / "packages" / "readmd-pet-rust"
    renderer = repo / "packages" / "readmd-hermes-pet-adapter" / "dist" / "renderer"
    adapter_dist = repo / "packages" / "readmd-hermes-pet-adapter" / "dist"
    platform = args.platform or ("windows" if os.name == "nt" else "macos" if sys.platform == "darwin" else "linux")
    host_machine = (os.environ.get("PROCESSOR_ARCHITECTURE", "") if os.name == "nt" else os.uname().machine).lower()
    machine = (args.arch or host_machine).lower()
    arch = "aarch64" if machine in {"arm64", "aarch64"} else "x86_64"
    target = target_for(platform, arch)
    if not target:
        raise SystemExit(f"unsupported runtime target: {platform}/{arch}")

    cargo_env = os.environ.copy()
    cargo_bin = Path(cargo_env.get("USERPROFILE", "")) / ".cargo" / "bin" / "cargo.exe"
    cargo = str(cargo_bin) if cargo_bin.is_file() else "cargo"
    if not args.skip_build:
        # Always name the target explicitly.  This keeps a package build
        # deterministic on native ARM runners and on x64 cross-build runners;
        # relying on Cargo's host default can silently emit the wrong binary.
        command = [
            cargo,
            "build",
            "--release",
            "--manifest-path",
            str(crate / "Cargo.toml"),
            "--target",
            target,
        ]
        subprocess.run(command, cwd=repo, check=True)

    executable_name = "readmd-pet-rust.exe" if platform == "windows" else "readmd-pet-rust"
    target_release = crate / "target" / target / "release" / executable_name
    native_release = crate / "target" / "release" / executable_name
    # A previous cross-target check may have created an otherwise empty target
    # directory. Select the release directory that actually contains the
    # executable instead of mistaking that cache directory for a build.
    native_platform = "windows" if os.name == "nt" else "macos" if sys.platform == "darwin" else "linux"
    native_arch = "aarch64" if host_machine in {"arm64", "aarch64"} else "x86_64"
    # A native build may be stored in Cargo's host release directory when the
    # target triple is implicit.  Only use that fallback for a package that
    # matches the current host; a missing cross-target binary must fail rather
    # than silently shipping an x64 executable under an ARM manifest.
    executable = target_release
    if not executable.is_file() and platform == native_platform and arch == native_arch:
        executable = native_release
    if not executable.is_file():
        raise SystemExit(f"built Rust executable is missing: {executable}")
    if not renderer.is_dir() or not (renderer / "index.html").is_file():
        raise SystemExit(f"renderer bundle is missing: {renderer}")

    output_dir = args.output or (crate / "dist")
    stage = output_dir / f"ReadMD-Pet-Rust-{platform}-{arch}"
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    shutil.copy2(executable, stage / executable.name)
    copy_tree(renderer, stage / "renderer")
    for name in ("assets", "models", "vendor"):
        source = adapter_dist / name
        # These directories are part of the production runtime contract. A
        # package without the sprite/model/Cubism payload can start a window
        # but can never become renderer-ready, so fail the build instead of
        # emitting a deceptively usable-looking archive.
        copy_tree(source, stage / name)
    if platform == "linux":
        companion = crate / "gnome-companion"
        if companion.is_dir():
            copy_tree(companion, stage / "gnome-companion")

    artifacts = []
    for path in sorted(item for item in stage.rglob("*") if item.is_file()):
        relative = path.relative_to(stage).as_posix()
        role = "executable" if relative == executable.name else "renderer" if relative.startswith("renderer/") else "model" if relative.startswith("models/") else "companion" if relative.startswith("gnome-companion/") else "asset"
        artifacts.append({"path": relative, "sha256": sha256(path), "size": path.stat().st_size, "role": role})
    manifest = {
        "runtime": "readmd-pet-rust",
        "version": package_version(crate),
        # Keep both spellings during the migration; protocol_version is the
        # public manifest field while protocol preserves older local bundles.
        "protocol_version": 1,
        "protocol": 1,
        "platform": platform,
        "arch": arch,
        "artifacts": artifacts,
    }
    (stage / "runtime-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    archive_name = (
        "ReadMD-Pet-Rust.zip"
        if platform == "windows" and arch == "x86_64"
        else f"ReadMD-Pet-Rust-{platform}-{arch}.zip"
    )
    archive = output_dir / archive_name
    output_dir.mkdir(parents=True, exist_ok=True)
    if archive.exists():
        archive.unlink()
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
        for path in sorted(item for item in stage.rglob("*") if item.is_file()):
            bundle.write(path, path.relative_to(stage).as_posix())
    print(json.dumps({"archive": str(archive), "stage": str(stage), "platform": platform, "arch": arch, "files": len(artifacts)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
