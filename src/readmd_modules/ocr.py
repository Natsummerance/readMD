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

OCR_PDF_EMPTY_PLACEHOLDER = '> （PDF 未提取到文字，且 OCR 无结果）'

_OCR_IMAGE_EXTS = ('.png', '.jpg', '.jpeg', '.bmp', '.gif', '.tif', '.tiff', '.webp')


# ---------------------------------------------------------------- Windows WinRT OCR

def _winrt_pick_language():
    from winrt.windows.media.ocr import OcrEngine
    tags = [l.language_tag for l in OcrEngine.available_recognizer_languages]
    if not tags:
        raise RuntimeError('ocr-no-engine：系统未安装任何 OCR 语言')
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
        raise RuntimeError('ocr-no-engine：macOS Vision OCR 需要 PyObjC：pip install pyobjc-framework-Vision pyobjc-framework-Quartz')
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
        raise RuntimeError('ocr-no-engine：Tesseract 未安装。请运行：brew install tesseract tesseract-lang（macOS）或 apt install tesseract-ocr（Linux）')
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
        raise RuntimeError('ocr-no-engine：无可用 OCR 引擎。Windows 需要 WinRT，macOS 需要 PyObjC，其他平台需要 Tesseract。')


def load():
    """提前验证 OCR 引擎可用。"""
    engine = _pick_engine()
    if engine is None:
        raise RuntimeError('ocr-no-engine：无可用 OCR 引擎')
    if engine == 'winrt':
        _engine_cache['lang'] = _winrt_pick_language()
    return True


def normalize_ocr_text(text):
    """智能清洗与格式化 OCR 原始文本，输出排版规范的 Markdown。

    处理：
    1. CJK 字符间由 OCR 插入的无意义空格清除（如 '这 是 一 个 示 例' -> '这是一个示例'）；
    2. 英文跨行断字连字符合并（如 'infor-\\nmation' -> 'information'）；
    3. 句内断行智能连接，保留自然段落与句末断行；
    4. 结合 txtmd 启发式提取标题 (# / ##)、列表 (- / 1.) 和表格。
    """
    if not text or not text.strip():
        return ''

    import re

    # 1. 规范化换行与特殊空格
    src = text.replace('\r\n', '\n').replace('\r', '\n')
    src = re.sub(r'[\u3000\u00a0\u200b\ufeff]', ' ', src)

    # 2. CJK 字符与标点间同行多余空格剔除（不能跨行吃掉换行符）
    cjk_char = r'[\u4e00-\u9fa5]'
    cjk_punc = r'[\u3002\uff01\uff1f\uff1b\uff0c\u3001\uff1a\uff08\uff09\u300a\u300b\u3010\u3011\u201c\u201d\u2018\u2019]'
    h_space = r'[^\S\n]+'
    src = re.sub(r'(%s)%s(?=%s)' % (cjk_char, h_space, cjk_char), r'\1', src)
    src = re.sub(r'(%s)%s(?=%s)' % (cjk_char, h_space, cjk_char), r'\1', src)
    src = re.sub(r'(%s)%s(?=%s)' % (cjk_char, h_space, cjk_punc), r'\1', src)
    src = re.sub(r'(%s)%s(?=%s)' % (cjk_punc, h_space, cjk_char), r'\1', src)


    # 3. 英文跨行连字符修复 (如 'auto-\ncomplete' -> 'autocomplete')
    src = re.sub(r'([a-zA-Z]{2,})-\n([a-zA-Z]{2,})', r'\1\2', src)

    # 4. 智能合并单句被 OCR 硬换行切断的行
    lines = [l.rstrip() for l in src.split('\n')]
    merged_lines = []

    _CN_NUM = u'\u4e00\u4e8c\u4e09\u56db\u4e94\u516d\u4e03\u516b\u4e5d\u5341\u767e\u5343\u4e07\u4e24'
    _HEAD_PATTERN = re.compile(r'^(第[%s0-9]+[章节回部篇卷]|[（(]?[%s0-9]{1,3}[）)、．.]|\d{1,3}\.\d|#{1,6}\s)' % (_CN_NUM, _CN_NUM))
    _LIST_PATTERN = re.compile(r'^([ \t]*[\u2022\u00b7\u25e6\u25aa\u25cf*\-+]|\d{1,3}[、\uff0e.]|[（(]\d{1,3}[）)])\s*')
    _SENT_END = u'。！？!?…:：；;'

    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if not stripped:
            merged_lines.append('')
            i += 1
            continue

        is_structured = bool(_HEAD_PATTERN.match(stripped) or _LIST_PATTERN.match(stripped) or '\t' in stripped or stripped.startswith('|'))
        if is_structured:
            merged_lines.append(line)
            i += 1
            continue

        curr = line
        while i + 1 < len(lines):
            next_line = lines[i + 1]
            next_stripped = next_line.strip()
            if not next_stripped:
                break
            if _HEAD_PATTERN.match(next_stripped) or _LIST_PATTERN.match(next_stripped) or '\t' in next_stripped or next_stripped.startswith('|'):
                break
            if curr.rstrip() and curr.rstrip()[-1] in _SENT_END:
                break
            if len(curr.strip()) <= 30 and not any(ch in curr for ch in u'，,。；'):
                break

            last_char = curr.rstrip()[-1] if curr.rstrip() else ''
            next_first = next_stripped[0] if next_stripped else ''
            is_cjk_boundary = bool(re.match(r'[\u4e00-\u9fa5]', last_char) and re.match(r'[\u4e00-\u9fa5]', next_first))
            sep = '' if is_cjk_boundary else ' '

            curr = curr.rstrip() + sep + next_stripped
            i += 1

        merged_lines.append(curr)
        i += 1


    cleaned_text = '\n'.join(merged_lines)

    # 5. 调用 txtmd 模块进行 Markdown 结构化整理
    try:
        from . import txtmd
        md_text, _ = txtmd.to_markdown(cleaned_text)
        return md_text.strip()
    except Exception:
        return cleaned_text.strip()


