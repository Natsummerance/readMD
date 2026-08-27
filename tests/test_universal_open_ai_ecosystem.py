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
            '.go', '.c', '.cpp', '.java', '.kt', '.sql', '.html', '.css',
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

        self.assertIn('id="btn-edit-ai-assistant"', html_content)
        self.assertIn('id="cm-sel-ai"', html_content)
        self.assertIn('id="edit-ai-bar"', html_content)
        self.assertIn('class="exp-ai-style-card"', html_content)
        self.assertIn('id="exp-ai-prompt"', html_content)
        self.assertIn('id="exp-ai-gen-btn"', html_content)

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


if __name__ == "__main__":
    unittest.main()
