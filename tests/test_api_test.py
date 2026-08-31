import sys
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
sys.path.insert(0, ROOT)

import readmd
import json
import threading
import tempfile
import time
import unittest
import urllib.error
import urllib.request
from unittest import mock


class TestHttpResponseLifecycle(unittest.TestCase):
    def test_send_treats_cancelled_browser_response_as_normal_disconnect(self):
        handler = object.__new__(readmd.Handler)
        handler.headers = {'Accept-Encoding': ''}
        handler.close_connection = False
        handler.send_response = mock.Mock()
        handler.send_header = mock.Mock()
        handler.end_headers = mock.Mock()
        handler.wfile = mock.Mock()
        handler.wfile.write.side_effect = ConnectionAbortedError(10053, 'cancelled')

        handler._send(200, 'application/json; charset=utf-8', b'{}')

        self.assertTrue(handler.close_connection)


class TestRenameFile(unittest.TestCase):
    def _metadata_paths(self, td):
        return {
            'RECENT_FILE': os.path.join(td, 'recent.json'),
            'SETTINGS_FILE': os.path.join(td, 'settings.json'),
            'HISTORY_FILE': os.path.join(td, 'chat_history.json'),
        }

    def test_rename_moves_file_backup_and_exact_metadata_references(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, '旧名.md')
            new = os.path.join(td, '新名.md')
            with open(old, 'w', encoding='utf-8') as handle:
                handle.write('# document')
            with open(old + '.bak', 'w', encoding='utf-8') as handle:
                handle.write('backup')
            paths = self._metadata_paths(td)
            readmd.save_json(paths['RECENT_FILE'], [old, os.path.join(td, 'other.md')])
            readmd.save_json(paths['SETTINGS_FILE'], {'last': old, 'theme': 'dark'})
            readmd.save_json(paths['HISTORY_FILE'], {
                'sessions': [{'id': '1', 'doc': old}, {'id': '2', 'doc': 'unrelated'}]
            })

            with mock.patch.multiple(readmd, **paths):
                result = readmd.Api().rename_file(old, '新名')

            self.assertTrue(result.get('ok'), result)
            self.assertEqual(result.get('path'), new)
            self.assertTrue(os.path.isfile(new))
            self.assertTrue(os.path.isfile(new + '.bak'))
            self.assertFalse(os.path.exists(old))
            self.assertEqual(readmd.load_json(paths['RECENT_FILE'], [])[0], new)
            self.assertEqual(readmd.load_json(paths['SETTINGS_FILE'], {})['last'], new)
            sessions = readmd.load_json(paths['HISTORY_FILE'], {})['sessions']
            self.assertEqual(sessions[0]['doc'], new)
            self.assertEqual(sessions[1]['doc'], 'unrelated')

    def test_rename_rejects_invalid_or_existing_target_without_modifying_source(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, 'old.md')
            existing = os.path.join(td, 'taken.md')
            for path in (old, existing):
                with open(path, 'w', encoding='utf-8') as handle:
                    handle.write(path)
            paths = self._metadata_paths(td)
            with mock.patch.multiple(readmd, **paths):
                invalid = readmd.Api().rename_file(old, '../escape')
                conflict = readmd.Api().rename_file(old, 'taken')
            self.assertFalse(invalid.get('ok'))
            self.assertEqual(invalid.get('code'), 'invalid_name')
            self.assertFalse(conflict.get('ok'))
            self.assertEqual(conflict.get('code'), 'target_exists')
            self.assertTrue(os.path.isfile(old))

    def test_case_only_rename_works_on_case_insensitive_filesystems(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, 'Title.md')
            with open(old, 'w', encoding='utf-8') as handle:
                handle.write('case')
            paths = self._metadata_paths(td)
            with mock.patch.multiple(readmd, **paths):
                result = readmd.Api().rename_file(old, 'title')
            self.assertTrue(result.get('ok'), result)
            self.assertEqual(os.path.basename(result['path']), 'title.md')
            self.assertTrue(os.path.isfile(result['path']))

    def test_case_only_rename_uses_samefile_on_macos_style_filesystem(self):
        with tempfile.TemporaryDirectory() as td:
            old = os.path.join(td, 'Title.md')
            new = os.path.join(td, 'title.md')
            with open(old, 'w', encoding='utf-8') as handle:
                handle.write('case')
            paths = self._metadata_paths(td)
            real_exists = os.path.exists
            with mock.patch.multiple(readmd, **paths), \
                    mock.patch.object(readmd.os.path, 'exists',
                                      side_effect=lambda path: True if path == new else real_exists(path)), \
                    mock.patch.object(readmd.os.path, 'samefile', return_value=True), \
                    mock.patch.object(readmd, '_paths_equal', return_value=False):
                result = readmd.Api().rename_file(old, 'title')
            self.assertTrue(result.get('ok'), result)
            self.assertEqual(result['path'], new)


