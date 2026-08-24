#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Hard QA gate for an authentic ReadMD Xiaohongshu content package."""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image

from content_memory import learning_fingerprint, load_records
from copy_profiles import (
    COMMENT_SCENARIOS,
    COMMENT_SHOT_FOCUS,
    MECHANISM_TOPIC_SETS,
    TITLE_FORMULA_CONTRACTS,
    RESONANCE_CONCERN_RESPONSE,
    resonance_frame_adjustment,
    resonance_topic_adjustment,
    resonance_title_adjustment,
    SUPPORT_PHRASES,
)
from write_copy import build_resonance_directive


BANNED = ("公众号", "微信", "闲鱼", "咸鱼", "转卖", "出票", "转让", "售票", "二维码", "淘口令", "淘宝")
IMAGE_RE = re.compile(r"^xhs-(0[1-9])-[a-z0-9-]+\.jpg$")


def topic_set_id(topics: list[Any]) -> str:
    clean_topics = [str(item).strip() for item in topics if str(item).strip()]
    payload = "\n".join(clean_topics)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()[:12]


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _topic_identity_errors(story: dict[str, Any] | None, metadata: dict[str, Any]) -> list[str]:
    """Recheck mechanism-bound topic identity independently of generated QA reports."""
    story_primary = str(story.get("primary_shot", "")).strip() if isinstance(story, dict) else ""
    metadata_primary = str(metadata.get("primary_shot", "")).strip()
    if metadata_primary != story_primary:
        return ["metadata.primary_shot differs from story.primary_shot"]

    topics = metadata.get("topics", [])
    normalized_topics = [str(item).strip() for item in topics] if isinstance(topics, list) else []
    expected_topic_set_id = topic_set_id(normalized_topics)
    approved_sets = MECHANISM_TOPIC_SETS.get(story_primary, [])
    approved_topic_set = next(
        (
            item
            for item in approved_sets
            if [str(topic).strip() for topic in item["topics"]] == normalized_topics
        ),
        None,
    )
    errors: list[str] = []
    if metadata.get("topic_set_id") != expected_topic_set_id:
        errors.append("topic_set_id does not match approved topic set")
    if approved_topic_set is None:
        errors.append("topics are not an approved experiment set for the release mechanism")
    elif metadata.get("topic_set_label") != approved_topic_set["label"]:
        errors.append("topic_set_label does not match approved topics")
    return errors


def _resolve_evidence(path_value: str, package_dir: Path, repo_root: Path) -> Path | None:
    for base in (repo_root, package_dir):
        candidate = (base / path_value).resolve()
        if candidate.exists():
            return candidate
    return None


def _topic_focus_selection_errors(story: dict[str, Any], metadata: dict[str, Any]) -> list[str]:
    """Recheck that the persisted topic experiment executed the comment directive."""
    selection = metadata.get("topic_set_selection")
    if not isinstance(selection, dict) or "resonance_topic_bonuses" not in selection:
        return []

    primary = str(story.get("primary_shot", ""))
    normalized_topics = [str(item).strip() for item in metadata.get("topics", [])]
    candidates = MECHANISM_TOPIC_SETS.get(primary, [])
    candidate = next((item for item in candidates if [str(topic).strip() for topic in item["topics"]] == normalized_topics), None)
    if candidate is None:
        return []

    errors: list[str] = []
    directive = metadata.get("resonance_directive")
    directive = directive if isinstance(directive, dict) else None
    bonuses = selection.get("resonance_topic_bonuses")
    scores = selection.get("scores")
    if not isinstance(bonuses, dict) or not isinstance(scores, dict):
        errors.append("topic selection experiment report is incomplete")
        return errors

    scores_complete = True
    scored_items: list[tuple[float, str]] = []
    for item in candidates:
        set_id = topic_set_id(item["topics"])
        expected_bonus, expected_reason = resonance_topic_adjustment(
            directive,
            topics=item["topics"],
        )
        try:
            reported_bonus = float(bonuses[set_id])
            reported_score = float(scores[set_id])
        except (KeyError, TypeError, ValueError):
            errors.append(f"topic selection experiment report is incomplete: {set_id}")
            scores_complete = False
            continue
        if abs(reported_bonus - expected_bonus) > 0.001:
            errors.append(
                "topic selection resonance bonus differs from directive: "
                f"report={reported_bonus}, expected={expected_bonus}"
            )
        reason = str(selection.get("reasons", {}).get(set_id, ""))
        if expected_reason and expected_reason not in reason:
            errors.append("topic selection omits comment-focus alignment reason")
        scored_items.append((reported_score, set_id))

    set_id = str(metadata.get("topic_set_id", ""))
    if scores_complete and scored_items:
        best_score = max(score for score, _ in scored_items)
        try:
            selected_score = float(scores[set_id])
        except (KeyError, TypeError, ValueError):
            selected_score = float("-inf")
        if selected_score + 0.001 < best_score:
            errors.append(
                "selected topic set is not the highest scoring eligible candidate: "
                f"selected={selected_score}, best={best_score}"
            )

    focus = str((directive or {}).get("evidence", {}).get("focus", "general"))
    if selection.get("resonance_focus") != focus:
        errors.append("topic selection resonance focus differs from directive")
    return errors


