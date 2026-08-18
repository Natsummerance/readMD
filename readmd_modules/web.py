# -*- coding: utf-8 -*-
"""Robust webpage-to-Markdown extraction for ReadMD.

Downloading and extraction are intentionally separate so the desktop shell can
fall back to a system WebView for JavaScript-rendered pages. Network access is
limited to public HTTP(S) resources and failures carry stable UI error codes.
"""

from __future__ import absolute_import

import hashlib
import ipaddress
import logging
import os
import re
import socket
import threading
import time
from urllib.parse import urljoin, urlparse, urlunparse

_deps = None
_cancelled = set()
_cancel_lock = threading.Lock()

MAX_HTML_BYTES = 50 * 1024 * 1024
MAX_IMAGE_BYTES = 15 * 1024 * 1024
MAX_IMAGE_TOTAL = 100 * 1024 * 1024
MAX_IMAGES = 100
MAX_REDIRECTS = 10
MIN_ARTICLE_CHARS = 40
ALLOWED_IMAGE_TYPES = {
    'image/jpeg': '.jpg', 'image/png': '.png', 'image/gif': '.gif',
    'image/webp': '.webp', 'image/avif': '.avif',
}
BLOCKED_TAGS = ('script', 'style', 'form', 'iframe',
                'object', 'embed', 'canvas', 'svg')
USER_AGENT = (
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) '
    'AppleWebKit/537.36 (KHTML, like Gecko) '
    'Chrome/131.0 Safari/537.36 ReadMD/2.2.3'
)

RETRY_STATUSES = {408, 425, 429, 502, 503, 504}


class WebError(Exception):
    """A stable, user-facing webpage conversion failure."""

    def __init__(self, code, message, http_status=422, detail=''):
        self.code = code
        self.message = message
        self.http_status = int(http_status)
        self.detail = detail
        super().__init__(message)

    def as_dict(self):
        return {'ok': False, 'code': self.code, 'error': self.message,
                'detail': self.detail}


def load():
    """Lazy-load the relatively heavy extraction dependencies."""
    global _deps
    if _deps is None:
        import requests
        import trafilatura
        from bs4 import BeautifulSoup
        from markdownify import markdownify
        _deps = (requests, trafilatura, BeautifulSoup, markdownify)
    return _deps


def cancel(task_id):
    if task_id:
        with _cancel_lock:
            _cancelled.add(str(task_id))
    return True


def reset_cancel(task_id):
    if task_id:
        with _cancel_lock:
            _cancelled.discard(str(task_id))


def is_cancelled(task_id):
    if not task_id:
        return False
    with _cancel_lock:
        return str(task_id) in _cancelled


def _check_cancel(task_id):
    if is_cancelled(task_id):
        raise WebError('cancelled', '已取消网页转换', 499)


def normalize_url(url):
    url = (url or '').strip()
    if not url:
        raise WebError('missing_url', '请输入网页地址', 400)
    if '://' not in url:
        url = 'https://' + url
    parsed = urlparse(url)
    if parsed.scheme.lower() not in ('http', 'https'):
        raise WebError('unsupported_scheme', '仅支持 HTTP 或 HTTPS 网页', 400)
    if not parsed.hostname:
        raise WebError('invalid_url', '网页地址格式不正确', 400)
    host = parsed.hostname.encode('idna').decode('ascii')
    netloc = host
    try:
        port = parsed.port
    except ValueError:
        raise WebError('invalid_url', '网页地址格式不正确', 400)
    if port:
        netloc += ':' + str(port)
    return urlunparse((parsed.scheme.lower(), netloc, parsed.path or '/',
                       parsed.params, parsed.query, ''))


