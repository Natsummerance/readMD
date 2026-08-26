#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Watch QA-green showcase packages and publish them through xhs-publish."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from content_memory import load_records, upsert_record
from audit_copy import audit_copy
from build_package import compose_and_validate
from copy_variants import (
    jaccard_similarity,
    text_fingerprints,
    text_trigrams,
    title_fingerprints,
)
from pattern_audit import audit_patterns
from package_content import validate_release_evidence, verify_package_manifest
import performance_report
from poster_style import resolve_poster_style
from review_dashboard import build_dashboard, collect_inputs, validate_dashboard
from validate_package import (
    publisher_asset_errors,
    publisher_directive_errors,
    publisher_input_errors,
    publisher_learning_materiality_errors,
    publisher_poster_style_errors,
    publisher_resonance_source_errors,
    variant_selection_integrity_errors,
)

DEFAULT_PUBLISHER = Path("Z:/Natsumer/.codex/skills/xhs-publish/scripts/xhs_publish.py")
STATE_VERSION = 1
SHOWCASE_ROOT = Path(__file__).resolve().parents[1]
ORIGINALITY_ENDPOINT_COOLDOWN_RELEASES = 8
SUCCESS_STATUSES = {"published", "drafted"}
FAILURE_STATUSES = {"failed", "abandoned"}


def publisher_environment() -> dict[str, str]:
    """Keep publisher JSON valid even when Windows defaults to a legacy code page."""
    environment = os.environ.copy()
    environment["PYTHONIOENCODING"] = "utf-8"
    return environment


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


def package_token(zip_path: Path) -> str:
    return hashlib.sha256(zip_path.read_bytes()).hexdigest()[:16]


def archive_terminal_package(zip_path: Path, state_path: Path) -> bool:
    """Move a completed package out of the live watch directory.

    Failed packages are retained in a sibling directory so their zip and QA
    evidence remain available without forcing the watcher to rescan them.
    """
    state = load_state(state_path)
    token = package_token(zip_path)
    record = state["packages"].get(token)
    status = str(record.get("status", "")) if record else ""
    if status == "published" and record.get("ledger_status") == "seed_failed":
        # Leave the package live so the watcher can finish feedback-ledger repair.
        return False
    if status in SUCCESS_STATUSES:
        outcome = "processed"
    elif status in FAILURE_STATUSES:
        outcome = "failed"
    else:
        return False

    archive_dir = zip_path.parent / outcome
    archive_dir.mkdir(parents=True, exist_ok=True)
    target = archive_dir / f"{token}-{zip_path.name}"
    record["zip"] = str(target)
    record["archived_at"] = datetime.now(timezone.utc).isoformat()
    # Commit the destination first. If interrupted before the move, the next
    # scan sees a terminal status and retries this idempotent operation.
    state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")
    zip_path.replace(target)
    return True


def reconcile_archived_packages(watch_dir: Path, state_path: Path) -> None:
    """Repair paths if the process stopped between archiving and state save."""
    state = load_state(state_path)
    changed = False
    for outcome in ("processed", "failed"):
        archive_dir = watch_dir / outcome
        if not archive_dir.exists():
            continue
        for archive_path in archive_dir.glob("*.zip"):
            token = package_token(archive_path)
            record = state["packages"].get(token)
            if record and Path(str(record.get("zip", ""))) != archive_path:
                record["zip"] = str(archive_path)
                changed = True
    if changed:
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


def package_identity(package_dir: Path) -> tuple[str, str]:
    story = json.loads((package_dir / "story.json").read_text(encoding="utf-8"))
    metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
    return str(story["release"]), str(metadata["title"])


def localize_image_paths(package_dir: Path) -> None:
    metadata_path = package_dir / "metadata.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata["images"] = [str((package_dir / "images" / Path(item).name).resolve()) for item in metadata["images"]]
    metadata_path.write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")


