# -*- coding: utf-8 -*-
"""Stage and switch GitHub Release assets without publishing a partial set."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Dict, Iterable, List, Optional


CHECKSUM_NAME = "SHA256SUMS.txt"
STAGING_MARKER = "__release_sync__"


@dataclass(frozen=True)
class Asset:
    asset_id: int
    name: str
    size: int
    state: str
    url: str


def expected_assets(version):
    return {
        f"ReadMDSetup-v{version}.exe",
        f"ReadMD-portable-v{version}.exe",
        f"readmd-vscode-{version}.vsix",
        f"readmd-mcp-server-{version}.zip",
        f"ReadMD-macos-x64-v{version}.zip",
        f"ReadMD-macos-arm64-v{version}.zip",
        f"ReadMD-linux-x86_64-v{version}.AppImage",
        f"readmd_{version}_amd64.deb",
        f"ReadMD-linux-aarch64-v{version}.AppImage",
        f"readmd_{version}_arm64.deb",
        CHECKSUM_NAME,
    }


def payload_assets(version):
    return expected_assets(version) - {CHECKSUM_NAME}


def prepare_assets(directory, version):
    root = Path(directory)
    files = {path.name: path for path in root.iterdir() if path.is_file()}
    required = payload_assets(version)
    if set(files) != required:
        raise RuntimeError(
            f"asset mismatch: missing={sorted(required-set(files))}, "
            f"extra={sorted(set(files)-required)}"
        )

    lines = []
    for name in sorted(required):
        path = files[name]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f"{digest}  {name}")
    checksum = root / CHECKSUM_NAME
    checksum.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return checksum


def clean_commit(commit):
    clean = re.sub(r"[^A-Za-z0-9]", "", commit)[:40]
    if not clean:
        raise ValueError("commit is required")
    return clean


def staging_prefix(commit):
    return f"{STAGING_MARKER}{clean_commit(commit)}_"


def asset_from_json(payload):
    return Asset(
        asset_id=int(payload["id"]),
        name=payload["name"],
        size=int(payload.get("size", 0)),
        state=payload.get("state", ""),
        url=payload["url"],
    )


def fetch_release(runner, tag, repo) -> Optional[dict]:
    completed = runner(["api", f"repos/{repo}/releases/tags/{tag}"])
    if completed.returncode != 0 and "HTTP 404" in completed.stderr:
        return None
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip() or "failed to fetch release")
    return json.loads(completed.stdout)


def delete_asset(runner, asset):
    runner(["api", "--method", "DELETE", asset.url])


def rename_asset(runner, asset, name):
    payload = json.dumps({"name": name})
    runner(["api", "--method", "PATCH", asset.url, "--input", "-"], input=payload)


def upload_staged_assets(runner, tag, directory, version, commit, repo):
    prefix = staging_prefix(commit)
    files = {path.name: path for path in Path(directory).iterdir() if path.is_file()}
    required = expected_assets(version)
    if set(files) != required:
        raise RuntimeError("release assets changed during synchronization")

    staging_dir = Path(directory) / f".staging-{clean_commit(commit)}"
    shutil.rmtree(staging_dir, ignore_errors=True)
    staging_dir.mkdir()
    staged_paths = []
    for name, source in sorted(files.items()):
        staged_path = staging_dir / f"{prefix}{name}"
        shutil.copy2(source, staged_path)
        staged_paths.append(staged_path)

    for path in staged_paths:
        runner(["release", "upload", tag, str(path), "--clobber"])

    release = fetch_release(runner, tag, repo)
    assets = {asset.name: asset for asset in map(asset_from_json, release.get("assets", []))}
    for name in required:
        staged_name = prefix + name
        asset = assets.get(staged_name)
        if not asset or asset.state != "uploaded" or asset.size != files[name].stat().st_size:
            raise RuntimeError(f"staged asset was not accepted: {staged_name}")
    staged = {name: assets[prefix + name] for name in required}
    return prefix, staged


def swap_staged_assets(runner, tag, version, commit, repo):
    prefix = staging_prefix(commit)
    release = fetch_release(runner, tag, repo)
    assets = {asset.name: asset for asset in map(asset_from_json, release.get("assets", []))}
    expected = expected_assets(version)

    # Uploads are complete before any public name changes.  Checksum goes last.
    ordered = sorted(expected - {CHECKSUM_NAME}) + [CHECKSUM_NAME]
    for name in ordered:
        staged_name = prefix + name
        staged = assets.get(staged_name)
        if not staged:
            raise RuntimeError(f"missing staged asset: {staged_name}")
        current = assets.get(name)
        if current:
            delete_asset(runner, current)
        rename_asset(runner, staged, name)
        assets.pop(staged_name, None)
        assets[name] = Asset(staged.asset_id, name, staged.size, staged.state, staged.url)

    release = fetch_release(runner, tag, repo)
    final = {asset["name"] for asset in release.get("assets", [])}
    if final != expected:
        raise RuntimeError(f"final asset mismatch: {sorted(final ^ expected)}")

    # Remove obsolete finals and staging left by cancelled older runs.
    release = fetch_release(runner, tag, repo)
    for payload in release.get("assets", []):
        if payload["name"] not in expected:
            delete_asset(runner, asset_from_json(payload))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assets-dir", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--repo", default=os.environ.get("GH_REPO"))
    args = parser.parse_args()

    if not os.environ.get("GH_TOKEN"):
        raise SystemExit("GH_TOKEN is required")
    if not args.repo:
        raise SystemExit("GH_REPO or --repo is required")
    version = Path("VERSION").read_text(encoding="utf-8").strip()

    def gh_runner(command, **kwargs):
        completed = subprocess.run(
            ["gh", *command],
            text=True,
            capture_output=True,
            check=False,
            **kwargs,
        )
        if completed.returncode != 0:
            error = completed.stderr.strip() or f"gh command failed: {command[0]}"
            completed.stderr = error
        return completed

    release = fetch_release(gh_runner, args.tag, args.repo)
    if release is None:
        print(f"release {args.tag} does not exist yet; skipping asset sync")
        return

    prepare_assets(args.assets_dir, version)
    prefix, staged = upload_staged_assets(
        gh_runner,
        args.tag,
        args.assets_dir,
        version,
        args.commit,
        args.repo,
    )
    swap_staged_assets(
        gh_runner,
        args.tag,
        version,
        args.commit,
        args.repo,
    )
    print(f"release assets synced: {args.tag} / {args.commit} ({len(staged)} staged via {prefix})")


if __name__ == "__main__":
    main()
