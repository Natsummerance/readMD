# -*- coding: utf-8 -*-
"""Unit tests for AI Assistant V2 features in ReadMD."""

import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))

import readmd


class TestAiFeaturesV2(unittest.TestCase):

    def test_builtin_prompts_integrity(self):
        """Verify built-in prompts have correct keys and quality prompt definitions."""
        prompts = readmd.BUILTIN_PROMPTS
        self.assertGreaterEqual(len(prompts), 8)
        ids = [p['id'] for p in prompts]
        self.assertIn('quick_read', ids)
        self.assertIn('polish', ids)
        self.assertIn('proofread', ids)
        self.assertIn('to_english', ids)
        self.assertIn('to_chinese', ids)
        self.assertIn('action_items', ids)
        self.assertIn('continue', ids)
        self.assertIn('ask', ids)

        # Ensure translation prompts are distinct and not mixed up with weekly report or continue
        en_p = next(p for p in prompts if p['id'] == 'to_english')
        self.assertIn('translator', en_p['system'].lower())
        self.assertEqual(en_p['action'], 'translate_en')

        zh_p = next(p for p in prompts if p['id'] == 'to_chinese')
        self.assertIn('翻译', zh_p['system'])
        self.assertEqual(zh_p['action'], 'translate_zh')

    def test_prompt_template_batch_save_and_crud(self):
        """Verify prompt templates CRUD and batch_save operation."""
        # Get baseline
        res = readmd.load_prompts()
        prompts = res.get('templates', [])
        self.assertIsInstance(prompts, list)
        self.assertGreaterEqual(len(prompts), len(readmd.BUILTIN_PROMPTS))

        # Save single custom prompt
        t1 = {
            'id': 'custom_test_reviewer',
            'name': '学术审稿人',
            'action': 'custom',
            'system': '你是顶会审稿人，对文档做出同行评审意见。',
            'user': '请评审：{doc}'
        }
        saved = readmd.save_prompt(t1)
        self.assertEqual(saved['id'], 'custom_test_reviewer')

        # Verify saved
        prompts_after = readmd.load_prompts().get('templates', [])
        custom_saved = next((p for p in prompts_after if p.get('id') == 'custom_test_reviewer'), None)
        self.assertIsNotNone(custom_saved)
        self.assertEqual(custom_saved['name'], '学术审稿人')

        # Clean up
        readmd.delete_prompt('custom_test_reviewer')
        restored = readmd.load_prompts().get('templates', [])
        self.assertIsNone(next((p for p in restored if p.get('id') == 'custom_test_reviewer'), None))

    def test_ai_html_structure(self):
        """Verify HTML contains split workspace elements and proper AI composer."""
        html_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'assets', 'index.html')
        with open(html_path, 'r', encoding='utf-8') as f:
            html = f.read()

        # Split workspace elements
        self.assertIn('id="ai-panel"', html)
        self.assertIn('id="ai-resize-handle"', html)
        self.assertIn('id="ai-expand-toggle"', html)

        # Top template bar
        self.assertIn('id="ai-template"', html)
        self.assertIn('id="ai-tpl-btn"', html)

        # Quick action chips
        self.assertIn('data-act="quick_read"', html)
        self.assertIn('data-act="polish"', html)
        self.assertIn('data-act="proofread"', html)
        self.assertIn('data-act="translate_en"', html)
        self.assertIn('data-act="translate_zh"', html)
        self.assertIn('data-act="todo"', html)

        # Template modal import/export
        self.assertIn('id="tpl-import-btn"', html)
        self.assertIn('id="tpl-file-input"', html)
        self.assertIn('id="tpl-export-btn"', html)
        self.assertIn('id="tpl-search"', html)


if __name__ == '__main__':
    unittest.main()
