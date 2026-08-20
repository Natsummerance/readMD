# -*- coding: utf-8 -*-
"""ReadMD 跨平台桌面原生集成模块 (linux_native / macos_native / windows_native) 单元测试。"""

import os
import sys
import unittest
from unittest.mock import MagicMock, patch, mock_open

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import linux_native, macos_native


class TestNativeDesktopModules(unittest.TestCase):
    """测试 Linux 信创系统适配、Wayland 协议探测与 macOS 原生桥接。"""

    def test_linux_detect_distro_uos(self):
        """测试统信 UOS 发行版标识检测。"""
        uos_os_release = 'NAME="UnionTech OS"\nID="uos"\nVERSION="20"'
        with patch.object(linux_native, 'IS_LINUX', True):
            with patch('os.path.exists', return_value=True):
                with patch('builtins.open', mock_open(read_data=uos_os_release)):
                    self.assertEqual(linux_native.detect_distro(), 'uos')
                    self.assertTrue(linux_native.is_uos())

    def test_linux_detect_distro_kylin(self):
        """测试银河麒麟 Kylin 发行版标识检测。"""
        kylin_os_release = 'NAME="Kylin Linux Advanced Server"\nID="kylin"\nVERSION="V10"'
        with patch.object(linux_native, 'IS_LINUX', True):
            with patch('os.path.exists', return_value=True):
                with patch('builtins.open', mock_open(read_data=kylin_os_release)):
                    self.assertEqual(linux_native.detect_distro(), 'kylinos')
                    self.assertTrue(linux_native.is_kylin())

    def test_linux_detect_distro_deepin(self):
        """测试深度 Deepin 发行版标识检测。"""
        deepin_os_release = 'NAME="Deepin"\nID="deepin"\nVERSION="23"'
        with patch.object(linux_native, 'IS_LINUX', True):
            with patch('os.path.exists', return_value=True):
                with patch('builtins.open', mock_open(read_data=deepin_os_release)):
                    self.assertEqual(linux_native.detect_distro(), 'deepin')
                    self.assertTrue(linux_native.is_deepin())

    def test_linux_wayland_detection(self):
        """测试 Wayland 与 X11 显示协议环境检测。"""
        with patch.dict(os.environ, {'WAYLAND_DISPLAY': 'wayland-0'}):
            self.assertTrue(linux_native.is_wayland())

        with patch.dict(os.environ, {'WAYLAND_DISPLAY': '', 'XDG_SESSION_TYPE': 'x11'}, clear=True):
            self.assertFalse(linux_native.is_wayland())

    def test_linux_dark_mode_detection(self):
        """测试 gsettings 深色模式探测。"""
        with patch.object(linux_native, 'IS_LINUX', True):
            with patch('shutil.which', return_value='/usr/bin/gsettings'):
                with patch('subprocess.check_output', return_value=b"'prefer-dark'\n"):
                    self.assertTrue(linux_native.detect_system_dark_mode())

    def test_macos_native_reveal_path_mock(self):
        """测试 macOS NSWorkspace 原生文件定位接口。"""
        mock_workspace = MagicMock()
        mock_nsworkspace = MagicMock()
        mock_nsworkspace.sharedWorkspace.return_value = mock_workspace

        mock_nsurl = MagicMock()
        mock_nsurl.fileURLWithPath_.return_value = 'file:///Users/test/doc.md'

        with patch.dict('sys.modules', {
            'AppKit': MagicMock(NSWorkspace=mock_nsworkspace),
            'Foundation': MagicMock(NSURL=mock_nsurl),
        }):
            res = macos_native.reveal_path('/Users/test/doc.md')
            self.assertTrue(res)
            mock_workspace.activateFileViewerSelectingURLs_.assert_called_once()

    def test_macos_native_open_path_mock(self):
        """测试 macOS NSWorkspace 打开路径接口。"""
        mock_workspace = MagicMock()
        mock_workspace.openURL_.return_value = True
        mock_nsworkspace = MagicMock()
        mock_nsworkspace.sharedWorkspace.return_value = mock_workspace

        mock_nsurl = MagicMock()
        mock_nsurl.fileURLWithPath_.return_value = 'file:///Users/test/dir'

        with patch.dict('sys.modules', {
            'AppKit': MagicMock(NSWorkspace=mock_nsworkspace),
            'Foundation': MagicMock(NSURL=mock_nsurl),
        }):
            res = macos_native.open_path('/Users/test/dir')
            self.assertTrue(res)
            mock_workspace.openURL_.assert_called_once()


    def test_windows_native_show_error(self):
        """测试 Windows 原生 MessageBox 报错弹窗。"""
        from src.readmd_modules import windows_native
        mock_ctypes = MagicMock()
        mock_ctypes.windll.user32.MessageBoxW.return_value = 1
        with patch.dict('sys.modules', {'ctypes': mock_ctypes}):
            res = windows_native.show_error('Title', 'Error message')
            self.assertTrue(res)


if __name__ == '__main__':
    unittest.main()