def title_provenance_errors(metadata: dict[str, Any]) -> list[str]:
    """Recheck the selected title against its declared source formula."""
    formula_id = str(metadata.get("title_formula_id", "")).strip()
    contract = TITLE_FORMULA_CONTRACTS.get(formula_id)
    if contract is None:
        return []
    errors: list[str] = []
    for field in ("source_template", "adaptation"):
        expected = str(contract.get(field, ""))
        actual = str(metadata.get(f"title_{field}", "")).strip()
        if actual != expected:
            errors.append(
                f"title provenance {field} differs from formula {formula_id}: "
                f"expected {expected}, got {actual or '<missing>'}"
            )
    return errors


def publisher_resonance_source_errors(package_dir: Path, ledger_path: Path | None) -> list[str]:
    """Recompute comment resonance from the local evidence ledger."""
    if ledger_path is None:
        return []
    try:
        metadata = _load_json(package_dir / "metadata.json")
        story = _load_json(package_dir / "story.json")
        records = load_records(ledger_path)
        expected = build_resonance_directive(story, records)
    except Exception as exc:
        return [f"recomputed resonance directive unreadable: {exc}"]

    actual = metadata.get("resonance_directive")
    if not isinstance(actual, dict):
        return ["package omits the recomputed resonance directive"]
    if json.dumps(actual, ensure_ascii=False, sort_keys=True) != json.dumps(
        expected, ensure_ascii=False, sort_keys=True
    ):
        return [
            "resonance directive differs from publication-ledger recomputation"
        ]
    return []


def publisher_learning_snapshot_errors(package_dir: Path, ledger_path: Path | None) -> list[str]:
    """Reject copy selected from a different feedback evidence snapshot."""
    if ledger_path is None:
        return []
    try:
        variants = _load_json(package_dir / "variants.json")
    except Exception as exc:
        return [f"learning snapshot variants unreadable: {exc}"]

    # Legacy packages predate explicit learning provenance and remain compatible.
    snapshot = variants.get("learning_snapshot") if isinstance(variants, dict) else None
    if not isinstance(snapshot, dict):
        return []

    records = load_records(ledger_path)
    expected = {
        "schema_version": 1,
        "record_count": len(records),
        "sha256": learning_fingerprint(records),
    }
    if snapshot != expected:
        return [
            "learning snapshot differs from publication-ledger recomputation"
        ]
    return []


def _image_metrics(path: Path, screenshot_box: dict[str, float] | None = None) -> dict[str, Any]:
    with Image.open(path) as image:
        rgb = image.convert("RGB")
        arr = np.asarray(rgb, dtype=np.int16)
        gray = np.asarray(rgb.convert("L"), dtype=np.int16)
        corners = np.concatenate((arr[:12, :12].reshape(-1, 3), arr[:12, -12:].reshape(-1, 3), arr[-12:, :12].reshape(-1, 3), arr[-12:, -12:].reshape(-1, 3)))
        background = corners.mean(axis=0)
        distance = np.sqrt(np.sum((arr - background) ** 2, axis=2))
        # Dark app UI can sit close to the poster background, so structural edges count as content too.
        gradient_x = np.abs(np.diff(gray, axis=1, prepend=gray[:, :1]))
        gradient_y = np.abs(np.diff(gray, axis=0, prepend=gray[:1]))
        mask = (distance > 40) | (gradient_x > 30) | (gradient_y > 30)
        width, height = rgb.size
        if screenshot_box:
            left = max(0, int(screenshot_box.get("x", 0)))
            top = max(0, int(screenshot_box.get("y", 0)))
            right = min(width, int(left + screenshot_box.get("width", 0)))
            bottom = min(height, int(top + screenshot_box.get("height", 0)))
            if right > left and bottom > top:
                mask[top:bottom, left:right] = True
        row_counts = mask.sum(axis=1)
        col_counts = mask.sum(axis=0)
        meaningful_rows = row_counts >= max(3, width * 0.01)
        blank_runs: list[int] = []
        run = 0
        for meaningful in meaningful_rows:
            if meaningful:
                if run:
                    blank_runs.append(run)
                run = 0
            else:
                run += 1
        if run:
            blank_runs.append(run)
        content_rows = np.flatnonzero(meaningful_rows)
        top_gap = int(content_rows[0]) if len(content_rows) else height
        bottom_gap = int(height - 1 - content_rows[-1]) if len(content_rows) else height
        side_clear = float((mask[:, :8].mean() + mask[:, -8:].mean()) / 2)
        edge_clean = all(band.mean() < 0.02 for band in (mask[:4], mask[-4:], mask[:, :4], mask[:, -4:]))
        return {
            "size": rgb.size,
            "format": image.format,
            "coverage": float(mask.mean()),
            "blank_band": max(blank_runs, default=0),
            "top_gap": top_gap,
            "bottom_gap": bottom_gap,
            "side_clear": side_clear,
            "edge_clean": edge_clean,
        }


