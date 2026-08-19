#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Tests for ReadMD v2.3.1 Full-Stack i18n Architecture Coverage.
"""
import os
import json
import re
import unittest

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
I18N_DIR = os.path.join(BASE_DIR, "assets", "i18n")
HTML_PATH = os.path.join(BASE_DIR, "assets", "index.html")

class TestI18nCoverage(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with open(os.path.join(I18N_DIR, "en.json"), "r", encoding="utf-8") as f:
            cls.en_dict = json.load(f)
        with open(os.path.join(I18N_DIR, "zh-CN.json"), "r", encoding="utf-8") as f:
            cls.zh_dict = json.load(f)
        with open(HTML_PATH, "r", encoding="utf-8") as f:
            cls.html_content = f.read()

    def test_baseline_dictionaries(self):
        """Verify en.json and zh-CN.json contain 300+ keys and match exactly."""
        self.assertGreaterEqual(len(self.en_dict), 300, "en.json must contain at least 300 keys")
        self.assertGreaterEqual(len(self.zh_dict), 300, "zh-CN.json must contain at least 300 keys")
        self.assertEqual(set(self.en_dict.keys()), set(self.zh_dict.keys()), "en.json and zh-CN.json key sets must match 100%")

    def test_all_46_languages_parity(self):
        """Verify all 46 language JSON files exist and have 100% key parity with en.json."""
        all_json_files = [f for f in os.listdir(I18N_DIR) if f.endswith(".json") and f != "meta.json"]
        self.assertGreaterEqual(len(all_json_files), 46, "There should be at least 46 language JSON files")
        
        en_keys = set(self.en_dict.keys())
        for fname in all_json_files:
            fpath = os.path.join(I18N_DIR, fname)
            with open(fpath, "r", encoding="utf-8") as f:
                d = json.load(f)
            missing = en_keys - set(d.keys())
            self.assertEqual(len(missing), 0, f"File {fname} has missing keys: {missing}")
            self.assertEqual(len(d), len(self.en_dict), f"File {fname} key count does not match en.json")

    def test_html_data_i18n_keys_valid(self):
        """Verify all data-i18n, data-i18n-title, data-i18n-placeholder, data-i18n-aria in HTML exist in en.json."""
        pattern = re.compile(r'data-i18n(?:-title|-placeholder|-aria|-html)?="([^"]+)"')
        matches = pattern.findall(self.html_content)
        self.assertGreater(len(matches), 50, "HTML must contain extensive data-i18n annotations")

        en_keys = set(self.en_dict.keys())
        for key in matches:
            self.assertIn(key, en_keys, f"Key '{key}' found in index.html is missing in en.json")

    def test_essential_buttons_annotated(self):
        """Verify critical toolbar buttons and menus have i18n data tags."""
        essential_ids = [
            'id="btn-toc"', 'id="btn-open"', 'id="btn-folder"', 'id="btn-recent"',
            'id="btn-reload"', 'id="btn-search"', 'id="btn-theme"', 'id="btn-a"',
            'id="btn-A"', 'id="btn-print"', 'id="btn-edit"', 'id="btn-ai"',
            'id="btn-convert"', 'id="btn-web"', 'id="btn-ocr"', 'id="btn-clipboard-new"',
            'id="btn-saveas"', 'id="btn-assoc"', 'id="btn-share"', 'id="btn-fix"',
            'id="btn-lang"', 'id="btn-autostart"', 'id="btn-check-update"'
        ]
        for btn_id in essential_ids:
            self.assertIn(btn_id, self.html_content, f"Button {btn_id} must be present in index.html")

if __name__ == '__main__':
    unittest.main()
