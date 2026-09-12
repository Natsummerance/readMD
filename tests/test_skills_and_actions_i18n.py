# -*- coding: utf-8 -*-
"""
Tests for full 46-language localization of Builtin Skills, Actions,
Template Categories, and Default Action text.
"""
import json
import os
import re
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
I18N_DIR = os.path.join(ROOT, "assets", "i18n")

ACTIONS = [
    "quick_read", "polish", "proofread", "to_english", "to_chinese",
    "action_items", "continue", "ask", "summary", "outline", "weekly",
    "code_review", "fix_format"
]

SKILL_IDS = [
    "readmd-quick-read", "readmd-polish", "readmd-proofread", "readmd-translate",
    "readmd-todo", "readmd-continue", "readmd-ask", "readmd-summary",
    "readmd-outline", "readmd-weekly", "readmd-code-review", "readmd-format-fix"
]

CATEGORIES = ["general", "writing", "coding", "academic", "custom"]


class TestSkillsAndActionsI18n(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cjk_re = re.compile(r'[\u4e00-\u9fff]')
        cls.json_files = [f for f in sorted(os.listdir(I18N_DIR)) if f.endswith('.json') and f != 'meta.json']
        with open(os.path.join(I18N_DIR, 'en.json'), 'r', encoding='utf-8') as f:
            cls.en_dict = json.load(f)

    def test_all_46_languages_present(self):
        self.assertEqual(len(self.json_files), 46, "Must have exactly 46 language JSON files")

    def test_default_action_localized_everywhere(self):
        """ai.defaultAction must exist, be non-empty, and not expose Chinese in non-Chinese languages."""
        for fn in self.json_files:
            lang = fn[:-5]
            with open(os.path.join(I18N_DIR, fn), 'r', encoding='utf-8') as f:
                d = json.load(f)
            val = d.get('ai.defaultAction')
            self.assertIsNotNone(val, f"ai.defaultAction missing in {fn}")
            self.assertNotEqual(val.strip(), "", f"ai.defaultAction empty in {fn}")
            self.assertNotEqual(val, "ai.defaultAction", f"ai.defaultAction copied key in {fn}")
            if not lang.startswith('zh') and not lang.startswith('ja'):
                self.assertFalse(self.cjk_re.search(val), f"ai.defaultAction has Chinese characters in {fn}: {val}")

    def test_categories_localized_everywhere(self):
        """All 5 categories must exist in all 46 languages."""
        for cat in CATEGORIES:
            key = f"ai.tplCategory.{cat}"
            for fn in self.json_files:
                lang = fn[:-5]
                with open(os.path.join(I18N_DIR, fn), 'r', encoding='utf-8') as f:
                    d = json.load(f)
                val = d.get(key)
                self.assertIsNotNone(val, f"{key} missing in {fn}")
                self.assertNotEqual(val.strip(), "", f"{key} empty in {fn}")
                self.assertNotEqual(val, key, f"{key} copied key in {fn}")
                if not lang.startswith('zh') and not lang.startswith('ja'):
                    self.assertFalse(self.cjk_re.search(val), f"{key} has Chinese characters in {fn}: {val}")

    def test_actions_localized_everywhere(self):
        """All 13 actions must exist in all 46 languages without Chinese leakage in non-CJK."""
        for act in ACTIONS:
            key = f"ai.action.{act}"
            for fn in self.json_files:
                lang = fn[:-5]
                with open(os.path.join(I18N_DIR, fn), 'r', encoding='utf-8') as f:
                    d = json.load(f)
                val = d.get(key)
                self.assertIsNotNone(val, f"{key} missing in {fn}")
                self.assertNotEqual(val.strip(), "", f"{key} empty in {fn}")
                self.assertNotEqual(val, key, f"{key} copied key in {fn}")
                if not lang.startswith('zh') and not lang.startswith('ja'):
                    self.assertFalse(self.cjk_re.search(val), f"{key} has Chinese characters in {fn}: {val}")

    def test_skills_localized_everywhere(self):
        """All Chinese-origin skills must have skill.<id>.name translated in all 46 languages."""
        for sid in SKILL_IDS:
            key = f"skill.{sid}.name"
            for fn in self.json_files:
                lang = fn[:-5]
                with open(os.path.join(I18N_DIR, fn), 'r', encoding='utf-8') as f:
                    d = json.load(f)
                val = d.get(key)
                self.assertIsNotNone(val, f"{key} missing in {fn}")
                self.assertNotEqual(val.strip(), "", f"{key} empty in {fn}")
                self.assertNotEqual(val, key, f"{key} copied key in {fn}")
                if not lang.startswith('zh') and not lang.startswith('ja'):
                    self.assertFalse(self.cjk_re.search(val), f"{key} has Chinese characters in {fn}: {val}")


if __name__ == '__main__':
    unittest.main()
