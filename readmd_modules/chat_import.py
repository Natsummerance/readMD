# -*- coding: utf-8 -*-
"""Offline, defensive chat-export importer.

The module deliberately has no UI or network dependencies.  Callers pass bytes
or text in and receive a small public model plus Markdown; raw conversation
content is never logged or written by this module.
"""

from __future__ import unicode_literals

import datetime
import html
import json
import os
import re
import zipfile
from dataclasses import dataclass, field
from html.parser import HTMLParser
from io import BytesIO
from pathlib import PurePosixPath

MAX_TEXT_BYTES = 10 * 1024 * 1024
MAX_ZIP_BYTES = 100 * 1024 * 1024
MAX_ZIP_EXPANDED = 200 * 1024 * 1024
MAX_ZIP_RATIO = 100
KNOWN_EXTENSIONS = ('.json', '.html', '.htm', '.md', '.txt')


class ChatImportError(Exception):
    def __init__(self, code, message):
        self.code, self.message = code, message
        super().__init__(message)

    def as_dict(self):
        return {'ok': False, 'code': self.code, 'error': self.message}


@dataclass
class Message:
    role: str
    content: str
    created_at: str = ''
    attachments: list = field(default_factory=list)


@dataclass
class Conversation:
    title: str
    source: str
    source_url: str = ''
    created_at: str = ''
    messages: list = field(default_factory=list)
    warnings: list = field(default_factory=list)


def _string(value, limit=2000000):
    if value is None:
        return ''
    if isinstance(value, str):
        return value[:limit]
    if isinstance(value, (int, float)):
        if value > 100000000:
            try:
                return datetime.datetime.fromtimestamp(value).isoformat(sep=' ', timespec='seconds')
            except (ValueError, OSError, OverflowError):
                pass
        return str(value)
    return ''


def _decode(data):
    if isinstance(data, str):
        return data
    if not isinstance(data, bytes):
        raise ChatImportError('invalid_input', '导入内容格式无效')
    for encoding in ('utf-8-sig', 'utf-8', 'utf-16', 'gb18030'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            continue
    return data.decode('utf-8', errors='replace')


def _role(value):
    value = _string(value).lower().strip()
    if value in ('user', 'human', 'prompt', 'you', '用户'):
        return 'user'
    if value in ('assistant', 'model', 'bot', 'ai', 'claude', 'chatgpt', 'gemini', '助手'):
        return 'assistant'
    return ''


def _content(value):
    """Flatten tolerant platform content blocks without exposing metadata."""
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, list):
        bits = [_content(item) for item in value]
        return '\n\n'.join(bit for bit in bits if bit).strip()
    if not isinstance(value, dict):
        return ''
    for key in ('text', 'content', 'parts', 'result', 'markdown', 'value'):
        if key in value:
            text = _content(value[key])
            if text:
                return text
    return ''


def _attachment(value):
    if not isinstance(value, dict):
        return []
    items = value.get('attachments') or value.get('files') or []
    if not isinstance(items, list):
        items = [items]
    result = []
    for item in items:
        if isinstance(item, str):
            result.append({'name': os.path.basename(item)[:200]})
        elif isinstance(item, dict):
            name = _string(item.get('name') or item.get('filename') or item.get('file_name'), 200)
            if name:
                result.append({'name': name})
    return result[:30]


def _message(item):
    if not isinstance(item, dict):
        return None
    author = item.get('author')
    author_role = author.get('role') if isinstance(author, dict) else author
    role = _role(item.get('role') or item.get('author_role') or author_role)
    if not role:
        return None  # system/tool/developer content is never exported
    content = _content(item.get('content') if 'content' in item else
                       item.get('text') if 'text' in item else
                       item.get('message') if 'message' in item else item.get('parts'))
    if not content:
        return None
    return Message(role, content, _string(item.get('created_at') or item.get('create_time') or item.get('timestamp')),
                   _attachment(item))


def _generic(data, source='通用 JSON', source_url=''):
    root = data if isinstance(data, dict) else {}
    entries = (root.get('messages') or root.get('chat_messages') or root.get('turns') or
               root.get('items') or (data if isinstance(data, list) else []))
    if not isinstance(entries, list):
        entries = []
    messages = [_message(item) for item in entries]
    messages = [item for item in messages if item]
    return Conversation(_string(root.get('title') or root.get('name') or root.get('conversation_title') or '导入的对话', 300),
                        source, source_url, _string(root.get('created_at') or root.get('create_time') or root.get('timestamp')),
                        messages)


def _chatgpt(data, source_url=''):
    item = data[0] if isinstance(data, list) and data else data
    if not isinstance(item, dict):
        return None
    mapping = item.get('mapping')
    if not isinstance(mapping, dict):
        return None
    messages = []
    for node in mapping.values():
        message = node.get('message') if isinstance(node, dict) else None
        parsed = _message(message)
        if parsed:
            messages.append(parsed)
    return Conversation(_string(item.get('title') or 'ChatGPT 对话', 300), 'ChatGPT', source_url,
                        _string(item.get('create_time') or item.get('created_at')), messages)