def _validate_public_url(url, allow_private=False):
    normalized = normalize_url(url)
    host = urlparse(normalized).hostname
    try:
        infos = socket.getaddrinfo(host, None, type=socket.SOCK_STREAM)
    except socket.gaierror as exc:
        raise WebError('dns_failed', '无法解析网页域名', 502, str(exc))
    addresses = {item[4][0].split('%', 1)[0] for item in infos}
    if not addresses:
        raise WebError('dns_failed', '无法解析网页域名', 502)
    if not allow_private:
        for value in addresses:
            try:
                address = ipaddress.ip_address(value)
            except ValueError:
                continue
            if not address.is_global:
                raise WebError('private_address', '出于安全原因不能抓取本机或局域网地址', 403)
    return normalized


def _session():
    requests, _tra, _bs, _md = load()
    session = requests.Session()
    session.trust_env = True
    session.headers.update({
        'User-Agent': USER_AGENT,
        'Accept': 'text/html,application/xhtml+xml;q=0.9,*/*;q=0.2',
        'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.7',
        'Accept-Encoding': 'gzip, deflate',
        'Cache-Control': 'no-cache',
    })
    return session


def _request_error(exc):
    requests, _tra, _bs, _md = load()
    if isinstance(exc, requests.exceptions.Timeout):
        return WebError('timeout', '连接网页超时，请稍后重试', 504, str(exc))
    if isinstance(exc, requests.exceptions.SSLError):
        return WebError('tls_failed', '网页 TLS/证书连接失败', 502, str(exc))
    if isinstance(exc, requests.exceptions.ProxyError):
        return WebError('proxy_failed', '代理服务器连接失败', 502, str(exc))
    return WebError('network_failed', '无法连接到网页服务器', 502, str(exc))


def fetch_html(url, timeout=25, max_bytes=MAX_HTML_BYTES, task_id=None,
               allow_private=False, session=None):
    """Download an HTML document with bounded redirects and response size."""
    requests, _tra, _bs, _md = load()
    current = _validate_public_url(url, allow_private=allow_private)
    sess = session or _session()
    history = []
    try:
        retry_count = 0
        for _ in range(MAX_REDIRECTS + 1):
            _check_cancel(task_id)
            try:
                response = sess.get(current, timeout=(8, timeout), stream=True,
                                    allow_redirects=False)
            except requests.exceptions.RequestException as exc:
                raise _request_error(exc)
            status = response.status_code
            if status in (301, 302, 303, 307, 308):
                location = response.headers.get('Location')
                response.close()
                if not location:
                    raise WebError('redirect_failed', '网页重定向缺少目标地址', 502)
                history.append(current)
                current = _validate_public_url(urljoin(current, location),
                                               allow_private=allow_private)
                continue
            if status in RETRY_STATUSES and retry_count < 2:
                retry_after = response.headers.get('Retry-After')
                response.close()
                retry_count += 1
                try:
                    delay = min(2.0, max(0.0, float(retry_after)))
                except (TypeError, ValueError):
                    delay = 0.15 * retry_count
                if delay:
                    time.sleep(delay)
                continue
            if status == 401:
                response.close()
                raise WebError('login_required', '该网页需要登录后访问', 401)
            if status == 403:
                response.close()
                raise WebError('forbidden', '服务器拒绝访问该网页（403）', 403)
            if status == 429:
                response.close()
                raise WebError('rate_limited', '请求过于频繁，服务器要求稍后重试（429）', 429)
            if status < 200 or status >= 300:
                response.close()
                raise WebError('http_error', '网页服务器返回 HTTP %d' % status,
                               502, str(status))
            ctype = (response.headers.get('Content-Type') or '').lower()
            declared = response.headers.get('Content-Length')
            if declared:
                try:
                    if int(declared) > max_bytes:
                        response.close()
                        raise WebError('too_large', '网页内容超过 50 MB 限制', 413)
                except ValueError:
                    pass
            chunks, total = [], 0
            try:
                for chunk in response.iter_content(64 * 1024):
                    _check_cancel(task_id)
                    if not chunk:
                        continue
                    total += len(chunk)
                    if total > max_bytes:
                        raise WebError('too_large', '网页内容超过 50 MB 限制', 413)
                    chunks.append(chunk)
                raw = b''.join(chunks)
                if not raw:
                    raise WebError('empty_response', '网页服务器返回了空内容', 502)
                encoding = response.encoding
                if not encoding or encoding.lower() == 'iso-8859-1':
                    try:
                        encoding = response.apparent_encoding
                    except Exception:
                        encoding = None
            finally:
                response.close()
            try:
                html = raw.decode(encoding or 'utf-8', errors='replace')
            except LookupError:
                html = raw.decode('utf-8', errors='replace')
            declared_html = (not ctype or any(
                x in ctype for x in ('text/html', 'application/xhtml+xml')))
            sniffed_html = bool(re.search(
                r'<!doctype\s+html|<html\b|<head\b|<body\b|<article\b|<main\b',
                html[:8192], re.I))
            if not declared_html and not sniffed_html:
                raise WebError('not_html', '该地址返回的不是 HTML 网页', 415, ctype)
            return {'url': current, 'requested_url': normalize_url(url),
                    'html': html, 'status': status, 'content_type': ctype,
                    'bytes': total, 'redirects': history,
                    'encoding': encoding or 'utf-8',
                    'content_type_mismatch': bool(not declared_html and sniffed_html)}
        raise WebError('too_many_redirects', '网页重定向次数过多', 502)
    finally:
        if session is None:
            sess.close()


