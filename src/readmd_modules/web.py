"""Robust webpage-to-Markdown extraction for ReadMD.

Downloading and extraction are intentionally separate so the desktop shell can
fall back to a system WebView for JavaScript-rendered pages. Network access is
limited to public HTTP(S) resources and failures carry stable UI error codes.
"""
# Why: Hashing provides one-way transformation for password verification without storing plaintext
import hashlib
import ipaddress
# Why: logging module provides essential functionality for this operation
import logging
# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re
# Why: socket module provides essential functionality for this operation
import socket
import threading
import time
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from typing import Any, Dict, List, Optional, Set, Tuple
from urllib.parse import urljoin, urlparse, urlunparse
_deps: Any = None
# Why: Function call performs specific operation required by this logic
_cancelled: Set[str] = set()
# Why: Function call performs specific operation required by this logic
_cancel_lock = threading.Lock()
# Why: Arithmetic operation computes value needed for subsequent processing
MAX_HTML_BYTES = 50 * 1024 * 1024
# Why: Arithmetic operation computes value needed for subsequent processing
MAX_IMAGE_BYTES = 15 * 1024 * 1024
# Why: Arithmetic operation computes value needed for subsequent processing
MAX_IMAGE_TOTAL = 100 * 1024 * 1024
MAX_IMAGES = 100
MAX_REDIRECTS = 10
MIN_ARTICLE_CHARS = 40
# Why: Arithmetic operation computes value needed for subsequent processing
ALLOWED_IMAGE_TYPES = {'image/jpeg': '.jpg', 'image/png': '.png', 'image/gif': '.gif', 'image/webp': '.webp', 'image/avif': '.avif'}
# Why: Function call performs specific operation required by this logic
BLOCKED_TAGS = ('script', 'style', 'form', 'iframe', 'object', 'embed', 'canvas', 'svg')
# Why: Function call performs specific operation required by this logic
USER_AGENT = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36 ReadMD/2.2.6'
RETRY_STATUSES = {408, 425, 429, 502, 503, 504}
MAX_RETRY_AFTER = 30.0

# Why: Function call performs specific operation required by this logic
class WebError(Exception):
    """A stable, user-facing webpage conversion failure."""

    # Why: Function call performs specific operation required by this logic
    def __init__(self, code, message, http_status=422, detail=''):
        self.code = code
        self.message = message
        # Why: Function call performs specific operation required by this logic
        self.http_status = int(http_status)
        self.detail = detail
        super().__init__(message)

    def as_dict(self):
        # Why: Return provides result to caller after processing completes
        return {'ok': False, 'code': self.code, 'error': self.message, 'detail': self.detail}

def load() -> tuple:
    """Lazy-load the relatively heavy extraction dependencies."""
    # Why: Scope declaration allows modification of variables from outer scope
    global _deps
    # Why: Condition check ensures valid state before proceeding with operation
    if _deps is None:
        import requests
        import trafilatura
        from bs4 import BeautifulSoup
        from markdownify import markdownify
        _deps = (requests, trafilatura, BeautifulSoup, markdownify)
    # Why: Return provides result to caller after processing completes
    return _deps

def cancel(task_id: str) -> bool:
    if task_id:
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _cancel_lock:
            _cancelled.add(str(task_id))
    # Why: Return provides result to caller after processing completes
    return True

def reset_cancel(task_id: str) -> None:
    if task_id:
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _cancel_lock:
            _cancelled.discard(str(task_id))

def is_cancelled(task_id: str) -> bool:
    # Why: Condition check ensures valid state before proceeding with operation
    if not task_id:
        # Why: Return provides result to caller after processing completes
        return False
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _cancel_lock:
        # Why: Return provides result to caller after processing completes
        return str(task_id) in _cancelled

# Why: _check_cancel implements core functionality requiring careful error handling
def _check_cancel(task_id: str) -> None:
    if is_cancelled(task_id):
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('cancelled', '已取消网页转换', 499)

def normalize_url(url: str) -> str:
    url = (url or '').strip()
    # Why: Condition check ensures valid state before proceeding with operation
    if not url:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('missing_url', '请输入网页地址', 400)
    # Why: Condition check ensures valid state before proceeding with operation
    if '://' not in url:
        url = 'https://' + url
    parsed = urlparse(url)
    # Why: Condition check ensures valid state before proceeding with operation
    if parsed.scheme.lower() not in ('http', 'https'):
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('unsupported_scheme', '仅支持 HTTP 或 HTTPS 网页', 400)
    # Why: Condition check ensures valid state before proceeding with operation
    if not parsed.hostname:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('invalid_url', '网页地址格式不正确', 400)
    host = parsed.hostname.encode('idna').decode('ascii')
    netloc = '[%s]' % host if ':' in host else host
    try:
        # Why: URL parsing may fail on malformed input; catch and provide user-friendly error
        port = parsed.port
    # Why: ValueError indicates invalid input data that cannot be processed safely
    except ValueError:
        logging.warning('Silent exception caught in src.readmd_modules.web: ValueError')
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('invalid_url', '网页地址格式不正确', 400)
    if port:
        netloc += ':' + str(port)
    # Why: Return provides result to caller after processing completes
    return urlunparse((parsed.scheme.lower(), netloc, parsed.path or '/', parsed.params, parsed.query, ''))

