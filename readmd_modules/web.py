# -*- coding: utf-8 -*-
"""网页转 md：trafilatura 提取正文（对文章类页面效果好），支持同站批量爬取（懒加载）。"""

import logging
import re

_fetcher = None


def load():
    global _fetcher
    if _fetcher is None:
        import trafilatura
        _fetcher = trafilatura
    return _fetcher


def _title(html):
    if not html:
        return ''
    m = re.search(r'<title[^>]*>(.*?)</title>', html, re.S | re.I)
    if m:
        t = re.sub(r'<[^>]+>', '', m.group(1)).strip()
        return t[:120]
    return ''


def fetch_url(url, timeout=25):
    """抓取单个 URL → Markdown 文本（含来源标题）。"""
    tra = load()
    html = tra.fetch_url(url)
    if not html:
        return None
    md = tra.extract(html, output_format='markdown',
                     include_comments=False, include_tables=True)
    if not md:
        return None
    title = _title(html)
    head = '# %s\n\n> 来源：%s\n' % (title or url, url)
    return head + '\n' + md.strip()


def _extract_links(html, base_url, limit=10):
    """提取同域链接，去重、去锚点、过滤文件型后缀。"""
    from urllib.parse import urljoin, urlparse
    try:
        from lxml import html as lhtml
        doc = lhtml.fromstring(html)
        hrefs = [a.get('href') for a in doc.xpath('//a[@href]')]
    except Exception:  # noqa: BLE001
        hrefs = re.findall(r'<a[^>]+href=["\']([^"\']+)["\']', html, re.I)
    base = urlparse(base_url)
    out = []
    seen = set()
    for h in hrefs:
        if not h or h.startswith(('#', 'javascript:', 'mailto:')):
            continue
        try:
            full = urljoin(base_url, h)
        except Exception:  # noqa: BLE001
            continue
        u = urlparse(full)
        if u.scheme not in ('http', 'https') or u.netloc != base.netloc:
            continue
        clean = u._replace(fragment='').geturl()
        if clean in seen:
            continue
        if re.search(r'\.(pdf|zip|rar|7z|png|jpe?g|gif|webp|mp4|mp3|docx?|xlsx?|pptx?)(\?|$)', clean, re.I):
            continue
        seen.add(clean)
        out.append(clean)
        if len(out) >= limit:
            break
    return out


def crawl(url, max_links=10, timeout=25):
    """批量爬取：主页 + 最多 max_links 个同站链接，合并为一个 Markdown。"""
    tra = load()
    main = fetch_url(url, timeout=timeout)
    if main is None:
        return None
    html = tra.fetch_url(url)
    links = _extract_links(html, url, limit=max_links) if html else []
    sections = [main]
    done = 0
    for link in links:
        try:
            md = fetch_url(link, timeout=timeout)
        except Exception as e:  # noqa: BLE001
            logging.warning('crawl %s failed: %s', link, e)
            md = None
        if md:
            sections.append(md)
            done += 1
    if done:
        sections.append('\n---\n\n## 爬取统计\n\n共合并 %d 个页面。' % (done + 1))
    return '\n\n---\n\n'.join(sections)