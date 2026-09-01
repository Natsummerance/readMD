# -*- coding: utf-8 -*-
"""Tray menu label localization (C8): en/zh fallback per system locale."""
import os
import tempfile
import unittest
from unittest import mock

import readmd


class TrayLabelsTest(unittest.TestCase):
    def test_zh_cn_labels_from_locale_file(self):
        with mock.patch.object(readmd, 'get_system_language', return_value='zh-CN'):
            labels = readmd._tray_labels()
        self.assertEqual(labels['tray.show'], '显示 ReadMD')
        self.assertEqual(labels['menu.open'], '打开文件…')
        self.assertEqual(labels['tray.quit'], '退出 ReadMD')

    def test_en_labels_from_locale_file(self):
        with mock.patch.object(readmd, 'get_system_language', return_value='en'):
            labels = readmd._tray_labels()
        self.assertEqual(labels['tray.show'], 'Show ReadMD')
        self.assertEqual(labels['menu.open'], 'Open File…')
        self.assertEqual(labels['tray.quit'], 'Quit ReadMD')

    def test_zh_tw_labels_from_zh_tw_file(self):
        with mock.patch.object(readmd, 'get_system_language', return_value='zh-TW'):
            labels = readmd._tray_labels()
        self.assertEqual(labels['menu.open'], '打开檔案…')
        self.assertEqual(labels['tray.show'], '顯示 ReadMD')
        self.assertEqual(labels['tray.quit'], '退出 ReadMD')

    def test_unknown_lang_falls_back_to_en_file(self):
        with mock.patch.object(readmd, 'get_system_language', return_value='xx-YY'):
            labels = readmd._tray_labels()
        self.assertEqual(labels['tray.show'], 'Show ReadMD')
        self.assertEqual(labels['menu.open'], 'Open File…')
        self.assertEqual(labels['tray.quit'], 'Quit ReadMD')

    def test_detection_failure_falls_back_to_english(self):
        with mock.patch.object(readmd, 'get_system_language', side_effect=RuntimeError('boom')):
            labels = readmd._tray_labels()
        self.assertEqual(labels['tray.show'], 'Show ReadMD')
        self.assertEqual(labels['menu.open'], 'Open File…')
        self.assertEqual(labels['tray.quit'], 'Quit ReadMD')

    def test_missing_locale_dir_uses_defaults(self):
        bogus = os.path.join(tempfile.gettempdir(), 'readmd-no-such-dir-xyz')
        with mock.patch.object(readmd, 'get_system_language', return_value='en'), \
                mock.patch.object(readmd, 'APP_DIR', bogus):
            labels = readmd._tray_labels()
        self.assertEqual(labels['tray.show'], 'Show ReadMD')
        self.assertEqual(labels['menu.open'], 'Open File…')
        self.assertEqual(labels['tray.quit'], 'Quit ReadMD')


if __name__ == '__main__':
    unittest.main()