def resonance_directive_errors(directive: Any) -> list[str]:
    """Validate the auditable keep/strengthen/compress/delete decision record."""
    if not isinstance(directive, dict):
        return ["resonance directive must be an object"]

    errors: list[str] = []
    if directive.get("schema_version") != 1:
        errors.append("resonance directive schema_version must be 1")
    if not isinstance(directive.get("applied"), bool):
        errors.append("resonance directive applied must be boolean")
    if not isinstance(directive.get("support_available"), bool):
        errors.append("resonance directive support_available must be boolean")

    evidence = directive.get("evidence")
    if not isinstance(evidence, dict):
        errors.append("resonance directive evidence must be an object")
    else:
        if not str(evidence.get("focus", "")).strip():
            errors.append("resonance directive evidence.focus is empty")
        if str(evidence.get("confidence", "")) not in {"low", "medium", "high"}:
            errors.append("resonance directive evidence.confidence is invalid")
        for key in ("release_count", "mentions", "weighted_score"):
            value = evidence.get(key)
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                errors.append(f"resonance directive evidence.{key} must be a non-negative integer")
        intents = evidence.get("top_intents")
        if not isinstance(intents, list) or any(not str(item).strip() for item in intents):
            errors.append("resonance directive evidence.top_intents must be a list of labels")

    applied = directive.get("applied")
    focus = str(evidence.get("focus", "")) if isinstance(evidence, dict) else ""
    confidence = str(evidence.get("confidence", "")) if isinstance(evidence, dict) else ""
    support_available = directive.get("support_available")
    expected_applied = focus != "general" and confidence in {"medium", "high"} and support_available is True
    if isinstance(applied, bool) and applied != expected_applied:
        errors.append("resonance directive applied differs from evidence confidence")

    decisions = directive.get("decisions")
    required_decisions = {"keep", "strengthen", "compress", "delete"}
    if not isinstance(decisions, dict):
        errors.append("resonance directive decisions must be an object")
    else:
        missing = sorted(required_decisions - set(decisions))
        if missing:
            errors.append("resonance directive missing decisions: " + ", ".join(missing))
        if any(not str(decisions.get(key, "")).strip() for key in required_decisions & set(decisions)):
            errors.append("resonance directive decisions contain empty values")

    return errors


def publisher_directive_errors(package_dir: Path) -> list[str]:
    """Validate both the directive record and its execution in publisher copy."""
    try:
        metadata = _load_json(package_dir / "metadata.json")
        story = _load_json(package_dir / "story.json")
        body = (package_dir / "body.txt").read_text(encoding="utf-8").strip()
    except Exception as exc:
        return [f"publisher directive metadata unreadable: {exc}"]
    directive = metadata.get("resonance_directive")
    errors = resonance_directive_errors(directive)
    if not isinstance(directive, dict) or not isinstance(story, dict):
        return errors

    applied = directive.get("applied") is True
    focus = str(directive.get("evidence", {}).get("focus", "general"))
    reader_focus = focus if applied else "general"
    scenario = COMMENT_SCENARIOS.get(reader_focus)
    if scenario and scenario not in body:
        errors.append(f"publisher body omits resonance scenario: {reader_focus}")
    concern_intents = set(directive.get("evidence", {}).get("top_intents", []))
    if applied and "concern" in concern_intents and RESONANCE_CONCERN_RESPONSE not in body:
        errors.append("publisher body omits resonance concern response")

    if not applied:
        return errors

    focus_shot = COMMENT_SHOT_FOCUS.get(focus)
    selected_shots = set(story.get("selected_shots", []))
    if focus_shot not in selected_shots:
        errors.append(f"resonance focus shot is missing from authentic captures: {focus_shot}")
        return errors

    if focus_shot != story.get("primary_shot"):
        focused_phrase = SUPPORT_PHRASES[focus_shot]
        if focused_phrase not in body:
            errors.append(f"publisher body omits focused support: {focused_phrase}")
        else:
            recognized_positions = [
                body.index(phrase)
                for phrase in SUPPORT_PHRASES.values()
                if phrase in body
            ]
            if recognized_positions and min(recognized_positions) != body.index(focused_phrase):
                errors.append("focused support is not the first supporting capability")
    return errors


