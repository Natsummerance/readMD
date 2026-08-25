# -*- coding: utf-8 -*-
"""Contracts for launching local files and external links."""

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from src.readmd_core.safe_open import safe_external_url, safe_file_target


class SafeOpenTest(unittest.TestCase):
    def test_allows_existing_document_and_media_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ('notes.md', 'manual.pdf', 'diagram.png'):
                path = root / name
                path.write_bytes(b'x')
                self.assertEqual(os.path.abspath(path), safe_file_target(path))

    def test_rejects_missing_or_executable_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertRaises(ValueError, safe_file_target, root / 'missing.md')
            for name in ('payload.exe', 'script.bat', 'shortcut.lnk', 'page.hta'):
                path = root / name
                path.write_bytes(b'x')
                self.assertRaises(ValueError, safe_file_target, path)

    def test_rejects_non_regular_and_trailing_character_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            nested = root / 'folder.md'
            nested.mkdir()
            self.assertRaises(ValueError, safe_file_target, nested)
            self.assertRaises(ValueError, safe_file_target, root / 'document.pdf.')

    def test_allows_web_and_simple_mailto_links(self):
        self.assertEqual(
            safe_external_url('https://github.com/Natsummerance/readMD'),
            'https://github.com/Natsummerance/readMD',
        )
        self.assertEqual(
            safe_external_url('mailto:support@example.com?subject=ReadMD'),
            'mailto:support@example.com?subject=ReadMD',
        )

    def test_blocks_unsafe_url_schemes(self):
        for url in (
            'javascript:alert(1)',
            'file:///C:/Windows/System32/calc.exe',
            'data:text/html;base64,PHNjcmlwdD4=',
            'mailto:victim@example.com\nBcc:attacker@example.com',
        ):
            with self.assertRaises(ValueError):
                safe_external_url(url)
