# -*- coding: utf-8 -*-
"""Small, renderer-independent state machine for the optional desktop pet."""

from __future__ import annotations

import time


class PetController:
    """Owns visible state and a strict frame-rate budget, never rendering itself."""

    _EVENT_STATES = {
        "work_started": "busy",
        "work_succeeded": "success",
        "work_failed": "error",
        "idle": "idle",
    }
    _ACTIVE_STATES = {"busy", "success", "error"}

    def __init__(self, enabled: bool = False, reduced_motion: bool = False):
        self._enabled = bool(enabled)
        self._reduced_motion = bool(reduced_motion)
        self._fullscreen = False
        self._state = "idle" if self._enabled else "hidden"
        self._updated_at = time.monotonic()

    def enable(self):
        self._enabled = True
        if not self._fullscreen:
            self._state = "idle"
        self._touch()
        return self.snapshot()

    def disable(self):
        self._enabled = False
        self._state = "hidden"
        self._touch()
        return self.snapshot()

    def set_reduced_motion(self, enabled: bool):
        self._reduced_motion = bool(enabled)
        self._touch()
        return self.snapshot()

    def set_fullscreen(self, active: bool):
        self._fullscreen = bool(active)
        if self._fullscreen:
            self._state = "hidden"
        elif self._enabled:
            self._state = "idle"
        self._touch()
        return self.snapshot()

    def handle_event(self, event: str):
        if event not in self._EVENT_STATES:
            raise ValueError("unknown_pet_event")
        if self._enabled and not self._fullscreen:
            self._state = self._EVENT_STATES[event]
        self._touch()
        return self.snapshot()

    def snapshot(self):
        visible = self._enabled and not self._fullscreen
        animation_enabled = visible and not self._reduced_motion
        if not animation_enabled:
            fps_cap = 0
        elif self._state in self._ACTIVE_STATES:
            fps_cap = 30
        else:
            fps_cap = 6
        return {
            "enabled": self._enabled,
            "visible": visible,
            "state": self._state,
            "animation_enabled": animation_enabled,
            "fps_cap": fps_cap,
            "updated_at": self._updated_at,
        }

    def _touch(self):
        self._updated_at = time.monotonic()