def ocr_image(path, dpi=None):
    """识别单张图片，返回识别文本。"""
    with open(path, 'rb') as f:
        data = f.read()
    text = _ocr_bytes(data).strip()
    return text


def ocr_image_to_md(path):
    """图片 → Markdown（经智能规范化排版，并附原图引用）。"""
    text = ocr_image(path)
    body = []
    if text:
        formatted_text = normalize_ocr_text(text)
        body.append(formatted_text or text)
    else:
        body.append('> （未识别出文字，仅保留原图）')
    md = '![原图](%s)\n\n%s' % (path, '\n\n'.join(body))
    return md


def ocr_pdf_to_md(path, max_pages=200):
    """PDF → Markdown：有文字层直接提取，否则逐页 OCR，并统一执行智能排版规范化。"""
    import fitz
    doc = fitz.open(path)
    pages = list(doc)[:max_pages]
    try:
        total = int(doc.page_count)
    except Exception:
        total = len(pages)
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
        formatted_page = normalize_ocr_text(text)
        parts.append('## 第 %d 页\n\n%s' % (idx, formatted_page or text))
    doc.close()
    if not parts:
        return OCR_PDF_EMPTY_PLACEHOLDER
    if total > len(pages):
        parts.append('> （注意：文档共 %d 页，本次仅处理前 %d 页，其余未转换）' % (total, len(pages)))
    return '\n\n---\n\n'.join(parts)


def ocr_any(path):
    """按扩展名分发：PDF → 文字层/OCR；已知图片 → OCR；其他类型拒绝。"""
    ext = os.path.splitext(path)[1].lower()
    if ext == '.pdf':
        return ocr_pdf_to_md(path)
    if ext in _OCR_IMAGE_EXTS:
        return ocr_image_to_md(path)
    raise ValueError('ocr-unsupported-type：%s 不是可识别的图片或 PDF 文件' % (ext or '未知类型'))