def variant_selection_integrity_errors(
    metadata: dict[str, Any],
    variants: dict[str, Any],
) -> tuple[list[str], str | None]:
    """Recheck ranking arithmetic, comment alignment, and winner selection."""
    errors: list[str] = []
    chosen_ranked_frame: str | None = None
    if variants.get("ok") is not True or not variants.get("chosen_strategy"):
        errors.append("variant selection report is incomplete")
        return errors, chosen_ranked_frame

    chosen_strategy = variants.get("chosen_strategy")
    ranked_items = variants.get("ranked", [])
    chosen_ranked = None
    has_variant_ids = bool(ranked_items) and all(item.get("variant_id") for item in ranked_items)
    if has_variant_ids and not variants.get("chosen_variant_id"):
        errors.append("variant selection missing chosen_variant_id")
        return errors, chosen_ranked_frame
    if has_variant_ids:
        chosen_ranked = next(
            (item for item in ranked_items if item.get("variant_id") == variants["chosen_variant_id"]),
            None,
        )
    else:
        chosen_ranked = next((item for item in ranked_items if item.get("strategy") == chosen_strategy), None)
    if not chosen_ranked:
        errors.append("variant selection missing chosen ranking")
        return errors, chosen_ranked_frame

    chosen_ranked_frame = str(chosen_ranked.get("copy_frame") or "")
    if chosen_ranked.get("ok") is not True or chosen_ranked.get("originality_failures"):
        errors.append(
            "variant originality gate failed: "
            + "; ".join(map(str, chosen_ranked.get("hard_failures", [])))
        )

    directive = metadata.get("resonance_directive")
    directive = directive if isinstance(directive, dict) else None
    scored_items: list[tuple[float, dict[str, Any]]] = []
    scores_complete = True
    # Legacy fixed-six reports predate score arithmetic and resonance attribution;
    # retain their simpler chosen-item gate.
    modern_ranking = any(
        "adjusted_score" in item or "resonance_frame_bonus" in item
        for item in ranked_items
    )
    for item in ranked_items:
        if not modern_ranking:
            break
        try:
            semantic_score = float(item["semantic_score"])
            history_adjustment = float(item["history_adjustment"])
            reported_bonus = float(item.get("resonance_frame_bonus", 0))
            reported_title_bonus = float(item.get("resonance_title_bonus", 0))
            adjusted_score = float(item["adjusted_score"])
        except (KeyError, TypeError, ValueError):
            errors.append(f"variant ranking score contract is incomplete: {item.get('variant_id', 'legacy')}")
            scores_complete = False
            continue

        expected_bonus, expected_reasons = resonance_frame_adjustment(
            directive,
            copy_frame=str(item.get("copy_frame") or ""),
        )
        if abs(reported_bonus - expected_bonus) > 0.001:
            errors.append(
                "variant resonance frame bonus differs from directive: "
                f"report={reported_bonus}, expected={expected_bonus}"
            )
        if expected_reasons and not all(reason in item.get("reasons", []) for reason in expected_reasons):
            errors.append("variant selection omits comment-intent alignment reason")
        expected_title_bonus, expected_title_reasons = resonance_title_adjustment(
            directive,
            title_formula_id=str(item.get("title_formula_id") or ""),
        )
        if abs(reported_title_bonus - expected_title_bonus) > 0.001:
            errors.append(
                "variant resonance title bonus differs from directive: "
                f"report={reported_title_bonus}, expected={expected_title_bonus}"
            )
        if expected_title_reasons and not all(
            reason in item.get("reasons", []) for reason in expected_title_reasons
        ):
            errors.append("variant selection omits comment-intent title reason")
        projected_total = (
            semantic_score
            + history_adjustment
            + reported_bonus
            + reported_title_bonus
        )
        if abs(adjusted_score - projected_total) > 0.001:
            errors.append(
                "variant adjusted score is inconsistent: "
                f"variant_id={item.get('variant_id', 'legacy')}, "
                f"report={adjusted_score}, expected={projected_total}"
            )
        if item.get("ok") is True and not item.get("originality_failures"):
            scored_items.append((adjusted_score, item))

    if scores_complete and chosen_ranked.get("ok") is True and scored_items:
        best_score = max(score for score, _ in scored_items)
        try:
            chosen_score = float(chosen_ranked["adjusted_score"])
        except (KeyError, TypeError, ValueError):
            chosen_score = float("-inf")
        if chosen_score + 0.001 < best_score:
            errors.append(
                "selected variant is not the highest scoring eligible variant: "
                f"selected={chosen_score}, best={best_score}"
            )

    focus = str((directive or {}).get("evidence", {}).get("focus", ""))
    if focus and variants.get("resonance_focus") != focus:
        errors.append("variant resonance focus differs from directive")
    return errors, chosen_ranked_frame