class TestSaveAuthorization(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.td = tempfile.TemporaryDirectory()
        cls.document = os.path.join(cls.td.name, 'document.md')
        cls.unopened = os.path.join(cls.td.name, 'unopened.md')
        for path in (cls.document, cls.unopened):
            with open(path, 'w', encoding='utf-8') as handle:
                handle.write('# original')
        cls.server = readmd.ReadMDHTTPServer(('127.0.0.1', 0), readmd.Handler)
        cls.port = cls.server.server_port
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)
        cls.td.cleanup()

    def _save(self, path, token=None):
        headers = {'Content-Type': 'application/json', 'Connection': 'close'}
        if token is not None:
            headers['X-ReadMD-App-Token'] = token
        request = urllib.request.Request(
            'http://127.0.0.1:%d/api/save' % self.port,
            data=json.dumps({'path': path, 'content': '# saved'}).encode('utf-8'),
            method='POST', headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=3) as response:
                return response.status, json.loads(response.read())
        except urllib.error.HTTPError as error:
            try:
                return error.code, json.loads(error.read())
            except json.JSONDecodeError:
                return error.code, {'error': 'forbidden'}
        except (ConnectionError, ConnectionAbortedError):
            # Windows can reset a rejected loopback request before the 403 body is read.
            return 403, {'error': 'forbidden'}

    def _open_document(self):
        url = 'http://127.0.0.1:%d/api/file?p=%s' % (
            self.port, urllib.request.quote(self.document))
        with urllib.request.urlopen(url, timeout=3) as response:
            self.assertEqual(response.status, 200)

    def test_save_requires_server_instance_app_token(self):
        with open(self.document, 'w', encoding='utf-8') as handle:
            handle.write('# original')
        for token in (None, 'wrong'):
            status, result = self._save(self.document, token)
            self.assertEqual(status, 403)
            self.assertEqual(result, {'error': 'forbidden'})
            with open(self.document, encoding='utf-8') as handle:
                self.assertEqual(handle.read(), '# original')

    def test_authorized_document_can_save_but_unopened_path_is_rejected(self):
        self._open_document()
        status, result = self._save(self.document, self.server.app_token)
        self.assertEqual(status, 200)
        self.assertTrue(result.get('ok'), result)
        with open(self.document, encoding='utf-8') as handle:
            self.assertEqual(handle.read(), '# saved')

        status, result = self._save(self.unopened, self.server.app_token)
        self.assertEqual(status, 403)
        self.assertEqual(result, {'error': '文件未被授权保存'})
        with open(self.unopened, encoding='utf-8') as handle:
            self.assertEqual(handle.read(), '# original')

    def test_index_injects_instance_token_for_frontend(self):
        with urllib.request.urlopen(
                'http://127.0.0.1:%d/' % self.port, timeout=3) as response:
            body = response.read()
        self.assertIn(('content="%s"' % self.server.app_token).encode('ascii'), body)
        self.assertNotIn(b'content=""', body)


