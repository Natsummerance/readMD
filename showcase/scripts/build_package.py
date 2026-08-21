#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Orchestrate story, copy, composition, and QA into one publish package."""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

from build_story import build_story
from validate_package import validate_package
from write_copy import generate_copy


def build_package(
    *,
    release: str,
    previous_release: str,
    notes_text: str,
    diff_text: str,
    package_dir: Path,
    repo_root: Path,
    repository: str = "Natsummerance/readMD",
) -> tuple[dict, dict]:
    package_dir.mkdir(parents=True, exist_ok=True)
    (package_dir / "images").mkdir(exist_ok=True)
    story = build_story(
        release=release,
        previous_release=previous_release,
        notes=notes_text,
        diff=diff_text,
        shot_library_path=repo_root / "showcase" / "shot_library.json",
    )
    (package_dir / "story.json").write_text(json.dumps(story, ensure_ascii=False, indent=2), encoding="utf-8")
    metadata = generate_copy(story, repository=repository, previous_release=previous_release)
    (package_dir / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")
    (package_dir / "title.txt").write_text(metadata["title"], encoding="utf-8")
    (package_dir / "body.txt").write_text(metadata["body"], encoding="utf-8")
    (package_dir / "topics.txt").write_text("\n".join(metadata["topics"]), encoding="utf-8")
    return story, metadata


def compose_and_validate(package_dir: Path, repo_root: Path) -> list[str]:
    compose_script = repo_root / "showcase" / "scripts" / "compose_cards.js"
    subprocess.run(["node", str(compose_script), str(package_dir)], cwd=repo_root, check=True)
    return validate_package(package_dir, repo_root=repo_root)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", required=True)
    parser.add_argument("--previous-release", required=True)
    parser.add_argument("--notes", type=Path, required=True)
    parser.add_argument("--diff", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repository", default="Natsummerance/readMD")
    parser.add_argument("--skip-compose", action="store_true", help="Prepare text/story only; used before capture")
    args = parser.parse_args()
    repo_root = Path(__file__).resolve().parents[2]
    story, _ = build_package(
        release=args.release,
        previous_release=args.previous_release,
        notes_text=args.notes.read_text(encoding="utf-8"),
        diff_text=args.diff.read_text(encoding="utf-8") if args.diff else "",
        package_dir=args.output,
        repo_root=repo_root,
        repository=args.repository,
    )
    errors: list[str] = []
    if not args.skip_compose:
        errors = compose_and_validate(args.output, repo_root)
        report = {"ok": not errors, "errors": errors}
        (args.output / "qa.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False))
        if errors:
            return 1
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
