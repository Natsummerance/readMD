# -*- coding: utf-8 -*-
"""Request-boundary regression tests for destructive HTTP operations."""

import io
import unittest

from readmd import Handler


class _BodyHandler:
    _read_request_body_limited = Handler._read_request_body_limited

    def __init__(self, payload):
        self.rfile = io.BytesIO(payload)


class TestHttpBoundaries(unittest.TestCase):
    def test_bounded_reader_consumes_short_reads_exactly(self):
        body = b'{"confirm":true}'
        reader = _BodyHandler(body)
        self.assertEqual(reader._read_request_body_limited(len(body), 64), body)

    def test_bounded_reader_rejects_incomplete_body(self):
        reader = _BodyHandler(b'{}')
        with self.assertRaisesRegex(ValueError, 'incomplete_request'):
            reader._read_request_body_limited(3, 64)

    def test_bounded_reader_rejects_oversized_body_before_reading(self):
        reader = _BodyHandler(b'should remain unread')
        with self.assertRaisesRegex(ValueError, 'request_too_large'):
            reader._read_request_body_limited(65, 64)
        self.assertEqual(reader.rfile.tell(), 0)


if __name__ == '__main__':
    unittest.main()
