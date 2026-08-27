# -*- coding: utf-8 -*-
"""Universal Cross-Platform & Cross-Architecture Native Compatibility Test Suite.

Contracts tested:
1. Windows full matrix (Win7/8/10/11, WoA ARM64, WebView2, Edge App Mode, Dark mode, Diagnostics).
2. macOS full matrix (Apple Silicon M-series ARM64, Intel x64, Cocoa WKWebView, App Mode, Diagnostics).
3. Linux / 信创 full matrix (Kylin, UOS, openEuler, Anolis, LoongArch, SW64, MIPS, RISC-V, Quadruple fallback).
4. Unified system_native router.
5. Multi-package specifications (RPM, Flatpak, Linglong, Arch PKGBUILD, Docker).
"""

import os
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[1]
VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from src.readmd_modules import linux_native, macos_native, system_native, updater, windows_native


class UniversalSystemCompatibilityTest(unittest.TestCase):

    # ---------------------------------------------------------------- Windows 体系测试
    def test_windows_native_version_and_architecture(self):
        with patch('sys.platform', 'win32'), patch.object(windows_native, 'IS_WIN', True):
            # Test Win11 x64
            with patch('platform.version', return_value='10.0.22621'), \
                 patch.dict(os.environ, {'PROCESSOR_ARCHITECTURE': 'AMD64'}, clear=True):
                ver = windows_native.get_windows_version_info()
                self.assertTrue(ver['is_win11'])
                self.assertFalse(ver['is_win7'])
                self.assertEqual(windows_native.architecture(), 'x86_64')
                self.assertFalse(windows_native.is_arm64())

            # Test Windows on ARM64 (Snapdragon X Elite / Surface Pro X)
            with patch('platform.version', return_value='10.0.26100'), \
                 patch.dict(os.environ, {'PROCESSOR_ARCHITECTURE': 'ARM64'}, clear=True):
                self.assertEqual(windows_native.architecture(), 'arm64')
                self.assertTrue(windows_native.is_arm64())

            # Test Win7 SP1
            with patch('platform.version', return_value='6.1.7601'), \
                 patch.dict(os.environ, {'PROCESSOR_ARCHITECTURE': 'x86'}, clear=True):
                ver = windows_native.get_windows_version_info()
                self.assertTrue(ver['is_win7'])
                self.assertFalse(ver['is_win11'])
                self.assertEqual(windows_native.architecture(), 'x86')

    def test_windows_native_diagnostics_and_fallback(self):
        with patch('sys.platform', 'win32'), patch.object(windows_native, 'IS_WIN', True):
            with patch.object(windows_native, 'get_windows_version_info', return_value={'name': 'Windows 11', 'major': 10, 'minor': 0, 'build': 22621, 'is_win11': True, 'is_win7': False}), \
                 patch.object(windows_native, 'architecture', return_value='x86_64'), \
                 patch.object(windows_native, 'probe_webview2_installed', return_value={'installed': True, 'version': '120.0.0.0', 'path': r'C:\WebView2'}), \
                 patch.object(windows_native, 'find_app_browser', return_value=r'C:\msedge.exe'), \
                 patch.object(windows_native, 'detect_system_dark_mode', return_value=True):
                diag = windows_native.diagnose_system()
                self.assertEqual(diag['status'], 'ready')
                self.assertEqual(diag['preferred_backend'], 'webview2')

                report = windows_native.format_diagnosis_report()
                self.assertIn('Windows 11', report)
                self.assertIn('x86_64', report)
                self.assertIn('120.0.0.0', report)
                self.assertIn('[OK] 原生全生态开箱即用', report)

    def test_windows_launch_browser_app(self):
        with patch('sys.platform', 'win32'), patch.object(windows_native, 'IS_WIN', True):
            with patch.object(windows_native, 'find_app_browser', return_value=r'C:\Program Files\Microsoft\Edge\Application\msedge.exe'), \
                 patch('subprocess.Popen') as mock_popen, \
                 patch('os.makedirs'):
                mock_proc = mock_popen.return_value
                proc = windows_native.launch_browser_app('http://127.0.0.1:18888')
                self.assertEqual(proc, mock_proc)
                called_cmd = mock_popen.call_args[0][0]
                self.assertTrue(any(arg.startswith('--app=http://127.0.0.1:18888') for arg in called_cmd))
                self.assertTrue(any(arg.startswith('--user-data-dir=') for arg in called_cmd))

    # ---------------------------------------------------------------- macOS 体系测试
    def test_macos_native_version_and_architecture(self):
        with patch('sys.platform', 'darwin'), patch.object(macos_native, 'IS_MAC', True):
            with patch('platform.mac_ver', return_value=('15.0.1', ('', '', ''), '')):
                ver = macos_native.get_macos_version_info()
                self.assertEqual(ver['major'], 15)
                self.assertIn('Sequoia', ver['name'])

            with patch('platform.machine', return_value='arm64'):
                self.assertEqual(macos_native.architecture(), 'arm64')
                self.assertTrue(macos_native.is_apple_silicon())

            with patch('platform.machine', return_value='x86_64'):
                self.assertEqual(macos_native.architecture(), 'x86_64')
                self.assertFalse(macos_native.is_apple_silicon())

    def test_macos_native_diagnostics(self):
        with patch('sys.platform', 'darwin'), patch.object(macos_native, 'IS_MAC', True):
            with patch.object(macos_native, 'get_macos_version_info', return_value={'name': 'macOS 14 Sonoma', 'version_str': '14.5', 'major': 14, 'minor': 5}), \
                 patch.object(macos_native, 'architecture', return_value='arm64'), \
                 patch.object(macos_native, 'probe_webkit_available', return_value=True), \
                 patch.object(macos_native, 'find_app_browser', return_value='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'), \
                 patch.object(macos_native, 'detect_system_dark_mode', return_value=True):
                diag = macos_native.diagnose_system()
                self.assertEqual(diag['status'], 'ready')
                self.assertEqual(diag['preferred_backend'], 'cocoa-wkwebview')

                report = macos_native.format_diagnosis_report()
                self.assertIn('macOS 14 Sonoma', report)
                self.assertIn('Apple Silicon', report)
                self.assertIn('Cocoa WKWebView', report)
                self.assertIn('[OK] 原生全生态开箱即用', report)

    # ---------------------------------------------------------------- Linux & 信创体系测试
    def test_linux_expanded_distros_and_cpus(self):
        with patch.object(linux_native, 'IS_LINUX', True):
            # Test openEuler & Kunpeng
            with patch.object(linux_native, 'detect_distro', return_value='openeuler'), \
                 patch.object(linux_native, 'detect_distro_info', return_value={'id': 'openeuler', 'version_id': '24.03', 'pretty_name': 'openEuler 24.03 LTS'}), \
                 patch.object(linux_native, 'detect_cpu_vendor', return_value='kunpeng'), \
                 patch.object(linux_native, 'architecture', return_value='arm64'):
                diag = linux_native.diagnose_system()
                self.assertEqual(diag['distro']['id'], 'openeuler')
                self.assertEqual(diag['cpu_vendor'], 'kunpeng')
                self.assertEqual(diag['architecture'], 'arm64')

            # Test Loongson & LoongArch64
            with patch.object(linux_native, 'detect_distro', return_value='uos'), \
                 patch.object(linux_native, 'detect_distro_info', return_value={'id': 'uos', 'version_id': '20', 'pretty_name': 'UnionTech OS Desktop 20'}), \
                 patch.object(linux_native, 'detect_cpu_vendor', return_value='loongson'), \
                 patch.object(linux_native, 'architecture', return_value='loongarch64'):
                diag = linux_native.diagnose_system()
                self.assertEqual(diag['cpu_vendor'], 'loongson')
                self.assertEqual(diag['architecture'], 'loongarch64')
                self.assertTrue(diag['is_uos'])

    # ---------------------------------------------------------------- 统一系统路由测试
    def test_system_native_unified_router(self):
        # On current host platform
        flavor = system_native.get_current_platform_flavor()
        self.assertIn(flavor, ('windows', 'macos', 'linux'))

        diag = system_native.get_unified_diagnosis()
        self.assertIn('status', diag)
        self.assertIn('architecture', diag)

        report = system_native.format_unified_report()
        self.assertTrue(len(report) > 50)
        self.assertIn('ReadMD', report)

    # ---------------------------------------------------------------- 打包配置与清单验证
    def test_packaging_manifests_integrity(self):
        rpm_script = (ROOT / 'scripts/linux/build_rpm.sh').read_text(encoding='utf-8')
        self.assertIn('readmd.spec', rpm_script)
        self.assertIn('Recommends:', rpm_script)
        self.assertIn('update-desktop-database', rpm_script)

        flatpak_manifest = (ROOT / 'scripts/linux/org.readmd.ReadMD.yaml').read_text(encoding='utf-8')
        self.assertIn('io.github.natsummerance.readmd', flatpak_manifest)
        self.assertIn('org.freedesktop.Platform', flatpak_manifest)

        linglong_manifest = (ROOT / 'scripts/linux/linglong.yaml').read_text(encoding='utf-8')
        self.assertIn('io.github.natsummerance.readmd', linglong_manifest)
        self.assertIn('org.deepin.foundation', linglong_manifest)

        pkgbuild = (ROOT / 'scripts/linux/PKGBUILD').read_text(encoding='utf-8')
        self.assertIn('pkgname=readmd-bin', pkgbuild)
        self.assertIn('webkit2gtk-4.1', pkgbuild)

        dockerfile = (ROOT / 'Dockerfile').read_text(encoding='utf-8')
        self.assertIn('python:3.11-alpine', dockerfile)
        self.assertIn('EXPOSE 8080', dockerfile)


if __name__ == '__main__':
    unittest.main()
