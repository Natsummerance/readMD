import sys
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
sys.path.insert(0, ROOT)

import readmd
import unittest
from unittest import mock
import json




class TestUpgradeCheck(unittest.TestCase):

    def test_parse_version(self):
        self.assertEqual(readmd._parse_version('v2.2.6'), ((2, 2, 6), 1, ()))
        self.assertEqual(readmd._parse_version('2.10.0'), ((2, 10, 0), 1, ()))
        self.assertEqual(readmd._parse_version('2.2.6-rc1'), ((2, 2, 6), 0, ((1, 'rc1'),)))
        self.assertIsNone(readmd._parse_version(''))
        self.assertIsNone(readmd._parse_version('latest'))

    def test_newer_release_wins(self):
        readmd._UPGRADE_CACHE['done'] = False
        readmd._UPGRADE_CACHE['result'] = None
        with mock.patch.object(readmd, 'VERSION', '2.2.5'):
            with mock.patch(
                    'urllib.request.urlopen',
                    return_value=mock.MagicMock(
                        __enter__=lambda s: s,
                        __exit__=lambda *a: None,
                        read=lambda n: b'{"tag_name": "v2.2.6", "html_url": "https://github.com/Natsummerance/readMD/releases/tag/v2.2.6"}'),
                    create=True):
                result = readmd.check_latest_release()
        self.assertEqual(result['latest'], 'v2.2.6')
        self.assertEqual(result['url'], 'https://github.com/Natsummerance/readMD/releases/tag/v2.2.6')
        readmd._UPGRADE_CACHE['done'] = False
        readmd._UPGRADE_CACHE['result'] = None

    def test_same_version_is_silent(self):
        readmd._UPGRADE_CACHE['done'] = False
        readmd._UPGRADE_CACHE['result'] = None
        with mock.patch.object(readmd, 'VERSION', '2.2.6'):
            with mock.patch(
                    'urllib.request.urlopen',
                    return_value=mock.MagicMock(
                        __enter__=lambda s: s,
                        __exit__=lambda *a: None,
                        read=lambda n: b'{"tag_name": "v2.2.6", "html_url": "https://github.com/Natsummerance/readMD/releases/tag/v2.2.6"}'),
                    create=True):
                self.assertIsNone(readmd.check_latest_release())
        readmd._UPGRADE_CACHE['done'] = False
        readmd._UPGRADE_CACHE['result'] = None

    def test_failure_is_silent_and_cached(self):
        readmd._UPGRADE_CACHE['done'] = False
        readmd._UPGRADE_CACHE['result'] = None
        with mock.patch('urllib.request.urlopen', side_effect=OSError('offline'), create=True):
            self.assertIsNone(readmd.check_latest_release())
        self.assertTrue(readmd._UPGRADE_CACHE['done'])
        readmd._UPGRADE_CACHE['done'] = False
        readmd._UPGRADE_CACHE['result'] = None

    def test_startup_beta_detects_formal_release(self):
        payload = json.dumps([
            {'tag_name': 'v2.3.7-beta.5', 'draft': False, 'prerelease': True},
            {'tag_name': 'v2.3.7', 'draft': False, 'prerelease': False,
             'html_url': 'https://github.com/Natsummerance/readMD/releases/tag/v2.3.7'},
        ]).encode('utf-8')
        readmd._UPGRADE_CACHE.update(done=False, result=None)
        with mock.patch.object(readmd, 'VERSION', '2.3.7-beta.3'), mock.patch(
                'urllib.request.urlopen',
                return_value=mock.MagicMock(
                    __enter__=lambda s: s,
                    __exit__=lambda *a: None,
                    read=lambda n: payload),
                create=True):
            result = readmd.check_latest_release()
        self.assertEqual(result['latest'], 'v2.3.7')

    def test_startup_formal_build_ignores_beta_release(self):
        payload = json.dumps([
            {'tag_name': 'v2.3.7', 'draft': False, 'prerelease': False},
            {'tag_name': 'v2.3.8-beta.1', 'draft': False, 'prerelease': True},
        ]).encode('utf-8')
        readmd._UPGRADE_CACHE.update(done=False, result=None)
        with mock.patch.object(readmd, 'VERSION', '2.3.7'), mock.patch(
                'urllib.request.urlopen',
                return_value=mock.MagicMock(
                    __enter__=lambda s: s,
                    __exit__=lambda *a: None,
                    read=lambda n: payload),
                create=True):
            self.assertIsNone(readmd.check_latest_release())
        readmd._UPGRADE_CACHE['done'] = False
        readmd._UPGRADE_CACHE['result'] = None


if __name__ == '__main__':
    unittest.main(verbosity=2)
