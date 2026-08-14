# -*- coding: utf-8 -*-
"""ReadMD 导出模块（md -> PDF / DOCX / HTML），全部重依赖惰性加载。

设计：不进入 readmd_modules.MODULES 自动加载列表，只在用户发起导出时
由 Api.export_doc 显式 import；渲染器内部再按需 import reportlab /
python-docx / matplotlib，保证启动与空闲内存不受影响。
"""

import os
import shutil
import sys
import tempfile

from . import styles as _styles
from . import parser as _parser

APP_DIR = (sys._MEIPASS if getattr(sys, 'frozen', False)
           else os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
ASSETS_DIR = os.path.join(APP_DIR, 'assets')

EXTS = {'pdf': '.pdf', 'docx': '.docx', 'html': '.html'}


def load():
    """轻量加载（无重依赖）。"""
    return True


class ImageResolver(object):
    """把 markdown 里的图片 src 解析为本地绝对路径；缺失返回 None。"""

    def __init__(self, base_dir, warns):
        self.base_dir = base_dir or ''
        self.warns = warns
        self._cache = {}

    def resolve(self, src):
        src = (src or '').strip()
        if not src:
            return None
        if src.startswith(('http://', 'https://'), ) or src.startswith('data:'):
            self.warns.append('远程/内联图片不支持嵌入，已跳过：%s' % src[:80])
            return None
        if src in self._cache:
            return self._cache[src]
        out = None
        cand = src
        if not os.path.isabs(cand):
            cand = os.path.join(self.base_dir or '', cand)
        cand = os.path.normpath(cand)
        if os.path.isfile(cand):
            out = cand
        else:
            self.warns.append('图片不存在，已跳过：%s' % src)
        self._cache[src] = out
        return out


def export(fmt, content, base_dir, out_path, options=None, source_name=''):
    """执行导出，返回 {ok, path, size, warns}。"""
    fmt = (fmt or '').lower()
    if fmt not in EXTS:
        return {'ok': False, 'error': '不支持的导出格式：%s' % fmt}
    style = _styles.sanitize(options)
    warns = []
    blocks = _parser.parse(content)
    tmpdir = tempfile.mkdtemp(prefix='readmd-export-')
    try:
        # 公式预渲染（PDF/DOCX 用图；HTML 走 MathJax，不在此渲染）
        if fmt in ('pdf', 'docx'):
            from . import formula
            formula.prepare(blocks, style, warns)
        resolve = ImageResolver(base_dir, warns).resolve
        if fmt == 'pdf':
            from . import pdf_render
            pdf_render.render(blocks, out_path, style, tmpdir, resolve, warns)
        elif fmt == 'docx':
            from . import docx_render
            docx_render.render(blocks, out_path, style, tmpdir, resolve, warns)
        elif fmt == 'html':
            from . import html_render
            html_render.render(content, out_path, style, source_name, ASSETS_DIR, warns)
        size = os.path.getsize(out_path) if os.path.exists(out_path) else 0
        return {'ok': True, 'path': out_path, 'size': size, 'warns': warns}
    except Exception as e:
        import logging
        logging.exception('export %s failed', fmt)
        return {'ok': False, 'error': str(e), 'warns': warns}
    finally:
        try:
            shutil.rmtree(tmpdir, ignore_errors=True)
        except Exception:
            pass