def _clean_soup(html, base_url):
    _requests, _tra, BeautifulSoup, _markdownify = load()
    soup = BeautifulSoup(html or '', 'lxml')
    for tag in soup.find_all(BLOCKED_TAGS):
        tag.decompose()
    for tag in soup.find_all(True):
        for name in list(tag.attrs):
            low = name.lower()
            if low.startswith('on') or low in ('style', 'srcdoc'):
                del tag.attrs[name]
        if tag.name == 'img' and not tag.get('src'):
            lazy = tag.get('data-src') or tag.get('data-original')
            if lazy:
                tag['src'] = lazy
        for attr in ('href', 'src', 'poster'):
            value = tag.get(attr)
            if not value:
                continue
            absolute = urljoin(base_url, str(value).strip())
            if urlparse(absolute).scheme.lower() not in ('http', 'https'):
                del tag.attrs[attr]
            else:
                tag.attrs[attr] = absolute
    return soup


def _plain_length(markdown):
    text = re.sub(r'!\[[^]]*\]\([^)]*\)', ' ', markdown or '')
    text = re.sub(r'\[([^]]+)\]\([^)]*\)', r'\1', text)
    text = re.sub(r'[`*_>#|\-]+', ' ', text)
    return len(re.sub(r'\s+', '', text))


def _metadata(soup, url):
    def meta(*names):
        for name in names:
            tag = (soup.find('meta', attrs={'property': name}) or
                   soup.find('meta', attrs={'name': name}))
            if tag and tag.get('content'):
                return str(tag.get('content')).strip()
        return ''
    title = meta('og:title', 'twitter:title')
    if not title and soup.title:
        title = soup.title.get_text(' ', strip=True)
    canonical = soup.find('link', rel=lambda value: value and 'canonical' in value)
    canonical_url = urljoin(url, canonical.get('href')) if canonical and canonical.get('href') else url
    return {
        'title': title[:300] if title else url,
        'author': meta('author', 'article:author'),
        'date': meta('article:published_time', 'date', 'datePublished'),
        'site': meta('og:site_name') or (urlparse(url).hostname or ''),
        'canonical_url': canonical_url,
    }


def _candidate_links(soup, base_url, limit=30):
    base = urlparse(base_url)
    base_host = (base.hostname or '').lower().removeprefix('www.')
    output, seen = [], set()
    for tag in soup.find_all('a', href=True):
        try:
            full = normalize_url(urljoin(base_url, tag.get('href')))
        except WebError:
            continue
        parsed = urlparse(full)
        host = (parsed.hostname or '').lower().removeprefix('www.')
        if host != base_host:
            continue
        clean = urlunparse((parsed.scheme, parsed.netloc, parsed.path or '/',
                            parsed.params, parsed.query, ''))
        if clean in seen or clean == base_url:
            continue
        if re.search(r'\.(pdf|zip|rar|7z|png|jpe?g|gif|webp|avif|mp4|mp3|docx?|xlsx?|pptx?)(\?|$)', clean, re.I):
            continue
        seen.add(clean)
        output.append(clean)
        if len(output) >= limit:
            break
    return output