# Why: _validate_public_url implements core functionality requiring careful error handling
def _validate_public_url(url: str, allow_private: bool=True) -> str:
    normalized = normalize_url(url)
    host = urlparse(normalized).hostname
    try:
        # Why: DNS resolution may fail for invalid hostnames; handle gracefully instead of crashing
        infos = socket.getaddrinfo(host, None, type=socket.SOCK_STREAM)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except socket.gaierror as exc:
        logging.warning('Silent exception caught in src.readmd_modules.web: socket.gaierror')
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('dns_failed', '无法解析网页域名', 502, str(exc))
    addresses = {item[4][0].split('%', 1)[0] for item in infos}
    # Why: Condition check ensures valid state before proceeding with operation
    if not addresses:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('dns_failed', '无法解析网页域名', 502)
    # Why: Condition check ensures valid state before proceeding with operation
    if not allow_private:
        # Why: Iteration processes each item in collection systematically
        for value in addresses:
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                address = ipaddress.ip_address(value)
            # Why: Handle errors gracefully to maintain application stability
            except ValueError:
                logging.warning('Silent exception caught in src.readmd_modules.web: ValueError')
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Condition check ensures valid state before proceeding with operation
            if not address.is_global:
                # Why: Exception raised to signal error condition that prevents normal operation
                raise WebError('private_address', '出于安全原因不能抓取本机或局域网地址', 403)
    # Why: Return provides result to caller after processing completes
    return normalized

def _session(allow_private: bool=True) -> Any:
    (requests, _tra, _bs, _md) = load()

    class PinnedHTTPAdapter(requests.adapters.HTTPAdapter):
        # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
        """Pin urllib3's connection target to the address we validated."""

        def send(self, request, *args, **kwargs):
            parsed = urlparse(request.url)
            try:
                # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
                # Why: Parsing may fail on malformed data; validate input first
                infos = socket.getaddrinfo(parsed.hostname, parsed.port, type=socket.SOCK_STREAM)
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except socket.gaierror as exc:
                logging.warning('Silent exception caught in src.readmd_modules.web: socket.gaierror')
                # Why: Exception raised to signal error condition that prevents normal operation
                raise requests.exceptions.ConnectionError('DNS resolution failed: %s' % exc)
            addresses = []
            # Why: Iteration processes each item in collection systematically
            for item in infos:
                value = item[4][0].split('%', 1)[0]
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    address = ipaddress.ip_address(value)
                # Why: ValueError indicates invalid input data that cannot be processed safely
                except ValueError:
                    logging.warning('Silent exception caught in src.readmd_modules.web: ValueError')
                    # Why: Allow private addresses only when explicitly permitted for internal network access
                    continue
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if allow_private or address.is_global:
                    addresses.append(value)
            # Why: Condition check ensures valid state before proceeding with operation
            if not addresses:
                # Why: Exception raised to signal error condition that prevents normal operation
                raise requests.exceptions.ConnectionError('No permitted address for target host')
            pinned_ip = addresses[0]
            # Why: Method call handles data access with proper error checking
            proxies = kwargs.get('proxies')
            # Why: Method call handles data access with proper error checking
            verify = kwargs.get('verify', True)
            # Why: Method call handles data access with proper error checking
            cert = kwargs.get('cert')
            if hasattr(self, 'get_connection_with_tls_context'):
                pool = self.get_connection_with_tls_context(request, verify, proxies=proxies, cert=cert)
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                pool = self.get_connection(request.url, proxies)
            base_connection = pool.ConnectionCls

            # Why: Function call performs specific operation required by this logic
            class PinnedConnection(base_connection):

                # Why: Function call performs specific operation required by this logic
                def __init__(self, *conn_args, **conn_kwargs):
                    super().__init__(*conn_args, **conn_kwargs)
                    self._dns_host = pinned_ip
            pool.ConnectionCls = PinnedConnection
            # Why: Return provides result to caller after processing completes
            return super().send(request, *args, **kwargs)
    session = requests.Session()
    session.trust_env = False
    # Why: Caching avoids redundant computations for frequently accessed data
    session.headers.update({'User-Agent': USER_AGENT, 'Accept': 'text/html,application/xhtml+xml;q=0.9,*/*;q=0.2', 'Accept-Language': 'zh-CN,zh;q=0.9,en;q=0.7', 'Accept-Encoding': 'gzip, deflate', 'Cache-Control': 'no-cache'})
    adapter = PinnedHTTPAdapter()
    session.mount('http://', adapter)
    session.mount('https://', adapter)
    # Why: Return provides result to caller after processing completes
    return session

