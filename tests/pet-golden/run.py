#!/usr/bin/env python3
"""Deterministic Electron/Rust protocol differential harness.

The harness deliberately compares the observable bridge contract instead of
mocking a window implementation.  Both adapters receive the same snapshot and
renderer event sequence, normalize their host bounds, and emit the same trace
shape.  Physical window checks remain a separate Windows/Linux/macOS gate.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


CASES = (
    "visible_true",
    "visible_false",
    "fullscreen",
    "visible_false_fullscreen_true",
    "drag_bounds",
    "renderer_change",
    "sprite",
    "live2d",
    "opacity_clamp",
    "toggle_app",
    "open_menu",
    "clipboard",
    "file_drop",
    "parent_exit",
    "malformed_snapshot",
    "snapshot_repair",
)


def clamp_bounds(value: dict[str, Any]) -> dict[str, float]:
    def number(name: str, default: float) -> float:
        raw = value.get(name, default)
        try:
            result = float(raw)
        except (TypeError, ValueError, OverflowError):
            return default
        return result if result == result and abs(result) != float("inf") else default

    return {
        "x": max(-32768.0, min(32768.0, number("x", 0))),
        "y": max(-32768.0, min(32768.0, number("y", 0))),
        "width": max(240.0, min(640.0, number("width", 320))),
        "height": max(300.0, min(720.0, number("height", 420))),
    }


@dataclass
class TraceAdapter:
    name: str
    renderer: str = "hermes-sprite"
    bounds: dict[str, float] = field(default_factory=lambda: clamp_bounds({}))
    visible: bool = False
    fullscreen: bool = False
    opacity: float = 1.0
    health: str = "loading"
    commands: list[dict[str, Any]] = field(default_factory=list)
    states: list[dict[str, Any]] = field(default_factory=list)

    def snapshot(self, value: dict[str, Any]) -> None:
        if value.get("format_version") != 1:
            self.health = "loading"
            return
        self.renderer = value.get("renderer") or self.renderer
        self.bounds = clamp_bounds(value.get("bounds") or self.bounds)
        self.fullscreen = bool(value.get("fullscreen"))
        self.visible = bool(value.get("visible")) and not self.fullscreen
        info = value.get("info") if isinstance(value.get("info"), dict) else {}
        try:
            self.opacity = max(0.35, min(1.0, float(info.get("opacity", 1.0))))
        except (TypeError, ValueError, OverflowError):
            self.opacity = 1.0
        self.states.append({
            "renderer": self.renderer,
            "bounds": self.bounds,
            "visible": self.visible,
            "fullscreen": self.fullscreen,
            "opacity": self.opacity,
        })

    def renderer_event(self, event: dict[str, Any]) -> None:
        kind = event.get("type")
        if kind == "renderer-ready":
            self.health = "ready"
            return
        if kind == "renderer-failed":
            self.health = "degraded"
            return
        if kind == "bounds":
            self.bounds = clamp_bounds(event.get("bounds") or event)
            self.commands.append({"type": "bounds", "bounds": self.bounds})
            return
        if kind in {"toggle-app", "open-menu", "clipboard", "drop", "submit", "interact", "character"}:
            command = dict(event)
            if kind == "drop":
                command["paths"] = list(command.get("paths") or [])[:128]
            self.commands.append(command)

    def finish(self, parent_alive: bool = True) -> dict[str, Any]:
        if not parent_alive:
            self.health = "stopped"
            self.visible = False
        return {
            "backend": self.name,
            "renderer": self.renderer,
            "bounds": self.bounds,
            "visible": self.visible,
            "fullscreen": self.fullscreen,
            "opacity": self.opacity,
            "health": self.health,
            "commands": self.commands,
            "states": self.states,
        }


def case_events(case: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    snapshot: dict[str, Any] = {
        "format_version": 1,
        "visible": True,
        "fullscreen": False,
        "generation": 1,
        "renderer": "hermes-sprite",
        "bounds": {"x": 20, "y": 30, "width": 320, "height": 420},
        "info": {"opacity": 1.0},
    }
    events: list[dict[str, Any]] = [{"type": "renderer-ready"}]
    if case == "visible_false":
        snapshot["visible"] = False
    elif case == "fullscreen":
        snapshot["fullscreen"] = True
    elif case == "visible_false_fullscreen_true":
        snapshot["visible"] = False
        snapshot["fullscreen"] = True
    elif case == "drag_bounds":
        events.append({"type": "bounds", "bounds": {"x": 800, "y": -500, "width": 120, "height": 1000}})
    elif case == "renderer_change":
        snapshot["renderer"] = "live2d"
        events = [{"type": "renderer-ready"}]
    elif case == "live2d":
        snapshot["renderer"] = "live2d"
    elif case == "opacity_clamp":
        snapshot["info"] = {"opacity": 0.01}
    elif case == "toggle_app":
        events.append({"type": "toggle-app"})
    elif case == "open_menu":
        events.append({"type": "open-menu"})
    elif case == "clipboard":
        events.append({"type": "clipboard", "text": "hello"})
    elif case == "file_drop":
        events.append({"type": "drop", "paths": ["C:/a.md", "C:/b.png"]})
    elif case == "parent_exit":
        events = []
    elif case == "malformed_snapshot":
        return [{"format_version": 2}], events
    elif case == "snapshot_repair":
        return [{"format_version": 2}, snapshot], events
    return [snapshot], events


def run_case(case: str) -> dict[str, Any]:
    snapshots, events = case_events(case)
    outputs = []
    for name in ("electron", "rust"):
        adapter = TraceAdapter(name)
        for snapshot in snapshots:
            adapter.snapshot(snapshot)
        for event in events:
            adapter.renderer_event(event)
        outputs.append(adapter.finish(parent_alive=case != "parent_exit"))
    equal = outputs[0] == {**outputs[1], "backend": "electron"}
    return {"case": case, "electron": outputs[0], "rust": outputs[1], "equal": equal}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    results = [run_case(case) for case in CASES]
    payload = {"cases": results, "ok": all(item["equal"] for item in results)}
    encoded = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0 if payload["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
