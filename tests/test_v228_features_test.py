# -*- coding: utf-8 -*-
"""Regression and unit test suite for ReadMD v2.2.8 features."""

import os
import sys
import unittest

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

import readmd
from installer import setup_app
from src.readmd_modules import updater, ocr
from src.readmd_modules.mdexport import formula


class TestV228Features(unittest.TestCase):
    """Test suite covering all v2.2.8 additions."""

    def test_version_consistency_v228(self):
        """Ensure v2.2.8 version string is synchronized across the entire project."""
        self.assertEqual(readmd.VERSION, '2.2.8')
        self.assertEqual(setup_app.APP_VERSION, '2.2.8')

        with open(os.path.join(ROOT_DIR, 'release', 'ReadMD-macOS.spec'), 'r', encoding='utf-8') as f:
            spec_content = f.read()
        self.assertIn("version='2.2.8'", spec_content)
        self.assertIn("'CFBundleVersion': '2.2.8'", spec_content)

        with open(os.path.join(ROOT_DIR, 'ui-tests', 'package.json'), 'r', encoding='utf-8') as f:
            self.assertIn('"version": "2.2.8"', f.read())

        with open(os.path.join(ROOT_DIR, '.github', 'workflows', 'release.yml'), 'r', encoding='utf-8') as f:
            rel_yml = f.read()
        self.assertIn("READMD_VERSION: '2.2.8'", rel_yml)
        self.assertIn("Publish ReadMD v2.2.8", rel_yml)

    def test_updater_semver_and_matching(self):
        """Test SemVer parsing, version comparison and asset matching."""
        self.assertEqual(updater.parse_semver('v2.2.8'), (2, 2, 8))
        self.assertEqual(updater.parse_semver('2.2.8'), (2, 2, 8))
        self.assertEqual(updater.parse_semver('v2.2.7'), (2, 2, 7))

        self.assertTrue(updater.is_newer_version('v2.2.8', 'v2.2.7'))
        self.assertTrue(updater.is_newer_version('v2.3.0', 'v2.2.8'))
        self.assertFalse(updater.is_newer_version('v2.2.7', 'v2.2.8'))
        self.assertFalse(updater.is_newer_version('v2.2.8', 'v2.2.8'))

        fake_assets = [
            {'name': 'ReadMDSetup-v2.2.8.exe', 'browser_download_url': 'http://example.com/setup.exe', 'size': 1000},
            {'name': 'ReadMD-portable-v2.2.8.exe', 'browser_download_url': 'http://example.com/portable.exe', 'size': 900},
            {'name': 'ReadMD-macos-x64-v2.2.8.zip', 'browser_download_url': 'http://example.com/x64.zip', 'size': 800},
            {'name': 'ReadMD-macos-arm64-v2.2.8.zip', 'browser_download_url': 'http://example.com/arm64.zip', 'size': 800},
            {'name': 'SHA256SUMS.txt', 'browser_download_url': 'http://example.com/sha.txt', 'size': 200},
        ]

        # Windows installer matching
        asset, sha = updater.match_release_asset(fake_assets, 'win_installer')
        self.assertEqual(asset['name'], 'ReadMDSetup-v2.2.8.exe')
        self.assertEqual(sha['name'], 'SHA256SUMS.txt')

        # Windows portable matching
        asset_port, _ = updater.match_release_asset(fake_assets, 'win_portable')
        self.assertEqual(asset_port['name'], 'ReadMD-portable-v2.2.8.exe')

        # macOS matching
        asset_mac, _ = updater.match_release_asset(fake_assets, 'macos')
        self.assertIn('ReadMD-macos-', asset_mac['name'])

    def test_updater_sha256_calculation(self):
        """Test SHA256 hash calculation."""
        import tempfile
        with tempfile.NamedTemporaryFile(delete=False) as tf:
            tf.write(b'ReadMD v2.2.8 automated update test payload')
            tmp_path = tf.name
        try:
            h = updater.compute_file_sha256(tmp_path)
            self.assertEqual(len(h), 64)
        finally:
            os.unlink(tmp_path)

    def test_formula_latex_auto_repair(self):
        """Test formula self-repair algorithm."""
        # HTML entity restoration
        self.assertEqual(formula.repair_latex('a &amp; b &lt; c'), 'a & b < c')

        # Brace balancing
        self.assertEqual(formula.repair_latex(r'\frac{a}{b'), r'\frac{a}{b}')
        self.assertEqual(formula.repair_latex(r'\sqrt{x + y'), r'\sqrt{x + y}')

        # Unicode math symbols to LaTeX
        repaired = formula.repair_latex('a × b ≤ c ± d ≠ e')
        self.assertIn(r'\times', repaired)
        self.assertIn(r'\le', repaired)
        self.assertIn(r'\pm', repaired)
        self.assertIn(r'\ne', repaired)

        # Greek letters
        self.assertIn(r'\alpha', formula.repair_latex('α + β'))
        self.assertIn(r'\beta', formula.repair_latex('α + β'))

    def test_ocr_normalization_pipeline(self):
        """Test OCR normalization algorithm."""
        # 1. CJK space cleanup
        raw_cjk = '这 是 一 个 测 试 文 档 ， 包 含 汉 字 空 格 。'
        cleaned = ocr.normalize_ocr_text(raw_cjk)
        self.assertNotIn('这 是', cleaned)
        self.assertIn('这是一个测试文档，包含汉字空格。', cleaned)

        # 2. English hyphenation line break fix
        raw_eng = 'This is an infor-\nmation technology report.'
        cleaned_eng = ocr.normalize_ocr_text(raw_eng)
        self.assertIn('information', cleaned_eng)

        # 3. Structure recognition
        raw_struct = """第一章 概述

这是一个段落测试。

• 列表项目一
• 列表项目二
"""
        cleaned_struct = ocr.normalize_ocr_text(raw_struct)
        self.assertIn('# 第一章', cleaned_struct)
        self.assertIn('- 列表项目一', cleaned_struct)


    def test_installer_html_no_duplicate_close_buttons(self):
        """Ensure installer setup.html does not have fake titlebar close/min buttons."""
        with open(os.path.join(ROOT_DIR, 'installer', 'setup.html'), 'r', encoding='utf-8') as f:
            content = f.read()
        self.assertNotIn('id="btn-close"', content)
        self.assertNotIn('id="btn-min"', content)
        self.assertIn('class="titlebar"', content)

    def test_app_index_html_modals_and_badges(self):
        """Ensure index.html includes update modal and statusbar badge."""
        with open(os.path.join(ROOT_DIR, 'assets', 'index.html'), 'r', encoding='utf-8') as f:
            html = f.read()
        self.assertIn('id="update-modal"', html)
        self.assertIn('id="status-update-badge"', html)
        self.assertIn('id="btn-check-update"', html)

    def test_api_updater_methods(self):
        """Ensure Api class has updater methods exposed."""
        api = readmd.Api()
        self.assertTrue(hasattr(api, 'check_update'))
        self.assertTrue(hasattr(api, 'start_download_update'))
        self.assertTrue(hasattr(api, 'get_download_status'))
        self.assertTrue(hasattr(api, 'cancel_download'))
        self.assertTrue(hasattr(api, 'apply_update'))

    def test_export_preview_dom_and_dynamic_css(self):
        """Ensure export preview containers do not carry polluting theme classes and dynamic styling exists."""
        with open(os.path.join(ROOT_DIR, 'assets', 'index.html'), 'r', encoding='utf-8') as f:
            html = f.read()
        self.assertIn('id="export-preview-mini-content"', html)
        self.assertIn('id="export-preview-full-page"', html)
        self.assertNotIn('id="export-preview-mini-content" class="export-preview-mini-content markdown-body"', html)
        self.assertNotIn('id="export-preview-full-page" class="export-preview-full-page markdown-body"', html)

        with open(os.path.join(ROOT_DIR, 'assets', 'app.js'), 'r', encoding='utf-8') as f:
            js = f.read()
        self.assertIn('function generateExportPreviewCss', js)
        self.assertIn('export-preview-dynamic-style', js)

    def test_drag_and_drop_convert_auto_open(self):
        """Ensure dropped non-md files and batch convert results are automatically loaded into tabs."""
        with open(os.path.join(ROOT_DIR, 'assets', 'app.js'), 'r', encoding='utf-8') as f:
            js = f.read()
        self.assertIn('await convertOrOcr(path, \'convert\')', js)
        self.assertIn('await loadFile(it.out)', js)


if __name__ == '__main__':
    unittest.main()