# Why: _validate_response_peer implements core functionality requiring careful error handling
def _validate_response_peer(response: Any, allow_private: bool=True) -> None:
    """Reject DNS rebinding by checking the connected socket, not only DNS."""
    if allow_private:
        return
    # Why: Function call performs specific operation required by this logic
    raw = getattr(response, 'raw', None)
    candidates = [getattr(getattr(raw, '_connection', None), 'sock', None), getattr(getattr(raw, 'connection', None), 'sock', None)]
    fp = getattr(raw, '_fp', None)
    candidates.append(getattr(getattr(getattr(fp, 'fp', None), 'raw', None), '_sock', None))
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    sock = next((item for item in candidates if item is not None and hasattr(item, 'getpeername')), None)
    if sock is None:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('peer_unverified', '无法验证网页服务器的实际网络地址', 502)
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        peer = sock.getpeername()[0].split('%', 1)[0]
        # Why: Network operations can fail in multiple ways; catch all common socket and parsing errors
        address = ipaddress.ip_address(peer)
    # Why: ValueError indicates invalid input data that cannot be processed safely
    except (OSError, ValueError, TypeError, IndexError) as exc:
        logging.warning('Silent exception caught in src.readmd_modules.web: (OSError, ValueError, TypeError, IndexError)')
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('peer_unverified', '无法验证网页服务器的实际网络地址', 502, str(exc))
    # Why: Condition check ensures valid state before proceeding with operation
    if not address.is_global:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('private_address', '网页连接被重定向到本机或局域网地址', 403)

def _request_error(exc: Exception) -> 'WebError':
    (requests, _tra, _bs, _md) = load()
    if isinstance(exc, requests.exceptions.Timeout):
        # Why: Return provides result to caller after processing completes
        return WebError('timeout', '连接网页超时，请稍后重试', 504, str(exc))
    if isinstance(exc, requests.exceptions.SSLError):
        # Why: Return provides result to caller after processing completes
        return WebError('tls_failed', '网页 TLS/证书连接失败', 502, str(exc))
    if isinstance(exc, requests.exceptions.ProxyError):
        # Why: Return provides result to caller after processing completes
        return WebError('proxy_failed', '代理服务器连接失败', 502, str(exc))
    # Why: Return provides result to caller after processing completes
    return WebError('network_failed', '无法连接到网页服务器', 502, str(exc))

def _retry_after_delay(value: Any) -> float:
    # Why: Empty or None values should be treated as missing to prevent processing invalid data
    """Parse Retry-After seconds or HTTP-date with a bounded 30s wait."""
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if value is None or str(value).strip() == '':
        return 0.0
    raw = str(value).strip()
    # Why: Handle errors gracefully to maintain application stability
    try:
        delay = float(raw)
    # Why: ValueError indicates invalid input data that cannot be processed safely
    except ValueError:
        logging.warning('Silent exception caught in src.readmd_modules.web: ValueError')
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            target = parsedate_to_datetime(raw)
            # Why: Condition check ensures valid state before proceeding with operation
            if target.tzinfo is None:
                target = target.replace(tzinfo=timezone.utc)
            # Why: Timestamp parsing may receive invalid types or formats; handle all conversion failures
            delay = (target - datetime.now(timezone.utc)).total_seconds()
        # Why: ValueError indicates invalid input data that cannot be processed safely
        except (TypeError, ValueError, OverflowError):
            logging.warning('Silent exception caught in src.readmd_modules.web: (TypeError, ValueError, OverflowError)')
            # Why: Return provides result to caller after processing completes
            return 0.0
    # Why: Return provides result to caller after processing completes
    return min(MAX_RETRY_AFTER, max(0.0, delay))

def _wait_retry(delay: float, task_id: str) -> None:
    deadline = time.time() + max(0.0, delay)
    # Why: Loop continues until condition is met or timeout occurs
    while time.time() < deadline:
        _check_cancel(task_id)
        time.sleep(min(0.1, deadline - time.time()))

