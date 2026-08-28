# -*- coding: utf-8 -*-
"""Kylin V10 and Phytium ARM64 compatibility contracts."""

import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from src.readmd_modules import linux_native, updater


class KylinV10CompatibilityTest(unittest.TestCase):
    def test_phytium_kylin_v10_uses_software_webkit(self):
        env = {
            'WAYLAND_DISPLAY': 'wayland-0',
            'GDK_BACKEND': '',
            'WEBKIT_DISABLE_COMPOSITING_MODE': '',
            'WEBKIT_DISABLE_DMABUF_RENDERER': '',
            'LIBGL_ALWAYS_SOFTWARE': '',
            'GALLIUM_DRIVER': '',
            'MESA_LOADER_DRIVER_OVERRIDE': '',
        }
        with patch.object(linux_native, 'IS_LINUX', True), \
             patch.dict(os.environ, env, clear=True), \
             patch.object(linux_native, 'is_wayland', return_value=True), \
             patch.object(linux_native, 'is_kylin_v10', return_value=True), \
             patch.object(linux_native, 'architecture', return_value='arm64'), \
             patch.object(linux_native, 'is_phytium', return_value=True):
            linux_native.setup_linux_env()
            self.assertEqual(os.environ['GDK_BACKEND'], 'wayland,x11')
            self.assertEqual(os.environ['WEBKIT_DISABLE_COMPOSITING_MODE'], '1')
            self.assertEqual(os.environ['WEBKIT_DISABLE_DMABUF_RENDERER'], '1')
            self.assertEqual(os.environ['LIBGL_ALWAYS_SOFTWARE'], '1')
            self.assertEqual(os.environ['GALLIUM_DRIVER'], 'llvmpipe')

    def test_mainstream_linux_keeps_hardware_path(self):
        env = {'GDK_BACKEND': '', 'WEBKIT_DISABLE_COMPOSITING_MODE': ''}
        with patch.object(linux_native, 'IS_LINUX', True), \
             patch.dict(os.environ, env, clear=True), \
             patch.object(linux_native, 'is_wayland', return_value=False), \
             patch.object(linux_native, 'is_kylin_v10', return_value=False), \
             patch.object(linux_native, 'architecture', return_value='x86_64'), \
             patch.object(linux_native, 'is_phytium', return_value=False):
            linux_native.setup_linux_env()
            self.assertEqual(os.environ['GDK_BACKEND'], 'x11')
            self.assertEqual(os.environ['WEBKIT_DISABLE_COMPOSITING_MODE'], '0')
            self.assertNotIn('WEBKIT_DISABLE_DMABUF_RENDERER', os.environ)

    def test_linux_updater_selects_machine_architecture(self):
        assets = [
            {'name': f'readmd_{VERSION}_{arch}.deb'}
            for arch in ('amd64', 'arm64', 'loongarch64', 'mips64el', 'sw64', 'riscv64', 'armhf')
        ]
        with patch('sys.platform', 'linux'), patch('platform.machine', return_value='aarch64'):
            asset, _ = updater.match_release_asset(assets, flavor='linux')
            self.assertEqual(asset['name'], f'readmd_{VERSION}_arm64.deb')

        with patch('sys.platform', 'linux'), patch('platform.machine', return_value='x86_64'):
            asset, _ = updater.match_release_asset(assets, flavor='linux')
            self.assertEqual(asset['name'], f'readmd_{VERSION}_amd64.deb')

        with patch('sys.platform', 'linux'), patch('platform.machine', return_value='loongarch64'):
            asset, _ = updater.match_release_asset(assets, flavor='linux')
            self.assertEqual(asset['name'], f'readmd_{VERSION}_loongarch64.deb')

        with patch('sys.platform', 'linux'), patch('platform.machine', return_value='sw_64'):
            asset, _ = updater.match_release_asset(assets, flavor='linux')
            self.assertEqual(asset['name'], f'readmd_{VERSION}_sw64.deb')

    def test_release_keeps_generic_arm64_distinct_from_kylin_evidence(self):
        workflow = (ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8')
        build_script = (ROOT / 'scripts/linux/build_linux.sh').read_text(encoding='utf-8')
        notes = (ROOT / 'release/release_notes.md').read_text(encoding='utf-8')

        self.assertIn('linux-arm64-compat-package:', workflow)
        self.assertNotIn('kylin-v10-arm64-package:', workflow)
        self.assertIn('runs-on: ubuntu-24.04-arm', workflow)
        self.assertIn('image: ubuntu:20.04', workflow)
        self.assertIn('Python-3.11.16.tgz', workflow)
        self.assertIn('--enable-shared', workflow)
        self.assertIn('libwebkit2gtk-4.0-dev', workflow)
        self.assertIn('ReadMD-linux-aarch64-', workflow)
        self.assertIn('readmd_${{ env.READMD_VERSION }}_arm64.deb', workflow)
        self.assertIn('not Kylin/UOS evidence', workflow)
        self.assertIn('appimagetool-${APPIMAGE_TOOL_ARCH}.AppImage', build_script)
        self.assertIn('libwebkit2gtk-4.0-37 | libwebkit2gtk-4.1-0', build_script)
        self.assertIn('Recommends: gir1.2-webkit2-4.0 | gir1.2-webkit2-4.1 | gir1.2-webkit-6.0', build_script)
        self.assertIn('postinst', build_script)
        self.assertIn('postrm', build_script)
        self.assertIn('READMD_DEB_ARCH', build_script)
        self.assertIn(f'ReadMD-linux-aarch64-v{VERSION}.AppImage', notes)
        self.assertIn(f'readmd_{VERSION}_arm64.deb', notes)

    def test_probe_webkit_versions(self):
        with patch.object(linux_native, 'IS_LINUX', True):
            # Probing returns None on mock failure
            self.assertIn(linux_native.probe_webkit_version(), (None, '4.1', '4.0', '6.0'))

    def test_find_app_browser_detects_domestic_and_chromium(self):
        with patch('shutil.which', side_effect=lambda name: f'/usr/bin/{name}' if name in ('kylin-browser', 'google-chrome') else None), \
             patch('os.path.isfile', return_value=True), \
             patch('os.access', return_value=True):
            browser = linux_native.find_app_browser()
            self.assertEqual(browser, '/usr/bin/kylin-browser')

    def test_launch_browser_app_uses_app_mode(self):
        with patch.object(linux_native, 'find_app_browser', return_value='/usr/bin/kylin-browser'), \
             patch('subprocess.Popen') as mock_popen, \
             patch('os.makedirs'):
            mock_proc = mock_popen.return_value
            proc = linux_native.launch_browser_app('http://127.0.0.1:18888', window_title='ReadMD')
            self.assertEqual(proc, mock_proc)
            called_cmd = mock_popen.call_args[0][0]
            self.assertEqual(called_cmd[0], '/usr/bin/kylin-browser')
            self.assertTrue(any(arg.startswith('--app=http://127.0.0.1:18888') for arg in called_cmd))
            self.assertTrue(any(arg.startswith('--user-data-dir=') for arg in called_cmd))

    def test_probe_gui_backends_prioritizes_fallbacks(self):
        # 1. When GTK WebKit is available -> preferred is gtk
        with patch.object(linux_native, 'probe_webkit_version', return_value='4.1'), \
             patch.object(linux_native, 'probe_qt_webengine', return_value=True), \
             patch.object(linux_native, 'find_app_browser', return_value='/usr/bin/chromium'):
            backends = linux_native.probe_gui_backends()
            self.assertEqual(backends['preferred_backend'], 'gtk')
            self.assertEqual(backends['gtk_webkit'], '4.1')

        # 2. When GTK WebKit is missing, but Qt is available -> preferred is qt
        with patch.object(linux_native, 'probe_webkit_version', return_value=None), \
             patch.object(linux_native, 'probe_qt_webengine', return_value=True), \
             patch.object(linux_native, 'find_app_browser', return_value='/usr/bin/chromium'):
            backends = linux_native.probe_gui_backends()
            self.assertEqual(backends['preferred_backend'], 'qt')

        # 3. When both GTK and Qt are missing, but browser app is available -> preferred is browser-app
        with patch.object(linux_native, 'probe_webkit_version', return_value=None), \
             patch.object(linux_native, 'probe_qt_webengine', return_value=False), \
             patch.object(linux_native, 'find_app_browser', return_value='/usr/bin/kylin-browser'):
            backends = linux_native.probe_gui_backends()
            self.assertEqual(backends['preferred_backend'], 'browser-app')

    def test_diagnose_system_and_report(self):
        with patch.object(linux_native, 'IS_LINUX', True), \
             patch.object(linux_native, 'detect_distro_info', return_value={'id': 'kylinos', 'version_id': 'v10', 'pretty_name': 'Kylin Linux Advanced Server V10'}), \
             patch.object(linux_native, 'architecture', return_value='arm64'), \
             patch.object(linux_native, 'detect_cpu_vendor', return_value='phytium'), \
             patch.object(linux_native, 'probe_gui_backends', return_value={'gtk_webkit': None, 'qt_webengine': False, 'app_browser': '/usr/bin/kylin-browser', 'xdg_open': True, 'preferred_backend': 'browser-app'}), \
             patch.object(linux_native, 'detect_system_dark_mode', return_value=True), \
             patch.object(linux_native, 'is_wayland', return_value=False):
            diag = linux_native.diagnose_system()
            self.assertEqual(diag['architecture'], 'arm64')
            self.assertEqual(diag['cpu_vendor'], 'phytium')
            self.assertTrue(diag['is_kylin_v10'])
            self.assertTrue(diag['is_phytium'])
            # A browser-app fallback is not feature-equivalent to the
            # pywebview bridge, so diagnostics must mark it degraded.
            self.assertEqual(diag['status'], 'degraded')

            report = linux_native.format_diagnosis_report()
            self.assertIn('Kylin Linux Advanced Server V10', report)
            self.assertIn('arm64', report)
            self.assertIn('phytium', report)
            self.assertIn('Mesa llvmpipe', report)
            self.assertIn('browser-app', report)
            self.assertIn('[WARNING] 仅有浏览器降级', report)


if __name__ == '__main__':
    unittest.main()
