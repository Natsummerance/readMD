# -*- coding: utf-8 -*-
"""Security contracts for downloading and applying in-app updates."""

import hashlib
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from src.readmd_modules import updater


class UpdaterSecurityTest(unittest.TestCase):
    def setUp(self):
        self._state = dict(updater._download_state)

    def tearDown(self):
        updater._download_state.clear()
        updater._download_state.update(self._state)

    def test_start_rejects_unverified_or_unofficial_downloads(self):
        official = 'https://github.com/Natsummerance/readMD/releases/download/v1/app.exe'
        sha = hashlib.sha256(b'payload').hexdigest()

        self.assertFalse(updater.start_download_update('http://example.com/app.exe', 'app.exe', sha)[0])
        self.assertFalse(updater.start_download_update(official, 'app.exe', None)[0])
        self.assertFalse(updater.start_download_update(official, '../app.exe', sha)[0])

    def test_official_url_validation_allows_known_mirror_wrapper(self):
        official = 'https://github.com/Natsummerance/readMD/releases/download/v1/ReadMDSetup-v1.exe'
        foreign = 'https://github.com/example/readMD/releases/download/v1/ReadMDSetup-v1.exe'

        self.assertTrue(updater._is_official_release_url(official))
        self.assertFalse(updater._is_official_release_url(foreign))
        self.assertEqual(
            updater.MIRROR_PREFIXES[0] + official,
            'https://ghfast.top/' + official,
        )

    def test_apply_rejects_paths_outside_verified_task(self):
        with tempfile.TemporaryDirectory() as directory:
            trusted = Path(directory) / 'trusted.exe'
            trusted.write_bytes(b'payload')
            digest = hashlib.sha256(trusted.read_bytes()).hexdigest()
            updater._download_state.update({
                'status': 'ready',
                'target_file': str(trusted),
                'expected_sha': digest,
                'verified_sha': digest,
            })

            path, flavor, error = updater._validate_ready_update(
                str(Path(directory) / 'untrusted.exe'), 'win_installer'
            )
            self.assertIsNone(path)
            self.assertIn('只允许应用', error)

    def test_apply_rejects_tampered_verified_file(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / 'ReadMDSetup-v1.exe'
            target.write_bytes(b'payload')
            updater._download_state.update({
                'status': 'ready',
                'target_file': str(target),
                'expected_sha': hashlib.sha256(b'verified').hexdigest(),
                'verified_sha': hashlib.sha256(b'verified').hexdigest(),
            })

            path, flavor, error = updater._validate_ready_update(str(target), 'win_installer')
            self.assertIsNone(path)
            self.assertIn('校验失败', error)

    def test_validate_accepts_only_matching_verified_file_and_flavor(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / 'ReadMDSetup-v1.exe'
            payload = b'verified payload'
            target.write_bytes(payload)
            digest = hashlib.sha256(payload).hexdigest()
            updater._download_state.update({
                'status': 'ready',
                'target_file': str(target),
                'expected_sha': digest,
                'verified_sha': digest,
            })

            with patch.object(updater, 'detect_app_flavor', return_value='win_installer'):
                path, flavor, error = updater._validate_ready_update(str(target), 'win_portable')
                self.assertIsNone(path)
                self.assertIn('平台不匹配', error)

                path, flavor, error = updater._validate_ready_update(str(target), 'win_installer')
                self.assertIsNone(error)
                self.assertEqual(os.path.abspath(path), os.path.abspath(target))
                self.assertEqual(flavor, 'win_installer')
