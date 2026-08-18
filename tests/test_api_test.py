# -*- coding: utf-8 -*-
"""Regression tests for desktop bridge file operations."""

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
import json
import os
import sys
import tempfile
import time
import unittest
from unittest import mock

ROOT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(ROOT, '..'))



class TestRenameFile(unittest.TestCase):
    def _metadata_paths(self, td):
        return {
            'RECENT_FILE': os.path.join(td, 'recent.json'),
            'SETTINGS_FILE': os.path.join(td, 'settings.json'),
            'HISTORY_FILE': os.path.join(td, 'chat_history.json'),
        }

    def test_rename_moves_file_backup_and_exact_metadata_references(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, '旧名.md')
            new = os.path.join(td, '新名.md')
            with open(old, 'w', encoding='utf-8') as handle:
                handle.write('# document')
            with open(old + '.bak', 'w', encoding='utf-8') as handle:
                handle.write('backup')
            paths = self._metadata_paths(td)
            readmd.save_json(paths['RECENT_FILE'], [old, os.path.join(td, 'other.md')])
            readmd.save_json(paths['SETTINGS_FILE'], {'last': old, 'theme': 'dark'})
            readmd.save_json(paths['HISTORY_FILE'], {
                'sessions': [{'id': '1', 'doc': old}, {'id': '2', 'doc': 'unrelated'}]
            })

            with mock.patch.multiple(readmd, **paths):
                result = readmd.Api().rename_file(old, '新名')

            self.assertTrue(result.get('ok'), result)
            self.assertEqual(result.get('path'), new)
            self.assertTrue(os.path.isfile(new))
            self.assertTrue(os.path.isfile(new + '.bak'))
            self.assertFalse(os.path.exists(old))
            self.assertEqual(readmd.load_json(paths['RECENT_FILE'], [])[0], new)
            self.assertEqual(readmd.load_json(paths['SETTINGS_FILE'], {})['last'], new)
            sessions = readmd.load_json(paths['HISTORY_FILE'], {})['sessions']
            self.assertEqual(sessions[0]['doc'], new)
            self.assertEqual(sessions[1]['doc'], 'unrelated')

    def test_rename_rejects_invalid_or_existing_target_without_modifying_source(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, 'old.md')
            existing = os.path.join(td, 'taken.md')
            for path in (old, existing):
                with open(path, 'w', encoding='utf-8') as handle:
                    handle.write(path)
            paths = self._metadata_paths(td)
            with mock.patch.multiple(readmd, **paths):
                invalid = readmd.Api().rename_file(old, '../escape')
                conflict = readmd.Api().rename_file(old, 'taken')
            self.assertFalse(invalid.get('ok'))
            self.assertEqual(invalid.get('code'), 'invalid_name')
            self.assertFalse(conflict.get('ok'))
            self.assertEqual(conflict.get('code'), 'target_exists')
            self.assertTrue(os.path.isfile(old))

    def test_case_only_rename_works_on_case_insensitive_filesystems(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, 'Title.md')
            with open(old, 'w', encoding='utf-8') as handle:
                handle.write('case')
            paths = self._metadata_paths(td)
            with mock.patch.multiple(readmd, **paths):
                result = readmd.Api().rename_file(old, 'title')
            self.assertTrue(result.get('ok'), result)
            self.assertEqual(os.path.basename(result['path']), 'title.md')
            self.assertTrue(os.path.isfile(result['path']))

    def test_case_only_rename_uses_samefile_on_macos_style_filesystem(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, 'Title.md')
            new = os.path.join(td, 'title.md')
            with open(old, 'w', encoding='utf-8') as handle:
                handle.write('case')
            paths = self._metadata_paths(td)
            real_exists = os.path.exists
            with mock.patch.multiple(readmd, **paths), \
                    mock.patch.object(readmd.os.path, 'exists',
                                      side_effect=lambda path: True if path == new else real_exists(path)), \
                    mock.patch.object(readmd.os.path, 'samefile', return_value=True), \
                    mock.patch.object(readmd, '_paths_equal', return_value=False):
                result = readmd.Api().rename_file(old, 'title')
            self.assertTrue(result.get('ok'), result)
            self.assertEqual(result['path'], new)


class TestPrivateWebAuthorization(unittest.TestCase):
    def test_wk_origin_filter_handles_default_ports_and_ipv6(self):
        https_filter = readmd.Api._web_origin_url_filter('https://example.com/docs')
        self.assertRegex('https://example.com/page', https_filter)
        self.assertRegex('https://example.com:443/page', https_filter)
        self.assertNotRegex('https://example.com.evil/page', https_filter)
        self.assertEqual(readmd.Api._web_origin('http://[::1]/docs'),
                         'http://[::1]:80')

    def test_grant_is_task_origin_bound_and_revocable(self):
        api = readmd.Api()
        granted = api.authorize_private_web('http://127.0.0.1:8080/page', 'task-a')
        self.assertTrue(granted.get('ok'), granted)
        token = granted['grant']
        self.assertTrue(api._private_web_allowed(
            'http://127.0.0.1:8080/next', 'task-a', token))
        self.assertFalse(api._private_web_allowed(
            'http://127.0.0.1:8081/next', 'task-a', token))
        self.assertFalse(api._private_web_allowed(
            'http://127.0.0.1:8080/next', 'task-b', token))
        self.assertTrue(api.revoke_private_web('task-a'))
        self.assertFalse(api._private_web_allowed(
            'http://127.0.0.1:8080/next', 'task-a', token))

    def test_expired_private_grant_is_rejected(self):
        api = readmd.Api()
        granted = api.authorize_private_web('http://10.0.0.2/docs', 'task-expired')
        self.assertTrue(granted.get('ok'), granted)
        api._web_private_grants['task-expired']['expires_at'] = time.time() - 1
        self.assertFalse(api._private_web_allowed(
            'http://10.0.0.2/next', 'task-expired', granted['grant']))

    def test_request_guard_blocks_ungranted_private_resources(self):
        api = readmd.Api()
        with mock.patch('src.readmd_modules.web.socket.getaddrinfo') as lookup:
            lookup.return_value = [(2, 1, 6, '', ('127.0.0.1', 0))]
            self.assertFalse(api._web_request_allowed(
                'http://local.invalid/metadata', 'task-a', ''))
            granted = api.authorize_private_web(
                'http://local.invalid/docs', 'task-private')
            self.assertTrue(granted.get('ok'), granted)
            self.assertTrue(api._web_request_allowed(
                'http://local.invalid/image.png', 'task-private', granted['grant']))
            self.assertFalse(api._web_request_allowed(
                'http://other.invalid/image.png', 'task-private', granted['grant']))


class TestClipboardAuthorization(unittest.TestCase):
    def test_clipboard_read_requires_grant_token(self):
        api = readmd.Api()
        self.assertEqual(api.read_clipboard().get('source_type'), 'unauthorized')
        token = api.authorize_clipboard_read()['token']
        with mock.patch.dict('sys.modules', {'tkinter': None}):
            result = api.read_clipboard(token)
        self.assertNotEqual(result.get('source_type'), 'unauthorized')


if __name__ == '__main__':
    unittest.main(verbosity=2)
