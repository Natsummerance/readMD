# -*- coding: utf-8 -*-
"""Shared offline asset-serving contracts."""

import os
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from src.readmd_core.static_assets import STARTUP_SCRIPTS, build_startup_bundle, resolve_asset


class StaticAssetsTest(unittest.TestCase):
    def test_bundle_exactly_matches_ordered_scripts_and_lazy_qr(self):
        expected_chunks = []
        for relative_path in STARTUP_SCRIPTS:
            script_path = ROOT / 'assets' / Path(*relative_path.split('/'))
            expected_chunks.append(script_path.read_bytes())

        bundle = build_startup_bundle(os.fspath(ROOT))
        self.assertEqual(bundle, b'\n;\n'.join(expected_chunks))
        self.assertNotIn(b'qrcode.stringToBytesFuncs', bundle)

        lazy_qrcode = resolve_asset(ROOT, '/assets/vendor/qrcode.min.js')
        self.assertFalse(lazy_qrcode.forbidden)
        self.assertIsNotNone(lazy_qrcode.path)
        self.assertIn(b'qrcode.stringToBytesFuncs', Path(lazy_qrcode.path).read_bytes())

    def test_bundle_preserves_runtime_order(self):
        root = os.fspath(ROOT)
        first_build = build_startup_bundle(root)
        second_build = build_startup_bundle(root)
        self.assertIs(first_build, second_build)
        self.assertIn(b'marked v15.0.12', first_build[:200])
        self.assertIn(b"window.addEventListener('DOMContentLoaded', init)", first_build)
        self.assertGreater(len(first_build), 450_000)

    def test_regular_and_versioned_assets_have_cache_policy(self):
        resolved = resolve_asset(ROOT, '/assets/js/core/state.js')
        self.assertFalse(resolved.forbidden)
        self.assertEqual(Path(resolved.path).name, 'state.js')
        self.assertEqual(resolved.mime, 'application/javascript; charset=utf-8')
        self.assertFalse(resolved.immutable)

        versioned = resolve_asset(ROOT, '/assets/js/core/state.js', {'v': ['test']})
        self.assertTrue(versioned.immutable)

        localized = resolve_asset(ROOT, '/i18n/en.json')
        self.assertTrue(localized.immutable)
        self.assertEqual(localized.mime, 'application/json; charset=utf-8')

    def test_path_traversal_is_rejected(self):
        resolved = resolve_asset(ROOT, '/assets/../../../config/requirements.txt')
        self.assertTrue(resolved.forbidden)
        self.assertIsNone(resolved.path)

    def test_unknown_url_is_not_an_asset(self):
        self.assertIsNone(resolve_asset(ROOT, '/api/file'))