def parse_json(data, source_url=''):
    chatgpt = _chatgpt(data, source_url)
    if chatgpt and chatgpt.messages:
        return chatgpt
    root = data[0] if isinstance(data, list) and data and isinstance(data[0], dict) else data
    if isinstance(root, dict) and isinstance(root.get('sessions'), list):
        result = Conversation('ReadMD 历史对话', 'ReadMD 历史', source_url)
        for session in root['sessions']:
            trial = _generic(session, 'ReadMD 历史', source_url)
            if trial.messages:
                result = trial
                result.source = 'ReadMD 历史'
                break
    elif isinstance(root, dict) and ('chat_messages' in root or root.get('provider') == 'claude'):
        result = _generic(root, 'Claude', source_url)
    elif isinstance(root, dict) and ('chunkedPrompt' in root or root.get('model') == 'gemini'):
        result = _generic(root, 'Gemini', source_url)
    else:
        result = _generic(root if isinstance(root, dict) else data, '通用 JSON', source_url)
    if not result.messages and isinstance(data, list):
        # ChatGPT exports may be an array of conversations.  Import the first
        # usable record; ZIP input handles each entry separately.
        for candidate in data:
            trial = _chatgpt(candidate, source_url) or _generic(candidate, '通用 JSON', source_url)
            if trial.messages:
                result = trial
                break
    return result


class _ChatHTML(HTMLParser):
    def __init__(self):
        HTMLParser.__init__(self, convert_charrefs=True)
        self.title, self._title_depth = '', 0
        self.messages, self._current = [], None
        self._skip, self._pre, self._table = 0, 0, []
        self._links = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag in ('script', 'style', 'iframe', 'object', 'embed', 'form'):
            self._skip += 1
            return
        if self._skip:
            return
        if tag == 'title': self._title_depth += 1
        role = _role(attrs.get('data-message-author-role') or attrs.get('data-role') or attrs.get('role'))
        klass = (attrs.get('class') or '').lower()
        if not role and ('assistant' in klass or 'model' in klass): role = 'assistant'
        if not role and ('user' in klass or 'human' in klass): role = 'user'
        if role:
            if self._current and self._current.content.strip(): self.messages.append(self._current)
            self._current = Message(role, '')
        if tag == 'pre': self._pre += 1; self._append('\n```\n')
        elif tag == 'code' and not self._pre: self._append('`')
        elif tag == 'br': self._append('\n')
        elif tag in ('p', 'div', 'section', 'li', 'tr', 'h1', 'h2', 'h3'): self._append('\n')
        elif tag == 'a' and attrs.get('href') and not attrs['href'].lower().startswith('javascript:'):
            self._links.append(attrs['href']); self._append('[')
        elif tag in ('td', 'th'): self._append('| ')

    def handle_endtag(self, tag):
        if tag in ('script', 'style', 'iframe', 'object', 'embed', 'form'):
            self._skip = max(0, self._skip - 1); return
        if self._skip: return
        if tag == 'title': self._title_depth = max(0, self._title_depth - 1)
        elif tag == 'pre': self._append('\n```\n'); self._pre = max(0, self._pre - 1)
        elif tag == 'code' and not self._pre: self._append('`')
        elif tag == 'a' and self._links: self._append('](%s)' % self._links.pop())
        elif tag in ('td', 'th'): self._append(' ')
        elif tag in ('p', 'div', 'section', 'li', 'tr', 'h1', 'h2', 'h3'): self._append('\n')

    def handle_data(self, data):
        if self._skip: return
        if self._title_depth: self.title += data
        self._append(data)

    def _append(self, text):
        if self._current is not None: self._current.content += text

    def finish(self):
        if self._current and self._current.content.strip(): self.messages.append(self._current)


def parse_html(value, source_url=''):
    parser = _ChatHTML()
    parser.feed(value)
    parser.close()
    parser.finish()
    return Conversation((parser.title.strip() or '导入的网页对话')[:300], 'HTML 对话', source_url,
                        messages=parser.messages)


def _clean_markdown(text):
    # Stored exports sometimes contain executable HTML.  Keep the textual
    # conversation and harmless Markdown/HTML while dropping active payloads.
    text = re.sub(r'(?is)<(script|style|iframe|object|embed|form)\b.*?</\1\s*>', '', text)
    text = re.sub(r'\s+on[a-z]+\s*=\s*(?:"[^"]*"|\'[^\']*\'|[^\s>]+)', '', text, flags=re.I)
    text = re.sub(r'(?i)(href|src)\s*=\s*(["\'])\s*javascript:[^"\']*\2', r'\1=\2#\2', text)
    return text.strip()


def to_markdown(conversation):
    title = re.sub(r'[\r\n#]+', ' ', conversation.title or '导入的对话').strip()
    lines = ['# ' + title, '', '> 来源：' + (conversation.source or '未知')]
    if conversation.source_url: lines.append('> 原始地址：' + conversation.source_url)
    lines.append('> 转换时间：' + datetime.datetime.now().astimezone().strftime('%Y-%m-%d %H:%M'))
    if conversation.created_at: lines.append('> 对话创建时间：' + conversation.created_at)
    for message in conversation.messages:
        lines.extend(['', '## ' + ('用户' if message.role == 'user' else 'AI 助手'), '',
                      _clean_markdown(message.content)])
        for attachment in message.attachments:
            if attachment.get('name'): lines.append('\n附件：' + attachment['name'])
    return '\n'.join(lines).strip() + '\n'


