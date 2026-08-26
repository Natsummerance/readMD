# -*- coding: utf-8 -*-
"""Image save endpoint boundary tests."""
from __future__ import annotations

import base64
import io
import json
import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from readmd import Handler


class ImageSaveTest(unittest.TestCase):
    def call(self, payload: dict):
        handler = object.__new__(Handler)
        body = json.dumps(payload).encode('utf-8')
        handler.headers = {'Content-Length': str(len(body))}
        handler.rfile = io.BytesIO(body)
        responses = []
        handler._send_json = lambda code, obj: responses.append((code, obj))
        handler._api_image_save()
        return responses

    def png_base64(self) -> str:
        from PIL import Image

        output = io.BytesIO()
        Image.new('RGB', (2, 2), '#3b6ef5').save(output, format='PNG')
        return base64.b64encode(output.getvalue()).decode('ascii')

    def test_saves_verified_png_inside_images_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            responses = self.call({
                'dir': tmp,
                'data': self.png_base64(),
                'format': 'png',
                'name': 'diagram.png',
            })
            code, result = responses[-1]
            self.assertEqual(code, 200)
            self.assertTrue(result['ok'])
            self.assertEqual(result['rel'], 'images/diagram.png')
            self.assertTrue(os.path.isfile(os.path.join(tmp, 'images', 'diagram.png')))

    def test_rejects_non_image_payload(self):
        with tempfile.TemporaryDirectory() as tmp:
            responses = self.call({
                'dir': tmp,
                'data': base64.b64encode(b'not an image').decode('ascii'),
                'format': 'png',
                'name': 'safe',
            })
            self.assertEqual(responses[-1][0], 400)
            self.assertFalse(os.path.exists(os.path.join(tmp, 'images')))

    def test_generated_name_receives_verified_extension(self):
        with tempfile.TemporaryDirectory() as tmp:
            responses = self.call({
                'dir': tmp,
                'data': self.png_base64(),
                'format': 'png',
                'name': 'img_123',
            })
            code, result = responses[-1]
            self.assertEqual(code, 200)
            self.assertEqual(result['rel'], 'images/img_123.png')

    def test_malicious_name_cannot_leave_document_scope(self):
        with tempfile.TemporaryDirectory() as tmp:
            responses = self.call({
                'dir': tmp,
                'data': self.png_base64(),
                'format': 'png',
                'name': 'safe..\\..\\outside.png',
            })
            code, result = responses[-1]
            self.assertEqual(code, 200)
            self.assertTrue(result['path'].startswith(os.path.realpath(os.path.join(tmp, 'images'))))
            self.assertFalse(os.path.isfile(os.path.join(os.path.dirname(tmp), 'outside.png')))