def _markdownify_html(html):
    _requests, _tra, _BeautifulSoup, markdownify = load()
    return markdownify(html or '', heading_style='ATX', bullets='-',
                       strip=('script', 'style', 'form', 'iframe'),
                       table_infer_header=True).strip()


def _format_document(markdown, meta):
    title = (meta.get('title') or meta.get('canonical_url') or '网页').strip()
    body = _sanitize_markdown(markdown, meta.get('canonical_url') or '').strip()
    body_lines = body.splitlines()
    if body_lines and body_lines[0].startswith('# '):
        extracted_title = re.sub(r'\s+', ' ', body_lines[0][2:].strip()).casefold()
        document_title = re.sub(r'\s+', ' ', title).casefold()
        if extracted_title == document_title:
            body = '\n'.join(body_lines[1:]).lstrip()
    lines = ['# ' + title, '', '> 来源：' + (meta.get('canonical_url') or '')]
    extra = []
    if meta.get('author'):
        extra.append('作者：' + meta['author'])
    if meta.get('date'):
        extra.append('发布时间：' + meta['date'])
    if meta.get('site'):
        extra.append('站点：' + meta['site'])
    if extra:
        lines.append('> ' + ' · '.join(extra))
    lines.extend(['', body])
    return '\n'.join(lines).strip() + '\n'


def _useful(markdown, soup=None, minimum=MIN_ARTICLE_CHARS):
    length = _plain_length(markdown)
    if length >= minimum:
        return True
    if length < 20 or soup is None:
        return False
    root = soup.find('article') or soup.find('main')
    return bool(root and root.find(['p', 'pre', 'table', 'ul', 'ol', 'blockquote']))


def _sanitize_markdown(markdown, base_url=''):
    value = markdown or ''
    value = re.sub(r'<\s*(script|style|iframe|object|embed)\b[^>]*>.*?<\s*/\s*\1\s*>',
                   '', value, flags=re.I | re.S)
    value = re.sub(r'<[^>]+>', '', value)
    value = re.sub(r'(\]\()\s*(?:javascript|data|file):[^)]*(\))', r'\1#\2',
                   value, flags=re.I)
    def absolute_link(match):
        prefix, target, suffix = match.groups()
        raw = target.strip().strip('<>')
        absolute = urljoin(base_url, raw) if base_url else raw
        if urlparse(absolute).scheme.lower() not in ('http', 'https'):
            absolute = '#'
        return prefix + absolute + suffix
    value = re.sub(r'(!?\[[^\]]*\]\()([^\s)]+)([^)]*\))', absolute_link, value)
    return value.strip()