def publish_command(publisher: Path, package_dir: Path, draft: bool, *, reuse_edge: bool = False) -> list[str]:
    metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
    images = [Path(item) for item in metadata["images"]]
    command = [
        sys.executable,
        str(publisher),
        "publish",
        "--bootstrap-edge",
        "--title-file", str(package_dir / "title.txt"),
        "--body-file", str(package_dir / "body.txt"),
        "--cover", str(images[0]),
    ]
    if not reuse_edge:
        command.insert(4, "--restart-edge")
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
        env=publisher_environment(),
        timeout=120,
    )
    if completed.returncode != 0:
        raise RuntimeError((completed.stderr or completed.stdout or "status query failed").strip())
    for line in reversed((completed.stdout or "").splitlines()):
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict):
            return parsed
    return {}


def _status_confirms_note(status: dict[str, Any]) -> bool:
    """A found note is authoritative even when its audit tab is still unknown."""
    return bool(status.get("noteId") or status.get("status") in {"审核中", "已发布"})


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
    story = json.loads((package_dir / "story.json").read_text(encoding="utf-8"))
    composition = json.loads((package_dir / "composition.json").read_text(encoding="utf-8"))
    body_path = package_dir / "body.txt"
    body = body_path.read_text(encoding="utf-8").strip() if body_path.exists() else ""
    record = {
        "release": release,
        "title": title,
        "variant_id": str(metadata.get("variant_id", "unknown")),
        "copy_frame": str(metadata.get("copy_frame", "unknown")),
        "poster_style": str(composition.get("poster_style") or story.get("poster_style") or "evidence-paper"),
        "title_formula_id": str(metadata.get("title_formula_id", "unknown")),
        "title_source_template": str(metadata.get("title_source_template", "unknown")),
        "title_adaptation": str(metadata.get("title_adaptation", "unknown")),
        "hook_type": str(metadata.get("hook_type", metadata.get("strategy", "unknown"))),
        "primary_shot": str(metadata.get("primary_shot", "unknown")),
        "topic_set_id": str(metadata.get("topic_set_id", "unknown")),
        "topic_set_label": str(metadata.get("topic_set_label", "unknown")),
        "topics": [str(item) for item in metadata.get("topics", [])],
        "resonance_directive": metadata.get("resonance_directive"),
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
        **title_fingerprints(title),
        **text_fingerprints(body),
        "body_trigrams": sorted(text_trigrams(body)),
        "lessons": "Published automatically; awaiting platform metrics and manual review.",
    }
    return upsert_record(ledger_path, record)


def repair_failed_feedback_ledger(
    record: dict[str, Any],
    zip_path: Path,
    package_dir: Path,
    ledger_path: Path,
) -> None:
    """Restore feedback identity after a successful publish whose ledger write failed."""
    if not (package_dir / "metadata.json").is_file():
        safe_extract(zip_path, package_dir)
    release = record.get("release")
    title = record.get("title")
    if not release or not title:
        raise ValueError("published record is missing release/title identity")

    if any(item.get("release") == release for item in load_records(ledger_path)):
        # Never replay zero counters over metrics that may have been imported already.
        record["ledger_status"] = "seeded"
        record.pop("ledger_error", None)
        record.pop("ledger_repair_error", None)
        return

    result = record.get("result") or {
        "published": True,
        "noteId": record.get("note_id"),
        "targetId": record.get("publisher_target_id"),
        "url": record.get("published_url"),
    }
    feedback = seed_feedback_ledger(
        ledger_path,
        package_dir,
        release=release,
        title=title,
        publisher_result=result,
        audit_status=record.get("audit_status"),
    )
    record["ledger_status"] = "seeded"
    record["ledger_release"] = feedback["release"]
    record.pop("ledger_error", None)
    record.pop("ledger_repair_error", None)


