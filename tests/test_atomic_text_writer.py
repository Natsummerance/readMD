# -*- coding: utf-8 -*-
"""Regression tests for safe editor persistence."""

from pathlib import Path
from unittest.mock import patch
import os
import tempfile
import unittest

from src.readmd_core.file_writer import save_text_atomic
from readmd import Api


class SaveTextAtomicTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="readmd-writer-"))

    def tearDown(self):
        for child in sorted(self.root.rglob("*"), reverse=True):
            if child.is_file() or child.is_symlink():
                child.unlink()
            elif child.is_dir():
                child.rmdir()
        self.root.rmdir()

    def test_creates_parent_and_writes_exact_bytes(self):
        target = self.root / "nested" / "note.md"
        result = save_text_atomic(target, "# first\r\nvalue", "utf-8")
        self.assertTrue(result["ok"])
        self.assertEqual(target.read_bytes(), b"# first\r\nvalue")
        self.assertIsNone(result["backup"])
        self.assertGreater(result["mtime"], 0)

    def test_first_save_creates_single_backup(self):
        target = self.root / "note.md"
        target.write_text("old", encoding="utf-8")
        first = save_text_atomic(target, "new", "utf-8")
        second = save_text_atomic(target, "newer", "utf-8")
        self.assertEqual(first["backup"], str(target) + ".bak")
        self.assertIsNone(second["backup"])
        self.assertEqual((Path(first["backup"])).read_text(encoding="utf-8"), "old")
        self.assertEqual(target.read_text(encoding="utf-8"), "newer")

    def test_matching_mtime_is_accepted(self):
        target = self.root / "note.md"
        target.write_text("old", encoding="utf-8")
        mtime = os.stat(target).st_mtime
        result = save_text_atomic(target, "new", "utf-8", expected_mtime=mtime)
        self.assertTrue(result["ok"])
        self.assertEqual(target.read_text(encoding="utf-8"), "new")

    def test_external_modification_is_conflict(self):
        target = self.root / "note.md"
        target.write_text("external", encoding="utf-8")
        result = save_text_atomic(target, "new", "utf-8", expected_mtime=1.0)
        self.assertFalse(result["ok"])
        self.assertTrue(result["conflict"])
        self.assertEqual(target.read_text(encoding="utf-8"), "external")

    def test_missing_expected_file_is_conflict(self):
        target = self.root / "gone.md"
        result = save_text_atomic(target, "recreated", "utf-8", expected_mtime=1.0)
        self.assertFalse(result["ok"])
        self.assertTrue(result["conflict"])
        self.assertIsNone(result["current_mtime"])
        self.assertFalse(target.exists())

    def test_failed_replace_leaves_original_and_no_temporary_files(self):
        target = self.root / "note.md"
        target.write_text("original", encoding="utf-8")
        with patch("src.readmd_core.file_writer.os.replace", side_effect=OSError("disk")):
            result = save_text_atomic(target, "replacement", "utf-8")
        self.assertFalse(result["ok"])
        self.assertNotIn("conflict", result)
        self.assertEqual(target.read_text(encoding="utf-8"), "original")
        self.assertEqual(list(self.parent_temp_files()), [])

    def parent_temp_files(self):
        return [p for p in self.root.iterdir() if p.name.startswith(".note.md.")]


class DesktopSaveBridgeTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="readmd-bridge-"))
        self.target = self.root / "note.md"
        self.target.write_text("external", encoding="utf-8")

    def tearDown(self):
        for child in sorted(self.root.rglob("*"), reverse=True):
            if child.is_file():
                child.unlink()
        self.root.rmdir()

    def test_save_file_reports_external_modification(self):
        result = Api().save_file(str(self.target), "new", "utf-8", 1.0)
        self.assertFalse(result["ok"])
        self.assertTrue(result["conflict"])
        self.assertEqual(self.target.read_text(encoding="utf-8"), "external")


if __name__ == "__main__":
    unittest.main()
