# -*- coding: utf-8 -*-
import io
import os
import tempfile
import unittest
import zipfile
from src.readmd_modules.convert import extract_zip_archive


class TestZipArchiveSecurity(unittest.TestCase):
    def test_extract_supported_and_skip_unsupported(self):
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'w') as zf:
            zf.writestr('doc1.md', '# Hello')
            zf.writestr('notes.txt', 'some notes')
            zf.writestr('script.py', 'print(1)')
            zf.writestr('malicious.exe', b'binary data')
            zf.writestr('archive.iso', b'iso content')
            zf.writestr('sub/deep/test.docx', b'docx content')

        data = buf.getvalue()
        with tempfile.TemporaryDirectory() as td:
            res = extract_zip_archive(data, base_temp_dir=td)
            self.assertTrue(res['ok'])
            self.assertEqual(len(res['paths']), 4)
            self.assertEqual(res['skipped'], 2)
            self.assertEqual(res['reasons']['unsupported_format'], 2)

    def test_path_traversal_rejected(self):
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'w') as zf:
            zf.writestr('../evil.md', '# Evil')
            zf.writestr('valid.md', '# Valid')

        data = buf.getvalue()
        with tempfile.TemporaryDirectory() as td:
            res = extract_zip_archive(data, base_temp_dir=td)
            self.assertTrue(res['ok'])
            self.assertEqual(len(res['paths']), 1)
            self.assertEqual(res['skipped'], 1)
            self.assertEqual(res['reasons']['invalid_path'], 1)


if __name__ == '__main__':
    unittest.main()