def recomputed_gate_errors(package_dir: Path) -> list[str]:
    """Recompute semantic and mechanism gates; persisted green labels are not trust roots."""
    load = lambda name: json.loads((package_dir / name).read_text(encoding="utf-8"))
    errors: list[str] = []
    try:
        story = load("story.json")
        metadata = load("metadata.json")
        composition = load("composition.json")
    except Exception as exc:
        return [f"recomputed gate inputs unreadable: {exc}"]

    try:
        semantic = audit_copy(story=story, metadata=metadata, composition=composition)
        if semantic.get("ok") is not True:
            errors.extend(f"semantic gate: {item}" for item in semantic.get("hard_failures", []))
            errors.append(f"semantic score {semantic.get('total_score')} is below the publication contract")
    except Exception as exc:
        errors.append(f"semantic gate crashed: {exc}")

    try:
        pattern = audit_patterns(
            story=story,
            metadata=metadata,
            composition=composition,
            library_path=SHOWCASE_ROOT / "content" / "pattern-library.json",
        )
        if pattern.get("ok") is not True:
            errors.extend(f"hot-post gate: {item}" for item in pattern.get("errors", []))
    except Exception as exc:
        errors.append(f"hot-post gate crashed: {exc}")

    variants_path = package_dir / "variants.json"
    if variants_path.exists():
        try:
            variants = load("variants.json")
            integrity_errors, _ = variant_selection_integrity_errors(metadata, variants)
            errors.extend(f"variant selection gate: {item}" for item in integrity_errors)
        except Exception as exc:
            errors.append(f"variant selection gate crashed: {exc}")

    dashboard_path = package_dir / "review-dashboard.html"
    if dashboard_path.exists():
        try:
            inputs = collect_inputs(package_dir)
            expected_dashboard = build_dashboard(inputs)
            actual_dashboard = dashboard_path.read_text(encoding="utf-8")
            if actual_dashboard != expected_dashboard:
                errors.append("review dashboard is stale or does not match package data")
            dashboard_errors = validate_dashboard(expected_dashboard)
            if dashboard_errors:
                errors.append("review dashboard uses forbidden artifacts: " + "; ".join(dashboard_errors))
        except Exception as exc:
            errors.append(f"review dashboard gate crashed: {exc}")

    try:
        dashboard_report = load("dashboard-qa.json")
        expected_dashboard_ok = not any(error.startswith("review dashboard") for error in errors)
        if dashboard_report.get("ok") is not expected_dashboard_ok:
            errors.append("dashboard QA verdict differs from recomputed dashboard")
        expected_overall = "PASS" if inputs.get("qa", {}).get("ok") is True else "NEEDS FIX"
        if dashboard_report.get("overall_status") != expected_overall:
            errors.append("dashboard overall status differs from package QA")
    except Exception as exc:
        errors.append(f"dashboard QA gate crashed: {exc}")
    return errors


def publisher_originality_errors(package_dir: Path, ledger_path: Path | None) -> list[str]:
    """Recheck the winning copy against the real publication ledger."""
    if ledger_path is None or not ledger_path.exists():
        return []
    try:
        metadata = json.loads((package_dir / "metadata.json").read_text(encoding="utf-8"))
        records = load_records(ledger_path)
    except Exception as exc:
        return [f"publisher originality inputs unreadable: {exc}"]

    body = str(metadata.get("body", ""))
    fingerprints = text_fingerprints(body)
    trigrams = text_trigrams(body)
    title = str(metadata.get("title", ""))
    title_prints = title_fingerprints(title)
    title_trigrams = set(title_prints["title_trigrams"])
    release = str(metadata.get("release", ""))
    priors = [record for record in records if str(record.get("release", "")) != release]
    errors: list[str] = []

    for prior in priors:
        release_name = str(prior.get("release") or "previous release")
        prior_trigrams = set(prior.get("body_trigrams") or [])
        similarity = jaccard_similarity(trigrams, prior_trigrams)
        if fingerprints["body_sha256"] and prior.get("body_sha256") == fingerprints["body_sha256"]:
            errors.append(f"body hash matches {release_name}")
        if similarity >= 0.85:
            errors.append(
                f"near-duplicate body ({similarity:.2f}) matches {release_name}"
            )

        prior_title_trigrams = set(prior.get("title_trigrams") or []) or set(text_trigrams(str(prior.get("title", ""))))
        title_similarity = jaccard_similarity(title_trigrams, prior_title_trigrams)
        if title_prints["title_sha256"] and prior.get("title_sha256") == title_prints["title_sha256"]:
            errors.append(f"title hash matches {release_name}")
        if title_similarity >= 0.85:
            errors.append(
                f"near-duplicate title ({title_similarity:.2f}) matches {release_name}"
            )

    for prior in priors[-ORIGINALITY_ENDPOINT_COOLDOWN_RELEASES:]:
        release_name = str(prior.get("release") or "previous release")
        if fingerprints["opening"] and prior.get("opening") == fingerprints["opening"]:
            errors.append(f"opening matches {release_name}")
        if fingerprints["closing"] and prior.get("closing") == fingerprints["closing"]:
            errors.append(f"closing matches {release_name}")
    return sorted(set(errors))


