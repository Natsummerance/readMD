# -*- coding: utf-8 -*-
"""Local static-resource conditional request tests."""
from __future__ import annotations

import http.client
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from readmd import start_server


class StaticCacheTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = start_server(0)
        cls.port = cls.server.server_port

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()

    def request(self, path, headers=None):
        connection = http.client.HTTPConnection('127.0.0.1', self.port, timeout=3)
        try:
            connection.request('GET', path, headers=headers or {})
            response = connection.getresponse()
            return response.status, dict(response.getheaders()), response.read()
        finally:
            connection.close()

    def test_immutable_vendor_resource_exposes_strong_validator(self):
        status, headers, body = self.request('/assets/vendor/marked.min.js')
        self.assertEqual(status, 200)
        self.assertTrue(body)
        self.assertIn('immutable', headers['Cache-Control'])
        self.assertIn('Last-Modified', headers)
        self.assertTrue(headers['ETag'].startswith('"'))
        self.assertTrue(headers['ETag'].endswith('"'))

    def test_matching_etag_returns_empty_304_with_cache_policy(self):
        _, first, _ = self.request('/assets/style.css')
        status, second, body = self.request(
            '/assets/style.css',
            {'If-None-Match': first['ETag']},
        )
        self.assertEqual(status, 304)
        self.assertEqual(body, b'')
        self.assertEqual(second['ETag'], first['ETag'])
        self.assertIn('no-cache', second['Cache-Control'])

    def test_mismatched_validator_returns_full_response(self):
        _, first, _ = self.request('/assets/app.js')
        stale = first['ETag'][:-1] + '-stale"'
        status, _, body = self.request(
            '/assets/app.js',
            {'If-None-Match': stale},
        )
        self.assertEqual(status, 200)
        self.assertTrue(body)

    def test_modified_since_can_revalidate(self):
        _, first, _ = self.request('/assets/vendor/qrcode.min.js')
        status, second, body = self.request(
            '/assets/vendor/qrcode.min.js',
            {'If-Modified-Since': first['Last-Modified']},
        )
        self.assertEqual(status, 304)
        self.assertEqual(body, b'')
        self.assertIn('immutable', second['Cache-Control'])