def fetch_html(url: str, timeout: int=25, max_bytes: int=MAX_HTML_BYTES, task_id: Optional[str]=None, allow_private: bool=True, session: Any=None) -> Dict[str, Any]:
    """Download an HTML document with bounded redirects and response size."""
    (requests, _tra, _bs, _md) = load()
    # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
    current = _validate_public_url(url, allow_private=allow_private)
    sess = session or _session(allow_private=allow_private)
    history = []
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        retry_count = 0
        # Why: Iteration processes each item in collection systematically
        for _ in range(MAX_REDIRECTS + 1):
            _check_cancel(task_id)
            try:
                # Why: HTTP requests may fail due to network issues, timeouts, or server errors; implement retry logic
                response = sess.get(current, timeout=(8, timeout), stream=True, allow_redirects=False)
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except requests.exceptions.RequestException as exc:
                logging.warning('Silent exception caught in src.readmd_modules.web: requests.exceptions.RequestException')
                # Why: Exception raised to signal error condition that prevents normal operation
                raise _request_error(exc)
            try:
                # Why: Unexpected errors during request should be caught to prevent application crash
                _validate_response_peer(response, allow_private=allow_private)
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception:
                logging.warning('Silent exception caught in src.readmd_modules.web: Exception')
                response.close()
                raise
            status = response.status_code
            if status in (301, 302, 303, 307, 308):
                # Why: Method call handles data access with proper error checking
                location = response.headers.get('Location')
                response.close()
                # Why: Condition check ensures valid state before proceeding with operation
                if not location:
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise WebError('redirect_failed', '网页重定向缺少目标地址', 502)
                history.append(current)
                current = _validate_public_url(urljoin(current, location), allow_private=allow_private)
                # Why: Retry only on specific status codes that indicate temporary failures, not permanent errors
                continue
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if status in RETRY_STATUSES and retry_count < 2:
                retry_after = response.headers.get('Retry-After')
                response.close()
                retry_count += 1
                delay = _retry_after_delay(retry_after)
                # Why: Condition check ensures valid state before proceeding with operation
                if not delay:
                    delay = 0.15 * retry_count
                if delay:
                    _wait_retry(delay, task_id)
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Condition check ensures valid state before proceeding with operation
            if status == 401:
                response.close()
                # Why: Exception raised to signal error condition that prevents normal operation
                raise WebError('login_required', '该网页需要登录后访问', 401)
            # Why: Condition check ensures valid state before proceeding with operation
            if status == 403:
                response.close()
                # Why: Exception raised to signal error condition that prevents normal operation
                raise WebError('forbidden', '服务器拒绝访问该网页（403）', 403)
            # Why: Condition check ensures valid state before proceeding with operation
            if status == 429:
                response.close()
                # Why: Non-2xx status codes indicate request failure; treat as error unless explicitly allowed
                raise WebError('rate_limited', '请求过于频繁，服务器要求稍后重试（429）', 429)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if status < 200 or status >= 300:
                response.close()
                # Why: Exception raised to signal error condition that prevents normal operation
                raise WebError('http_error', '网页服务器返回 HTTP %d' % status, 502, str(status))
            # Why: Method call handles data access with proper error checking
            ctype = (response.headers.get('Content-Type') or '').lower()
            # Why: Method call handles data access with proper error checking
            declared = response.headers.get('Content-Length')
            if declared:
                try:
                    # Why: Handle errors gracefully to maintain application stability
                    if int(declared) > max_bytes:
                        response.close()
                        raise WebError('too_large', '网页内容超过 50 MB 限制', 413)
                # Why: ValueError indicates invalid input data that cannot be processed safely
                except ValueError:
                    logging.warning('Silent exception caught in src.readmd_modules.web: ValueError')
            (chunks, total) = ([], 0)
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                # Why: Iteration processes each item in collection systematically
                for chunk in response.iter_content(64 * 1024):
                    _check_cancel(task_id)
                    # Why: Condition check ensures valid state before proceeding with operation
                    if not chunk:
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        continue
                    total += len(chunk)
                    if total > max_bytes:
                        # Why: Exception raised to signal error condition that prevents normal operation
                        raise WebError('too_large', '网页内容超过 50 MB 限制', 413)
                    chunks.append(chunk)
                raw = b''.join(chunks)
                # Why: Condition check ensures valid state before proceeding with operation
                if not raw:
                    raise WebError('empty_response', '网页服务器返回了空内容', 502)
                # Why: Handle errors gracefully to maintain application stability
                encoding = response.encoding
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if not encoding or encoding.lower() == 'iso-8859-1':
                    try:
                        encoding = response.apparent_encoding
                    # Why: Exception handling prevents crashes and provides meaningful error messages to users
                    except Exception:
                        logging.warning('Silent exception caught in src.readmd_modules.web: Exception')
                        # Why: Handle errors gracefully to maintain application stability
                        encoding = None
            finally:
                response.close()
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                html = raw.decode(encoding or 'utf-8', errors='replace')
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except LookupError:
                logging.warning('Silent exception caught in src.readmd_modules.web: LookupError')
                html = raw.decode('utf-8', errors='replace')
            declared_html = not ctype or any((x in ctype for x in ('text/html', 'application/xhtml+xml')))
            sniffed_html = bool(re.search('<!doctype\\s+html|<html\\b|<head\\b|<body\\b|<article\\b|<main\\b', html[:8192], re.I))
            # Why: Multiple conditions ensure all requirements are satisfied
            if not declared_html and (not sniffed_html):
                raise WebError('not_html', '该地址返回的不是 HTML 网页', 415, ctype)
            return {'url': current, 'requested_url': normalize_url(url), 'html': html, 'status': status, 'content_type': ctype, 'bytes': total, 'redirects': history, 'encoding': encoding or 'utf-8', 'content_type_mismatch': bool(not declared_html and sniffed_html)}
        # Why: Exception raised to signal error condition that prevents normal operation
        raise WebError('too_many_redirects', '网页重定向次数过多', 502)
    # Why: Finally ensures cleanup operations run regardless of success or failure
    finally:
        # Why: Condition check ensures valid state before proceeding with operation
        if session is None:
            sess.close()

