# -*- coding: utf-8 -*-
"""Focused regression tests for the v2.2.4 on-demand startup path.

Run with: ``python -m unittest test_performance_test``
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))


import importlib
import json
import os
import tempfile
import threading
import time
import types
import unittest
import urllib.error
import urllib.request
from unittest import mock

import src.readmd_modules as registry


def wait_for(predicate, timeout=2):
    until = time.time() + timeout
    while time.time() < until:
        if predicate():
            return True
        time.sleep(.01)
    return predicate()


class RegistryTest(unittest.TestCase):
    def setUp(self):
        self.registry = importlib.reload(registry)

    def test_whitelist_and_concurrent_idempotence(self):
        entered = threading.Event()
        release = threading.Event()
        calls = []

        def fake_import(name):
            calls.append(name)
            entered.set()
            release.wait(1)
            return types.SimpleNamespace(load=lambda: None)

        with mock.patch.object(self.registry.importlib, 'import_module', fake_import), \
             mock.patch.object(self.registry.logging, 'exception'):
            threads = [threading.Thread(target=lambda: self.registry.load('convert'))
                       for _ in range(12)]
            for thread in threads:
                thread.start()
            self.assertTrue(entered.wait(1))
            self.assertEqual(['src.readmd_modules.convert'], calls)
            release.set()
            for thread in threads:
                thread.join(1)
            self.assertTrue(wait_for(lambda: self.registry.is_ready('convert')))
        with self.assertRaises(ValueError):
            self.registry.load('nope')

    def test_failed_load_retries(self):
        attempts = [0]

        def fake_import(_name):
            attempts[0] += 1
            if attempts[0] == 1:
                raise RuntimeError('temporary failure')
            return types.SimpleNamespace(load=lambda: None)

        with mock.patch.object(self.registry.importlib, 'import_module', fake_import), \
             mock.patch.object(self.registry.logging, 'exception'):
            self.assertEqual('loading', self.registry.load('web'))
            self.assertTrue(wait_for(lambda: self.registry.status()[0]['web'] == 'error'))
            self.assertEqual('loading', self.registry.load('web'))
            self.assertTrue(wait_for(lambda: self.registry.is_ready('web')))
        self.assertEqual(2, attempts[0])


class HttpModuleTest(unittest.TestCase):
    def setUp(self):
        self.server = readmd.ThreadingHTTPServer(('127.0.0.1', 0), readmd.Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base = 'http://127.0.0.1:%d' % self.server.server_port

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(1)

    def request(self, path, body=None):
        req = urllib.request.Request(self.base + path, data=body,
                                     method='POST' if body is not None else 'GET',
                                     headers={'Content-Type': 'application/json'} if body else {})
        try:
            with urllib.request.urlopen(req, timeout=3) as response:
                return response.status, json.loads(response.read().decode('utf-8'))
        except urllib.error.HTTPError as exc:
            return exc.code, json.loads(exc.read().decode('utf-8'))

    def test_get_status_does_not_load_and_post_loads_one_module(self):
        calls = []
        with mock.patch.object(readmd.RM, 'load', side_effect=lambda name: calls.append(name) or 'loading'), \
             mock.patch.object(readmd.RM, 'status', return_value=({'convert': 'idle', 'ocr': 'idle',
                                                                    'web': 'idle', 'ai': 'idle'}, {})):
            status, payload = self.request('/api/modules')
            self.assertEqual(200, status)
            self.assertIn('modules', payload)
            self.assertEqual([], calls)
            status, payload = self.request('/api/modules/load', b'{"name":"convert"}')
            self.assertEqual(202, status)
            self.assertEqual('convert', payload['name'])
            self.assertEqual(['convert'], calls)
            status, _ = self.request('/api/modules/load', b'{"name":"nope"}')
            self.assertEqual(400, status)


class StartupProbeAndTrayTest(unittest.TestCase):
    def test_probe_summary_and_atomic_write_are_path_free(self):
        report = readmd.startup_probe_summary({'server_up': 1, 'page_loaded': 9})
        self.assertEqual(1, report['milestones_ms']['server_up'])
        self.assertIsNone(report['milestones_ms']['first_document'])
        with tempfile.TemporaryDirectory() as directory:
            target = os.path.join(directory, 'probe.json')
            readmd.write_startup_probe(target)
            with open(target, encoding='utf-8') as handle:
                self.assertEqual(set(report), set(json.load(handle)))

    def test_tray_is_deferred_and_created_once(self):
        old = dict(readmd._tray_icon)
        calls = []
        try:
            readmd._tray_icon.update({'icon': None, 'started': False})
            with mock.patch.object(readmd, '_start_tray', side_effect=lambda window: calls.append(window) or 'tray'):
                threads = [threading.Thread(target=lambda: readmd._start_tray_once('window'))
                           for _ in range(8)]
                for thread in threads:
                    thread.start()
                for thread in threads:
                    thread.join(1)
            self.assertEqual(['window'], calls)
        finally:
            readmd._tray_icon.clear()
            readmd._tray_icon.update(old)

    def test_report_ready_defers_tray_until_the_page_signal(self):
        api = readmd.Api()
        calls = []
        api._on_page_ready = lambda: calls.append('ready')
        self.assertFalse(api._page_ready)
        self.assertEqual([], calls)
        api.report_ready()
        api.report_ready()
        self.assertTrue(api._page_ready)
        self.assertEqual(['ready'], calls)


if __name__ == '__main__':
    unittest.main()
