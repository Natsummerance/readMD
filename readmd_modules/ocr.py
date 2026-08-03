# -*- coding: utf-8 -*-
"""扫描 / 图片转 md：使用 Windows 10/11 内置 OCR（离线、免费、无次数限制）。

图片 → OCR 文本；PDF 先尝试提取文字层，无文字（扫描件）则逐页渲染后 OCR。
""" 

import asyncio
import logging

_engine_cache = {}


def load():
    # 提前验证 WinRT OCR 可用
    _pick_language()
    return True


def _pick_language():
    from winrt.windows.media.ocr import OcrEngine
    tags = [l.language_tag for l in OcrEngine.available_recognizer_languages]
    if not tags:
        raise RuntimeError('系统未安装任何 OCR 语言')
    for cand in ('zh-Hans', 'zh-CN', 'zh', 'en-US', 'en'):
        for t in tags:
            if t.lower().startswith(cand.lower()):
                return t
    return tags[0]


def _ocr_bytes(data, lang_tag):
    from winrt.windows.globalization import Language
    from winrt.windows.graphics.imaging import BitmapDecoder
    from winrt.windows.media.ocr import OcrEngine
    from winrt.windows.storage.streams import DataWriter, InMemoryRandomAccessStream

    async def run():
        stream = InMemoryRandomAccessStream()
        writer = DataWriter(stream)
        writer.write_bytes(data)
        await writer.store_async()
        stream.seek(0)
        decoder = await BitmapDecoder.create_async(stream)
        bitmap = await decoder.get_software_bitmap_async()
        engine = OcrEngine.try_create_from_language(Language(lang_tag))
        if engine is None:
            engine = OcrEngine.try_create_from_user_profile_languages()
        if engine is None:
            return ''
        result = await engine.recognize_async(bitmap)
        return result.text

    return asyncio.run(run())


def _lang_tag():
    key = 'lang'
    if key not in _engine_cache:
        _engine_cache[key] = _pick_language()
    return _engine_cache[key]


def ocr_image(path, dpi=None):
    """识别单张图片，返回 (识别文本, 是否空)。"""
    with open(path, 'rb') as f:
        data = f.read()
    text = _ocr_bytes(data, _lang_tag()).strip()
    return text


def ocr_image_to_md(path):
    """图片 → Markdown（附原图引用，避免信息丢失）。"""
    import os
    text = ocr_image(path)
    body = []
    if text:
        body.append(text)
    else:
        body.append('> （未识别出文字，仅保留原图）')
    md = '![原图](%s)\n\n%s' % (path, '\n\n'.join(body))
    return md


def ocr_pdf_to_md(path, max_pages=200):
    """PDF → Markdown：有文字层直接提取，否则逐页 OCR。"""
    import fitz
    doc = fitz.open(path)
    pages = list(doc)[:max_pages]
    lang = _lang_tag()
    parts = []
    for idx, page in enumerate(pages, 1):
        text = (page.get_text() or '').strip()
        if not text:
            try:
                pix = page.get_pixmap(dpi=200)
                png = pix.tobytes('png')
                text = _ocr_bytes(png, lang).strip()
            except Exception as e:  # noqa: BLE001
                logging.exception('page %d ocr failed', idx)
                text = ''
        if not text:
            continue
        parts.append('## 第 %d 页\n\n%s' % (idx, text))
    doc.close()
    if not parts:
        return '> （PDF 未提取到文字，且 OCR 无结果）'
    return '\n\n---\n\n'.join(parts)


def ocr_any(path):
    """按扩展名分发：图片 → OCR；PDF → 文字层/OCR；其他交给 convert。"""
    import os
    ext = os.path.splitext(path)[1].lower()
    if ext == '.pdf':
        return ocr_pdf_to_md(path)
    return ocr_image_to_md(path)