def extract_html(url, html, mode='smart', readability=None, defuddle=None,
                 rendered=False):
    """Extract sanitized Markdown from downloaded or WebView-rendered HTML."""
    url = normalize_url(url)
    _requests, tra, BeautifulSoup, _markdownify = load()
    source_soup = BeautifulSoup(html or '', 'lxml')
    soup = _clean_soup(html, url)
    meta = _metadata(source_soup, url)
    warnings = []
    candidates = _candidate_links(source_soup, url)
    engine_chain = []
    for engine, options in (
            ('trafilatura', {}),
            ('trafilatura-recall', {'favor_recall': True})):
        engine_chain.append(engine)
        try:
            markdown = tra.extract(
                html or '', url=url, output_format='markdown',
                include_comments=False, include_tables=True,
                include_images=True, include_links=True,
                include_formatting=True, deduplicate=True, **options)
        except Exception as exc:
            logging.warning('%s extraction failed: %s', engine, exc)
            warnings.append('%s 提取器失败' % engine)
            markdown = None
        if markdown and _useful(markdown, source_soup):
            return {'ok': True, 'content': _format_document(markdown, meta),
                    'meta': meta, 'engine': engine, 'warnings': warnings,
                    'links': candidates, 'word_count': _plain_length(markdown),
                    'engine_chain': engine_chain, 'attempts': len(engine_chain)}

    if defuddle and (defuddle.get('contentMarkdown') or defuddle.get('markdown')):
        engine_chain.append('defuddle')
        markdown = _sanitize_markdown(
            defuddle.get('contentMarkdown') or defuddle.get('markdown'), url)
        for source, target in (('title', 'title'), ('author', 'author'),
                               ('published', 'date'), ('site', 'site')):
            if defuddle.get(source):
                meta[target] = str(defuddle[source]).strip()
        if _useful(markdown, minimum=20):
            return {'ok': True, 'content': _format_document(markdown, meta),
                    'meta': meta, 'engine': 'defuddle', 'warnings': warnings,
                    'links': candidates, 'word_count': _plain_length(markdown),
                    'engine_chain': engine_chain, 'attempts': len(engine_chain)}

    if readability and readability.get('content'):
        engine_chain.append('mozilla-readability')
        reader_url = readability.get('url') or url
        reader_soup = _clean_soup(readability.get('content'), reader_url)
        markdown = _markdownify_html(str(reader_soup))
        for source, target in (('title', 'title'), ('byline', 'author'),
                               ('publishedTime', 'date'), ('siteName', 'site')):
            if readability.get(source):
                meta[target] = str(readability[source]).strip()
        if markdown and _useful(markdown, reader_soup, minimum=20):
            return {'ok': True, 'content': _format_document(markdown, meta),
                    'meta': meta, 'engine': 'mozilla-readability',
                    'warnings': warnings, 'links': candidates,
                    'word_count': _plain_length(markdown),
                    'engine_chain': engine_chain, 'attempts': len(engine_chain)}

    semantic_root = soup.find('article') or soup.find('main')
    if semantic_root is not None:
        engine_chain.append('semantic-page')
        markdown = _markdownify_html(str(semantic_root))
        if _useful(markdown, soup, minimum=20):
            return {'ok': True, 'content': _format_document(markdown, meta),
                    'meta': meta, 'engine': 'semantic-page', 'warnings': warnings,
                    'links': candidates, 'word_count': _plain_length(markdown),
                    'engine_chain': engine_chain, 'attempts': len(engine_chain)}

    if mode == 'full' or rendered:
        engine_chain.append('full-page')
        root = soup.find('article') or soup.find('main') or soup.body or soup
        markdown = _markdownify_html(str(root))
        if markdown and _plain_length(markdown) >= 20:
            warnings.append('未识别出标准文章结构，已保留清理后的完整页面')
            return {'ok': True, 'content': _format_document(markdown, meta),
                    'meta': meta, 'engine': 'full-page', 'warnings': warnings,
                    'links': candidates, 'word_count': _plain_length(markdown),
                    'engine_chain': engine_chain, 'attempts': len(engine_chain)}

    return {'ok': False, 'code': 'render_required',
            'error': '下载成功，但静态页面中没有足够正文',
            'render_required': True, 'meta': meta, 'warnings': warnings,
            'links': candidates, 'engine_chain': engine_chain,
            'attempts': len(engine_chain), 'fallback_reason': 'content_too_short'}


def fetch_document(url, mode='smart', timeout=25, task_id=None,
                   allow_private=False):
    fetched = fetch_html(url, timeout=timeout, task_id=task_id,
                         allow_private=allow_private)
    result = extract_html(fetched['url'], fetched['html'], mode=mode)
    result['fetch'] = {key: fetched[key] for key in
                       ('url', 'requested_url', 'status', 'content_type',
                        'bytes', 'redirects', 'encoding')}
    return result


def _image_urls(markdown):
    pattern = r'!\[[^]]*\]\((https?://[^)\s]+)(?:\s+["\'][^"\']*["\'])?\)'
    return [match.group(1).strip().strip('<>')
            for match in re.finditer(pattern, markdown or '', re.I)]


