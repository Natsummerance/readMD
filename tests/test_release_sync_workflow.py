# -*- coding: utf-8 -*-
"""Workflow contract for staged main-branch release synchronization."""

import unittest
import yaml
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = ROOT / ".github/workflows/release-sync.yml"


class ReleaseSyncWorkflowTest(unittest.TestCase):
    def setUp(self):
        self.workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
        self.raw = WORKFLOW_PATH.read_text(encoding="utf-8")

    def test_only_follows_successful_same_repo_main_builds(self):
        trigger = self.workflow[True]["workflow_run"]
        job_if = self.workflow["jobs"]["update-beta-release"]["if"]
        self.assertEqual(trigger["workflows"], ["Test, package and release ReadMD"])
        for condition in (
            "success",
            "head_branch == 'main'",
            "head_repository.full_name == github.repository",
        ):
            self.assertIn(condition, job_if)

    def test_serializes_all_release_syncs(self):
        concurrency = self.workflow["concurrency"]
        self.assertEqual(concurrency["group"], "release-sync")
        self.assertFalse(concurrency["cancel-in-progress"])

    def test_uses_staged_sync_script_without_creating_release(self):
        raw = self.raw
        self.assertIn("python tools/release_asset_sync.py", raw)
        self.assertIn("--assets-dir release-assets", raw)
        self.assertIn('--commit "${{ github.event.workflow_run.head_sha }}"', raw)
        self.assertIn("gh release edit \"$tag\" --notes-file release/release_notes.md", raw)
        self.assertNotIn("gh release create", raw)
        self.assertNotIn("gh release upload", raw)
