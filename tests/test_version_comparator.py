# -*- coding: utf-8 -*-
"""Prerelease-aware version ordering for both update surfaces."""

import unittest
from unittest.mock import patch

import readmd
from src.readmd_core.versioning import compare_versions, parse_version
from src.readmd_modules import updater


class VersionComparatorTest(unittest.TestCase):
    def test_semver_prerelease_order(self):
        self.assertEqual(parse_version('v2.3.7'), ((2, 3, 7), 1, ()))
        self.assertEqual(
            parse_version('v2.3.7-beta.4'),
            ((2, 3, 7), 0, ((1, 'beta'), (0, 4))),
        )
        self.assertEqual(compare_versions('v2.3.7', '2.3.7'), 0)
        self.assertLess(compare_versions('v2.3.7-beta.3', 'v2.3.7-beta.4'), 0)
        self.assertLess(compare_versions('v2.3.7-beta.4', 'v2.3.7-rc.1'), 0)
        self.assertLess(compare_versions('v2.3.7-rc.1', 'v2.3.7'), 0)
        self.assertIsNone(compare_versions('not-a-version', 'v2.3.7'))
        self.assertLess(compare_versions('1.0.0-1', '1.0.0-alpha'), 0)

    def test_readmd_startup_check_uses_shared_comparator(self):
        self.assertEqual(readmd._compare_versions('beta-placeholder', 'beta-placeholder'), None)
        self.assertLess(readmd._compare_versions('v2.3.7-beta.3', 'v2.3.7-beta.4'), 0)
        self.assertGreater(readmd._compare_versions('v2.4.0', 'v2.3.7-beta.4'), 0)

    def test_updater_keeps_legacy_core_parser_and_orders_prereleases(self):
        self.assertEqual(updater.parse_semver('v3.0.0-beta'), (3, 0, 0))
        self.assertEqual(updater.parse_semver('invalid'), (0, 0, 0))
        self.assertTrue(updater.is_newer_version('v2.3.7-beta.5', 'v2.3.7-beta.4'))
        self.assertFalse(updater.is_newer_version('v2.3.7-beta.4', 'v2.3.7-beta.4'))

    def test_beta_channel_scans_release_list(self):
        urls = updater._release_check_urls('2.3.7-beta.4')
        self.assertIn(updater.GITHUB_API_RELEASES, urls)
        stable_urls = updater._release_check_urls('2.3.7')
        self.assertIn(updater.GITHUB_API_LATEST, stable_urls)

    def test_check_update_selects_newest_prerelease(self):
        releases = [
            {'tag_name': 'v2.3.7-beta.4', 'draft': False, 'assets': []},
            {'tag_name': 'v2.3.7-beta.5', 'draft': False, 'assets': []},
            {'tag_name': 'draft', 'draft': True},
        ]
        with patch.object(updater, '_fetch_release_json', return_value=releases):
            result = updater.check_update('2.3.7-beta.4')
        self.assertTrue(result['ok'])
        self.assertTrue(result['has_update'])
        self.assertEqual(result['latest_version'], 'v2.3.7-beta.5')

    def test_beta_build_detects_formal_release_of_the_same_core(self):
        releases = [
            {'tag_name': 'v2.3.7-beta.5', 'draft': False, 'prerelease': True, 'assets': []},
            {'tag_name': 'v2.3.7', 'draft': False, 'prerelease': False, 'assets': []},
        ]
        with patch.object(updater, '_fetch_release_json', return_value=releases):
            result = updater.check_update('2.3.7-beta.3')
        self.assertTrue(result['ok'])
        self.assertTrue(result['has_update'])
        self.assertEqual(result['latest_version'], 'v2.3.7')

    def test_formal_build_never_receives_a_prerelease(self):
        releases = [
            {'tag_name': 'v2.3.7', 'draft': False, 'prerelease': False, 'assets': []},
            {'tag_name': 'v2.3.8-beta.1', 'draft': False, 'prerelease': True, 'assets': []},
        ]
        with patch.object(updater, '_fetch_release_json', return_value=releases):
            result = updater.check_update('2.3.7')
        self.assertTrue(result['ok'])
        self.assertFalse(result['has_update'])
        self.assertEqual(result['latest_version'], 'v2.3.7')
