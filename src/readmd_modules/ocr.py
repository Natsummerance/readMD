# -*- coding: utf-8 -*-
"""扫描 / 图片转 md：跨平台 OCR。

Windows: WinRT OCR（离线、免费、无次数限制）。
macOS:   Vision 框架 VNRecognizeTextRequest（原生、离线、高质量）。
其他:    Tesseract OCR（需安装 tesseract 命令行）。

图片 → OCR 文本；PDF 先尝试提取文字层，无文字（扫描件）则逐页渲染后 OCR。
"""

import asyncio
import logging
import os
import subprocess
import sys

IS_MAC = sys.platform == 'darwin'
IS_WIN = sys.platform == 'win32'

_engine_cache = {}


# ---------------------------------------------------------------- Windows WinRT OCR

def _winrt_pick_language():
    from winrt.windows.media.ocr import OcrEngine
    tags = [l.language_tag for l in OcrEngine.available_recognizer_languages]
    if not tags:
        raise RuntimeError('系统未安装任何 OCR 语言')
    for cand in ('zh-Hans', 'zh-CN', 'zh', 'en-US', 'en'):
        for t in tags:
            if t.lower().startswith(cand.lower()):
                return t
    return tags[0]


def _winrt_ocr_bytes(data, lang_tag):
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


# ---------------------------------------------------------------- macOS Vision OCR

def _mac_vision_ocr_bytes(data):
    """macOS Vision 框架 OCR：直接接受图片 bytes，返回识别文本。"""
    try:
        import objc
        from Foundation import NSData
        from Quartz import CGImageSourceCreateWithData, CGImageSourceCreateImageAtIndex
        import Vision

        ns_data = NSData.dataWithBytes_length_(data, len(data))
        src = CGImageSourceCreateWithData(ns_data, None)
        if src is None:
            return ''
        cg_img = CGImageSourceCreateImageAtIndex(src, 0, None)
        if cg_img is None:
            return ''

        result_text = []

        def handler(request, error):
            if error:
                logging.warning('Vision OCR error: %s', error)
                return
            observations = request.results()
            for obs in observations:
                text = obs.topCandidates_(1)[0].string()
                if text:
                    result_text.append(str(text))

        request = Vision.VNRecognizeTextRequest.alloc().initWithCompletionHandler_(handler)
        request.setRecognitionLanguages_(['zh-Hans', 'zh-CN', 'en-US', 'en'])
        request.setRecognitionLevel_(1)  # accurate

        handler_obj = Vision.VNImageRequestHandler.alloc().initWithCGImage_options_(cg_img, None)
        success = handler_obj.performRequests_error_([request], None)
        if not success:
            logging.warning('Vision performRequests failed')
            return ''

        return '\n'.join(result_text)
    except ImportError:
        raise RuntimeError('macOS Vision OCR 需要 PyObjC：pip install pyobjc-framework-Vision pyobjc-framework-Quartz')
    except Exception as e:
        logging.exception('macOS Vision OCR failed')
        raise


# ---------------------------------------------------------------- Tesseract 兜底（Linux / 无原生 OCR）

def _tesseract_ocr_bytes(data):
    """Tesseract OCR 兜底：需要系统安装 tesseract 命令行。"""
    import tempfile
    with tempfile.NamedTemporaryFile(suffix='.png', delete=False) as f:
        f.write(data)
        tmp = f.name
    try:
        result = subprocess.run(
            ['tesseract', tmp, 'stdout', '-l', 'chi_sim+eng'],
            capture_output=True, timeout=30)
        return result.stdout.decode('utf-8', errors='replace').strip()
    except FileNotFoundError:
        raise RuntimeError('Tesseract 未安装。请运行：brew install tesseract tesseract-lang（macOS）或 apt install tesseract-ocr（Linux）')
    except subprocess.TimeoutExpired:
        return ''
    finally:
        try:
            os.unlink(tmp)
        except Exception:
            pass


# ---------------------------------------------------------------- 统一入口

def _pick_engine():
    """选择当前平台的 OCR 引擎（结果缓存）。"""
    cached = _engine_cache.get('_engine')
    if cached is not None:
        return cached
    engine = None
    if IS_WIN:
        try:
            from winrt.windows.media.ocr import OcrEngine  # noqa: F401
            engine = 'winrt'
        except ImportError:
            pass
    if engine is None and IS_MAC:
        try:
            import Vision  # noqa: F401
            engine = 'mac_vision'
        except ImportError:
            pass
    if engine is None:
        # Tesseract 兜底
        try:
            subprocess.run(['tesseract', '--version'], capture_output=True, timeout=5)
            engine = 'tesseract'
        except Exception:
            pass
    _engine_cache['_engine'] = engine  # None 也缓存，避免重复检测
    return engine


def _ocr_bytes(data):
    """根据平台选择 OCR 引擎执行识别。"""
    engine = _pick_engine()
    if engine == 'winrt':
        lang = _engine_cache.setdefault('lang', _winrt_pick_language())
        return _winrt_ocr_bytes(data, lang)
    elif engine == 'mac_vision':
        return _mac_vision_ocr_bytes(data)
    elif engine == 'tesseract':
        return _tesseract_ocr_bytes(data)
    else:
        raise RuntimeError('无可用 OCR 引擎。Windows 需要 WinRT，macOS 需要 PyObjC，其他平台需要 Tesseract。')


def load():
    """提前验证 OCR 引擎可用。"""
    engine = _pick_engine()
    if engine is None:
        raise RuntimeError('无可用 OCR 引擎')
    if engine == 'winrt':
        _engine_cache['lang'] = _winrt_pick_language()
    return True


def ocr_image(path, dpi=None):
    """识别单张图片，返回识别文本。"""
    with open(path, 'rb') as f:
        data = f.read()
    text = _ocr_bytes(data).strip()
    return text


def ocr_image_to_md(path):
    """图片 → Markdown（附原图引用，避免信息丢失）。"""
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
    parts = []
    for idx, page in enumerate(pages, 1):
        text = (page.get_text() or '').strip()
        if not text:
            try:
                pix = page.get_pixmap(dpi=200)
                png = pix.tobytes('png')
                text = _ocr_bytes(png).strip()
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
    ext = os.path.splitext(path)[1].lower()
    if ext == '.pdf':
        return ocr_pdf_to_md(path)
    return ocr_image_to_md(path)
