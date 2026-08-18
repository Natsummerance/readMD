"""Regression and unit test suite for ReadMD v2.2.7 features."""

import os
import unittest
import re
import readmd
from installer import setup_app

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INDEX_HTML = os.path.join(ROOT_DIR, 'assets', 'index.html')
STYLE_CSS = os.path.join(ROOT_DIR, 'assets', 'style.css')
APP_JS = os.path.join(ROOT_DIR, 'assets', 'app.js')


class TestV227Features(unittest.TestCase):

    def test_version_bump_consistency(self):
        self.assertEqual(readmd.VERSION, '2.2.7')
        self.assertEqual(setup_app.APP_VERSION, '2.2.7')

    def test_index_html_multi_tab_and_dom_elements(self):
        with open(INDEX_HTML, 'r', encoding='utf-8') as f:
            html = f.read()

        # Multi-tab DOM
        self.assertIn('id="doc-tabs-container"', html)
        self.assertIn('id="doc-tabs-bar"', html)
        self.assertIn('id="doc-tabs-overflow-wrap"', html)
        self.assertIn('id="doc-tabs-overflow-btn"', html)
        self.assertIn('id="doc-tabs-dropdown"', html)
        self.assertIn('id="doc-tabs-secondary-bar"', html)
        self.assertIn('id="tab-context-menu"', html)

        # Drag Overlay
        self.assertIn('id="drag-overlay"', html)
        self.assertIn('id="drag-title"', html)
        self.assertIn('id="drag-desc"', html)

        # Statusbar Home Button
        self.assertIn('<button id="btn-home" class="status-btn-home hidden"', html)
        # Verify statusbar contains btn-home
        statusbar_match = re.search(r'<footer id="statusbar">([\s\S]*?)</footer>', html)
        self.assertTrue(statusbar_match)
        self.assertIn('id="btn-home"', statusbar_match.group(1))

        # Preview Grid direction fix (top button must be column 2)
        pv_grid_match = re.search(r'<div class="pv-grid"[^>]*>([\s\S]*?)</div>', html)
        self.assertTrue(pv_grid_match)
        pv_content = pv_grid_match.group(1)
        self.assertTrue(re.search(r'<span></span>\s*<button[^>]*data-pv="top"', pv_content))

        # URL local network allowed by default
        self.assertTrue(re.search(r'<input id="url-private" type="checkbox"[^>]*checked', html))

        # Export preview card & enlarged modal
        self.assertIn('id="export-preview-card"', html)
        self.assertIn('id="export-preview-modal"', html)
        self.assertIn('id="export-preview-mini-content"', html)
        self.assertIn('id="export-preview-full-page"', html)

    def test_style_css_rules(self):
        with open(STYLE_CSS, 'r', encoding='utf-8') as f:
            css = f.read()

        self.assertIn('.doc-tabs-container', css)
        self.assertIn('.doc-tabs-bar', css)
        self.assertIn('.tab-item', css)
        self.assertIn('.tab-item.active', css)
        self.assertIn('.tab-item.tab-dragging', css)
        self.assertIn('.tab-dirty', css)
        self.assertIn('.tab-close', css)
        self.assertIn('.doc-tabs-dropdown', css)
        self.assertIn('.doc-tabs-secondary-bar', css)
        self.assertIn('.tab-context-menu', css)

        self.assertIn('.drag-overlay', css)
        self.assertIn('.drag-box', css)
        self.assertIn('@keyframes dragBoxPulse', css)

        self.assertIn('.status-btn-home', css)
        self.assertIn('.export-preview-card', css)
        self.assertIn('.export-preview-mini-page', css)
        self.assertIn('#export-preview-modal', css)
        self.assertIn('.export-preview-full-page', css)

    def test_app_js_logic(self):
        with open(APP_JS, 'r', encoding='utf-8') as f:
            js = f.read()

        # Multi-tab methods
        self.assertIn('state.tabs', js)
        self.assertIn('state.activeTabId', js)
        self.assertIn('function renderTabsBar()', js)
        self.assertIn('function switchTab(', js)
        self.assertIn('function closeTab(', js)
        self.assertIn('function closeOtherTabs(', js)
        self.assertIn('function closeAllTabs()', js)
        self.assertIn('function renameTab(', js)
        self.assertIn('function reorderTabs(', js)
        self.assertIn('function startTabInlineRename(', js)

        # Drag and Drop handlers
        self.assertIn('function bindGlobalDragAndDrop()', js)
        self.assertIn('dragenter', js)
        self.assertIn('dragover', js)
        self.assertIn('dragleave', js)
        self.assertIn('drop', js)

        # Sidebar toggle logic fix
        toggle_side_match = re.search(r'function toggleSide\([^)]*\)\s*\{([\s\S]*?)\}', js)
        self.assertTrue(toggle_side_match)
        toggle_body = toggle_side_match.group(1)
        self.assertIn("if (!side.classList.contains('hidden'))", toggle_body)
        self.assertIn("side.classList.add('hidden')", toggle_body)

        # Export live preview
        self.assertIn('function updateExportLivePreview()', js)
        self.assertIn("wrap.className = 'exp-sec';", js)  # Default collapsed

        # Keyboard shortcuts
        self.assertIn("createFromClipboard()", js)
        self.assertIn("saveAs()", js)


if __name__ == '__main__':
    unittest.main()
