# -*- coding: utf-8 -*-
"""tests for tools/i18n_sync.py sync_locale_file — i18n fill path (TDD)."""

import importlib.util
import json
import os
import tempfile
import unittest

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOL_FILE = os.path.join(ROOT_DIR, 'tools', 'i18n_sync.py')


def _load_tool():
    spec = importlib.util.spec_from_file_location('i18n_sync_tool', TOOL_FILE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _fake_translate(text, lang):
    return '[' + lang + '] ' + text


class TestSyncLocaleFile(unittest.TestCase):
    def test_fills_only_missing_keys_and_preserves_existing(self):
        mod = _load_tool()
        with tempfile.TemporaryDirectory() as td:
            fp = os.path.join(td, 'xx.json')
            base = {
                'batch.title': '批量转换工作台',
                'menu.batch': '批量转换',
                'batch.cancel': '取消',
            }
            with open(fp, 'w', encoding='utf-8') as f:
                json.dump({'batch.cancel': 'KeepExisting', 'extra.key': 'keep-me'},
                          f, ensure_ascii=False)

            def fake(text, lang):
                return '[' + lang + '] ' + text

            filled = mod.sync_locale_file(fp, base, 'en', fake)

            self.assertEqual(sorted(filled), ['batch.title', 'menu.batch'])
            with open(fp, 'r', encoding='utf-8') as f:
                d = json.load(f)
            self.assertEqual(d['batch.cancel'], 'KeepExisting')
            self.assertEqual(d['extra.key'], 'keep-me')
            self.assertEqual(d['batch.title'], '[en] 批量转换工作台')
            self.assertEqual(d['menu.batch'], '[en] 批量转换')

    def test_translate_failure_none_leaves_key_unwritten(self):
        mod = _load_tool()
        with tempfile.TemporaryDirectory() as td:
            fp = os.path.join(td, 'xx.json')
            with open(fp, 'w', encoding='utf-8') as f:
                json.dump({'batch.cancel': 'Cancel'}, f, ensure_ascii=False)

            def failing(text, lang):
                return None

            filled = mod.sync_locale_file(
                fp, {'batch.title': '批量转换工作台'}, 'en', failing)

            self.assertEqual(filled, [])
            with open(fp, 'r', encoding='utf-8') as f:
                d = json.load(f)
            self.assertNotIn('batch.title', d)
            self.assertEqual(d['batch.cancel'], 'Cancel')

    def test_no_missing_keys_returns_empty_and_keeps_file(self):
        mod = _load_tool()
        with tempfile.TemporaryDirectory() as td:
            fp = os.path.join(td, 'xx.json')
            with open(fp, 'w', encoding='utf-8') as f:
                json.dump({'batch.title': 'already-there'}, f, ensure_ascii=False)
            before = open(fp, 'r', encoding='utf-8').read()

            def boom(text, lang):
                raise AssertionError('translate must not be called when complete')

            filled = mod.sync_locale_file(fp, {'batch.title': '批量'}, 'en', boom)

            self.assertEqual(filled, [])
            self.assertEqual(open(fp, 'r', encoding='utf-8').read(), before)


if __name__ == '__main__':
    unittest.main()