def reconcile_auto_poster_style(package_dir: Path, ledger_path: Path | None) -> dict[str, Any] | None:
    """Re-render a CI exploration package with the operator's local evidence choice."""
    if ledger_path is None:
        return None

    load = lambda name: json.loads((package_dir / name).read_text(encoding="utf-8"))
    try:
        story = load("story.json")
        variants = load("variants.json")
    except Exception as exc:
        raise ValueError(f"poster style reconciliation inputs unreadable: {exc}") from exc

    selection = variants.get("poster_style_selection") if isinstance(variants, dict) else None
    if not isinstance(selection, dict) or selection.get("mode") == "fixed":
        return None

    precheck = publisher_poster_style_errors(package_dir, ledger_path)
    if not precheck:
        return None

    recoverable = all(
        "selects a different poster style" in error
        or "changes the poster selection mode" in error
        for error in precheck
    )
    if not recoverable:
        raise ValueError("publisher poster style evidence contract failed: " + "; ".join(precheck))

    previous = str(story.get("poster_style") or "evidence-paper")
    selected, recomputed_selection = resolve_poster_style("auto", ledger_path)
    story["poster_style"] = selected
    variants["poster_style_selection"] = recomputed_selection
    metadata = load("metadata.json")
    metadata["poster_style"] = selected

    (package_dir / "story.json").write_text(
        json.dumps(story, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    (package_dir / "variants.json").write_text(
        json.dumps(variants, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    (package_dir / "metadata.json").write_text(
        json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    performance_report.generate_report(load_records(ledger_path), package_dir)

    # The raw PNGs remain immutable; only the evidence wrapper is recomposed and
    # then forced through every semantic, pixel, mechanism, and dashboard gate.
    compose_and_validate(package_dir, SHOWCASE_ROOT.parent, memory_path=None)
    residual = publisher_poster_style_errors(package_dir, ledger_path)
    if residual:
        raise ValueError("poster style reconciliation did not converge: " + "; ".join(residual))
    return {
        "schema_version": 1,
        "from": previous,
        "to": selected,
        "mode": recomputed_selection.get("mode"),
        "recomposed": True,
    }


def process_package(
    zip_path: Path,
    work_root: Path,
    state_path: Path,
    publisher: Path,
    max_attempts: int,
    draft: bool,
    ledger_path: Path | None = None,
    *,
    reuse_edge: bool = False,
) -> bool:
    try:
        verify_package_manifest(zip_path)
    except Exception as exc:
        record = {"attempts": 1, "status": "failed", "error": str(exc)}
        state = load_state(state_path)
        token = package_token(zip_path)
        state["packages"][token] = record
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"ok": False, "token": token, "status": "failed", "error": str(exc)}, ensure_ascii=False), file=sys.stderr)
        return False
    state = load_state(state_path)
    token = package_token(zip_path)
    record = state["packages"].setdefault(token, {"zip": str(zip_path), "attempts": 0, "status": "pending"})
    if record["status"] in {"published", "drafted"}:
        needs_repair = (
            record["status"] == "published"
            and ledger_path is not None
            and record.get("ledger_status") == "seed_failed"
        )
        if not needs_repair:
            return False
    if record["attempts"] >= max_attempts:
        record["status"] = "abandoned"
        state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")
        return False

    package_dir = work_root / token
    try:
        safe_extract(zip_path, package_dir)
        if record["status"] == "published":
            try:
                repair_failed_feedback_ledger(record, zip_path, package_dir, ledger_path)
            except Exception as repair_error:
                # The platform already accepted this note; never demote it to a retry.
                record["ledger_status"] = "seed_failed"
                record["ledger_repair_error"] = str(repair_error)
            return False
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
        input_errors = publisher_input_errors(package_dir)
        if input_errors:
            raise ValueError("publisher input contract failed: " + "; ".join(input_errors))
        style_reconciliation = reconcile_auto_poster_style(package_dir, ledger_path)
        if style_reconciliation:
            record["poster_style_reconciliation"] = style_reconciliation
        resonance_source_errors = publisher_resonance_source_errors(package_dir, ledger_path)
        if resonance_source_errors:
            raise ValueError("publisher resonance evidence contract failed: " + "; ".join(resonance_source_errors))
        directive_errors = publisher_directive_errors(package_dir)
        if directive_errors:
            raise ValueError("publisher directive contract failed: " + "; ".join(directive_errors))
        gate_errors = recomputed_gate_errors(package_dir)
        if gate_errors:
            raise ValueError("recomputed publication gates failed: " + "; ".join(gate_errors))
        originality_errors = publisher_originality_errors(package_dir, ledger_path)
        if originality_errors:
            raise ValueError("publisher originality contract failed: " + "; ".join(originality_errors))
        learning_errors = publisher_learning_materiality_errors(package_dir, ledger_path)
        if learning_errors:
            raise ValueError("publisher learning evidence contract failed: " + "; ".join(learning_errors))
        poster_style_errors = publisher_poster_style_errors(package_dir, ledger_path)
        if poster_style_errors:
            raise ValueError("publisher poster style evidence contract failed: " + "; ".join(poster_style_errors))
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
        validate_release_evidence(package_dir)
        localize_image_paths(package_dir)
        asset_errors = publisher_asset_errors(package_dir)
        if asset_errors:
            raise ValueError("publisher asset contract failed: " + "; ".join(asset_errors))
        release, title = package_identity(package_dir)
        previous = [item for item in state["packages"].values() if item.get("release") == release and item.get("status") == "published"]
        if previous:
            raise ValueError(f"release already published: {release}")
        if ledger_path and any(
            record.get("release") == release
            for record in load_records(ledger_path)
        ):
            raise ValueError(f"release already exists in publication ledger: {release}")
        record["release"] = release
        record["variant_id"] = selected_variant_id
        record["title"] = title
        if not draft:
            try:
                platform_status = query_status(publisher, title)
            except Exception as exc:
                raise ValueError(f"preflight platform status check failed: {exc}")
            record["preflight_status_result"] = platform_status
            if _status_confirms_note(platform_status):
                record["status"] = "published"
                record["reconciled"] = True
                record["preflight_reconciled"] = True
                record["note_id"] = platform_status.get("noteId")
                record["audit_status"] = platform_status.get("status")
                record["result"] = {
                    "published": True,
                    "reconciled": True,
                    "preflight": True,
                    "noteId": record["note_id"],
                    "url": platform_status.get("url"),
                }
                if ledger_path:
                    try:
                        feedback = seed_feedback_ledger(
                            ledger_path,
                            package_dir,
                            release=release,
                            title=title,
                            publisher_result=record["result"],
                            audit_status=record["audit_status"],
                        )
                        record["ledger_status"] = "seeded"
                        record["ledger_release"] = feedback["release"]
                    except Exception as ledger_error:
                        # The platform has the note; retain the ledger failure without clicking again.
                        record["ledger_status"] = "seed_failed"
                        record["ledger_error"] = str(ledger_error)
                print(json.dumps({
                    "ok": True,
                    "token": token,
                    "status": "published",
                    "action": "preflight-deduplicated",
                    "release": release,
                    "title": title,
                }, ensure_ascii=False))
                return False
        command = publish_command(publisher, package_dir, draft=draft, reuse_edge=reuse_edge)
        completed = subprocess.run(
            command,
            text=True,
            capture_output=True,
            encoding="utf-8",
            env=publisher_environment(),
            timeout=600,
        )
        result: dict[str, Any] = {}
        for line in reversed((completed.stdout or "").splitlines()):
            try:
                parsed = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(parsed, dict):
                result = parsed
                break
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
                try:
                    status = query_status(publisher, title)
                    record["audit_status"] = status.get("status")
                    record["status_result"] = status
                except Exception as status_error:
                    # A zero publisher exit already committed the note; status is supplementary evidence.
                    record["audit_status"] = "unknown"
                    record["status_query_error"] = str(status_error)
                if ledger_path:
                    try:
                        feedback = seed_feedback_ledger(
                            ledger_path,
                            package_dir,
                            release=release,
                            title=title,
                            publisher_result=result,
                            audit_status=record.get("audit_status"),
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
        if not draft:
            try:
                recovery_status = query_status(publisher, title)
                record["failure_status_result"] = recovery_status
                if _status_confirms_note(recovery_status):
                    # The click may have succeeded before a timeout/output failure.
                    # Never retry a note the platform has already accepted.
                    record["status"] = "published"
                    record["reconciled"] = True
                    record["note_id"] = recovery_status.get("noteId")
                    record["audit_status"] = recovery_status.get("status")
                    record["status_result"] = recovery_status
                    record["result"] = {
                        "published": True,
                        "reconciled": True,
                        "noteId": record["note_id"],
                        "url": recovery_status.get("url"),
                    }
                    if ledger_path:
                        feedback = seed_feedback_ledger(
                            ledger_path,
                            package_dir,
                            release=release,
                            title=title,
                            publisher_result=record["result"],
                            audit_status=record["audit_status"],
                        )
                        record["ledger_status"] = "seeded"
                        record["ledger_release"] = feedback["release"]
                    print(json.dumps({
                        "ok": True,
                        "token": token,
                        "status": "published",
                        "reconciled": True,
                        "release": release,
                        "title": title,
                    }, ensure_ascii=False))
                    return True
            except Exception as reconcile_error:
                record["reconcile_error"] = str(reconcile_error)
        record["status"] = "retrying" if record["attempts"] < max_attempts else "failed"
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
    parser.add_argument("--reuse-edge", action="store_true", help="require an already-ready Edge CDP session instead of restarting Edge")
    args = parser.parse_args()
    args.watch_dir.mkdir(parents=True, exist_ok=True)
    args.work_dir.mkdir(parents=True, exist_ok=True)
    reconcile_archived_packages(args.watch_dir, args.state)
    while True:
        zips = sorted(args.watch_dir.glob("*.zip"), key=lambda path: path.stat().st_mtime)
        for zip_path in zips:
            try:
                process_package(
                    zip_path,
                    args.work_dir,
                    args.state,
                    args.publisher,
                    args.max_attempts,
                    args.draft,
                    ledger_path=args.ledger,
                    reuse_edge=args.reuse_edge,
                )
                archive_terminal_package(zip_path, args.state)
            except Exception as exc:
                # A malformed or transiently locked package must not stop other releases.
                print(json.dumps({
                    "ok": False,
                    "zip": str(zip_path),
                    "error": str(exc),
                }, ensure_ascii=False), file=sys.stderr)
        if args.once:
            return 0
        time.sleep(max(5.0, args.interval))


if __name__ == "__main__":
    raise SystemExit(main())
