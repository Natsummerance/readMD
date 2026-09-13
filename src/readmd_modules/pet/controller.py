# -*- coding: utf-8 -*-
"""Thread-safe task state and animation policy, independent of the renderer."""
from __future__ import annotations

import threading
import time


class PetController:
    MAX_TASKS = 256
    _EVENT_STATES = {"work_started": "busy", "work_succeeded": "success",
                     "work_failed": "error", "work_cancelled": "idle", "idle": "idle"}

    def __init__(self, enabled=False, reduced_motion=False, *, clock=time.monotonic):
        self._lock = threading.RLock()
        self._clock = clock
        self._enabled = bool(enabled)
        self._reduced_motion = bool(reduced_motion)
        self._fullscreen = False
        self._quiet = False
        self._tasks = set()
        self._feedback = None
        self._feedback_until = 0.0
        self._updated_at = clock()
        self._revision = 0

    def _set(self, key, value):
        with self._lock:
            if getattr(self, key) != bool(value):
                setattr(self, key, bool(value))
                self._touch()
            return self.snapshot()

    def enable(self):
        return self._set('_enabled', True)

    def disable(self):
        return self._set('_enabled', False)

    def set_reduced_motion(self, enabled):
        return self._set('_reduced_motion', enabled)

    def set_quiet(self, enabled):
        return self._set('_quiet', enabled)

    def set_fullscreen(self, active):
        # Visibility is an overlay on activity; hiding must not erase running jobs.
        return self._set('_fullscreen', active)

    def handle_event(self, event, task_id=None):
        if event not in self._EVENT_STATES:
            raise ValueError('unknown_pet_event')
        key = str(task_id) if task_id is not None else '__legacy__'
        if not key or len(key) > 256:
            raise ValueError('invalid_pet_task_id')
        with self._lock:
            before = (frozenset(self._tasks), self._feedback, self._feedback_until)
            moment = self._clock()
            if event == 'work_started':
                if key not in self._tasks and len(self._tasks) >= self.MAX_TASKS:
                    raise ValueError('pet_task_capacity')
                self._tasks.add(key)
            elif event == 'idle':
                self._tasks.discard('__legacy__')
                self._feedback = None
                self._feedback_until = 0
            else:
                # Identified completion is idempotent, including late duplicate events.
                if task_id is not None and key not in self._tasks:
                    return self.snapshot()
                self._tasks.discard(key)
                if event == 'work_failed':
                    self._feedback, self._feedback_until = 'error', moment + 5
                elif event == 'work_succeeded' and not (self._feedback == 'error' and moment < self._feedback_until):
                    self._feedback, self._feedback_until = 'success', moment + 3
            if before != (frozenset(self._tasks), self._feedback, self._feedback_until):
                self._touch()
            return self.snapshot()

    def snapshot(self):
        with self._lock:
            if self._feedback and self._clock() >= self._feedback_until:
                self._feedback = None
                self._feedback_until = 0
                self._touch()
            visible = self._enabled and not self._fullscreen
            activity = 'error' if self._feedback == 'error' else 'busy' if self._tasks else self._feedback or 'idle'
            animation_enabled = visible and not self._reduced_motion
            fps = 0 if not animation_enabled else 30 if activity != 'idle' else 6
            return {
                'enabled': self._enabled, 'visible': visible,
                'state': activity if visible else 'hidden', 'activity_state': activity,
                'active_tasks': len(self._tasks), 'quiet': self._quiet,
                'animation_enabled': animation_enabled, 'fps_cap': fps,
                'updated_at': self._updated_at, 'revision': self._revision,
            }

    def _touch(self):
        self._updated_at = self._clock()
        self._revision += 1
