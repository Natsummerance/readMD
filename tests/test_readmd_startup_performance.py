# -*- coding: utf-8 -*-
"""Startup-critical local server contracts."""

import http.client
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.server import ReadMDHTTPHandler, start_server
import readmd


class ReadmdStartupPerformanceTest(unittest.TestCase):
    def test_local_ui_uses_persistent_connections(self):
        """The 28-script startup path must not pay a connection teardown per asset."""
        self.assertEqual(ReadMDHTTPHandler.protocol_version, 'HTTP/1.1')
        self.assertEqual(readmd.Handler.protocol_version, 'HTTP/1.1')

        server = readmd.start_server(port=0)
        port = server.server_port
        try:
            connection = http.client.HTTPConnection('127.0.0.1', port, timeout=5)
            for path in ('/', '/assets/js/core/state.js'):
                connection.request('GET', path)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertTrue(response.read())
            connection.close()
        finally:
            server.shutdown()
            server.server_close()

    def test_modular_server_uses_persistent_connections(self):
        """The extracted server keeps the same startup transport contract."""

        server, port = start_server(port=0, app_dir=ROOT)
        try:
            connection = http.client.HTTPConnection('127.0.0.1', port, timeout=5)
            connection.request('GET', '/')
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            self.assertIn('ReadMD', response.read().decode('utf-8'))

            connection.request('GET', '/assets/js/core/state.js')
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            self.assertTrue(response.read())
            connection.close()
        finally:
            server.shutdown()
            server.server_close()

    def test_ui_boots_from_one_ordered_script_bundle(self):
        """A cold WebView should fetch one application bundle instead of 28 scripts."""
        html = open(os.path.join(ROOT, 'assets', 'index.html'), encoding='utf-8').read()
        self.assertEqual(html.count('<script '), 1)
        self.assertIn('/assets/readmd.boot.js', html)

        server = readmd.start_server(port=0)
        try:
            connection = http.client.HTTPConnection('127.0.0.1', server.server_port, timeout=5)
            connection.request('GET', f'/assets/readmd.boot.js?v={getattr(readmd, "VERSION", "2.3.7")}')
            response = connection.getresponse()
            body = response.read().decode('utf-8')
            self.assertEqual(response.status, 200)
            self.assertEqual(response.headers.get_content_type(), 'application/javascript')
            self.assertIn('function loadFile', body)
            self.assertIn('window.addEventListener(\'DOMContentLoaded\', init)', body)
            connection.close()
        finally:
            server.shutdown()
            server.server_close()


if __name__ == '__main__':
    unittest.main()
