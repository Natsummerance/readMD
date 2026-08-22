#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Watch QA-green showcase packages and publish them through xhs-publish."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import time
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from content_memory import load_records, upsert_record
from copy_variants import text_fingerprints, text_trigrams

DEFAULT_PUBLISHER = Path("Z:/Natsumer/.codex/skills/xhs-publish/scripts/xhs_publish.py")
STATE_VERSION = 1


def load_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"schema_version": STATE_VERSION, "packages": {}}
    data = json.loads(path.read_text(encoding="utf-8"))
    data.setdefault("schema_version", STATE_VERSION)
    data.setdefault("packages", {})
    return data


def safe_extract(zip_path: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    resolved_destination = destination.resolve()
    with zipfile.ZipFile(zip_path) as archive:
        for member in archive.infolist():
            target = (destination / member.filename).resolve()
            if not target.is_relative_to(resolved_destination):
                raise ValueError(f"unsafe zip path: {member.filename}")
            if member.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            if (member.external_attr >> 16) & 0xF000 == 0xA000:
                raise ValueError(f"zip symlinks are not allowed: {member.filename}")
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(member) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)


def package_identity(package_dir: Path) -> tuple[str, str]:
    story = json.loads((package_dir / "story.json").read_text(encoding="utf-8"))
    metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
    return str(story["release"]), str(metadata["title"])


def localize_image_paths(package_dir: Path) -> None:
    metadata_path = package_dir / "metadata.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata["images"] = [str((package_dir / "images" / Path(item).name).resolve()) for item in metadata["images"]]
    metadata_path.write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")


def publish_command(publisher: Path, package_dir: Path, draft: bool) -> list[str]:
    metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
    images = [Path(item) for item in metadata["images"]]
    command = [
        sys.executable,
        str(publisher),
        "publish",
        "--bootstrap-edge",
        "--restart-edge",
        "--title-file", str(package_dir / "title.txt"),
        "--body-file", str(package_dir / "body.txt"),
        "--cover", str(images[0]),
    ]
    for image in images[1:]:
        command.extend(("--image", str(image)))
    for topic in metadata["topics"]:
        command.extend(("--topic", str(topic)))
    if draft:
        command.append("--no-publish")
    return command


def query_status(publisher: Path, title: str) -> dict[str, Any]:
    completed = subprocess.run(
        [sys.executable, str(publisher), "status", "--note-title", title],
        text=True,
        capture_output=True,
        encoding="utf-8",
        timeout=120,
    )
    if completed.returncode != 0:
        raise RuntimeError((completed.stderr or completed.stdout or "status query failed").strip())
    for line in reversed((completed.stdout or "").splitlines()):
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    return {}


def seed_feedback_ledger(
    ledger_path: Path,
    package_dir: Path,
    *,
    release: str,
    title: str,
    publisher_result: dict[str, Any],
    audit_status: str | None,
) -> dict[str, Any]:
    metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
    body_path = package_dir / "body.txt"
    body = body_path.read_text(encoding="utf-8").strip() if body_path.exists() else ""
    record = {
        "release": release,
        "title": title,
        "variant_id": str(metadata.get("variant_id", "unknown")),
        "copy_frame": str(metadata.get("copy_frame", "unknown")),
        "title_formula_id": str(metadata.get("title_formula_id", "unknown")),
        "hook_type": str(metadata.get("hook_type", metadata.get("strategy", "unknown"))),
        "published_at": datetime.now(timezone.utc).isoformat(),
        "impressions": 0,
        "likes": 0,
        "collects": 0,
        "comments": 0,
        "shares": 0,
        "follows": 0,
        "metrics_status": "pending",
        "note_id": publisher_result.get("noteId"),
        "publisher_target_id": publisher_result.get("targetId"),
        "published_url": publisher_result.get("url"),
        "audit_status": audit_status,
        **text_fingerprints(body),
        "body_trigrams": sorted(text_trigrams(body)),
        "lessons": "Published automatically; awaiting platform metrics and manual review.",
    }
    return upsert_record(ledger_path, record)