def publisher_input_errors(package_dir: Path) -> list[str]:
    """Keep the exact strings handed to Xiaohongshu identical to experiment metadata."""
    package_dir = package_dir.resolve()
    try:
        metadata = _load_json(package_dir / "metadata.json")
    except Exception as exc:
        return [f"publisher input metadata unreadable: {exc}"]

    errors: list[str] = []
    try:
        story = _load_json(package_dir / "story.json")
    except Exception as exc:
        errors.append(f"publisher input story unreadable: {exc}")
    else:
        errors.extend(_topic_identity_errors(story, metadata))
        errors.extend(_topic_focus_selection_errors(story, metadata))
        errors.extend(title_provenance_errors(metadata))

    def text(name: str) -> str | None:
        path = package_dir / name
        if not path.is_file():
            errors.append(f"publisher input missing: {name}")
            return None
        try:
            return path.read_text(encoding="utf-8").strip()
        except Exception as exc:
            errors.append(f"publisher input unreadable: {name}: {exc}")
            return None

    title = text("title.txt")
    if title is not None and title != str(metadata.get("title", "")).strip():
        errors.append("title.txt differs from metadata.title")

    body = text("body.txt")
    if body is not None and body != str(metadata.get("body", "")).strip():
        errors.append("body.txt differs from metadata.body")

    topics_text = text("topics.txt")
    expected_topics = [str(item).strip() for item in metadata.get("topics", [])]
    if topics_text is not None:
        actual_topics = [line.strip() for line in topics_text.splitlines() if line.strip()]
        if actual_topics != expected_topics:
            errors.append("topics.txt differs from metadata.topics")

    image_names = [Path(str(item)).name for item in metadata.get("images", [])]
    if len(image_names) != len(set(image_names)):
        errors.append("publisher input image names are not unique")
    for name in image_names:
        image_path = package_dir / "images" / name
        if not image_path.is_file():
            errors.append(f"publisher input image missing: images/{name}")

    return errors


