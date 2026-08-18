# -*- coding: utf-8 -*-
"""Focused regression tests for the recovery-safe Windows installer."""

import json
import os
import shutil
import tempfile
import unittest
from collections import namedtuple
from unittest import mock

from installer import setup_app as setup


class InstallerPreflightTests(unittest.TestCase):
    def test_invalid_dir_and_running_process_have_stable_codes(self):
        self.assertEqual(setup.preflight_install('', write_probe=False)['code'], 'invalid_dir')
        with tempfile.TemporaryDirectory() as parent, mock.patch.object(setup, '_bundle_size', return_value=1):
            result = setup.preflight_install(os.path.join(parent, 'ReadMD'), running_check=lambda: True,
                                             write_probe=False)
        self.assertEqual(result['code'], 'file_in_use')
        self.assertIn('close_app_retry', result['actions'])

    def test_permission_and_no_space_are_reported_before_install(self):
        with tempfile.TemporaryDirectory() as parent, mock.patch.object(setup, '_bundle_size', return_value=100):
            path = os.path.join(parent, 'ReadMD')
            Usage = namedtuple('Usage', 'total used free')
            no_space = setup.preflight_install(path, disk_usage=lambda _: Usage(0, 0, 1), write_probe=False)
            self.assertEqual(no_space['code'], 'no_space')
            protected = mock.patch.object(setup, '_protected_install_path', return_value=True)
            with protected:
                denied = setup.preflight_install(path, admin_check=lambda: False, write_probe=False)
            self.assertEqual(denied['code'], 'requires_admin')
            with mock.patch.object(setup.os, 'makedirs', side_effect=PermissionError('denied')):
                denied = setup.preflight_install(path, write_probe=False)
            self.assertEqual(denied['code'], 'permission_denied')


class InstallerSwapTests(unittest.TestCase):
    def _tree(self, parent, name, content):
        path = os.path.join(parent, name)
        os.mkdir(path)
        with open(os.path.join(path, setup.APP_EXE), 'w', encoding='utf-8') as f:
            f.write(content)
        with open(os.path.join(path, 'install.json'), 'w', encoding='utf-8') as f:
            json.dump({'version': content}, f)
        return path

    def test_staged_install_replaces_only_after_validation(self):
        with tempfile.TemporaryDirectory() as parent:
            target = self._tree(parent, 'ReadMD', 'old')
            stage = self._tree(parent, '.readmd.staging-test', 'new')
            setup._commit_staged_install(stage, target)
            with open(os.path.join(target, setup.APP_EXE), encoding='utf-8') as f:
                self.assertEqual(f.read(), 'new')
            self.assertFalse(os.path.exists(stage))
            self.assertEqual([], [p for p in os.listdir(parent) if '.readmd.backup-' in p])

    def test_do_install_uses_a_sibling_staging_directory(self):
        with tempfile.TemporaryDirectory() as parent:
            source = self._tree(parent, 'source', 'new')
            target = self._tree(parent, 'ReadMD', 'old')
            progress = []
            ok = {'ok': True, 'code': 'ok', 'path': target, 'message': '', 'actions': []}
            with mock.patch.object(setup, 'preflight_install', return_value=ok), \
                 mock.patch.object(setup, 'bundled_app_dir', return_value=source), \
                 mock.patch.object(setup, 'bundled_exe', return_value=None), \
                 mock.patch.object(setup, 'app_running', return_value=False), \
                 mock.patch.object(setup, 'write_uninstall_entry'):
                setup.do_install({'dir': target, 'assoc': False, 'desktop': False, 'startmenu': False},
                                 lambda *event: progress.append(event))
            with open(os.path.join(target, setup.APP_EXE), encoding='utf-8') as f:
                self.assertEqual(f.read(), 'new')
            self.assertEqual('done', progress[-1][1])
            self.assertFalse(any('.readmd.staging-' in item for item in os.listdir(parent)))

    def test_failed_switch_restores_old_install(self):
        with tempfile.TemporaryDirectory() as parent:
            target = self._tree(parent, 'ReadMD', 'old')
            stage = self._tree(parent, '.readmd.staging-test', 'new')
            actual_replace = os.replace
            def fail_new_switch(src, dst):
                if os.path.abspath(src) == os.path.abspath(stage):
                    raise PermissionError('locked')
                return actual_replace(src, dst)
            with mock.patch.object(setup.os, 'replace', side_effect=fail_new_switch):
                with self.assertRaises(setup.InstallError) as raised:
                    setup._commit_staged_install(stage, target)
            self.assertEqual(raised.exception.code, 'file_in_use')
            with open(os.path.join(target, setup.APP_EXE), encoding='utf-8') as f:
                self.assertEqual(f.read(), 'old')
            shutil.rmtree(stage)


class ElevationPayloadTests(unittest.TestCase):
    def test_payload_is_one_shot_and_token_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            path, token = setup.create_elevation_payload({'dir': os.path.join(directory, 'ReadMD')}, temp_dir=directory, now=100)
            opts = setup.consume_elevation_payload(path, token, temp_dir=directory, now=101,
                                                   owner_check=lambda _: True)
            self.assertEqual(opts['dir'], os.path.abspath(os.path.join(directory, 'ReadMD')))
            self.assertFalse(os.path.exists(path))
            with self.assertRaises(setup.InstallError):
                setup.consume_elevation_payload(path, token, temp_dir=directory, now=101,
                                                owner_check=lambda _: True)

    def test_tampered_or_expired_payload_is_rejected_and_deleted(self):
        with tempfile.TemporaryDirectory() as directory:
            path, token = setup.create_elevation_payload({'dir': os.path.join(directory, 'ReadMD')}, temp_dir=directory, now=100)
            with open(path, 'r+', encoding='utf-8') as f:
                data = json.load(f); data['schema'] = 999; f.seek(0); json.dump(data, f); f.truncate()
            with self.assertRaises(setup.InstallError) as raised:
                setup.consume_elevation_payload(path, token, temp_dir=directory, now=101, owner_check=lambda _: True)
            self.assertEqual(raised.exception.code, 'invalid_elevation_payload')
            self.assertFalse(os.path.exists(path))
            path, token = setup.create_elevation_payload({'dir': os.path.join(directory, 'ReadMD')}, temp_dir=directory, now=100)
            with self.assertRaises(setup.InstallError):
                setup.consume_elevation_payload(path, token, temp_dir=directory, now=100 + setup.ELEVATION_TTL_SECONDS + 1,
                                                owner_check=lambda _: True)

    def test_uac_request_can_be_mocked_without_leaking_a_failed_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            created = []
            def writer(options):
                path, token = setup.create_elevation_payload(options, temp_dir=directory)
                created.append(path)
                return path, token
            result = setup.request_elevation({'dir': os.path.join(directory, 'ReadMD')},
                                             payload_writer=writer, shell_execute=lambda *args: 5)
            self.assertEqual(result['code'], 'elevation_failed')
            self.assertFalse(os.path.exists(created[0]))


class InstallerHtmlTests(unittest.TestCase):
    def test_recovery_controls_and_accessibility_are_present(self):
        with open(os.path.join(os.path.dirname(__file__), 'installer', 'setup.html'), encoding='utf-8') as f:
            html = f.read()
        for required in ('preflight_install', 'btn-err-close-retry', 'btn-err-admin', 'btn-err-change',
                         'aria-live="polite"', 'prefers-reduced-motion', '默认用户目录无需管理员'):
            self.assertIn(required, html)


if __name__ == '__main__':
    unittest.main()
