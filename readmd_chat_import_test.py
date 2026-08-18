# -*- coding: utf-8 -*-
"""Focused regression coverage for chat export import (no network required)."""

import io
import json
import os
import sys
import tempfile
import unittest
import zipfile
from unittest import mock

ROOT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ROOT)

from readmd_modules import chat_import as CI


class ChatImportTest(unittest.TestCase):
    def parse(self, data, name='export.json'):
        return CI.result(CI.import_bytes(json.dumps(data).encode('utf-8'), name))

    def test_known_json_shapes_and_generic_messages(self):
        cases = [
            ({'sessions': [{'title': 'ReadMD', 'messages': [
                {'role': 'user', 'content': '你好'}, {'role': 'assistant', 'content': '你好！'}]}]}, 'ReadMD'),
            ({'title': 'GPT', 'mapping': {'a': {'message': {
                'author': {'role': 'user'}, 'content': {'parts': ['```py\nprint(1)\n```']}}},
                'b': {'message': {'author': {'role': 'assistant'}, 'content': {'parts': ['ok']}}}}}, 'GPT'),
            ({'title': 'Claude', 'chat_messages': [
                {'role': 'human', 'text': 'a'}, {'role': 'assistant', 'text': 'b'}]}, 'Claude'),
            ({'title': 'Gemini', 'model': 'gemini', 'messages': [
                {'role': 'user', 'parts': ['a']}, {'role': 'model', 'parts': ['b']}]}, 'Gemini'),
            ({'name': 'Generic', 'items': [
                {'author_role': 'user', 'message': 'a'}, {'author_role': 'bot', 'message': 'b'}]}, 'Generic'),
        ]
        for export, title in cases:
            with self.subTest(title=title):
                result = self.parse(export)
                self.assertEqual(result['message_count'], 2, result)
                self.assertIn('## 用户', result['content'])
                self.assertIn('## AI 助手', result['content'])
                self.assertNotIn('system', result['content'].lower())

    def test_html_plain_text_and_markdown_preserve_rich_content(self):
        page = '''<html><head><title>Chat</title></head><body>
          <div data-message-author-role="user">公式 $x^2$</div>
          <div data-message-author-role="assistant"><pre><code>print(1)</code></pre>
          <table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table></div>
          <script>secret()</script></body></html>'''
        result = CI.result(CI.import_bytes(page.encode('utf-8'), 'chat.html'))
        self.assertIn('$x^2$', result['content'])
        self.assertIn('```', result['content'])
        self.assertIn('| A', result['content'])
        self.assertNotIn('secret()', result['content'])
        text = CI.result(CI.import_bytes(b'# Kept\n\n| A | B |\n|---|---|\n|1|2|', 'note.md'))
        self.assertIn('| A | B |', text['content'])

    def test_system_content_and_dangerous_html_are_removed(self):
        export = {'messages': [
            {'role': 'system', 'content': 'you are secret'},
            {'role': 'user', 'content': '<img src=x onerror="steal()"><script>bad()</script>safe'},
            {'role': 'assistant', 'content': '[click](javascript:bad())'}]}
        result = self.parse(export)
        self.assertNotIn('secret', result['content'])
        self.assertNotIn('onerror', result['content'])
        self.assertNotIn('<script', result['content'].lower())
        self.assertNotIn('javascript:', result['content'].lower())

    def test_metadata_attachments_html_and_markdown_uris_are_safe(self):
        conversation = CI.Conversation('<img src=x onerror=bad()> title', '<b>source</b>',
                                       'javascript:alert(1)', '<svg onload=bad()>today', [
            CI.Message('user', '&lt;img src=x onerror=bad()&gt;safe [bad](data:text/html,x) '
                       '![bad](file:///secret) [good](https://example.com/a)', attachments=[
                           {'name': '<img onerror=bad()>file.md'}])])
        result = CI.result(conversation)
        self.assertNotIn('<img', result['content'].lower())
        self.assertNotIn('onerror', result['content'].lower())
        self.assertNotIn('javascript:', result['content'].lower())
        self.assertNotIn('data:text', result['content'].lower())
        self.assertNotIn('file:', result['content'].lower())
        self.assertIn('[good](https://example.com/a)', result['content'])
        self.assertEqual(result['source_url'], '')

    def test_zip_rejects_traversal_bomb_and_expansion_limit(self):
        def archive(name, data, compression=zipfile.ZIP_DEFLATED):
            buf = io.BytesIO()
            with zipfile.ZipFile(buf, 'w', compression) as zf: zf.writestr(name, data)
            return buf.getvalue()
        with self.assertRaises(CI.ChatImportError) as caught:
            CI.import_bytes(archive('../escape.json', '{}'), 'bad.zip')
        self.assertEqual(caught.exception.code, 'unsafe_zip_path')
        with self.assertRaises(CI.ChatImportError) as caught:
            CI.import_bytes(archive('bomb.txt', b'a' * (2 * 1024 * 1024)), 'bomb.zip')
        self.assertEqual(caught.exception.code, 'suspicious_zip')
        with mock.patch.object(CI, 'MAX_ZIP_EXPANDED', 32):
            with self.assertRaises(CI.ChatImportError) as caught:
                CI.import_bytes(archive('large.json', b'{' + b' ' * 64 + b'}', zipfile.ZIP_STORED), 'large.zip')
        self.assertEqual(caught.exception.code, 'zip_expanded_too_large')

    def test_zip_allows_partial_success_and_file_type_boundary(self):
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'w') as zf:
            zf.writestr('bad.json', b'{')
            zf.writestr('good.json', json.dumps({'messages': [
                {'role': 'user', 'content': 'a'}, {'role': 'assistant', 'content': 'b'}]}))
        result = CI.result(CI.import_bytes(buf.getvalue(), 'export.zip'))
        self.assertTrue(result['warnings'])
        with tempfile.TemporaryDirectory() as td:
            path = os.path.join(td, 'bad.exe')
            with open(path, 'wb') as handle: handle.write(b'x')
            with self.assertRaises(CI.ChatImportError) as caught:
                CI.import_file(path)
            self.assertEqual(caught.exception.code, 'unsupported_type')

    def test_json_text_and_zip_member_limits_apply_before_parsing(self):
        payload = b'{' + b' ' * (11 * 1024 * 1024) + b'}'
        with self.assertRaises(CI.ChatImportError) as caught:
            CI.import_bytes(payload, 'large.json')
        self.assertEqual(caught.exception.code, 'too_large')
        with tempfile.TemporaryDirectory() as td:
            path = os.path.join(td, 'large.json')
            with open(path, 'wb') as handle:
                handle.write(payload)
            with self.assertRaises(CI.ChatImportError) as caught:
                CI.import_file(path)
        self.assertEqual(caught.exception.code, 'too_large')
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'w', zipfile.ZIP_STORED) as zf:
            zf.writestr('large.json', b'{' + b' ' * 64 + b'}')
        with mock.patch.object(CI, 'MAX_TEXT_BYTES', 32):
            with self.assertRaises(CI.ChatImportError) as caught:
                CI.import_bytes(buf.getvalue(), 'large.zip')
        self.assertEqual(caught.exception.code, 'zip_member_too_large')

    def test_zip_member_count_is_rejected_before_processing(self):
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'w') as zf:
            for index in range(CI.MAX_ZIP_MEMBERS + 1):
                zf.writestr('skip-%04d.bin' % index, b'x')
        with self.assertRaises(CI.ChatImportError) as caught:
            CI.import_bytes(buf.getvalue(), 'many.zip')
        self.assertEqual(caught.exception.code, 'too_many_zip_members')

    def test_login_or_non_chat_html_has_actionable_error(self):
        page = '<html><title>Sign in</title><body><form>Log in</form></body></html>'
        conversation = CI.import_bytes(page.encode('utf-8'), 'login.html')
        with self.assertRaises(CI.ChatImportError) as caught:
            CI.result(conversation)
        self.assertEqual(caught.exception.code, 'no_conversation')
        self.assertIn('登录页', caught.exception.message)


if __name__ == '__main__':
    unittest.main()
