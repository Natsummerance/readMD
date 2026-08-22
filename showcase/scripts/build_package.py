#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Orchestrate story, copy, composition, and QA into one publish package."""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

from audit_copy import audit_package
from content_memory import load_learning_records, load_records
from build_story import build_story
from copy_variants import select_variant
from export_wechat import export_package
import pattern_audit
import performance_report
import review_dashboard
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
    memory_path: Path | None = None,
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
    history = load_learning_records(memory_path) if memory_path else []
    metadata = generate_copy(
        story,
        repository=repository,
        previous_release=previous_release,
        history=history,
    )
    metadata, variant_selection = select_variant(
        story=story,
        base_metadata=metadata,
        history=history,
    )
    (package_dir / "variants.json").write_text(json.dumps(variant_selection, ensure_ascii=False, indent=2), encoding="utf-8")
    (package_dir / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")
    publication_records = load_records(memory_path) if memory_path else []
    performance_report.generate_report(publication_records, package_dir)
    (package_dir / "title.txt").write_text(metadata["title"], encoding="utf-8")
    (package_dir / "body.txt").write_text(metadata["body"], encoding="utf-8")
    (package_dir / "topics.txt").write_text("\n".join(metadata["topics"]), encoding="utf-8")
    return story, metadata


def compose_and_validate(package_dir: Path, repo_root: Path) -> list[str]:
    compose_script = repo_root / "showcase" / "scripts" / "compose_cards.js"
    package_dir.mkdir(parents=True, exist_ok=True)
    errors: list[str] = []
    composed = True
    try:
        subprocess.run(["node", str(compose_script), str(package_dir)], cwd=repo_root, check=True)
    except subprocess.CalledProcessError as exc:
        errors.append(f"card composition failed with exit code {exc.returncode}")
        composed = False

    copy_report = audit_package(package_dir)
    if not copy_report["ok"]:
        errors.append(f"semantic alignment gate failed: {json.dumps(copy_report, ensure_ascii=False)}")
    else:
        wechat_report = export_package(package_dir)
        if not wechat_report["ok"]:
            errors.append(f"WeChat adapter gate failed: {json.dumps(wechat_report['errors'], ensure_ascii=False)}")

    if composed:
        try:
            pattern_audit.audit_package(package_dir)
        except Exception as exc:
            errors.append(f"hot-post pattern gate crashed: {exc}")
        errors.extend(validate_package(package_dir, repo_root=repo_root))
        # The preflight panel must describe this run's gates, not a stale prior package.
        provisional_qa = {"ok": not errors, "errors": errors}
        (package_dir / "qa.json").write_text(json.dumps(provisional_qa, ensure_ascii=False, indent=2), encoding="utf-8")
        try:
            dashboard_report = review_dashboard.generate_package(package_dir)
            if not dashboard_report.get("ok"):
                errors.append(
                    "review dashboard gate failed: "
                    f"{json.dumps(dashboard_report.get('errors', []), ensure_ascii=False)}"
                )
        except Exception as exc:
            errors.append(f"review dashboard gate crashed: {exc}")

    qa_report = {"ok": not errors, "errors": errors}
    (package_dir / "qa.json").write_text(json.dumps(qa_report, ensure_ascii=False, indent=2), encoding="utf-8")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release")
    parser.add_argument("--previous-release")
    parser.add_argument("--notes", type=Path)
    parser.add_argument("--diff", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repository", default="Natsummerance/readMD")
    parser.add_argument("--memory", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    parser.add_argument("--skip-compose", action="store_true", help="Prepare text/story only; used before capture")
    parser.add_argument("--finalize", action="store_true", help="Compose and aggregate QA for an already prepared package")
    args = parser.parse_args()
    repo_root = Path(__file__).resolve().parents[2]
    if args.finalize:
        errors = compose_and_validate(args.output, repo_root)
        report = json.loads((args.output / "qa.json").read_text(encoding="utf-8"))
        print(json.dumps(report, ensure_ascii=False))
        if errors:
            return 1
        print(args.output)
        return 0
    if not args.release or not args.previous_release or not args.notes:
        parser.error("--release, --previous-release and --notes are required unless --finalize is used")
    story, _ = build_package(
        release=args.release,
        previous_release=args.previous_release,
        notes_text=args.notes.read_text(encoding="utf-8"),
        diff_text=args.diff.read_text(encoding="utf-8") if args.diff else "",
        package_dir=args.output,
        repo_root=repo_root,
        repository=args.repository,
        memory_path=args.memory,
    )
    errors: list[str] = []
    if not args.skip_compose:
        errors = compose_and_validate(args.output, repo_root)
        report = json.loads((args.output / "qa.json").read_text(encoding="utf-8"))
        print(json.dumps(report, ensure_ascii=False))
        if errors:
            return 1
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