def publisher_asset_errors(package_dir: Path) -> list[str]:
    """Recheck the immutable image evidence chain immediately before publishing."""
    package_dir = package_dir.resolve()
    try:
        story = _load_json(package_dir / "story.json")
        metadata = _load_json(package_dir / "metadata.json")
        capture = _load_json(package_dir / "raw" / "capture.json")
    except Exception as exc:
        return [f"publisher asset manifest unreadable: {exc}"]

    errors: list[str] = []
    images_dir = (package_dir / "images").resolve()
    raw_dir = (package_dir / "raw").resolve()
    plan = story.get("card_plan", [])
    expected_names = [str(item.get("file", "")) for item in plan]
    image_names = [Path(str(item)).name for item in metadata.get("images", [])]

    if not 4 <= len(image_names) <= 9 or len(image_names) != len(set(image_names)):
        errors.append("publisher asset images must contain 4-9 unique paths")
    if image_names != expected_names:
        errors.append("publisher asset image order differs from story.card_plan")

    image_hashes: set[str] = set()
    for name in dict.fromkeys(image_names):
        if not IMAGE_RE.fullmatch(name):
            errors.append(f"publisher asset has an unsafe image name: {name}")
            continue
        path = images_dir / name
        if not path.resolve().is_relative_to(images_dir) or not path.is_file():
            errors.append(f"publisher asset image missing: images/{name}")
            continue
        try:
            with Image.open(path) as image:
                if image.format != "JPEG" or image.size != (1080, 1440):
                    errors.append(
                        f"publisher asset image must be 1080x1440 JPEG: {name}"
                        f" ({image.size}, {image.format})"
                    )
        except Exception as exc:
            errors.append(f"publisher asset image unreadable: {name}: {exc}")
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest in image_hashes:
            errors.append(f"publisher asset image hash is duplicated: {name}")
        image_hashes.add(digest)

    selected = story.get("selected_shots", [])
    capture_items = capture.get("shots", [])
    capture_by_id = {str(item.get("shot_id")): item for item in capture_items}
    if str(capture.get("release")) != str(story.get("release")):
        errors.append("capture release differs from story release")
    if not selected or selected[0] != "overview.reader":
        errors.append("publisher assets must begin with overview.reader")
    if set(selected) != set(capture_by_id) or len(selected) != len(capture_items):
        errors.append("capture shots differ from story.selected_shots")

    raw_hashes: set[str] = set()
    story_shot_by_id = {str(item.get("id")): item for item in story.get("shots", [])}
    for shot_id in dict.fromkeys(selected):
        entry = capture_by_id.get(shot_id, {})
        relative = Path(str(entry.get("file", "")))
        path = package_dir / relative
        if not path.resolve().is_relative_to(raw_dir) or not path.is_file():
            errors.append(f"captured PNG missing: {shot_id}")
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != str(entry.get("sha256", "")):
            errors.append(f"SHA-256 mismatch in capture.json: {shot_id}")
        story_hash = str(story_shot_by_id.get(shot_id, {}).get("sha256", ""))
        if story_hash and digest != story_hash:
            errors.append(f"SHA-256 mismatch in story.json: {shot_id}")
        try:
            with Image.open(path) as image:
                if image.format != "PNG":
                    errors.append(f"captured evidence is not PNG: {shot_id}")
        except Exception as exc:
            errors.append(f"captured evidence unreadable: {shot_id}: {exc}")
            continue
        if digest in raw_hashes:
            errors.append(f"captured evidence hash is duplicated: {shot_id}")
        raw_hashes.add(digest)

    plan_shot_ids = {
        str(item.get("shot_id"))
        for item in plan
        if item.get("shot_id") not in {None, ""}
    }
    unknown_plan_shots = plan_shot_ids - set(selected)
    if unknown_plan_shots:
        errors.append(f"card plan references uncaptured shots: {sorted(unknown_plan_shots)}")
    return errors


