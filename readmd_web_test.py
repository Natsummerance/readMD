# -*- coding: utf-8 -*-
"""Deterministic regression tests for ReadMD webpage extraction."""

import gzip
import io
import json
import os
import sys
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from readmd_modules import web as WEB


ARTICLE = '''<!doctype html><html><head><title>本地测试文章</title>
<meta name="author" content="测试作者"><meta property="og:site_name" content="ReadMD Tests">
<link rel="canonical" href="/article"></head><body><nav>菜单菜单菜单</nav>
<article><h1>本地测试文章</h1><p>%s</p><pre><code>print("ok")</code></pre>
<table><tr><th>名称</th><th>值</th></tr><tr><td>A</td><td>1</td></tr></table>
<a href="/next#part">下一页</a><img alt="封面" src="/image.png" onerror="bad()"></article>
<script>alert(1)</script></body></html>''' % ('这是用于网页提取回归测试的中文正文。' * 40)


class FixtureHandler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def _send(self, code, body=b'', ctype='text/html; charset=utf-8', headers=None):
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        for key, value in (headers or {}).items():
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
            self._send(200, gzip.compress(ARTICLE.encode('utf-8')),
                       headers={'Content-Encoding': 'gzip'})
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
        elif self.path == '/image.png':
            # Header is enough for the localizer; image decoding is not its job.
            self._send(200, b'\x89PNG\r\n\x1a\nfixture', 'image/png')
        elif self.path == '/image-redirect':
            self._send(302, headers={'Location': '/image.png'})
        else:
            self._send(404, b'not found', 'text/plain')


class TestWebExtraction(unittest.TestCase):
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

    def test_static_article_metadata_links_and_formatting(self):
        result = WEB.fetch_document(self.base + '/article', allow_private=True)
        self.assertTrue(result['ok'], result)
        self.assertEqual(result['engine'], 'trafilatura')
        self.assertIn('本地测试文章', result['content'])
        self.assertEqual(result['content'].count('# 本地测试文章'), 1)
        self.assertIn('测试作者', result['content'])
        self.assertIn('print("ok")', result['content'])
        self.assertIn(self.base + '/next', result['links'])
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
        cases = [('/forbidden', 'forbidden', 403), ('/rate', 'rate_limited', 429),
                 ('/plain', 'not_html', 415)]
        for path, code, status in cases:
            with self.subTest(path=path), self.assertRaises(WEB.WebError) as caught:
                WEB.fetch_html(self.base + path, allow_private=True)
            self.assertEqual(caught.exception.code, code)
            self.assertEqual(caught.exception.http_status, status)

    def test_size_limit_and_private_address_block(self):
        with self.assertRaises(WEB.WebError) as caught:
            WEB.fetch_html(self.base + '/article', max_bytes=100, allow_private=True)
        self.assertEqual(caught.exception.code, 'too_large')
        with self.assertRaises(WEB.WebError) as caught:
            WEB.fetch_html(self.base + '/article')
        self.assertEqual(caught.exception.code, 'private_address')
        for value in ('file:///etc/passwd', 'javascript:alert(1)'):
            with self.assertRaises(WEB.WebError):
                WEB.normalize_url(value)

    def test_dynamic_page_requests_render_and_full_mode_falls_back(self):
        smart = WEB.fetch_document(self.base + '/dynamic', allow_private=True)
        self.assertFalse(smart['ok'])
        self.assertTrue(smart['render_required'])
        full = WEB.fetch_document(self.base + '/dynamic', mode='full', allow_private=True)
        self.assertTrue(full['ok'], full)
        self.assertEqual(full['engine'], 'full-page')

    def test_readability_payload_fallback(self):
        shell = '<html><head><title>Shell</title></head><body><div id="app"></div></body></html>'
        reader = {'title': '渲染后的文章', 'byline': 'WebView 作者',
                  'siteName': 'Dynamic Site', 'url': self.base + '/dynamic',
                  'content': '<article><h1>渲染后的文章</h1><p>%s</p></article>' % ('动态正文。' * 30)}
        result = WEB.extract_html(self.base + '/dynamic', shell,
                                  readability=reader, rendered=True)
        self.assertTrue(result['ok'], result)
        self.assertEqual(result['engine'], 'mozilla-readability')
        self.assertIn('WebView 作者', result['content'])

    def test_localize_images_and_manifest(self):
        markdown = '![封面](%s/image-redirect)' % self.base
        with tempfile.TemporaryDirectory() as directory:
            content, manifest, warnings = WEB.localize_images(
                markdown, directory, allow_private=True)
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
        request = urllib.request.Request(
            self.base + path, data=json.dumps(payload).encode('utf-8'),
            headers={'Content-Type': 'application/json'}, method='POST')
        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                return response.status, json.loads(response.read().decode('utf-8'))
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read().decode('utf-8'))

    def test_extract_endpoint_and_cancel(self):
        status, result = self.post('/api/web/extract', {
            'task_id': 'api-web', 'url': self.fixture + '/article',
            'mode': 'smart', 'download_images': False,
        })
        self.assertEqual(status, 200)
        self.assertTrue(result['ok'], result)
        status, result = self.post('/api/web/cancel', {'task_id': 'api-web'})
        self.assertEqual(status, 200)
        self.assertTrue(result['ok'])

    def test_rendered_html_endpoint(self):
        status, result = self.post('/api/web/extract', {
            'task_id': 'api-rendered', 'url': self.fixture + '/dynamic',
            'mode': 'smart', 'html': '<html><body><main><p>%s</p></main></body></html>' % ('渲染正文。' * 40),
            'readability': {'title': 'API 动态文章', 'content': '<p>%s</p>' % ('动态正文。' * 30)},
        })
        self.assertEqual(status, 200)
        self.assertTrue(result['ok'], result)


def main():
    suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == '__main__':
    sys.exit(main())
