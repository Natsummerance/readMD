# -*- coding: utf-8 -*-
"""LAN share authorization boundary tests."""
from __future__ import annotations

import os
import sys
import tempfile
import unittest
from types import SimpleNamespace

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from readmd import Handler, configure_lan_server


class ShareHeaders(dict):
    def get(self, name, default=None):
        return super().get(name.lower(), default)


class LanShareSecurityTest(unittest.TestCase):
    def handler(self, path, token='share-token', shared_root=''):
        handler = object.__new__(Handler)
        handler.LAN_TOKEN = token
        handler.path = path
        handler.headers = ShareHeaders({
            'host': '127.0.0.1:28473',
            'x-readmd-token': token,
        })
        handler.server = SimpleNamespace(server_port=28473, shared_root=shared_root)
        return handler

    def test_configure_removes_control_token_and_scopes_document_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            document = os.path.join(tmp, 'shared.md')
            open(document, 'wb').close()
            server = SimpleNamespace(app_token='desktop-secret')
            configure_lan_server(server, document)
            self.assertIsNone(server.app_token)
            self.assertEqual(server.shared_file, os.path.realpath(document))
            self.assertEqual(server.shared_root, os.path.realpath(tmp))

    def test_shared_file_access_must_stay_in_document_scope(self):
        with tempfile.TemporaryDirectory() as tmp:
            outside = os.path.join(os.path.dirname(tmp), 'outside.md')
            inside = os.path.join(tmp, 'shared.md').replace('\\', '/')
            outside_url = os.path.realpath(outside).replace('\\', '/')
            handler = self.handler(
                f'/api/file?p={inside}',
                shared_root=os.path.realpath(tmp),
            )
            self.assertTrue(handler._lan_authorized())

            escaped = f'/api/file?p={outside_url}'
            handler = self.handler(escaped, shared_root=os.path.realpath(tmp))
            self.assertFalse(handler._lan_authorized())

    def test_share_mode_blocks_privileged_routes(self):
        with tempfile.TemporaryDirectory() as tmp:
            for path in ('/api/code/run', '/api/update/apply', '/api/upload'):
                handler = self.handler(path + '?t=share-token', shared_root=tmp)
                self.assertFalse(handler._lan_authorized())


if __name__ == '__main__':
    unittest.main()