class TestRequestBoundary(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = readmd.ReadMDHTTPServer(('127.0.0.1', 0), readmd.Handler)
        cls.port = cls.server.server_port
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)

    def test_api_rejects_rebound_host(self):
        request = urllib.request.Request(
            'http://127.0.0.1:%d/api/ping' % self.port,
            headers={'Host': 'attacker.invalid', 'Connection': 'close'})
        with self.assertRaises(urllib.error.HTTPError) as raised:
            urllib.request.urlopen(request, timeout=3)
        self.assertEqual(raised.exception.code, 403)
        self.assertEqual(raised.exception.read(), b'forbidden')

    def test_post_rejects_cross_origin_request(self):
        request = urllib.request.Request(
            'http://127.0.0.1:%d/api/modules/load' % self.port,
            data=json.dumps({'name': 'convert'}).encode('utf-8'), method='POST',
            headers={
                'Content-Type': 'application/json',
                'Host': '127.0.0.1:%d' % self.port,
                'Origin': 'https://attacker.invalid',
                'Connection': 'close',
            })
        try:
            urllib.request.urlopen(request, timeout=3)
            self.fail('cross-origin POST reached the API')
        except urllib.error.HTTPError as error:
            self.assertEqual(error.code, 403)
        except (ConnectionError, ConnectionAbortedError):
            # Windows can reset the socket while the rejected connection closes.
            pass

    def test_main_responses_do_not_enable_cross_origin_reads(self):
        with urllib.request.urlopen(
                'http://127.0.0.1:%d/' % self.port, timeout=3) as response:
            self.assertIsNone(response.headers.get('Access-Control-Allow-Origin'))

    def test_static_assets_reject_parent_escape(self):
        request = urllib.request.Request(
            'http://127.0.0.1:%d/assets/../readmd.py' % self.port,
            headers={'Connection': 'close'})
        with self.assertRaises(urllib.error.HTTPError) as raised:
            urllib.request.urlopen(request, timeout=3)
        self.assertEqual(raised.exception.code, 403)


class TestPrivateWebAuthorization(unittest.TestCase):
    def test_wk_origin_filter_handles_default_ports_and_ipv6(self):
        https_filter = readmd.Api._web_origin_url_filter('https://example.com/docs')
        self.assertRegex('https://example.com/page', https_filter)
        self.assertRegex('https://example.com:443/page', https_filter)
        self.assertNotRegex('https://example.com.evil/page', https_filter)
        self.assertEqual(readmd.Api._web_origin('http://[::1]/docs'),
                         'http://[::1]:80')

    def test_grant_is_task_origin_bound_and_revocable(self):
        api = readmd.Api()
        granted = api.authorize_private_web('http://127.0.0.1:8080/page', 'task-a')
        self.assertTrue(granted.get('ok'), granted)
        token = granted['grant']
        self.assertTrue(api._private_web_allowed(
            'http://127.0.0.1:8080/next', 'task-a', token))
        self.assertFalse(api._private_web_allowed(
            'http://127.0.0.1:8081/next', 'task-a', token))
        self.assertFalse(api._private_web_allowed(
            'http://127.0.0.1:8080/next', 'task-b', token))
        self.assertTrue(api.revoke_private_web('task-a'))
        self.assertFalse(api._private_web_allowed(
            'http://127.0.0.1:8080/next', 'task-a', token))

    def test_expired_private_grant_is_rejected(self):
        api = readmd.Api()
        granted = api.authorize_private_web('http://10.0.0.2/docs', 'task-expired')
        self.assertTrue(granted.get('ok'), granted)
        api._web_private_grants['task-expired']['expires_at'] = time.time() - 1
        self.assertFalse(api._private_web_allowed(
            'http://10.0.0.2/next', 'task-expired', granted['grant']))

    def test_request_guard_blocks_ungranted_private_resources(self):
        api = readmd.Api()
        with mock.patch('src.readmd_modules.web.socket.getaddrinfo') as lookup:
            lookup.return_value = [(2, 1, 6, '', ('127.0.0.1', 0))]
            self.assertFalse(api._web_request_allowed(
                'http://local.invalid/metadata', 'task-a', ''))
            granted = api.authorize_private_web(
                'http://local.invalid/docs', 'task-private')
            self.assertTrue(granted.get('ok'), granted)
            self.assertTrue(api._web_request_allowed(
                'http://local.invalid/image.png', 'task-private', granted['grant']))
            self.assertFalse(api._web_request_allowed(
                'http://other.invalid/image.png', 'task-private', granted['grant']))


class TestClipboardAuthorization(unittest.TestCase):
    def test_clipboard_read_requires_grant_token(self):
        api = readmd.Api()
        self.assertEqual(api.read_clipboard().get('source_type'), 'unauthorized')
        token = api.authorize_clipboard_read()['token']
        with mock.patch.dict('sys.modules', {'tkinter': None}):
            result = api.read_clipboard(token)
        self.assertNotEqual(result.get('source_type'), 'unauthorized')


if __name__ == '__main__':
    unittest.main(verbosity=2)
