import os
import shutil
import tempfile
import unittest
from unittest.mock import patch

from src.readmd_modules import plugin_manager as pm
from src.readmd_modules.plugin_catalog import CAPABILITIES


class TestPluginHarness(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix='readmd_test_harness_')
        self._orig_dir = pm.PLUGINS_DIR
        self._orig_manifest = pm.PLUGINS_MANIFEST
        self._orig_site = pm.PLUGINS_SITE_PACKAGES
        self._orig_bin = pm.PLUGINS_BIN
        self._orig_tasks = dict(pm._install_tasks)
        self._orig_runtime = dict(pm._runtime_status)
        self._orig_active = dict(pm._active_uses)

        pm.PLUGINS_DIR = self.test_dir
        pm.PLUGINS_MANIFEST = os.path.join(self.test_dir, 'plugins.json')
        pm.PLUGINS_SITE_PACKAGES = os.path.join(self.test_dir, 'site-packages')
        pm.PLUGINS_BIN = os.path.join(self.test_dir, 'bin')
        os.makedirs(pm.PLUGINS_SITE_PACKAGES, exist_ok=True)
        os.makedirs(pm.PLUGINS_BIN, exist_ok=True)
        pm._install_tasks.clear()
        pm._runtime_status.clear()
        pm._active_uses.clear()

    def tearDown(self):
        pm.PLUGINS_DIR = self._orig_dir
        pm.PLUGINS_MANIFEST = self._orig_manifest
        pm.PLUGINS_SITE_PACKAGES = self._orig_site
        pm.PLUGINS_BIN = self._orig_bin
        pm._install_tasks.clear()
        pm._install_tasks.update(self._orig_tasks)
        pm._runtime_status.clear()
        pm._runtime_status.update(self._orig_runtime)
        pm._active_uses.clear()
        pm._active_uses.update(self._orig_active)
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def _mock_installed(self, plugin_ids):
        return patch.object(pm, 'is_plugin_installed', side_effect=lambda pid: pid in plugin_ids)

    def test_capabilities_structure(self):
        """Ensure all capabilities have providers defined in PLUGIN_SPECS."""
        for cap, providers in CAPABILITIES.items():
            self.assertTrue(len(providers) >= 1, f"Capability {cap} has no providers")
            for pid in providers:
                self.assertIn(pid, pm.PLUGIN_SPECS, f"Provider {pid} in {cap} missing from specs")
                self.assertEqual(pm.PLUGIN_SPECS[pid]['capability'], cap)

    def test_mutual_exclusivity_within_same_capability(self):
        """Toggling on a plugin in the same capability disables the competitor."""
        installed = {'rapidocr', 'easyocr'}
        with self._mock_installed(installed):
            # Initially enable rapidocr
            ok = pm.set_plugin_enabled('rapidocr', True)
            self.assertTrue(ok)
            manifest = pm.load_manifest()
            self.assertTrue(manifest['rapidocr']['enabled'])
            self.assertFalse(manifest['easyocr']['enabled'])

            # Now enable easyocr -> rapidocr should automatically be disabled
            ok = pm.set_plugin_enabled('easyocr', True)
            self.assertTrue(ok)
            manifest = pm.load_manifest()
            self.assertTrue(manifest['easyocr']['enabled'])
            self.assertFalse(manifest['rapidocr']['enabled'])
            self.assertEqual(pm.active_provider('ocr'), 'easyocr')

    def test_cross_capability_independence(self):
        """Plugins across different capabilities can be enabled simultaneously."""
        installed = {'rapidocr', 'rapid_table', 'pymupdf4llm', 'whisper'}
        with self._mock_installed(installed):
            pm.set_plugin_enabled('rapidocr', True)
            pm.set_plugin_enabled('rapid_table', True)
            pm.set_plugin_enabled('pymupdf4llm', True)
            pm.set_plugin_enabled('whisper', True)

            manifest = pm.load_manifest()
            self.assertTrue(manifest['rapidocr']['enabled'])
            self.assertTrue(manifest['rapid_table']['enabled'])
            self.assertTrue(manifest['pymupdf4llm']['enabled'])
            self.assertTrue(manifest['whisper']['enabled'])

            self.assertEqual(pm.active_provider('ocr'), 'rapidocr')
            self.assertEqual(pm.active_provider('table'), 'rapid_table')
            self.assertEqual(pm.active_provider('pdf'), 'pymupdf4llm')
            self.assertEqual(pm.active_provider('audio'), 'whisper')

    def test_run_plugin_success_and_fallback(self):
        """run_plugin executes callback when enabled and falls back on error."""
        with self._mock_installed({'rapidocr'}):
            pm.set_plugin_enabled('rapidocr', True)

            # 1. Success execution
            res = pm.run_plugin('rapidocr', lambda: "SUCCESS_RESULT")
            self.assertEqual(res, "SUCCESS_RESULT")
            self.assertEqual(pm._runtime_status.get('rapidocr', {}).get('state'), 'ready')

            # 2. Exception in callback triggers graceful fallback to default
            def failing_callback():
                raise RuntimeError("OCR engine crashed")

            res_fail = pm.run_plugin('rapidocr', failing_callback, default="FALLBACK_DEFAULT")
            self.assertEqual(res_fail, "FALLBACK_DEFAULT")
            self.assertEqual(pm._runtime_status.get('rapidocr', {}).get('state'), 'error')
            self.assertIn("OCR engine crashed", pm._runtime_status.get('rapidocr', {}).get('error', ''))

    def test_run_plugin_when_disabled_returns_default(self):
        """Disabled plugin immediately returns default without calling execute."""
        with self._mock_installed({'rapidocr'}):
            pm.set_plugin_enabled('rapidocr', False)
            executed = []
            res = pm.run_plugin('rapidocr', lambda: executed.append(1), default="SKIPPED")
            self.assertEqual(res, "SKIPPED")
            self.assertEqual(executed, [])


if __name__ == '__main__':
    unittest.main()
