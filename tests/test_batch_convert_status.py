# -*- coding: utf-8 -*-
"""Unit tests for batch conversion worker status lifecycle."""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

import readmd


class TestBatchConvertStatus(unittest.TestCase):
    """Verify batch conversion worker status lifecycle."""

    def test_convert_worker_status_progression(self):
        readmd.RM.load_forced('convert')
        with tempfile.NamedTemporaryFile(suffix='.md', mode='w', encoding='utf-8', delete=False) as f:
            f.write('# Header\nContent')
            tmp_path = f.name

        try:
            job = {
                'id': 'test_job_1',
                'cancel': False,
                'overwrite': True,
                'items': [
                    {'id': 'it_1', 'src': tmp_path, 'status': 'queued', 'done': False}
                ]
            }
            readmd._convert_worker(job)
            it = job['items'][0]
            self.assertTrue(it['done'])
            self.assertEqual(it['status'], 'ok')
        finally:
            if os.path.isfile(tmp_path):
                os.unlink(tmp_path)

    def test_convert_worker_respects_cancel_flag(self):
        with tempfile.NamedTemporaryFile(suffix='.md', mode='w', encoding='utf-8', delete=False) as f:
            f.write('# Cancel Test')
            tmp_path = f.name

        try:
            job = {
                'id': 'test_job_2',
                'cancel': True,
                'items': [
                    {'id': 'it_canceled', 'src': tmp_path, 'status': 'queued', 'done': False}
                ]
            }
            readmd._convert_worker(job)
            it = job['items'][0]
            self.assertTrue(it['done'])
            self.assertEqual(it['status'], 'canceled')
        finally:
            if os.path.isfile(tmp_path):
                os.unlink(tmp_path)


if __name__ == '__main__':
    unittest.main()
