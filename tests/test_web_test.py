"""Deterministic regression tests for ReadMD webpage extraction."""
import gzip
import io
import json
import logging
import os
import sys
import tempfile
import threading
from datetime import datetime, timedelta, timezone
import unittest
from unittest import mock
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))

# Check if required dependencies are available
try:
    import requests
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False

# Mock trafilatura and other heavy dependencies before importing WEB
if HAS_REQUESTS:
    try:
        import trafilatura
        import bs4
        import markdownify
        HAS_WEB_DEPS = True
    except ImportError:
        HAS_WEB_DEPS = False
        # Create mocks
        mock_trafilatura = mock.MagicMock()
        mock_trafilatura.extract.return_value = '<article><p>Mocked content</p></article>'
        mock_bs4 = mock.MagicMock()
        mock_markdownify = mock.MagicMock()
        mock_markdownify.markdownify.return_value = 'Mocked markdown content'
        sys.modules['trafilatura'] = mock_trafilatura
        sys.modules['bs4'] = mock_bs4
        sys.modules['markdownify'] = mock_markdownify
else:
    HAS_WEB_DEPS = False

if HAS_REQUESTS:
    import requests
    from src.readmd_modules import web as WEB
ARTICLE = '<!doctype html><html><head><title>本地测试文章</title>\n<meta name="author" content="测试作者"><meta property="og:site_name" content="ReadMD Tests">\n<link rel="canonical" href="/article"></head><body><nav>菜单菜单菜单</nav>\n<article><h1>本地测试文章</h1><p>%s</p><pre><code>print("ok")</code></pre>\n<table><tr><th>名称</th><th>值</th></tr><tr><td>A</td><td>1</td></tr></table>\n<a href="/next#part">下一页</a><img alt="封面" src="/image.png" onerror="bad()"></article>\n<script>alert(1)</script></body></html>' % ('这是用于网页提取回归测试的中文正文。' * 40)

class FixtureHandler(BaseHTTPRequestHandler):

    def log_message(self, *_args):
        pass

    def _send(self, code, body=b'', ctype='text/html; charset=utf-8', headers=None):
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        for (key, value) in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == '/article':
            self._send(200, ARTICLE.encode('utf-8'))
        elif self.path == '/next':
            self._send(200, ARTICLE.replace('本地测试文章', '第二篇文章').encode('utf-8'))
        elif self.path == '/redirect':
            self._send(302, headers={'Location': '/article'})
        elif self.path == '/gzip':
            self._send(200, gzip.compress(ARTICLE.encode('utf-8')), headers={'Content-Encoding': 'gzip'})
        elif self.path == '/gbk':
            html = ARTICLE.replace('utf-8', 'gb18030')
            self._send(200, html.encode('gb18030'), 'text/html; charset=gb18030')
        elif self.path == '/dynamic':
            body = '<html><head><title>Dynamic</title></head><body><div id="app"></div><footer>%s</footer><script>app.textContent="loaded"</script></body></html>' % ('动态应用入口。' * 5)
            self._send(200, body.encode('utf-8'))
        elif self.path == '/forbidden':
            self._send(403, b'forbidden', 'text/plain')
        elif self.path == '/rate':
            self._send(429, b'slow down', 'text/plain')
        elif self.path == '/plain':
            self._send(200, b'plain', 'text/plain')
        elif self.path == '/mislabel':
            self._send(200, ARTICLE.encode('utf-8'), 'text/plain')
        elif self.path == '/noscript':
            body = '<html><head><title>Fallback</title></head><body><div id="app"></div><noscript><article><h1>Fallback</h1><p>%s</p></article></noscript></body></html>' % ('noscript body ' * 30)
            self._send(200, body.encode('utf-8'))
        elif self.path == '/image.png':
            self._send(200, b'\x89PNG\r\n\x1a\nfixture', 'image/png')
        elif self.path == '/image-redirect':
            self._send(302, headers={'Location': '/image.png'})
        else:
            self._send(404, b'not found', 'text/plain')

