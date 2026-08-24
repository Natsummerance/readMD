# -*- coding: utf-8 -*-
"""Shared rules for serving ReadMD's offline UI assets."""

import mimetypes
import os
from dataclasses import dataclass
from typing import Optional
from urllib.parse import parse_qs

from src.readmd_modules.validators import paths_within


STARTUP_SCRIPTS = (
    'vendor/marked.min.js',
    'vendor/qrcode.min.js',
    'js/core/state.js',
    'js/core/i18n.js',
    'js/core/dialog.js',
    'js/core/settings.js',
    'js/core/modules.js',
    'js/core/tabs.js',
    'js/core/history.js',
    'js/core/dragdrop.js',
    'js/reader/formula.js',
    'js/reader/fixes.js',
    'js/reader/toc.js',
    'js/reader/search.js',
    'js/reader/folder.js',
    'js/reader/render.js',
    'js/editor/preview.js',
    'js/editor/image.js',
    'js/editor/editor.js',
    'js/features/ai.js',
    'js/features/share.js',
    'js/features/convert.js',
    'js/features/ocr.js',
    'js/features/web.js',
    'js/features/clipboard.js',
    'js/features/export.js',
    'js/features/updater.js',
    'app.js',
)

_startup_bundles = {}


@dataclass(frozen=True)
class ResolvedAsset:
    """A safe asset response; ``path`` is None when there is no file to read."""

    path: Optional[str]
    mime: str
    immutable: bool
    body: Optional[bytes] = None
    forbidden: bool = False


def build_startup_bundle(app_dir):
    """Combine the ordered classic scripts into one cold-start request."""
    root = os.path.abspath(app_dir)
    body = _startup_bundles.get(root)
    if body is None:
        chunks = []
        for relative_path in STARTUP_SCRIPTS:
            script_path = os.path.join(root, 'assets', *relative_path.split('/'))
            with open(script_path, 'rb') as handle:
                chunks.append(handle.read())
        body = b'\n;\n'.join(chunks)
        _startup_bundles[root] = body
    return body


def asset_mime(path):
    mime = mimetypes.guess_type(path)[0] or 'application/octet-stream'
    if mime.startswith('text/') or mime in ('application/javascript', 'application/json'):
        return f'{mime}; charset=utf-8'
    return mime


def resolve_asset(app_dir, path, query=''):
    """Resolve /assets and /i18n URLs without path-traversal ambiguity."""
    root = os.path.abspath(app_dir)
    if path == '/assets/readmd.boot.js':
        return ResolvedAsset(
            path=None,
            mime='application/javascript; charset=utf-8',
            immutable=True,
            body=build_startup_bundle(root),
        )
    if not (path.startswith('/assets/') or path.startswith('/i18n/')):
        return None

    rel = path[len('/assets/'):] if path.startswith('/assets/') else path.lstrip('/')
    full_path = os.path.normpath(os.path.join(root, 'assets', rel))
    base = os.path.normpath(os.path.join(root, 'assets'))
    if not paths_within(full_path, base):
        return ResolvedAsset(path=None, mime='text/plain', immutable=False, forbidden=True)

    parsed_query = parse_qs(query) if isinstance(query, str) else (query or {})
    cached_prefix = rel.startswith('vendor/') or rel.startswith('i18n/')
    immutable = bool(
        cached_prefix or parsed_query.get('v') or parsed_query.get('version') or parsed_query.get('hash')
    )
    return ResolvedAsset(path=full_path, mime=asset_mime(full_path), immutable=immutable)