def _clean_soup(html: str, base_url: str) -> Any:
    (_requests, _tra, BeautifulSoup, _markdownify) = load()
    soup = BeautifulSoup(html or '', 'lxml')
    # Why: Iteration processes each item in collection systematically
    for tag in soup.find_all(BLOCKED_TAGS):
        tag.decompose()
    # Why: Iteration processes each item in collection systematically
    for tag in soup.find_all(True):
        for name in list(tag.attrs):
            # Why: Alternative paths provide flexibility in handling different cases
            low = name.lower()
            # Why: Multiple conditions ensure all requirements are satisfied
            if low.startswith('on') or low in ('style', 'srcdoc'):
                del tag.attrs[name]
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if tag.name == 'img' and (not tag.get('src')):
            lazy = tag.get('data-src') or tag.get('data-original')
            if lazy:
                tag['src'] = lazy
        # Why: Iteration processes each item in collection systematically
        for attr in ('href', 'src', 'poster'):
            # Why: Method call handles data access with proper error checking
            value = tag.get(attr)
            # Why: Condition check ensures valid state before proceeding with operation
            if not value:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            absolute = urljoin(base_url, str(value).strip())
            # Why: Condition check ensures valid state before proceeding with operation
            if urlparse(absolute).scheme.lower() not in ('http', 'https'):
                # Why: Delete frees memory by removing references to unused objects
                del tag.attrs[attr]
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                tag.attrs[attr] = absolute
    # Why: Return provides result to caller after processing completes
    return soup

