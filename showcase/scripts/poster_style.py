#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Shared, auditable selection policy for authentic-screenshot poster styles."""
from __future__ import annotations

from pathlib import Path
from typing import Any

from content_memory import load_records
import performance_report


def load_poster_styles() -> list[str]:
    """Keep Python-side selection aligned with the JavaScript token registry."""
    design_dir = Path(__file__).resolve().parents[1] / "design"
    styles = ["evidence-paper"]
    styles.extend(path.stem for path in sorted((design_dir / "styles").glob("*.json")))
    return list(dict.fromkeys(styles))


def resolve_poster_style(
    requested: str | None,
    memory_path: Path | None,
) -> tuple[str, dict[str, Any]]:
    """Resolve explicit, learned, or balanced-exploration poster selection."""
    styles = load_poster_styles()
    if requested not in (None, "auto") and requested not in styles:
        raise ValueError(f"unknown poster style: {requested}")
    if requested != "auto":
        selected = requested or "evidence-paper"
        return selected, {
            "schema_version": 1,
            "mode": "fixed",
            "selected": selected,
        }

    records = load_records(memory_path) if memory_path else []
    recommendation = performance_report.recommend_poster_style(records)
    if recommendation["recommended"]:
        return str(recommendation["recommended"]), {
            "schema_version": 1,
            "mode": "learned",
            "selected": recommendation["recommended"],
            "recommendation": recommendation,
        }

    # Before any style reaches the publication-confidence threshold, collect
    # comparable evidence by rotating experimental styles around the stable default.
    candidates = styles[1:]
    usage = {
        style: sum(
            1 for record in records if str(record.get("poster_style", "")) == style
        )
        for style in candidates
    }
    fewest = min(usage.values())
    tied = [style for style in candidates if usage[style] == fewest]
    selected = tied[len(records) % len(tied)]
    return selected, {
        "schema_version": 1,
        "mode": "exploration",
        "selected": selected,
        "usage": usage,
        "recommendation": recommendation,
    }