def validate_package(package_dir: Path, *, repo_root: Path | None = None) -> list[str]:
    package_dir = package_dir.resolve()
    repo_root = (repo_root or Path(__file__).resolve().parents[2]).resolve()
    errors: list[str] = []

    pattern_audit_path = package_dir / "pattern-audit.json"
    if not pattern_audit_path.exists():
        errors.append("pattern-audit.json is missing")
    else:
        try:
            pattern_audit = _load_json(pattern_audit_path)
            if pattern_audit.get("ok") is not True:
                errors.append(
                    "hot-post pattern gate failed: "
                    + "; ".join(map(str, pattern_audit.get("errors", [])))
                )
        except Exception as exc:
            errors.append(f"pattern-audit.json unreadable: {exc}")

    try:
        story = _load_json(package_dir / "story.json")
        capture = _load_json(package_dir / "raw" / "capture.json")
        metadata = _load_json(package_dir / "metadata.json")
        composition = _load_json(package_dir / "composition.json")
    except Exception as exc:
        errors.append(f"package JSON unreadable: {exc}")
        return errors

    copy_review_path = package_dir / "copy-review.json"
    if copy_review_path.exists():
        try:
            copy_review = _load_json(copy_review_path)
            if copy_review.get("ok") is not True:
                errors.append("semantic alignment gate failed: " + "; ".join(map(str, copy_review.get("hard_failures", []))))
        except Exception as exc:
            errors.append(f"copy-review.json unreadable: {exc}")

    wechat_qa_path = package_dir / "wechat" / "wechat-qa.json"
    if wechat_qa_path.exists():
        try:
            wechat_qa = _load_json(wechat_qa_path)
            if wechat_qa.get("ok") is not True:
                errors.append("WeChat adapter gate failed: " + "; ".join(map(str, wechat_qa.get("errors", []))))
        except Exception as exc:
            errors.append(f"wechat-qa.json unreadable: {exc}")

    variants_path = package_dir / "variants.json"
    variant_report: dict[str, Any] = {}
    chosen_ranked_frame: str | None = None
    if variants_path.exists():
        try:
            variants = _load_json(variants_path)
            variant_report = variants
            integrity_errors, chosen_ranked_frame = variant_selection_integrity_errors(metadata, variants)
            errors.extend(integrity_errors)
        except Exception as exc:
            errors.append(f"variants.json unreadable: {exc}")

    dashboard_qa_path = package_dir / "dashboard-qa.json"
    if dashboard_qa_path.exists():
        try:
            dashboard = _load_json(dashboard_qa_path)
            if dashboard.get("ok") is not True:
                errors.append("review dashboard gate failed: " + "; ".join(map(str, dashboard.get("errors", []))))
        except Exception as exc:
            errors.append(f"dashboard-qa.json unreadable: {exc}")

    if story.get("schema_version") != 1 or capture.get("schema_version") != 1:
        errors.append("story/capture schema_version must be 1")
    release = story.get("release")
    version_state = story.get("version_state")
    if not release or version_state not in {"release", "prerelease"}:
        errors.append("release/version_state invalid")

    capture_by_id = {shot.get("shot_id"): shot for shot in capture.get("shots", [])}
    selected = story.get("selected_shots", [])
    if not selected or selected[0] != "overview.reader":
        errors.append("selected_shots must begin with overview.reader")
    if len(selected) < 2 or len(selected) > 7:
        errors.append("selected raw shots must support a 4-9 card package")
    for shot_id in selected:
        captured = capture_by_id.get(shot_id)
        if not captured:
            errors.append(f"shot missing from capture.json: {shot_id}")
            continue
        source = package_dir / captured.get("file", "")
        if not source.is_file():
            errors.append(f"captured PNG missing: {shot_id}")
            continue
        digest = hashlib.sha256(source.read_bytes()).digest()
        expected = str(captured.get("sha256", ""))
        if digest.hex() != expected:
            errors.append(f"SHA-256 mismatch: {shot_id}")
        story_shot = next((item for item in story.get("shots", []) if item.get("id") == shot_id), {})
        if story_shot.get("sha256") and story_shot["sha256"] != digest.hex():
            errors.append(f"SHA-256 mismatch: {shot_id}")

    claims = story.get("claims", [])
    if not claims:
        errors.append("story has no claims")
    known_shot_ids = set(selected)
    for claim in claims:
        if not claim.get("id") or not claim.get("user_value"):
            errors.append(f"claim incomplete: {claim.get('id', '<missing>')}")
        sources = claim.get("sources", [])
        if not sources:
            errors.append(f"claim has no evidence: {claim.get('id', '<missing>')}")
        for source in sources:
            if _resolve_evidence(source, package_dir, repo_root) is None:
                errors.append(f"evidence does not exist: {source}")
        unknown = set(claim.get("shot_ids", [])) - known_shot_ids
        if unknown:
            errors.append(f"claim references uncaptured shots: {sorted(unknown)}")

    cards = story.get("card_plan", [])
    if not 4 <= len(cards) <= 9:
        errors.append("card_plan must contain 4-9 cards")
    elif cards[1].get("role") != "pure_ui_hero" or cards[1].get("shot_id") != "overview.reader":
        errors.append("card 2 must be the pure UI overview.reader hero")
    names = [card.get("file", "") for card in cards]
    if len(names) != len(set(names)) or any(not IMAGE_RE.match(name) for name in names):
        errors.append("card filenames must be unique semantic xhs-NN-slug.jpg values")
    if composition.get("overflow_errors"):
        errors.append("composition DOM overflow errors: " + "; ".join(map(str, composition["overflow_errors"])))
    design_audit = composition.get("design_audit", {})
    for key in ("contrast_errors", "small_text", "images_failed"):
        if design_audit.get(key):
            errors.append(f"composition design audit {key}: " + "; ".join(map(str, design_audit[key])))

    title = str(metadata.get("title", "")).strip()
    body = str(metadata.get("body", "")).strip()
    topics = metadata.get("topics", [])
    images = metadata.get("images", [])
    if not title or len(title) > 20:
        errors.append("title must be 1-20 characters")
    if not 600 <= len(body) <= 900:
        errors.append(f"body must be 600-900 characters, got {len(body)}")
    if re.search(r"https?://|www\.", body, re.I):
        errors.append("body contains a URL")
    if banned := [word for word in BANNED if word.lower() in (title + body).lower()]:
        errors.append("banned words: " + ", ".join(banned))
    if not isinstance(topics, list) or len(topics) != 5 or any(not str(topic).strip() or topic.startswith("#") for topic in topics):
        errors.append("topics must be five non-empty values without #")
    if not isinstance(images, list) or not 4 <= len(images) <= 9:
        errors.append("metadata.images must contain 4-9 paths")
        images = []
    if metadata.get("version_state") != version_state:
        errors.append("metadata.version_state differs from story")
    if (
        variant_report.get("chosen_variant_id")
        and metadata.get("variant_id")
        and metadata["variant_id"] != variant_report["chosen_variant_id"]
    ):
        errors.append("metadata.variant_id differs from selected variant")
    copy_frames = {
        "metadata": str(metadata.get("copy_frame") or ""),
        "selection": str(variant_report.get("chosen_copy_frame") or ""),
        "ranking": chosen_ranked_frame,
    }
    frames_present = [source for source, value in copy_frames.items() if value]
    if frames_present:
        if len(frames_present) != len(copy_frames):
            missing = ", ".join(sorted(set(copy_frames) - set(frames_present)))
            errors.append(f"selected copy_frame is incomplete; missing {missing}")
        elif len({copy_frames[source] for source in frames_present}) != 1:
            errors.append(
                "selected copy_frame mismatch: "
                + f"metadata={copy_frames['metadata'] or '<missing>'}, "
                + f"selection={copy_frames['selection'] or '<missing>'}, "
                + f"ranking={copy_frames['ranking'] or '<missing>'}"
            )
    errors.extend(publisher_input_errors(package_dir))
    if not metadata.get("source_urls"):
        errors.append("metadata.source_urls is empty")
    if version_state == "prerelease":
        if "正式发布" in body or "正式版" in body:
            errors.append("prerelease uses formal-release wording")
        if not ("预览版" in body or "更新线" in body or "beta" in body.lower()):
            errors.append("prerelease wording missing")
    elif "正式发布" not in body and "正式版" not in body:
        errors.append("release should state formal availability")

    hashes: set[str] = set()
    metrics_by_name: dict[str, dict[str, Any]] = {}
    composition_by_name = {item.get("file"): item for item in composition.get("cards", [])}
    for index, raw_path in enumerate(images):
        path = Path(raw_path)
        expected_name = names[index] if index < len(names) else ""
        if path.name != expected_name:
            errors.append(f"image order/name mismatch at {index + 1}: {path.name} != {expected_name}")
        if not path.is_file():
            errors.append(f"image missing: {path}")
            continue
        try:
            metrics = _image_metrics(path, composition_by_name.get(path.name, {}).get("screenshot_box"))
        except Exception as exc:
            errors.append(f"image unreadable: {path.name}: {exc}")
            continue
        metrics_by_name[path.name] = metrics
        if metrics["size"] != (1080, 1440) or metrics["format"] != "JPEG":
            errors.append(f"image must be 1080x1440 JPEG: {path.name}")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest in hashes:
            errors.append(f"duplicate image hash: {path.name}")
        hashes.add(digest)
        if not metrics["edge_clean"]:
            errors.append(f"image edge is not clean: {path.name}")
        if metrics["coverage"] < 0.22:
            errors.append(f"image coverage below 22%: {path.name} ({metrics['coverage']:.2%})")
        if metrics["blank_band"] > 120:
            errors.append(f"blank band exceeds 120px: {path.name} ({metrics['blank_band']}px)")
        if metrics["top_gap"] > 220:
            errors.append(f"top gap exceeds 220px: {path.name} ({metrics['top_gap']}px)")
        if metrics["bottom_gap"] > 90:
            errors.append(f"bottom gap exceeds 90px: {path.name} ({metrics['bottom_gap']}px)")
        if metrics["side_clear"] >= 0.01:
            errors.append(f"content bleeds into side margin: {path.name}")

    for card in cards:
        name = card.get("file")
        metric = metrics_by_name.get(name)
        minimum = float(card.get("ui_min_ratio", 0))
        composed = composition_by_name.get(name)
        if metric and composed is None:
            errors.append(f"composition ratio missing: {name}")
        elif metric and minimum > 0 and float(composed.get("ui_area_ratio", 0)) + 0.01 < minimum:
            errors.append(f"UI area below contract: {name} ({composed.get('ui_area_ratio', 0):.2%} < {minimum:.2%})")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    parser.add_argument("--repo-root", type=Path)
    args = parser.parse_args()
    errors = validate_package(args.package, repo_root=args.repo_root)
    report = {"ok": not errors, "errors": errors}
    (args.package / "qa.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