def localize_images(markdown, asset_root, task_id=None, allow_private=False):
    """Download safe raster images and rewrite Markdown to temporary paths."""
    urls = []
    for value in _image_urls(markdown):
        if value not in urls:
            urls.append(value)
    urls = urls[:MAX_IMAGES]
    if not urls:
        return markdown, [], []
    os.makedirs(asset_root, exist_ok=True)
    session = _session()
    manifest, warnings, total = [], [], 0
    try:
        for image_url in urls:
            _check_cancel(task_id)
            response = None
            try:
                safe_url = _validate_public_url(image_url, allow_private=allow_private)
                for redirect_no in range(4):
                    response = session.get(safe_url, timeout=(8, 15), stream=True,
                                           allow_redirects=False)
                    if response.status_code not in (301, 302, 303, 307, 308):
                        break
                    location = response.headers.get('Location')
                    response.close()
                    response = None
                    if not location or redirect_no >= 3:
                        raise WebError('image_redirect', '图片重定向次数过多')
                    safe_url = _validate_public_url(
                        urljoin(safe_url, location), allow_private=allow_private)
                if response.status_code != 200:
                    raise WebError('image_http', 'HTTP %d' % response.status_code)
                ctype = (response.headers.get('Content-Type') or '').split(';', 1)[0].lower()
                ext = ALLOWED_IMAGE_TYPES.get(ctype)
                if not ext:
                    raise WebError('image_type', '不支持的图片类型 %s' % (ctype or 'unknown'))
                try:
                    declared_size = int(response.headers.get('Content-Length') or 0)
                except (TypeError, ValueError):
                    declared_size = 0
                if declared_size > MAX_IMAGE_BYTES or total + declared_size > MAX_IMAGE_TOTAL:
                    raise WebError('image_too_large', '图片超过下载大小限制')
                data, size = [], 0
                for chunk in response.iter_content(64 * 1024):
                    _check_cancel(task_id)
                    size += len(chunk)
                    if size > MAX_IMAGE_BYTES or total + size > MAX_IMAGE_TOTAL:
                        raise WebError('image_too_large', '图片超过下载大小限制')
                    data.append(chunk)
                response.close()
                digest = hashlib.sha256(image_url.encode('utf-8')).hexdigest()[:16]
                name = digest + ext
                path = os.path.join(asset_root, name)
                with open(path, 'wb') as handle:
                    handle.write(b''.join(data))
                total += size
                markdown_path = path.replace('\\', '/')
                markdown = markdown.replace(image_url, markdown_path)
                manifest.append({'url': image_url, 'path': path, 'name': name,
                                 'size': size, 'type': ctype})
            except Exception as exc:
                warnings.append('图片下载失败：%s（%s）' % (image_url, exc))
            finally:
                if response is not None:
                    response.close()
    finally:
        session.close()
    return markdown, manifest, warnings


# ---------------------------------------------------------------- compatibility API

def fetch_url(url, timeout=25):
    result = fetch_document(url, timeout=timeout)
    return result.get('content') if result.get('ok') else None


def _extract_links(html, base_url, limit=10):
    return _candidate_links(_clean_soup(html, base_url), base_url, limit)


def crawl(url, max_links=10, timeout=25):
    """Legacy synchronous crawl; the v2.2.3 UI adds progress and cancellation."""
    first = fetch_document(url, timeout=timeout)
    if not first.get('ok'):
        return None
    sections = [first['content']]
    seen = {first.get('fetch', {}).get('url') or normalize_url(url)}
    for link in first.get('links', [])[:max(0, max_links - 1)]:
        if link in seen:
            continue
        seen.add(link)
        try:
            result = fetch_document(link, timeout=timeout)
        except Exception as exc:
            logging.warning('crawl %s failed: %s', link, exc)
            continue
        if result.get('ok'):
            sections.append(re.sub(r'^# ', '## ', result['content'], count=1))
    count = len(sections)
    sections.append('\n---\n\n## 抓取统计\n\n成功合并 %d 个页面。' % count)
    return '\n\n---\n\n'.join(sections)
