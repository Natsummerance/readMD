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
            for arch in ('amd64', 'arm64')
        ]
        with patch('sys.platform', 'linux'), patch('platform.machine', return_value='aarch64'):
            asset, _ = updater.match_release_asset(assets, flavor='linux')
            self.assertEqual(asset['name'], f'readmd_{VERSION}_arm64.deb')

        with patch('sys.platform', 'linux'), patch('platform.machine', return_value='x86_64'):
            asset, _ = updater.match_release_asset(assets, flavor='linux')
            self.assertEqual(asset['name'], f'readmd_{VERSION}_amd64.deb')

    def test_release_publishes_kylin_assets_and_safe_fallback(self):
        workflow = (ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8')
        build_script = (ROOT / 'scripts/linux/build_linux.sh').read_text(encoding='utf-8')
        notes = (ROOT / 'release/release_notes.md').read_text(encoding='utf-8')

        self.assertIn('kylin-v10-arm64-package:', workflow)
        self.assertIn('runs-on: ubuntu-24.04-arm', workflow)
        self.assertIn('image: ubuntu:20.04', workflow)
        self.assertIn('Python-3.11.16.tgz', workflow)
        self.assertIn('--enable-shared', workflow)
        self.assertIn('libwebkit2gtk-4.0-dev', workflow)
        self.assertIn('ReadMD-linux-aarch64-', workflow)
        self.assertIn('readmd_${{ env.READMD_VERSION }}_arm64.deb', workflow)
        self.assertIn('appimagetool-${APPIMAGE_TOOL_ARCH}.AppImage', build_script)
        self.assertIn('libwebkit2gtk-4.0-37 | libwebkit2gtk-4.1-0', build_script)
        self.assertIn(f'ReadMD-linux-aarch64-v{VERSION}.AppImage', notes)
        self.assertIn(f'readmd_{VERSION}_arm64.deb', notes)


if __name__ == '__main__':
    unittest.main()