def process_package(
    zip_path: Path,
    work_root: Path,
    state_path: Path,
    publisher: Path,
    max_attempts: int,
    draft: bool,
    ledger_path: Path | None = None,
) -> bool:
    state = load_state(state_path)
    token = hashlib.sha256(zip_path.read_bytes()).hexdigest()[:16]
    record = state["packages"].setdefault(token, {"zip": str(zip_path), "attempts": 0, "status": "pending"})
    if record["status"] in {"published", "drafted"}:
        return False
    if record["attempts"] >= max_attempts:
        record["status"] = "abandoned"
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")
        return False

    package_dir = work_root / token
    try:
        safe_extract(zip_path, package_dir)
        qa = json.loads((package_dir / "qa.json").read_text(encoding="utf-8"))
        if qa.get("ok") is not True:
            raise ValueError("package qa.json is not green")
        copy_review = json.loads((package_dir / "copy-review.json").read_text(encoding="utf-8"))
        if copy_review.get("ok") is not True:
            raise ValueError("package copy-review.json is not green")
        variants = json.loads((package_dir / "variants.json").read_text(encoding="utf-8"))
        if variants.get("ok") is not True:
            raise ValueError("package variants.json is not green")
        metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
        selected_variant_id = metadata.get("variant_id")
        reported_variant_id = variants.get("chosen_variant_id")
        if selected_variant_id or reported_variant_id:
            if not selected_variant_id or not reported_variant_id or selected_variant_id != reported_variant_id:
                raise ValueError(
                    f"selected variant_id mismatch: metadata={selected_variant_id}, report={reported_variant_id}"
                )
            chosen_ranked = next(
                (item for item in variants.get("ranked", []) if item.get("variant_id") == reported_variant_id),
                None,
            )
        else:
            chosen_ranked = next((item for item in variants.get("ranked", []) if item.get("strategy") == variants.get("chosen_strategy")), None)
        if not chosen_ranked or chosen_ranked.get("ok") is not True or chosen_ranked.get("originality_failures"):
            raise ValueError("package variant originality gate is not green")
        copy_frames = {
            "metadata": str(metadata.get("copy_frame") or ""),
            "selection": str(variants.get("chosen_copy_frame") or ""),
            "ranking": str(chosen_ranked.get("copy_frame") or ""),
        }
        frames_present = [source for source, value in copy_frames.items() if value]
        if frames_present:
            if len(frames_present) != len(copy_frames):
                raise ValueError(
                    "selected copy_frame is incomplete: "
                    + ", ".join(f"{source}={copy_frames[source] or '<missing>'}" for source in frames_present)
                )
            if len({copy_frames[source] for source in frames_present}) != 1:
                raise ValueError(
                    "selected copy_frame mismatch: "
                    f"metadata={copy_frames['metadata']}, report={copy_frames['selection']}, "
                    f"ranking={copy_frames['ranking']}"
                )
        dashboard = json.loads((package_dir / "dashboard-qa.json").read_text(encoding="utf-8"))
        if dashboard.get("ok") is not True:
            raise ValueError("package dashboard-qa.json is not green")
        pattern_audit = json.loads((package_dir / "pattern-audit.json").read_text(encoding="utf-8"))
        if pattern_audit.get("ok") is not True:
            raise ValueError("package pattern-audit.json is not green")
        wechat_qa_path = package_dir / "wechat" / "wechat-qa.json"
        if not wechat_qa_path.exists():
            raise ValueError("package wechat-qa.json is missing")
        wechat_qa = json.loads(wechat_qa_path.read_text(encoding="utf-8"))
        if wechat_qa.get("ok") is not True:
            raise ValueError("package wechat-qa.json is not green")
        localize_image_paths(package_dir)
        release, title = package_identity(package_dir)
        previous = [item for item in state["packages"].values() if item.get("release") == release and item.get("status") == "published"]
        if previous:
            raise ValueError(f"release already published: {release}")
        if ledger_path and any(
            record.get("release") == release
            for record in load_records(ledger_path)
        ):
            raise ValueError(f"release already exists in publication ledger: {release}")
        command = publish_command(publisher, package_dir, draft=draft)
        completed = subprocess.run(command, text=True, capture_output=True, encoding="utf-8", timeout=600)
        result: dict[str, Any] = {}
        for line in reversed((completed.stdout or "").splitlines()):
            try:
                result = json.loads(line)
                break
            except json.JSONDecodeError:
                continue
        record["attempts"] += 1
        record["release"] = release
        record["variant_id"] = selected_variant_id
        record["title"] = title
        if completed.returncode == 0:
            record["status"] = "drafted" if draft else "published"
            record["note_id"] = result.get("noteId")
            record["publisher_target_id"] = result.get("targetId")
            record["published_url"] = result.get("url")
            if not draft:
                status = query_status(publisher, title)
                record["audit_status"] = status.get("status")
                record["status_result"] = status
                if ledger_path:
                    try:
                        feedback = seed_feedback_ledger(
                            ledger_path,
                            package_dir,
                            release=release,
                            title=title,
                            publisher_result=result,
                            audit_status=record["audit_status"],
                        )
                        record["ledger_status"] = "seeded"
                        record["ledger_release"] = feedback["release"]
                    except Exception as ledger_error:
                        # Publishing already happened; retain the failure for repair instead of losing it.
                        record["ledger_status"] = "seed_failed"
                        record["ledger_error"] = str(ledger_error)
            record["result"] = result
            print(json.dumps({"ok": True, "token": token, "status": record["status"], "release": release, "title": title}, ensure_ascii=False))
            return True
        record["status"] = "retrying" if record["attempts"] < max_attempts else "failed"
        record["error"] = (completed.stderr or completed.stdout or "unknown publisher error").strip()
        print(json.dumps({"ok": False, "token": token, "status": record["status"], "error": record["error"]}, ensure_ascii=False), file=sys.stderr)
        return False
    except Exception as exc:
        record["attempts"] += 1
        record["status"] = "retrying" if record["attempts"] < max_attempts else "failed"
        record["error"] = str(exc)
        print(json.dumps({"ok": False, "status": record["status"], "error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        return False
    finally:
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--watch-dir", type=Path, default=Path("showcase/downloads"))
    parser.add_argument("--work-dir", type=Path, default=Path("showcase/publish-work"))
    parser.add_argument("--state", type=Path, default=Path("showcase/publish-state.json"))
    parser.add_argument("--publisher", type=Path, default=DEFAULT_PUBLISHER)
    parser.add_argument("--ledger", type=Path, default=Path(__file__).parents[1] / "content" / "publication-ledger.jsonl")
    parser.add_argument("--max-attempts", type=int, choices={1, 2, 3}, default=3)
    parser.add_argument("--interval", type=float, default=30.0)
    parser.add_argument("--once", action="store_true")
    parser.add_argument("--draft", action="store_true", help="fill the Xiaohongshu form without clicking publish")
    args = parser.parse_args()
    args.watch_dir.mkdir(parents=True, exist_ok=True)
    args.work_dir.mkdir(parents=True, exist_ok=True)
    while True:
        zips = sorted(args.watch_dir.glob("*.zip"), key=lambda path: path.stat().st_mtime)
        for zip_path in zips:
            process_package(
                zip_path,
                args.work_dir,
                args.state,
                args.publisher,
                args.max_attempts,
                args.draft,
                ledger_path=args.ledger,
            )
        if args.once:
            return 0
        time.sleep(max(5.0, args.interval))


if __name__ == "__main__":
    raise SystemExit(main())
