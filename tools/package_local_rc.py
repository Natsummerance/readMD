# -*- coding: utf-8 -*-
"""Assemble a non-publishing Windows RC outside the repository.

The script only copies already-built files and writes review metadata.  It
never creates a tag, uploads an artifact, or changes the working tree.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read_version(root: Path) -> str:
    """Read the candidate version from the repository source of truth."""
    value = (root / "VERSION").read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", value):
        raise SystemExit(f"invalid VERSION value: {value!r}")
    return value


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def copy_item(src: Path, dst: Path) -> None:
    if src.is_dir():
        shutil.copytree(src, dst, dirs_exist_ok=True)
    else:
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)


def first_existing(*paths: Path) -> Path | None:
    return next((path for path in paths if path.exists()), None)


def newest_source_mtime(root: Path) -> float:
    """Return the newest mtime of files that can affect a packaged payload."""
    roots = (root / "readmd.py", root / "src", root / "assets", root / "config", root / "VERSION")
    files = []
    for item in roots:
        if item.is_file():
            files.append(item)
        elif item.is_dir():
            files.extend(path for path in item.rglob("*") if path.is_file())
    return max((path.stat().st_mtime for path in files), default=0.0)


def assert_fresh(path: Path, source_mtime: float, label: str) -> None:
    if path.stat().st_mtime + 1e-6 < source_mtime:
        raise SystemExit(
            f"stale local RC input: {label} is older than source files; rebuild before packaging ({path})"
        )


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True, encoding="utf-8").strip()


def build_sbom(root: Path, version: str) -> dict:
    """Describe declared runtime/build dependencies, never the host's global env."""
    components = []
    seen = set()
    for req_file in sorted((root / "config").glob("requirements*.txt")):
        for raw in req_file.read_text(encoding="utf-8", errors="replace").splitlines():
            line = raw.split("#", 1)[0].strip()
            if not line or line.startswith("-") or line.startswith("python"):
                continue
            # Keep environment markers and ranges as evidence rather than
            # pretending a host-installed version is the packaged version.
            requirement = line
            name_match = re.match(r"([A-Za-z0-9_.-]+)", requirement)
            if not name_match:
                continue
            name = name_match.group(1)
            key = (name.lower(), requirement)
            if key in seen:
                continue
            seen.add(key)
            components.append({
                "type": "library", "name": name, "version": requirement[len(name):].strip() or "unspecified",
                "properties": [{"name": "readmd:requirement-file", "value": req_file.name}],
            })
    components.sort(key=lambda item: (item["name"].lower(), item["version"]))
    return {
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "metadata": {"component": {"type": "application", "name": "ReadMD", "version": version}},
        "components": components,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--candidate-root", type=Path, default=ROOT)
    args = parser.parse_args()
    root = args.candidate_root.resolve()
    version = read_version(root)
    version_slug = version.replace("+", "-")
    out = args.output.resolve()
    if out.exists() and any(out.iterdir()):
        raise SystemExit(f"output directory must be new or empty: {out}")
    out.mkdir(parents=True, exist_ok=True)
    dist = root / "dist"
    # Prefer the freshly built canonical output.  A stale RC directory can
    # remain in dist from an earlier audit and must never silently become the
    # candidate payload; the RC name is only the fallback for older builders.
    onedir = first_existing(dist / "ReadMD", dist / f"ReadMD-v{version_slug}-RC")
    portable = first_existing(dist / f"ReadMD-portable-v{version_slug}-RC.exe", dist / "ReadMD-portable.exe")
    installer = first_existing(dist / f"ReadMDSetup-v{version_slug}-RC.exe", dist / "ReadMDSetup.exe")
    mcp = first_existing(dist / f"readmd-mcp-server-{version_slug}-RC.zip", dist / f"readmd-mcp-server-{version_slug}.zip")
    vsix = first_existing(
        root / "packages" / "vscode-extension" / f"readmd-vscode-{version_slug}.vsix",
        *sorted((root / "packages" / "vscode-extension").glob("*.vsix")),
    )
    required = {
        "ReadMD-windows-x64-onedir": onedir,
        "ReadMD-windows-x64-portable.exe": portable,
        f"ReadMDSetup-windows-x64-{version_slug}-RC.exe": installer,
        f"readmd-vscode-{version_slug}.vsix": vsix,
        f"readmd-mcp-server-{version_slug}.zip": mcp,
    }
    missing = [name for name, path in required.items() if path is None or not path.exists()]
    if missing:
        raise SystemExit("local RC inputs missing:\n" + "\n".join(missing))
    source_mtime = newest_source_mtime(root)
    for name, src in required.items():
        assert_fresh(src, source_mtime, name)
    for name, src in required.items():
        copy_item(src, out / name)
    manifest = root / "assets" / "upstream" / "manifest.json"
    shutil.copy2(manifest, out / "source-snapshot-manifest.json")
    (out / "THIRD_PARTY_LICENSES.md").write_text(
        f"# ReadMD {version} RC third-party licenses\n\n"
        "This RC embeds the immutable license/NOTICE files under `assets/upstream/`. "
        "The runtime uses only ReadMD adaptation metadata; upstream source remains read-only.\n\n"
        "- CC-SWITCH: MIT (`assets/upstream/farion1231-cc-switch/.../LICENSE`)\n"
        "- obra/superpowers writing skills: MIT (`assets/upstream/obra-superpowers/.../LICENSE`)\n"
        "- humanizer and avoid-ai-writing: MIT\n"
        "- creative-writing-skills: Apache-2.0\n"
        "- Codex skill-creator snapshot: Apache-2.0\n",
        encoding="utf-8", newline="\n",
    )
    (out / "sbom.cdx.json").write_text(json.dumps(build_sbom(root, version), ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    (out / "functional-acceptance.md").write_text(
        f"# ReadMD {version} Windows RC acceptance\n\n"
        "Candidate is intentionally non-publishing. Verify installer/portable cold start, "
        "open/edit/save, preview, TOC/search, export, offline upstream source viewer, Skills, "
        "provider settings, VSIX and MCP before approval.\n\n"
        "- [ ] installer install / upgrade / uninstall\n- [ ] portable offline startup\n"
        "- [ ] document open/edit/preview/export\n- [ ] AI provider and Skill workbench\n"
        "- [ ] VSIX and MCP client connection\n- [ ] no local paths or secrets in outputs\n",
        encoding="utf-8", newline="\n",
    )
    files = []
    # The checksum list covers distributable assets and audit evidence.  Its
    # own bytes and the self-describing candidate metadata are intentionally
    # excluded to avoid a self-referential hash; the metadata records this
    # exclusion explicitly and carries the candidate commit for verification.
    for path in sorted(out.rglob("*")):
        if path.is_file() and path.name not in {"SHA256SUMS.txt", "candidate.json"}:
            files.append({"file": path.relative_to(out).as_posix(), "bytes": path.stat().st_size, "sha256": sha256(path)})
    (out / "SHA256SUMS.txt").write_text(
        "".join(f"{item['sha256']}  {item['file']}\n" for item in files), encoding="utf-8", newline="\n"
    )
    metadata = {
        "schema_version": 1, "release": version, "candidate": "Windows x64 RC",
        "commit": git("rev-parse", "HEAD"), "branch": git("branch", "--show-current"),
        "dirty_at_packaging": bool(git("status", "--porcelain")),
        "built_at_utc": datetime.now(timezone.utc).isoformat(),
        "toolchain": {"python": platform.python_version(), "platform": platform.platform(), "pyinstaller": "6.22.2"},
        "files": files,
        "checksum_exclusions": ["SHA256SUMS.txt", "candidate.json"],
        "formal_release": False,
    }
    (out / "candidate.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(out)
    print("candidate commit: %s" % metadata["commit"])
    print("files: %d" % len(files))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
