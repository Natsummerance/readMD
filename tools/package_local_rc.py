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
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


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


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True, encoding="utf-8").strip()


def build_sbom() -> dict:
    components = []
    try:
        from importlib import metadata
        for dist in sorted(metadata.distributions(), key=lambda d: (d.metadata.get("Name") or "").lower()):
            name = dist.metadata.get("Name")
            version = dist.version
            if name and version:
                components.append({"type": "library", "name": name, "version": version})
    except Exception:
        pass
    return {
        "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
        "metadata": {"component": {"type": "application", "name": "ReadMD", "version": "2.3.7"}},
        "components": components,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--candidate-root", type=Path, default=ROOT)
    args = parser.parse_args()
    root = args.candidate_root.resolve()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    dist = root / "dist"
    onedir = first_existing(dist / "ReadMD-v2.3.7-RC", dist / "ReadMD")
    portable = first_existing(dist / "ReadMD-portable-v2.3.7-RC.exe", dist / "ReadMD-portable.exe")
    installer = first_existing(dist / "ReadMDSetup-v2.3.7-RC.exe", dist / "ReadMDSetup.exe")
    mcp = first_existing(dist / "readmd-mcp-server-2.3.7-RC.zip", dist / "readmd-mcp-server-2.3.7.zip")
    vsix = first_existing(
        root / "packages" / "vscode-extension" / "readmd-vscode-2.3.7.vsix",
        *sorted((root / "packages" / "vscode-extension").glob("*.vsix")),
    )
    required = {
        "ReadMD-windows-x64-onedir": onedir,
        "ReadMD-windows-x64-portable.exe": portable,
        "ReadMDSetup-windows-x64-RC.exe": installer,
        "readmd-vscode-2.3.7.vsix": vsix,
        "readmd-mcp-server-2.3.7.zip": mcp,
    }
    missing = [name for name, path in required.items() if path is None or not path.exists()]
    if missing:
        raise SystemExit("local RC inputs missing:\n" + "\n".join(missing))
    for name, src in required.items():
        copy_item(src, out / name)
    manifest = root / "assets" / "upstream" / "manifest.json"
    shutil.copy2(manifest, out / "source-snapshot-manifest.json")
    (out / "THIRD_PARTY_LICENSES.md").write_text(
        "# ReadMD V2.3.7 RC third-party licenses\n\n"
        "This RC embeds the immutable license/NOTICE files under `assets/upstream/`. "
        "The runtime uses only ReadMD adaptation metadata; upstream source remains read-only.\n\n"
        "- CC-SWITCH: MIT (`assets/upstream/farion1231-cc-switch/.../LICENSE`)\n"
        "- obra/superpowers writing skills: MIT (`assets/upstream/obra-superpowers/.../LICENSE`)\n"
        "- humanizer and avoid-ai-writing: MIT\n"
        "- creative-writing-skills: Apache-2.0\n"
        "- Codex skill-creator snapshot: Apache-2.0\n",
        encoding="utf-8", newline="\n",
    )
    (out / "sbom.cdx.json").write_text(json.dumps(build_sbom(), ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    (out / "functional-acceptance.md").write_text(
        "# ReadMD V2.3.7 Windows RC acceptance\n\n"
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
        "schema_version": 1, "release": "2.3.7", "candidate": "Windows x64 RC",
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
