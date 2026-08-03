# -*- coding: utf-8 -*-
"""万物转 md：基于 MarkItDown 的本地文件转换器（懒加载）。"""

_engine = None


def load():
    global _engine
    if _engine is None:
        from markitdown import MarkItDown
        _engine = MarkItDown()
    return _engine


def convert(path):
    """把任意支持的文件转换为 Markdown 文本。"""
    eng = load()
    result = eng.convert(path)
    text = (result.text_content or '').strip()
    return text


def supported_hint():
    return ('支持：PDF / Word / PowerPoint / Excel / HTML / CSV / JSON / XML / '
            '邮件 / 压缩包等；图片与扫描件请用 OCR 模块')