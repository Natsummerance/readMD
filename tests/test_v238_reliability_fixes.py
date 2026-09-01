# -*- coding: utf-8 -*-
"""Regression tests for the V2.3.8 production-audit blockers."""

import json
import os
import tempfile
import unittest
import zipfile
from unittest import mock

import readmd
from src.readmd_core import style_injector
from src.readmd_modules import code_chunk_runner, convert, skill_import


class TestV238ReliabilityFixes(unittest.TestCase):
    def test_code_runner_uses_keyword_contract_and_disposable_cwd(self):
        with tempfile.TemporaryDirectory() as data_dir:
            with mock.patch.dict(os.environ, {'READMD_DATA_DIR': data_dir}, clear=False):
                result = code_chunk_runner.execute_code_chunk(
                    code='print(2 + 2)', lang='python', timeout=5,
                )
                self.assertTrue(result['ok'], result)
                self.assertEqual(result['stdout'], '4')
                self.assertFalse(os.path.exists(os.path.join(data_dir, 'readmd-code-runner')))

                denied = code_chunk_runner.execute_code_chunk(
                    code='print(1)', lang='python', cwd=os.getcwd(), timeout=5,
                )
                self.assertFalse(denied['ok'])
                self.assertEqual(denied['error'], 'cwd_not_allowed')

    def test_code_runner_rejects_network_and_does_not_inherit_credentials(self):
        with mock.patch.dict(os.environ, {
            'READMD_DATA_DIR': tempfile.gettempdir(),
            'OPENROUTER_API_KEY': 'must-not-reach-child',
        }, clear=False):
            denied = code_chunk_runner.execute_code_chunk(
                code='import requests\nrequests.get("https://example.com")',
                lang='python', timeout=5,
            )
            self.assertFalse(denied['ok'])
            self.assertEqual(denied['error'], 'network_not_allowed')

            env_probe = code_chunk_runner.execute_code_chunk(
                code='import os; print(os.environ.get("OPENROUTER_API_KEY", "missing"))',
                lang='python', timeout=5,
            )
            self.assertFalse(env_probe['ok'], env_probe)
            self.assertEqual(env_probe['error'], 'path_access_not_allowed')

    def test_style_save_honors_data_dir_override(self):
        with tempfile.TemporaryDirectory() as data_dir:
            with mock.patch.dict(os.environ, {'READMD_DATA_DIR': data_dir}, clear=False):
                self.assertTrue(style_injector.save_custom_styles('body{}', '<meta>'))
                self.assertEqual(open(os.path.join(data_dir, 'custom.css'), encoding='utf-8').read(), 'body{}')
                self.assertEqual(open(os.path.join(data_dir, 'head.html'), encoding='utf-8').read(), '<meta>')

    def test_rtf_odt_epub_are_readable_or_explicit_errors(self):
        with tempfile.TemporaryDirectory() as td:
            rtf = os.path.join(td, 'sample.rtf')
            with open(rtf, 'wb') as handle:
                handle.write(br'{\rtf1\ansi Production {\b fixture}\par Done}')
            text, engine, error = convert.convert_verbose(rtf)
            self.assertEqual((engine, error), ('rtf', None))
            self.assertIn('Production', text)
            self.assertIn('Done', text)

            # Real-world RTF commonly starts with a font table destination;
            # skipping that scoped group must not swallow the body that follows.
            with open(rtf, 'wb') as handle:
                handle.write(br'{\rtf1\ansi{\fonttbl{\f0 Arial;}}\f0 Body after font table\par Done}')
            text, engine, error = convert.convert_verbose(rtf)
            self.assertEqual((engine, error), ('rtf', None))
            self.assertIn('Body after font table', text)
            self.assertIn('Done', text)

            odt = os.path.join(td, 'sample.odt')
            content_xml = '''<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
 xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
 <office:body><office:text><text:h text:outline-level="1">Heading</text:h>
 <text:p>Body</text:p></office:text></office:body></office:document-content>'''
            with zipfile.ZipFile(odt, 'w') as archive:
                archive.writestr('content.xml', content_xml)
            text, engine, error = convert.convert_verbose(odt)
            self.assertEqual((engine, error), ('odt', None))
            self.assertIn('# Heading', text)
            self.assertIn('Body', text)

            epub = os.path.join(td, 'sample.epub')
            with zipfile.ZipFile(epub, 'w') as archive:
                archive.writestr('OEBPS/chapter.xhtml', '<html><body><h1>Chapter</h1><p>Text</p></body></html>')
            text, engine, error = convert.convert_verbose(epub)
            self.assertEqual((engine, error), ('epub', None))
            self.assertIn('# Chapter', text)
            self.assertIn('Text', text)

    def test_batch_conversion_plans_unique_targets(self):
        with tempfile.TemporaryDirectory() as td:
            first = os.path.join(td, 'same.rtf')
            second = os.path.join(td, 'same.odt')
            with open(first, 'wb') as handle:
                handle.write(br'{\rtf1 First}')
            with zipfile.ZipFile(second, 'w') as archive:
                archive.writestr('content.xml', '<root/>')
            planned = readmd._batch_output_paths([first, second])
            self.assertEqual(os.path.basename(planned[first]), 'same.md')
            self.assertRegex(os.path.basename(planned[second]), r'^same-[0-9a-f]{8}\.md$')
            self.assertNotEqual(planned[first], planned[second])

    def test_skill_source_migration_is_persisted_without_absolute_path(self):
        with tempfile.TemporaryDirectory() as td:
            config_path = os.path.join(td, 'skills.json')
            legacy = {
                'schema_version': 1,
                'sources': [{'source_id': 'legacy', 'source_path': r'C:\\Users\\private\\skills'}],
            }
            with open(config_path, 'w', encoding='utf-8') as handle:
                json.dump(legacy, handle)
            with mock.patch.object(skill_import, 'SKILLS_FILE', config_path):
                loaded = skill_import.list_sources()
            self.assertEqual(loaded[0]['source_label'], 'skills')
            persisted = json.load(open(config_path, encoding='utf-8'))
            self.assertNotIn('source_path', persisted['sources'][0])
            self.assertNotIn('private', json.dumps(persisted))


if __name__ == '__main__':
    unittest.main()