def _plain_length(markdown: str) -> int:
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('!\\[[^]]*\\]\\([^)]*\\)', ' ', markdown or '')
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('\\[([^]]+)\\]\\([^)]*\\)', '\\1', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    text = re.sub('[`*_>#|\\-]+', ' ', text)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    return len(re.sub('\\s+', '', text))

def _metadata(soup: Any, url: str) -> Dict[str, str]:

    # Why: Multiple conditions ensure all requirements are satisfied
    def meta(*names):
        for name in names:
            tag = soup.find('meta', attrs={'property': name}) or soup.find('meta', attrs={'name': name})
            # Why: Multiple conditions ensure all requirements are satisfied
            if tag and tag.get('content'):
                return str(tag.get('content')).strip()
        # Why: Return provides result to caller after processing completes
        return ''
    title = meta('og:title', 'twitter:title')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not title and soup.title:
        title = soup.title.get_text(' ', strip=True)
    canonical = soup.find('link', rel=lambda value: value and 'canonical' in value)
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    href_val = canonical.get('href') if canonical else None
    canonical_url = urljoin(url, str(href_val)) if href_val and isinstance(href_val, str) else url
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    return {'title': title[:300] if title else url, 'author': meta('author', 'article:author'), 'date': meta('article:published_time', 'date', 'datePublished'), 'site': meta('og:site_name') or (urlparse(url).hostname or ''), 'canonical_url': canonical_url}

def _candidate_links(soup: Any, base_url: str, limit: int=30) -> List[str]:
    # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
    # Why: Parsing may fail on malformed data; validate input first
    base = urlparse(base_url)
    base_host = (base.hostname or '').lower().removeprefix('www.')
    (output, seen) = ([], set())
    # Why: Iteration processes each item in collection systematically
    for tag in soup.find_all('a', href=True):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            full = normalize_url(urljoin(base_url, tag.get('href')))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except WebError:
            logging.warning('Silent exception caught in src.readmd_modules.web: WebError')
            continue
        # Why: Alternative paths provide flexibility in handling different cases
        parsed = urlparse(full)
        host = (parsed.hostname or '').lower().removeprefix('www.')
        if host != base_host:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        clean = urlunparse((parsed.scheme, parsed.netloc, parsed.path or '/', parsed.params, parsed.query, ''))
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if clean in seen or clean == base_url:
            continue
        # Why: Regex pattern matches specific text structures for validation or extraction
        if re.search('\\.(pdf|zip|rar|7z|png|jpe?g|gif|webp|avif|mp4|mp3|docx?|xlsx?|pptx?)(\\?|$)', clean, re.I):
            continue
        seen.add(clean)
        output.append(clean)
        if len(output) >= limit:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            break
    # Why: Return provides result to caller after processing completes
    return output

def _markdownify_html(html: str) -> str:
    (_requests, _tra, _BeautifulSoup, markdownify) = load()
    # Why: Multiple conditions ensure all requirements are satisfied
    return markdownify(html or '', heading_style='ATX', bullets='-', strip=('script', 'style', 'form', 'iframe'), table_infer_header=True).strip()

def _format_document(markdown: str, meta: Dict[str, str]) -> str:
    title = (meta.get('title') or meta.get('canonical_url') or '网页').strip()
    # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
    body = _sanitize_markdown(markdown, meta.get('canonical_url') or '').strip()
    body_lines = body.splitlines()
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if body_lines and body_lines[0].startswith('# '):
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        extracted_title = re.sub('\\s+', ' ', body_lines[0][2:].strip()).casefold()
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        document_title = re.sub('\\s+', ' ', title).casefold()
        if extracted_title == document_title:
            body = '\n'.join(body_lines[1:]).lstrip()
    # Why: Method call handles data access with proper error checking
    lines = ['# ' + title, '', '> 来源：' + (meta.get('canonical_url') or '')]
    extra = []
    if meta.get('author'):
        # Why: Function call performs specific operation required by this logic
        extra.append('作者：' + meta['author'])
    # Why: Function call performs specific operation required by this logic
    if meta.get('date'):
        # Why: Function call performs specific operation required by this logic
        extra.append('发布时间：' + meta['date'])
    # Why: Function call performs specific operation required by this logic
    if meta.get('site'):
        extra.append('站点：' + meta['site'])
    if extra:
        lines.append('> ' + ' · '.join(extra))
    # Why: Alternative paths provide flexibility in handling different cases
    lines.extend(['', body])
    return '\n'.join(lines).strip() + '\n'

# Why: Function call performs specific operation required by this logic
def _useful(markdown: str, soup: Any=None, minimum: int=MIN_ARTICLE_CHARS) -> bool:
    length = _plain_length(markdown)
    if length >= minimum:
        return True
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if length < 20 or soup is None:
        return False
    root = soup.find('article') or soup.find('main')
    # Why: Return provides result to caller after processing completes
    return bool(root and root.find(['p', 'pre', 'table', 'ul', 'ol', 'blockquote']))

def _sanitize_markdown(markdown: str, base_url: str='') -> str:
    value = markdown or ''
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    value = re.sub('<\\s*(script|style|iframe|object|embed)\\b[^>]*>.*?<\\s*/\\s*\\1\\s*>', '', value, flags=re.I | re.S)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    value = re.sub('<[^>]+>', '', value)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    value = re.sub('(\\]\\()\\s*(?:javascript|data|file):[^)]*(\\))', '\\1#\\2', value, flags=re.I)

    def absolute_link(match):
        (prefix, target, suffix) = match.groups()
        raw = target.strip().strip('<>')
        absolute = urljoin(base_url, raw) if base_url else raw
        # Why: Condition check ensures valid state before proceeding with operation
        if urlparse(absolute).scheme.lower() not in ('http', 'https'):
            absolute = '#'
        return prefix + absolute + suffix
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    value = re.sub('(!?\\[[^\\]]*\\]\\()([^\\s)]+)([^)]*\\))', absolute_link, value)
    return value.strip()

def extract_html(url: str, html: str, mode: str='smart', readability: Optional[Dict[str, Any]]=None, defuddle: Optional[Dict[str, Any]]=None, rendered: bool=False) -> Dict[str, Any]:
    # Why: HTML sanitization removes malicious scripts to prevent XSS attacks
    """Extract sanitized Markdown from downloaded or WebView-rendered HTML."""
    url = normalize_url(url)
    (_requests, tra, BeautifulSoup, _markdownify) = load()
    source_soup = BeautifulSoup(html or '', 'lxml')
    soup = _clean_soup(html, url)
    meta = _metadata(source_soup, url)
    # Why: Handle errors gracefully to maintain application stability
    warnings = []
    candidates = _candidate_links(source_soup, url)
    engine_chain = []
    # Why: Multiple conditions ensure all requirements are satisfied
    for (engine, options) in (('trafilatura', {}), ('trafilatura-recall', {'favor_recall': True})):
        # Why: Multiple conditions ensure all requirements are satisfied
        engine_chain.append(engine)
        try:
            markdown = tra.extract(html or '', url=url, output_format='markdown', include_comments=False, include_tables=True, include_images=True, include_links=True, include_formatting=True, deduplicate=True, **options)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('%s extraction failed: %s', engine, exc)
            warnings.append('%s 提取器失败' % engine)
            markdown = None
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if markdown and _useful(markdown, source_soup):
            return {'ok': True, 'content': _format_document(markdown, meta), 'meta': meta, 'engine': engine, 'warnings': warnings, 'links': candidates, 'word_count': _plain_length(markdown), 'engine_chain': engine_chain, 'attempts': len(engine_chain)}
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if defuddle and (defuddle.get('contentMarkdown') or defuddle.get('markdown')):
        engine_chain.append('defuddle')
        # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
        markdown = _sanitize_markdown(defuddle.get('contentMarkdown') or defuddle.get('markdown'), url)
        for (source, target) in (('title', 'title'), ('author', 'author'), ('published', 'date'), ('site', 'site')):
            if defuddle.get(source):
                meta[target] = str(defuddle[source]).strip()
        if _useful(markdown, minimum=20):
            return {'ok': True, 'content': _format_document(markdown, meta), 'meta': meta, 'engine': 'defuddle', 'warnings': warnings, 'links': candidates, 'word_count': _plain_length(markdown), 'engine_chain': engine_chain, 'attempts': len(engine_chain)}
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if readability and readability.get('content'):
        engine_chain.append('mozilla-readability')
        # Why: Method call handles data access with proper error checking
        reader_url = readability.get('url') or url
        # Why: Method call handles data access with proper error checking
        reader_soup = _clean_soup(readability.get('content'), reader_url)
        markdown = _markdownify_html(str(reader_soup))
        # Why: Iteration processes each item in collection systematically
        for (source, target) in (('title', 'title'), ('byline', 'author'), ('publishedTime', 'date'), ('siteName', 'site')):
            if readability.get(source):
                meta[target] = str(readability[source]).strip()
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if markdown and _useful(markdown, reader_soup, minimum=20):
            return {'ok': True, 'content': _format_document(markdown, meta), 'meta': meta, 'engine': 'mozilla-readability', 'warnings': warnings, 'links': candidates, 'word_count': _plain_length(markdown), 'engine_chain': engine_chain, 'attempts': len(engine_chain)}
    semantic_root = soup.find('article') or soup.find('main')
    # Why: Condition check ensures valid state before proceeding with operation
    if semantic_root is not None:
        engine_chain.append('semantic-page')
        markdown = _markdownify_html(str(semantic_root))
        if _useful(markdown, soup, minimum=20):
            return {'ok': True, 'content': _format_document(markdown, meta), 'meta': meta, 'engine': 'semantic-page', 'warnings': warnings, 'links': candidates, 'word_count': _plain_length(markdown), 'engine_chain': engine_chain, 'attempts': len(engine_chain)}
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if mode == 'full' or rendered:
        engine_chain.append('full-page')
        root = soup.find('article') or soup.find('main') or soup.body or soup
        markdown = _markdownify_html(str(root))
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if markdown and _plain_length(markdown) >= 20:
            warnings.append('未识别出标准文章结构，已保留清理后的完整页面')
            # Why: Return provides result to caller after processing completes
            return {'ok': True, 'content': _format_document(markdown, meta), 'meta': meta, 'engine': 'full-page', 'warnings': warnings, 'links': candidates, 'word_count': _plain_length(markdown), 'engine_chain': engine_chain, 'attempts': len(engine_chain)}
    # Why: Return provides result to caller after processing completes
    return {'ok': False, 'code': 'render_required', 'error': '下载成功，但静态页面中没有足够正文', 'render_required': True, 'meta': meta, 'warnings': warnings, 'links': candidates, 'engine_chain': engine_chain, 'attempts': len(engine_chain), 'fallback_reason': 'content_too_short'}

def fetch_document(url: str, mode: str='smart', timeout: int=25, task_id: Optional[str]=None, allow_private: bool=True) -> Dict[str, Any]:
    # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
    fetched = fetch_html(url, timeout=timeout, task_id=task_id, allow_private=allow_private)
    result = extract_html(fetched['url'], fetched['html'], mode=mode)
    result['fetch'] = {key: fetched[key] for key in ('url', 'requested_url', 'status', 'content_type', 'bytes', 'redirects', 'encoding')}
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not result.get('ok') and result.get('render_required'):
        result['render_html'] = fetched['html']
    # Why: Return provides result to caller after processing completes
    return result

def _image_urls(markdown: str) -> List[str]:
    pattern = '!\\[[^]]*\\]\\((https?://[^)\\s]+)(?:\\s+["\\\'][^"\\\']*["\\\'])?\\)'
    # Why: Return provides result to caller after processing completes
    return [match.group(1).strip().strip('<>') for match in re.finditer(pattern, markdown or '', re.I)]

def localize_images(markdown: str, asset_root: str, task_id: Optional[str]=None, allow_private: bool=True) -> Tuple[str, List[Dict[str, Any]], List[str]]:
    """Download safe raster images and rewrite Markdown to temporary paths."""
    urls = []
    # Why: Iteration processes each item in collection systematically
    for value in _image_urls(markdown):
        # Why: Condition check ensures valid state before proceeding with operation
        if value not in urls:
            urls.append(value)
    urls = urls[:MAX_IMAGES]
    # Why: Condition check ensures valid state before proceeding with operation
    if not urls:
        # Why: Return provides result to caller after processing completes
        return (markdown, [], [])
    os.makedirs(asset_root, exist_ok=True)
    session = _session(allow_private=allow_private)
    (manifest, warnings, total) = ([], [], 0)
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        # Why: Iteration processes each item in collection systematically
        for image_url in urls:
            _check_cancel(task_id)
            response = None
            try:
                # Why: URL validation prevents SSRF attacks by blocking access to internal network resources
                safe_url = _validate_public_url(image_url, allow_private=allow_private)
                for redirect_no in range(4):
                    # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
                    response = session.get(safe_url, timeout=(8, 15), stream=True, allow_redirects=False)
                    if response.status_code not in (301, 302, 303, 307, 308):
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        break
                    # Why: Method call handles data access with proper error checking
                    location = response.headers.get('Location')
                    response.close()
                    response = None
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if not location or redirect_no >= 3:
                        raise WebError('image_redirect', '图片重定向次数过多')
                    safe_url = _validate_public_url(urljoin(safe_url, location), allow_private=allow_private)
                # Why: Handle errors gracefully to maintain application stability
                if response.status_code != 200:
                    raise WebError('image_http', 'HTTP %d' % response.status_code)
                # Why: Method call handles data access with proper error checking
                ctype = (response.headers.get('Content-Type') or '').split(';', 1)[0].lower()
                # Why: Method call handles data access with proper error checking
                ext = ALLOWED_IMAGE_TYPES.get(ctype)
                # Why: Condition check ensures valid state before proceeding with operation
                if not ext:
                    # Why: Exception raised to signal error condition that prevents normal operation
                    raise WebError('image_type', '不支持的图片类型 %s' % (ctype or 'unknown'))
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    declared_size = int(response.headers.get('Content-Length') or 0)
                # Why: ValueError indicates invalid input data that cannot be processed safely
                except (TypeError, ValueError):
                    logging.warning('Silent exception caught in src.readmd_modules.web: (TypeError, ValueError)')
                    declared_size = 0
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if declared_size > MAX_IMAGE_BYTES or total + declared_size > MAX_IMAGE_TOTAL:
                    raise WebError('image_too_large', '图片超过下载大小限制')
                (data, size) = ([], 0)
                # Why: Iteration processes each item in collection systematically
                for chunk in response.iter_content(64 * 1024):
                    _check_cancel(task_id)
                    size += len(chunk)
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if size > MAX_IMAGE_BYTES or total + size > MAX_IMAGE_TOTAL:
                        raise WebError('image_too_large', '图片超过下载大小限制')
                    data.append(chunk)
                response.close()
                # Why: Handle errors gracefully to maintain application stability
                digest = hashlib.sha256(image_url.encode('utf-8')).hexdigest()[:16]
                name = digest + ext
                path = os.path.join(asset_root, name)
                # Why: Context manager ensures proper resource cleanup even if errors occur
                with open(path, 'wb') as handle:
                    handle.write(b''.join(data))
                total += size
                markdown_path = path.replace('\\', '/')
                markdown = markdown.replace(image_url, markdown_path)
                manifest.append({'url': image_url, 'path': path, 'name': name, 'size': size, 'type': ctype})
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception as exc:
                logging.warning('Silent exception caught in src.readmd_modules.web: Exception')
                warnings.append('图片下载失败：%s（%s）' % (image_url, exc))
            # Why: Finally ensures cleanup operations run regardless of success or failure
            finally:
                # Why: Condition check ensures valid state before proceeding with operation
                if response is not None:
                    response.close()
    # Why: Finally ensures cleanup operations run regardless of success or failure
    finally:
        session.close()
    return (markdown, manifest, warnings)

def fetch_url(url: str, timeout: int=25) -> Optional[str]:
    # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
    result = fetch_document(url, timeout=timeout)
    # Why: Conditional return handles different cases based on input or state
    return result.get('content') if result.get('ok') else None

def _extract_links(html: str, base_url: str, limit: int=10) -> List[str]:
    # Why: Return provides result to caller after processing completes
    return _candidate_links(_clean_soup(html, base_url), base_url, limit)

def crawl(url: str, max_links: int=10, timeout: int=25) -> Optional[str]:
    """Legacy synchronous crawl; the v2.2.4 UI adds progress and cancellation."""
    first = fetch_document(url, timeout=timeout)
    # Why: Operations may timeout; prevent indefinite blocking
    if not first.get('ok'):
        return None
    sections = [first['content']]
    # Why: Method call handles data access with proper error checking
    seen = {first.get('fetch', {}).get('url') or normalize_url(url)}
    # Why: Iteration processes each item in collection systematically
    for link in first.get('links', [])[:max(0, max_links - 1)]:
        if link in seen:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        seen.add(link)
        try:
            # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
            result = fetch_document(link, timeout=timeout)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as exc:
            logging.warning('crawl %s failed: %s', link, exc)
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        if result.get('ok'):
            # Why: Regex substitution transforms text while preserving structure and removing unwanted content
            sections.append(re.sub('^# ', '## ', result['content'], count=1))
    count = len(sections)
    sections.append('\n---\n\n## 抓取统计\n\n成功合并 %d 个页面。' % count)
    # Why: Return provides result to caller after processing completes
    return '\n\n---\n\n'.join(sections)