# -*- coding: utf-8 -*-
"""Unit tests for Universal Open and AI Ecosystem integration."""

import os
import sys
import unittest
import tempfile
import shutil
import json
import re

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

from src.readmd_modules.convert import EXT_TO_LANG, code2md, csv2md, convert_verbose
import readmd


class TestUniversalOpenAndAiEcosystem(unittest.TestCase):
    """Test universal document handling, code conversion, and AI ecosystem UI integrity."""

    def setUp(self):
        self.temp_dir = tempfile.mkdtemp()

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_ext_to_lang_coverage(self):
        """Verify EXT_TO_LANG supports major programming, config, shell, and data formats."""
        expected_exts = [
            '.toml', '.yaml', '.yml', '.json', '.ini', '.cfg', '.conf',
            '.bat', '.cmd', '.ps1', '.sh', '.py', '.js', '.ts', '.rs',
            '.go', '.c', '.cpp', '.java', '.kt', '.sql', '.css',
            '.log', '.csv', '.tsv'
        ]
        for ext in expected_exts:
            self.assertIn(ext, EXT_TO_LANG, f"Missing extension mapping for {ext}")

    def test_code2md_generation(self):
        """Verify code2md wraps raw code in structured Markdown with metadata."""
        test_file = os.path.join(self.temp_dir, "config.toml")
        with open(test_file, "w", encoding="utf-8") as f:
            f.write("name = 'readmd'\nversion = '2.3.7'\n")
        md = code2md(test_file, ext=".toml")
        self.assertIn("# config.toml", md)
        self.assertIn("`toml`", md)
        self.assertIn("```toml\nname = 'readmd'", md)

    def test_csv2md_conversion(self):
        """Verify csv2md structures tabular data into clean Markdown tables."""
        test_file = os.path.join(self.temp_dir, "users.csv")
        with open(test_file, "w", encoding="utf-8") as f:
            f.write("id,name,role\n1,Alice,Admin\n2,Bob,User\n")
        md = csv2md(test_file, delimiter=",")
        self.assertIn("| id | name | role |", md)
        self.assertIn("| --- | --- | --- |", md)
        self.assertIn("| 1 | Alice | Admin |", md)
        self.assertIn("| 2 | Bob | User |", md)

    def test_convert_verbose_with_fallback(self):
        """Verify convert_verbose handles non-standard text formats gracefully."""
        test_file = os.path.join(self.temp_dir, "custom.log")
        with open(test_file, "w", encoding="utf-8") as f:
            f.write("2026-08-27 INFO ReadMD startup OK\n2026-08-27 DEBUG Memory clean\n")

        text, engine, err = convert_verbose(test_file)
        self.assertIsNone(err, f"convert_verbose failed with error: {err}")
        self.assertIn("custom.log", text)
        self.assertIn("```log", text)

    def test_html_ai_elements_presence(self):
        """Verify all new AI buttons and floating bars exist in assets/index.html."""
        html_path = os.path.join(ROOT_DIR, "assets", "index.html")
        with open(html_path, "r", encoding="utf-8") as f:
            html_content = f.read()

        self.assertNotIn('id="btn-edit-ai-assistant"', html_content)
        self.assertIn('id="btn-ai"', html_content)
        self.assertIn('id="cm-sel-ai"', html_content)
        self.assertIn('id="edit-ai-bar"', html_content)
        self.assertIn('class="export-main-col"', html_content)
        self.assertIn('class="exp-ai-style-card"', html_content)
        self.assertIn('id="exp-ai-prompt"', html_content)
        self.assertIn('id="exp-ai-gen-btn"', html_content)

        # Check exportai.title in zh-CN
        zh_path = os.path.join(ROOT_DIR, "assets", "i18n", "zh-CN.json")
        with open(zh_path, "r", encoding="utf-8") as f:
            zh_data = json.load(f)
        self.assertEqual(zh_data["exportai.title"], "AI 排版")

    def test_zero_emojis_in_new_i18n_keys(self):
        """Verify no emojis exist in the newly registered i18n keys across all languages."""
        emoji_re = re.compile(r'[\U00010000-\U0010ffff]|[\u2600-\u27bf]|[\u2300-\u23ff]|[\u2b50-\u2b55]')
        i18n_dir = os.path.join(ROOT_DIR, "assets", "i18n")

        target_keys = [
            "codebar.format", "codebar.lines", "codebar.aiToMd", "codebar.edit",
            "codebar.aiExplain", "codebar.copyCode", "editai.title", "editai.placeholder",
            "editai.actComplete", "editai.actPolish", "editai.actFix", "editai.actTranslate",
            "editai.apply", "editai.insert", "editai.discard", "editai.generating",
            "exportai.title", "exportai.placeholder", "exportai.generateBtn",
            "exportai.generating", "exportai.applied", "convert.aiFixFormat", "convert.aiFixDesc"
        ]

        for fname in os.listdir(i18n_dir):
            if not fname.endswith(".json") or fname.endswith("meta.json"):
                continue
            fpath = os.path.join(i18n_dir, fname)
            with open(fpath, "r", encoding="utf-8") as f:
                data = json.load(f)

            for k in target_keys:
                self.assertIn(k, data, f"Key {k} missing in {fname}")
                val = data[k]
                matches = emoji_re.findall(val)
                self.assertEqual(len(matches), 0, f"Found emoji in {fname} under key {k}: {matches}")


    def test_top_ai_button_dispatch_code_structure(self):
        """Verify handleTopAiButtonClick handles all three states in assets/js/features/ai.js."""
        ai_js_path = os.path.join(ROOT_DIR, "assets", "js", "features", "ai.js")
        with open(ai_js_path, "r", encoding="utf-8") as f:
            ai_js = f.read()

        self.assertIn("function handleTopAiButtonClick()", ai_js)
        self.assertIn("state.editing", ai_js)
        self.assertIn("state.pvLayout", ai_js)
        self.assertIn("openEditAiBar", ai_js)
        self.assertIn("toggleAiPanel", ai_js)

        app_js_path = os.path.join(ROOT_DIR, "assets", "app.js")
        with open(app_js_path, "r", encoding="utf-8") as f:
            app_js = f.read()
        self.assertIn("handleTopAiButtonClick", app_js)

    def test_export_ai_card_in_right_column(self):
        """Verify exp-ai-style-card is positioned in the right column above export-opts."""
        html_path = os.path.join(ROOT_DIR, "assets", "index.html")
        with open(html_path, "r", encoding="utf-8") as f:
            html = f.read()

        card_idx = html.find('class="exp-ai-style-card"')
        opts_idx = html.find('id="export-opts"')
        preview_card_idx = html.find('id="export-preview-card"')
        main_col_idx = html.find('class="export-main-col"')

        self.assertNotEqual(card_idx, -1)
        self.assertNotEqual(opts_idx, -1)
        self.assertNotEqual(main_col_idx, -1)
        # card is inside export-main-col and before export-opts
        self.assertTrue(main_col_idx < card_idx < opts_idx)
        # preview card is on the left before export-main-col
        self.assertTrue(preview_card_idx < main_col_idx)

    def test_export_preview_multipage_pagination(self):
        """Verify export.js contains splitMdForExportPreview and multi-page sheet rendering logic."""
        export_js_path = os.path.join(ROOT_DIR, "assets", "js", "features", "export.js")
        with open(export_js_path, "r", encoding="utf-8") as f:
            export_js = f.read()

        self.assertIn("function paginateHtmlIntoExportSheets(", export_js)
        self.assertIn("export-preview-page-sheet", export_js)
        self.assertIn("export-page-header", export_js)
        self.assertIn("export-page-body", export_js)
        self.assertIn("export-page-footer", export_js)

        css_path = os.path.join(ROOT_DIR, "assets", "style.css")
        with open(css_path, "r", encoding="utf-8") as f:
            css = f.read()

        self.assertIn(".export-preview-page-sheet", css)
        self.assertIn(".export-page-header", css)
        self.assertIn(".export-page-footer", css)
        self.assertIn("--preview-canvas-bg", css)

    def test_toc_cleared_on_home_and_close_all_tabs(self):
        """Verify history.js goHome clears toc-list and toc.js has welcome state guard."""
        history_js_path = os.path.join(ROOT_DIR, "assets", "js", "core", "history.js")
        with open(history_js_path, "r", encoding="utf-8") as f:
            history_js = f.read()
        self.assertIn("$('toc-list')", history_js)
        self.assertIn("tocCache = { source: null, pageCount: 0 }", history_js)

        toc_js_path = os.path.join(ROOT_DIR, "assets", "js", "reader", "toc.js")
        with open(toc_js_path, "r", encoding="utf-8") as f:
            toc_js = f.read()
        self.assertIn("state.mode === 'welcome'", toc_js)

    def test_presentation_zen_mode_and_fullscreen(self):
        """Verify presentation mode Zen mode, controls fade, and fullscreen toggle implementation."""
        render_js_path = os.path.join(ROOT_DIR, "assets", "js", "reader", "render.js")
        with open(render_js_path, "r", encoding="utf-8") as f:
            render_js = f.read()
        self.assertIn("togglePresentationFullscreen", render_js)
        self.assertIn("presZenActive", render_js)
        self.assertIn("handlePresPointerMove", render_js)
        self.assertIn("set-zen-controls", render_js)

        css_path = os.path.join(ROOT_DIR, "assets", "style.css")
        with open(css_path, "r", encoding="utf-8") as f:
            css = f.read()
        self.assertIn(".pres-zen-active", css)
        self.assertIn(".pres-toolbar-revealed", css)

    def test_fix_modal_ai_button_and_format_fix_action(self):
        """Verify fix modal has AI fix button and ai.py has format_fix prompt."""
        html_path = os.path.join(ROOT_DIR, "assets", "index.html")
        with open(html_path, "r", encoding="utf-8") as f:
            html = f.read()
        self.assertIn('id="fix-ai-btn"', html)

        fixes_js_path = os.path.join(ROOT_DIR, "assets", "js", "reader", "fixes.js")
        with open(fixes_js_path, "r", encoding="utf-8") as f:
            fixes_js = f.read()
        self.assertIn("handleAiDocumentFix", fixes_js)

        ai_py_path = os.path.join(ROOT_DIR, "src", "readmd_modules", "ai.py")
        with open(ai_py_path, "r", encoding="utf-8") as f:
            ai_py = f.read()
        self.assertIn('"format_fix":', ai_py)


if __name__ == "__main__":
    unittest.main()
