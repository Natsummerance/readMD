# -*- coding: utf-8 -*-
"""测试上传目录与拖入文件转换逻辑：确保同名文件不触发 skipped 误报。"""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

import readmd
from src.readmd_core.config import DATA_DIR


class TestConvertBatchUpload(unittest.TestCase):
    """测试拖拽上传与批量/单项转换流水线。"""

    def setUp(self):
        self.upload_dir = os.path.join(DATA_DIR, 'uploads')
        os.makedirs(self.upload_dir, exist_ok=True)
        readmd.RM.load_forced('convert')

    def test_upload_file_cleans_stale_md(self):
        """测试上传同名文件时，调用 _do_upload 自动清理旧有的同名 .md 产物。"""
        import io
        test_name = 'test_drag_clean.docx'
        test_doc = os.path.join(self.upload_dir, test_name)
        test_md = os.path.join(self.upload_dir, 'test_drag_clean.md')

        with open(test_md, 'w', encoding='utf-8') as f:
            f.write('# Stale Old Content')
        self.assertTrue(os.path.isfile(test_md))

        handler = object.__new__(readmd.Handler)
        body = b'dummy-docx-content'
        handler.headers = {'Content-Length': str(len(body))}
        handler.rfile = io.BytesIO(body)
        sent_status = []
        sent_json = []
        handler._send_json = lambda status, obj: (sent_status.append(status), sent_json.append(obj))

        handler._do_upload('.docx', test_name)

        self.assertEqual(sent_status, [200])
        self.assertTrue(os.path.isfile(test_doc))
        self.assertFalse(os.path.isfile(test_md))
        for p in (test_doc, test_md):
            try:
                os.remove(p)
            except OSError:
                pass

    def test_convert_worker_auto_overwrites_upload_dir(self):
        """测试 _convert_worker 在处理 uploads 目录文件时，不因旧文件已存在而判定为 skipped。"""
        test_tex = os.path.join(self.upload_dir, 'test_paper.tex')
        test_md = os.path.join(self.upload_dir, 'test_paper.md')

        with open(test_tex, 'w', encoding='utf-8') as f:
            f.write(r'\title{Sample Title}\begin{document}Hello LaTeX\end{document}')

        with open(test_md, 'w', encoding='utf-8') as f:
            f.write('# Old Markdown Content')
        self.assertTrue(os.path.isfile(test_md))

        job = {
            'id': 'test_job_1',
            'overwrite': False,
            'running': True,
            'finished': False,
            'cancel': False,
            'items': [{
                'src': test_tex,
                'planned_out': test_md,
                'status': 'queued',
                'done': False
            }]
        }

        readmd._convert_worker(job)

        it = job['items'][0]
        self.assertEqual(it['status'], 'ok')
        self.assertNotEqual(it.get('status'), 'skipped')
        self.assertTrue(it['done'])
        self.assertTrue(os.path.isfile(test_md))

        with open(test_md, 'r', encoding='utf-8') as f:
            content = f.read()
        self.assertIn('Hello LaTeX', content)
        self.assertNotIn('Old Markdown Content', content)

        try:
            os.remove(test_tex)
            os.remove(test_md)
        except OSError:
            pass


if __name__ == '__main__':
    unittest.main()