class TestWebExtraction(unittest.TestCase):

    def test_pinned_adapter_allows_private_ip_by_default(self):
        session = WEB._session()
        private_answer = [(2, 1, 6, '', ('127.0.0.1', 443))]
        request = requests.Request('GET', 'https://rebind.invalid/article').prepare()
        fake = mock.MagicMock()
        fake.url = 'https://rebind.invalid/article'
        fake.status_code = 200
        fake.headers = {}
        with mock.patch.object(WEB.socket, 'getaddrinfo', return_value=private_answer), mock.patch('requests.adapters.HTTPAdapter.send', return_value=fake) as base_send:
            adapter = session.get_adapter('https://rebind.invalid/article')
            adapter.send(request, timeout=1)
            base_send.assert_called_once()
        session.close()

    def test_pinned_adapter_rejects_private_ip_when_opt_out(self):
        session = WEB._session(allow_private=False)
        private_answer = [(2, 1, 6, '', ('127.0.0.1', 443))]
        with mock.patch.object(WEB.socket, 'getaddrinfo', return_value=private_answer), mock.patch('requests.adapters.HTTPAdapter.send') as base_send:
            with self.assertRaises(Exception):
                session.get('https://rebind.invalid/article', timeout=1)
            base_send.assert_not_called()
        session.close()

    def test_connected_peer_is_checked_after_dns_resolution(self):

        class PeerSocket:

            def __init__(self, address):
                self.address = address

            def getpeername(self):
                return (self.address, 443)

        def response_for(address):
            connection = type('Connection', (), {'sock': PeerSocket(address)})()
            raw = type('Raw', (), {'_connection': connection, 'connection': None, '_fp': None})()
            return type('Response', (), {'raw': raw})()
        WEB._validate_response_peer(response_for('93.184.216.34'))
        WEB._validate_response_peer(response_for('127.0.0.1'))
        with self.assertRaises(WEB.WebError) as raised:
            WEB._validate_response_peer(response_for('127.0.0.1'), allow_private=False)
        self.assertEqual(raised.exception.code, 'private_address')

    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(('127.0.0.1', 0), FixtureHandler)
        cls.server.daemon_threads = True
        threading.Thread(target=cls.server.serve_forever, daemon=True).start()
        cls.base = 'http://127.0.0.1:%d' % cls.server.server_port

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_static_article_metadata_links_and_formatting(self):
        result = WEB.fetch_document(self.base + '/article', allow_private=True)
        self.assertTrue(result['ok'], result)
        self.assertEqual(result['engine'], 'trafilatura')
        self.assertIn('本地测试文章', result['content'])
        self.assertEqual(result['content'].count('# 本地测试文章'), 1)
        self.assertIn('测试作者', result['content'])
        self.assertIn('print("ok")', result['content'])
        self.assertIn(self.base + '/next', result['links'])
        self.assertIn(self.base + '/next#part', result['content'])
        self.assertIn(self.base + '/image.png', result['content'])
        self.assertNotIn('alert(1)', result['content'])
        self.assertNotIn('onerror', result['content'])

    def test_redirect_gzip_and_gbk(self):
        redirected = WEB.fetch_html(self.base + '/redirect', allow_private=True)
        self.assertEqual(redirected['url'], self.base + '/article')
        self.assertEqual(len(redirected['redirects']), 1)
        zipped = WEB.fetch_html(self.base + '/gzip', allow_private=True)
        self.assertIn('本地测试文章', zipped['html'])
        gbk = WEB.fetch_html(self.base + '/gbk', allow_private=True)
        self.assertIn('本地测试文章', gbk['html'])

    def test_actionable_http_errors(self):
        cases = [('/forbidden', 'forbidden', 403), ('/rate', 'rate_limited', 429), ('/plain', 'not_html', 415)]
        for (path, code, status) in cases:
            with self.subTest(path=path):
                try:
                    WEB.fetch_html(self.base + path, allow_private=True)
                    self.fail(f'Expected WebError for {path}')
                except WEB.WebError as caught:
                    self.assertEqual(caught.code, code)
                    self.assertEqual(caught.http_status, status)

    def test_size_limit_and_scheme_validation(self):
        with self.assertRaises(WEB.WebError) as caught:
            WEB.fetch_html(self.base + '/article', max_bytes=100)
        self.assertEqual(caught.exception.code, 'too_large')
        fetched = WEB.fetch_html(self.base + '/article')
        self.assertIn('本地测试文章', fetched['html'])
        for value in ('file:///etc/passwd', 'javascript:alert(1)'):
            with self.assertRaises(WEB.WebError):
                WEB.normalize_url(value)

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_dynamic_page_requests_render_and_full_mode_falls_back(self):
        smart = WEB.fetch_document(self.base + '/dynamic', allow_private=True)
        self.assertFalse(smart['ok'])
        self.assertTrue(smart['render_required'])
        self.assertIn('<script>', smart['render_html'])
        full = WEB.fetch_document(self.base + '/dynamic', mode='full', allow_private=True)
        self.assertTrue(full['ok'], full)
        self.assertEqual(full['engine'], 'full-page')

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_readability_payload_fallback(self):
        shell = '<html><head><title>Shell</title></head><body><div id="app"></div></body></html>'
        reader = {'title': '渲染后的文章', 'byline': 'WebView 作者', 'siteName': 'Dynamic Site', 'url': self.base + '/dynamic', 'content': '<article><h1>渲染后的文章</h1><p>%s</p></article>' % ('动态正文。' * 30)}
        result = WEB.extract_html(self.base + '/dynamic', shell, readability=reader, rendered=True)
        self.assertTrue(result['ok'], result)
        self.assertEqual(result['engine'], 'mozilla-readability')
        self.assertIn('WebView 作者', result['content'])

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_defuddle_payload_precedes_readability_and_preserves_metadata(self):
        shell = '<html><head><title>Shell</title></head><body><div id="app"></div></body></html>'
        defuddle = {'title': 'Defuddle article', 'author': 'Extractor Author', 'published': '2026-08-18', 'site': 'Docs', 'contentMarkdown': '## Section\n\nShort but useful content with `code` and [docs](/docs).'}
        result = WEB.extract_html('https://example.com/post', shell, defuddle=defuddle, rendered=True)
        self.assertTrue(result['ok'], result)
        self.assertEqual(result['engine'], 'defuddle')
        self.assertIn('Extractor Author', result['content'])
        self.assertIn('`code`', result['content'])
        self.assertIn('https://example.com/docs', result['content'])

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_short_semantic_and_noscript_articles_are_not_rejected_by_length(self):
        short = WEB.extract_html('https://example.com/status', '<html><head><title>Status</title></head><body><main><h1>Status</h1><p>Service restored.</p><p>All systems operational.</p></main></body></html>')
        self.assertTrue(short['ok'], short)
        self.assertIn(short['engine'], ('trafilatura', 'trafilatura-recall', 'semantic-page'))
        fallback = WEB.fetch_document(self.base + '/noscript', allow_private=True)
        self.assertTrue(fallback['ok'], fallback)
        self.assertIn('noscript body', fallback['content'])

    def test_html_sniff_accepts_mislabelled_content_type(self):
        fetched = WEB.fetch_html(self.base + '/mislabel', allow_private=True)
        self.assertIn('<article>', fetched['html'])
        self.assertTrue(fetched.get('content_type_mismatch'))

    def test_offline_defuddle_bundle_is_packaged(self):
        root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
        bundle = os.path.join(root, 'assets', 'vendor', 'defuddle.bundle.js')
        license_path = os.path.join(root, 'assets', 'vendor', 'defuddle.LICENSE.txt')
        self.assertTrue(os.path.isfile(bundle) and os.path.getsize(bundle) > 100000)
        self.assertTrue(os.path.isfile(license_path) and os.path.getsize(license_path) > 500)

    def test_localize_images_and_manifest(self):
        markdown = '![封面](%s/image-redirect)' % self.base
        with tempfile.TemporaryDirectory() as directory:
            (content, manifest, warnings) = WEB.localize_images(markdown, directory, allow_private=True)
            self.assertEqual(warnings, [])
            self.assertEqual(len(manifest), 1)
            self.assertTrue(os.path.isfile(manifest[0]['path']))
            self.assertNotIn(self.base, content)

    def test_cancel_marker(self):
        WEB.cancel('fixture-task')
        with self.assertRaises(WEB.WebError) as caught:
            WEB.fetch_html(self.base + '/article', task_id='fixture-task', allow_private=True)
        self.assertEqual(caught.exception.code, 'cancelled')
        WEB.reset_cancel('fixture-task')

    def test_retry_after_supports_seconds_and_http_date_with_bounded_wait(self):
        self.assertEqual(WEB._retry_after_delay('5'), 5.0)
        future = datetime.now(timezone.utc) + timedelta(seconds=20)
        delay = WEB._retry_after_delay(future.strftime('%a, %d %b %Y %H:%M:%S GMT'))
        self.assertGreaterEqual(delay, 18)
        self.assertLessEqual(delay, 30)
        self.assertEqual(WEB._retry_after_delay('600'), 30.0)