def _parse_one(data, filename='', source_url=''):
    suffix = os.path.splitext(filename.lower())[1]
    if suffix in ('.html', '.htm'):
        if len(data) > MAX_TEXT_BYTES: raise ChatImportError('too_large', 'HTML 内容超过 10 MB 限制')
        return parse_html(_decode(data), source_url)
    if suffix in ('.md', '.txt'):
        if len(data) > MAX_TEXT_BYTES: raise ChatImportError('too_large', '文本内容超过 10 MB 限制')
        return Conversation(os.path.splitext(os.path.basename(filename))[0] or '导入的文本', '纯文本 / Markdown', source_url,
                            messages=[Message('user', _clean_markdown(_decode(data)))])
    try:
        return parse_json(json.loads(_decode(data)), source_url)
    except json.JSONDecodeError:
        raise ChatImportError('invalid_json', 'JSON 对话导出文件无法解析')


def _safe_zip_members(raw):
    if len(raw) > MAX_ZIP_BYTES: raise ChatImportError('zip_too_large', '压缩包超过 100 MB 限制')
    try: archive = zipfile.ZipFile(BytesIO(raw))
    except (zipfile.BadZipFile, OSError): raise ChatImportError('invalid_zip', '压缩包无效或已损坏')
    chosen, total, warnings = [], 0, []
    for info in archive.infolist():
        path = PurePosixPath(info.filename.replace('\\', '/'))
        if path.is_absolute() or '..' in path.parts or ':' in path.parts[0:1]:
            archive.close(); raise ChatImportError('unsafe_zip_path', '压缩包包含不安全的文件路径')
        if info.is_dir() or os.path.splitext(info.filename.lower())[1] not in KNOWN_EXTENSIONS:
            continue
        total += info.file_size
        if total > MAX_ZIP_EXPANDED:
            archive.close(); raise ChatImportError('zip_expanded_too_large', '压缩包展开后超过 200 MB 限制')
        if info.file_size > 1024 * 1024 and info.compress_size and info.file_size / info.compress_size > MAX_ZIP_RATIO:
            archive.close(); raise ChatImportError('suspicious_zip', '压缩包压缩比异常，已拒绝导入')
        chosen.append(info)
    return archive, chosen, warnings


def import_bytes(data, filename='', source_url=''):
    """Parse one export. ZIP members are processed in memory, never extracted."""
    raw = data.encode('utf-8') if isinstance(data, str) else data
    if not isinstance(raw, bytes): raise ChatImportError('invalid_input', '导入内容格式无效')
    suffix = os.path.splitext(filename.lower())[1]
    if suffix == '.zip' or raw[:4] == b'PK\x03\x04':
        archive, members, warnings = _safe_zip_members(raw)
        try:
            successes = []
            for info in members:
                try:
                    with archive.open(info) as item:
                        content = item.read(MAX_TEXT_BYTES + 1 if os.path.splitext(info.filename.lower())[1] in ('.html', '.htm', '.md', '.txt') else info.file_size + 1)
                    successes.append(_parse_one(content, info.filename, source_url))
                except ChatImportError as exc:
                    warnings.append('%s：%s' % (os.path.basename(info.filename), exc.message))
                except Exception:
                    warnings.append('%s：无法解析，已跳过' % os.path.basename(info.filename))
            if not successes: raise ChatImportError('no_supported_chat', '压缩包中没有可导入的对话')
            result = successes[0]
            if len(successes) > 1: warnings.append('压缩包包含多个对话，当前返回第一个可解析对话')
            result.warnings.extend(warnings)
            return result
        finally: archive.close()
    if suffix and suffix not in KNOWN_EXTENSIONS:
        raise ChatImportError('unsupported_type', '仅支持 JSON、HTML、ZIP、Markdown 或文本对话导出')
    return _parse_one(raw, filename, source_url)


def import_file(path):
    path = os.path.abspath(os.fspath(path))
    if not os.path.isfile(path): raise ChatImportError('not_found', '导入文件不存在或已被移动')
    if os.path.splitext(path.lower())[1] not in KNOWN_EXTENSIONS + ('.zip',):
        raise ChatImportError('unsupported_type', '仅支持 JSON、HTML、ZIP、Markdown 或文本对话导出')
    size = os.path.getsize(path)
    if size > MAX_ZIP_BYTES: raise ChatImportError('too_large', '导入文件超过大小限制')
    with open(path, 'rb') as handle: return import_bytes(handle.read(), path)


def result(conversation):
    if not conversation.messages:
        raise ChatImportError('no_conversation', '没有识别到用户与 AI 的对话；请确认页面已加载完整对话且未停留在登录页')
    return {'ok': True, 'content': to_markdown(conversation), 'title': conversation.title,
            'source': conversation.source, 'source_url': conversation.source_url,
            'warnings': conversation.warnings, 'message_count': len(conversation.messages)}
