# -*- coding: utf-8 -*-
"""Main-branch builds must refresh beta assets without creating releases."""

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
        self.assertEqual(trigger["workflows"], ["Test, package and release ReadMD"])
        job_if = self.workflow["jobs"]["update-beta-release"]["if"]
        for condition in ("success", "head_branch == 'main'", "head_repository.full_name == github.repository"):
            self.assertIn(condition, job_if)

    def test_updates_existing_release_without_creating_one(self):
        raw = self.raw
        self.assertIn("gh release view", raw)
        self.assertIn("export tag", raw)
        self.assertIn("gh release upload \"$tag\" --clobber", raw)
        self.assertNotIn("gh release create", raw)
        self.assertIn("gh release edit \"$tag\" --notes-file release/release_notes.md", raw)
        self.assertIn("releases/tags/{tag}", raw)
        self.assertIn("--method\", \"DELETE\"", raw.replace("['", '["'))

    def test_downloads_exact_build_and_rebuilds_checksums(self):
        steps = self.workflow["jobs"]["update-beta-release"]["steps"]
        download = next(
            step for step in steps
            if step.get("uses") == "actions/download-artifact@v4"
        )
        self.assertEqual(download["with"]["pattern"], "rc-*")
        self.assertEqual(download["with"]["run-id"], "${{ github.event.workflow_run.id }}")
        self.assertTrue(download["with"]["merge-multiple"])
        self.assertIn("SHA256SUMS.txt", raw := self.raw)
        self.assertIn("find . -maxdepth 1 -type f ! -name 'SHA256SUMS.txt'", raw)


if __name__ == "__main__":
    unittest.main()