class TestWebApi(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        os.environ['READMD_WEB_TEST_ALLOW_PRIVATE'] = '1'
        from readmd import Handler, RM
        RM.load_forced('web')
        cls.fixture_server = ThreadingHTTPServer(('127.0.0.1', 0), FixtureHandler)
        cls.fixture_server.daemon_threads = True
        threading.Thread(target=cls.fixture_server.serve_forever, daemon=True).start()
        cls.fixture = 'http://127.0.0.1:%d' % cls.fixture_server.server_port
        cls.api = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        cls.api.daemon_threads = True
        threading.Thread(target=cls.api.serve_forever, daemon=True).start()
        cls.base = 'http://127.0.0.1:%d' % cls.api.server_port

    @classmethod
    def tearDownClass(cls):
        cls.api.shutdown()
        cls.api.server_close()
        cls.fixture_server.shutdown()
        cls.fixture_server.server_close()
        os.environ.pop('READMD_WEB_TEST_ALLOW_PRIVATE', None)

    def post(self, path, payload):
        request = urllib.request.Request(self.base + path, data=json.dumps(payload).encode('utf-8'), headers={'Content-Type': 'application/json'}, method='POST')
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                return (response.status, json.loads(response.read().decode('utf-8')))
        except urllib.error.HTTPError as error:
            logging.warning('Silent exception caught in tests.test_web_test: urllib.error.HTTPError')
            return (error.code, json.loads(error.read().decode('utf-8')))

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_extract_endpoint_and_cancel(self):
        (status, result) = self.post('/api/web/extract', {'task_id': 'api-web', 'url': self.fixture + '/article', 'mode': 'smart', 'download_images': False})
        self.assertEqual(status, 200)
        self.assertTrue(result['ok'], result)
        (status, result) = self.post('/api/web/cancel', {'task_id': 'api-web'})
        self.assertEqual(status, 200)
        self.assertTrue(result['ok'])

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_rendered_html_endpoint(self):
        (status, result) = self.post('/api/web/extract', {'task_id': 'api-rendered', 'url': self.fixture + '/dynamic', 'mode': 'smart', 'html': '<html><body><main><p>%s</p></main></body></html>' % ('渲染正文。' * 40), 'readability': {'title': 'API 动态文章', 'content': '<p>%s</p>' % ('动态正文。' * 30)}})
        self.assertEqual(status, 200)
        self.assertTrue(result['ok'], result)

    def test_fetch_failures_request_system_webview_fallback(self):
        (status, result) = self.post('/api/web/extract', {'task_id': 'api-forbidden', 'url': self.fixture + '/forbidden', 'mode': 'smart', 'download_images': False})
        self.assertEqual(status, 200)
        self.assertFalse(result['ok'])
        self.assertTrue(result['render_required'])
        self.assertEqual(result['fallback_reason'], 'forbidden')
        self.assertIn('http', result['engine_chain'])

    @unittest.skipUnless(HAS_WEB_DEPS, 'Web extraction dependencies (trafilatura, bs4, markdownify) not available')
    def test_rendered_defuddle_payload_is_used_before_readability(self):
        (status, result) = self.post('/api/web/extract', {'task_id': 'api-defuddle', 'url': self.fixture + '/dynamic', 'mode': 'smart', 'html': '<html><body><div id="app"></div></body></html>', 'defuddle': {'title': 'Defuddle API', 'author': 'Bridge', 'contentMarkdown': '## Result\n\nUseful rendered content from Defuddle.'}, 'readability': {'title': 'Wrong fallback', 'content': '<p>Readability fallback content.</p>'}})
        self.assertEqual(status, 200)
        self.assertTrue(result['ok'], result)
        self.assertEqual(result['engine'], 'defuddle')
        self.assertIn('Bridge', result['content'])

def main():
    suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1
if __name__ == '__main__':
    sys.exit(main())