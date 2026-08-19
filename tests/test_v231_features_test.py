# -*- coding: utf-8 -*-
"""Unit tests for ReadMD v2.3.1 features:
1. Linux and Chinese Domestic OS (UOS, Kylin, Deepin) native module;
2. Enhanced clipboard bridge (CF_HDROP file list, image_path compatibility);
3. Web-to-MD & AI Assistant clean SVG UI validation (no tacky emojis);
4. Multi-file batch OCR selection support;
5. Language modal open button and closeMoreMenu helper.
"""

import os
import re
import sys
import unittest
from unittest.mock import MagicMock, patch

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

import readmd
import src.readmd_modules.linux_native as linux_native


class TestV231Features(unittest.TestCase):

    def test_version_bumped_to_v231(self):
        self.assertTrue(readmd.VERSION >= '2.3.1')

    def test_linux_native_module_functions(self):
        self.assertIsInstance(linux_native.is_linux(), bool)
        self.assertIsInstance(linux_native.is_wayland(), bool)
        distro = linux_native.detect_distro()
        self.assertIsInstance(distro, str)
        self.assertIsInstance(linux_native.is_uos(), bool)
        self.assertIsInstance(linux_native.is_kylin(), bool)
        self.assertIsInstance(linux_native.is_deepin(), bool)
        self.assertIsInstance(linux_native.detect_system_dark_mode(), bool)
        # Verify setup_linux_env executes without throwing
        linux_native.setup_linux_env()

    def test_clipboard_bridge_token_and_format(self):
        api = readmd.Api()
        permit = api.authorize_clipboard_read()
        self.assertTrue(permit.get('ok'))
        token = permit.get('token')
        self.assertTrue(token)

        res = api.read_clipboard(token)
        self.assertIsInstance(res, dict)
        self.assertIn('source_type', res)
        # Should provide text, html or image_path
        self.assertIn('text', res)
        self.assertIn('html', res)

    def test_html_ui_no_emojis_in_web_and_ai_modals(self):
        html_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'assets', 'index.html')
        with open(html_path, 'r', encoding='utf-8') as f:
            html = f.read()

        # Check url-modal
        url_modal_match = re.search(r'<div id="url-modal"[\s\S]*?</div>\s*</div>\s*</div>', html)
        self.assertIsNotNone(url_modal_match)
        url_modal_html = url_modal_match.group(0)

        # Check that old emojis are removed
        self.assertNotIn('&#127760;', url_modal_html)
        self.assertNotIn('&#128203;', url_modal_html)
        self.assertNotIn('📥', url_modal_html)
        self.assertNotIn('⚡', url_modal_html)
        self.assertNotIn('🖥️', url_modal_html)

        # Check AI head
        ai_head_match = re.search(r'<div class="ai-head">[\s\S]*?</div>', html)
        self.assertIsNotNone(ai_head_match)
        self.assertNotIn('&#129302;', ai_head_match.group(0))

        # Check update badge
        self.assertNotIn('✨', html)

    def test_app_js_has_close_more_menu_defined(self):
        app_js_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'assets', 'app.js')
        with open(app_js_path, 'r', encoding='utf-8') as f:
            app_js = f.read()
        self.assertIn('function closeMoreMenu()', app_js)

    def test_convert_js_supports_multi_file_ocr(self):
        convert_js_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'assets', 'js', 'features', 'convert.js')
        with open(convert_js_path, 'r', encoding='utf-8') as f:
            convert_js = f.read()
        self.assertIn('py.choose_many_files', convert_js)
        self.assertIn("mode === 'ocr'", convert_js)

    def test_statusbar_version_and_menu_version_label(self):
        html_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'assets', 'index.html')
        with open(html_path, 'r', encoding='utf-8') as f:
            html = f.read()
        self.assertIn('id="status-version"', html)
        self.assertIn('id="menu-version-label"', html)
        self.assertIn('当前版本 v2.3.1', html)

    def test_ai_settings_close_and_autostart(self):
        ai_js_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'assets', 'js', 'features', 'ai.js')
        with open(ai_js_path, 'r', encoding='utf-8') as f:
            ai_js = f.read()
        self.assertIn('function closeAiModal(id)', ai_js)

        html_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'assets', 'index.html')
        with open(html_path, 'r', encoding='utf-8') as f:
            html = f.read()
        self.assertIn('id="btn-autostart"', html)
        self.assertIn('id="autostart-status-label"', html)

        # Ensure no gradient in .ai-head in style.css
        css_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'assets', 'style.css')
        with open(css_path, 'r', encoding='utf-8') as f:
            css = f.read()
        ai_head_rule = re.search(r'\.ai-head\s*\{([^}]+)\}', css)
        self.assertIsNotNone(ai_head_rule)
        self.assertNotIn('gradient', ai_head_rule.group(1))

        # Test Api get_autostart
        api = readmd.Api()
        auto = api.get_autostart()
        self.assertIsInstance(auto, bool)

    def test_multi_platform_specs_and_harmonyos(self):
        root = os.path.dirname(os.path.dirname(__file__))
        
        # 1. Linglong spec for UOS / Deepin
        linglong_path = os.path.join(root, 'packages', 'linglong', 'linglong.yaml')
        self.assertTrue(os.path.exists(linglong_path))
        with open(linglong_path, 'r', encoding='utf-8') as f:
            ll = f.read()
        self.assertIn('io.github.natsummerance.readmd', ll)
        self.assertIn('version: 2.3.', ll)

        # 2. HarmonyOS NEXT project structure and bridge
        harmony_pkg = os.path.join(root, 'packages', 'harmonyos-app', 'package.json')
        self.assertTrue(os.path.exists(harmony_pkg))
        with open(harmony_pkg, 'r', encoding='utf-8') as f:
            hp = f.read()
        self.assertIn('"version": "2.3.', hp)

        harmony_bridge = os.path.join(root, 'packages', 'harmonyos-app', 'entry', 'src', 'main', 'ets', 'bridge', 'ReadMDBridge.ets')
        self.assertTrue(os.path.exists(harmony_bridge))
        with open(harmony_bridge, 'r', encoding='utf-8') as f:
            hb = f.read()
        self.assertIn('class ReadMDBridge', hb)
        self.assertIn('@ohos.pasteboard', hb)
        self.assertIn('@ohos.file.picker', hb)

        # 3. Linux build scripts and desktop integration
        desktop_path = os.path.join(root, 'scripts', 'linux', 'io.github.natsummerance.readmd.desktop')
        self.assertTrue(os.path.exists(desktop_path))
        with open(desktop_path, 'r', encoding='utf-8') as f:
            dt = f.read()
        self.assertIn('Exec=readmd', dt)
        self.assertIn('MimeType=text/markdown;', dt)

        # 4. MCP Server version consistency
        mcp_path = os.path.join(root, 'packages', 'mcp-server', 'readmd_mcp_server.py')
        self.assertTrue(os.path.exists(mcp_path))
        with open(mcp_path, 'r', encoding='utf-8') as f:
            mcp = f.read()
        self.assertIn('"version": "2.3.', mcp)

        # 5. VSCode extension version consistency
        vscode_pkg = os.path.join(root, 'packages', 'vscode-extension', 'package.json')
        self.assertTrue(os.path.exists(vscode_pkg))
        with open(vscode_pkg, 'r', encoding='utf-8') as f:
            vp = f.read()
        self.assertIn('"version": "2.3.', vp)


if __name__ == '__main__':
    unittest.main()


