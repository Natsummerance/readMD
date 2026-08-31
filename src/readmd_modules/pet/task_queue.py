# -*- coding: utf-8 -*-
"""A safe, renderer-free batch queue shared by pet entry points in the future."""

from __future__ import annotations

import os
import uuid
from collections import defaultdict


class PetBatchQueue:
    """Tracks batches without mutating inputs or choosing destructive outputs."""

    _MARKDOWN = {".md", ".markdown", ".mdown", ".mkd", ".mdx", ".txt"}
    _IMAGES = {".png", ".jpg", ".jpeg", ".webp", ".bmp", ".gif", ".tif", ".tiff"}
    _CONVERT = {".doc", ".docx", ".pdf", ".html", ".htm", ".epub", ".rtf", ".odt"}

    def __init__(self):
        self._tasks = []

    def submit(self, paths):
        created = []
        for raw_path in paths or ():
            path = os.path.abspath(os.fspath(raw_path))
            task = {
                "id": uuid.uuid4().hex,
                "source_path": path,
                "kind": self.classify(path),
                "status": "queued",
                "code": None,
                "output_path": None,
            }
            self._tasks.append(task)
            created.append(dict(task))
        return created

    @classmethod
    def classify(cls, path):
        suffix = os.path.splitext(os.fspath(path))[1].lower()
        if suffix in cls._MARKDOWN:
            return "markdown"
        if suffix in cls._IMAGES:
            return "image"
        if suffix in cls._CONVERT:
            return "convert"
        return "unsupported"

    def start(self, task_id):
        task = self._find(task_id)
        if task["status"] != "queued":
            raise ValueError("task_is_not_queued")
        task["status"] = "running"
        return dict(task)

    def complete(self, task_id, output_path=None):
        task = self._find(task_id)
        if task["status"] != "running":
            raise ValueError("task_is_not_running")
        if output_path and self._same_path(task["source_path"], output_path):
            task["status"] = "failed"
            task["code"] = "output_would_overwrite_source"
            return dict(task)
        task["status"] = "succeeded"
        task["output_path"] = os.path.abspath(os.fspath(output_path)) if output_path else None
        return dict(task)

    def fail(self, task_id, code):
        task = self._find(task_id)
        if task["status"] not in {"queued", "running"}:
            raise ValueError("task_is_not_active")
        task["status"] = "failed"
        task["code"] = str(code or "task_failed")
        return dict(task)

    def grouped_snapshot(self):
        grouped = defaultdict(list)
        for task in self._tasks:
            grouped[task["kind"]].append(dict(task))
        return dict(grouped)

    def snapshot(self):
        return [dict(task) for task in self._tasks]

    def _find(self, task_id):
        for task in self._tasks:
            if task["id"] == task_id:
                return task
        raise KeyError("unknown_pet_task")

    @staticmethod
    def _same_path(left, right):
        return os.path.normcase(os.path.realpath(left)) == os.path.normcase(os.path.realpath(os.fspath(right)))
