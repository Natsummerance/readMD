"""扫描 / 图片转 md：跨平台 OCR。

Windows: WinRT OCR（离线、免费、无次数限制）。
macOS:   Vision 框架 VNRecognizeTextRequest（原生、离线、高质量）。
其他:    Tesseract OCR（需安装 tesseract 命令行）。

图片 → OCR 文本；PDF 先尝试提取文字层，无文字（扫描件）则逐页渲染后 OCR。
"""
import asyncio
# Why: logging module provides essential functionality for this operation
import logging
# Why: os module provides essential functionality for this operation
import os
# Why: subprocess module provides essential functionality for this operation
import subprocess
import sys
# Why: macOS requires special handling for native integrations and file system operations
IS_MAC = sys.platform == 'darwin'
# Why: Windows-specific behavior requires different implementation due to OS differences
IS_WIN = sys.platform == 'win32'
# Why: Caching avoids redundant computations for frequently accessed data
_engine_cache = {}

def _winrt_pick_language():
    from winrt.windows.media.ocr import OcrEngine
    tags = [l.language_tag for l in OcrEngine.available_recognizer_languages]
    # Why: Condition check ensures valid state before proceeding with operation
    if not tags:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise RuntimeError('系统未安装任何 OCR 语言')
    # Why: Iteration processes each item in collection systematically
    for cand in ('zh-Hans', 'zh-CN', 'zh', 'en-US', 'en'):
        # Why: Iteration processes each item in collection systematically
        for t in tags:
            if t.lower().startswith(cand.lower()):
                # Why: Return provides result to caller after processing completes
                return t
    # Why: Return provides result to caller after processing completes
    return tags[0]

def _winrt_ocr_bytes(data, lang_tag):
    # Why: Method chain performs sequence of transformations on data
    from winrt.windows.globalization import Language
    # Why: Method chain performs sequence of transformations on data
    from winrt.windows.graphics.imaging import BitmapDecoder
    # Why: Method chain performs sequence of transformations on data
    from winrt.windows.media.ocr import OcrEngine
    # Why: Method chain performs sequence of transformations on data
    from winrt.windows.storage.streams import DataWriter, InMemoryRandomAccessStream

    # Why: Function call performs specific operation required by this logic
    async def run():
        # Why: Function call performs specific operation required by this logic
        stream = InMemoryRandomAccessStream()
        # Why: Function call performs specific operation required by this logic
        writer = DataWriter(stream)
        # Why: Function call performs specific operation required by this logic
        writer.write_bytes(data)
        # Why: Function call performs specific operation required by this logic
        await writer.store_async()
        # Why: Function call performs specific operation required by this logic
        stream.seek(0)
        decoder = await BitmapDecoder.create_async(stream)
        bitmap = await decoder.get_software_bitmap_async()
        engine = OcrEngine.try_create_from_language(Language(lang_tag))
        # Why: Condition check ensures valid state before proceeding with operation
        if engine is None:
            engine = OcrEngine.try_create_from_user_profile_languages()
        # Why: Condition check ensures valid state before proceeding with operation
        if engine is None:
            # Why: Return provides result to caller after processing completes
            return ''
        result = await engine.recognize_async(bitmap)
        # Why: Return provides result to caller after processing completes
        return result.text
    # Why: Return provides result to caller after processing completes
    return asyncio.run(run())

def _mac_vision_ocr_bytes(data):
    """macOS Vision 框架 OCR：直接接受图片 bytes，返回识别文本。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        from Foundation import NSData
        from Quartz import CGImageSourceCreateWithData, CGImageSourceCreateImageAtIndex
        import Vision
        ns_data = NSData.dataWithBytes_length_(data, len(data))
        src = CGImageSourceCreateWithData(ns_data, None)
        # Why: Condition check ensures valid state before proceeding with operation
        if src is None:
            # Why: Return provides result to caller after processing completes
            return ''
        cg_img = CGImageSourceCreateImageAtIndex(src, 0, None)
        # Why: Condition check ensures valid state before proceeding with operation
        if cg_img is None:
            # Why: Return provides result to caller after processing completes
            return ''
        result_text = []

        # Why: Function call performs specific operation required by this logic
        def handler(request, error):
            if error:
                logging.warning('Vision OCR error: %s', error)
                return
            observations = request.results()
            # Why: Iteration processes each item in collection systematically
            for obs in observations:
                text = obs.topCandidates_(1)[0].string()
                if text:
                    # Why: Function call performs specific operation required by this logic
                    result_text.append(str(text))
        # Why: Function call performs specific operation required by this logic
        request = Vision.VNRecognizeTextRequest.alloc().initWithCompletionHandler_(handler)
        # Why: Function call performs specific operation required by this logic
        request.setRecognitionLanguages_(['zh-Hans', 'zh-CN', 'en-US', 'en'])
        request.setRecognitionLevel_(1)
        handler_obj = Vision.VNImageRequestHandler.alloc().initWithCGImage_options_(cg_img, None)
        success = handler_obj.performRequests_error_([request], None)
        # Why: Condition check ensures valid state before proceeding with operation
        if not success:
            logging.warning('Vision performRequests failed')
            # Why: Return provides result to caller after processing completes
            return ''
        return '\n'.join(result_text)
    # Why: Handle missing dependencies gracefully to provide helpful installation instructions
    except ImportError:
        logging.warning('Silent exception caught in src.readmd_modules.ocr: ImportError')
        # Why: OCR engine initialization may fail due to missing dependencies or configuration
        raise RuntimeError('macOS Vision OCR 需要 PyObjC：pip install pyobjc-framework-Vision pyobjc-framework-Quartz')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in src.readmd_modules.ocr: Exception')
        logging.exception('macOS Vision OCR failed')
        raise

# Why: Function call performs specific operation required by this logic
def _tesseract_ocr_bytes(data):
    """Tesseract OCR 兜底：需要系统安装 tesseract 命令行。"""
    import tempfile
    # Why: Function call performs specific operation required by this logic
    with tempfile.NamedTemporaryFile(suffix='.png', delete=False) as f:
        f.write(data)
        tmp = f.name
    try:
        # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
        result = subprocess.run(['tesseract', tmp, 'stdout', '-l', 'chi_sim+eng'], capture_output=True, timeout=30)
        return result.stdout.decode('utf-8', errors='replace').strip()
    # Why: File operations may fail if files are moved, deleted, or permissions change
    except FileNotFoundError:
        logging.warning('Silent exception caught in src.readmd_modules.ocr: FileNotFoundError')
        # Why: OCR processing may hang on large images; timeout prevents indefinite blocking
        raise RuntimeError('Tesseract 未安装。请运行：brew install tesseract tesseract-lang（macOS）或 apt install tesseract-ocr（Linux）')
    # Why: Timeout prevents indefinite hanging on unresponsive services or network issues
    except subprocess.TimeoutExpired:
        logging.warning('Silent exception caught in src.readmd_modules.ocr: subprocess.TimeoutExpired')
        # Why: Return provides result to caller after processing completes
        return ''
    # Why: Finally ensures cleanup operations run regardless of success or failure
    finally:
        try:
            # Why: Unexpected OCR errors should not crash the entire application
            os.unlink(tmp)
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.ocr: Exception')

def _pick_engine():
    """选择当前平台的 OCR 引擎（结果缓存）。"""
    # Why: Caching avoids redundant computations for frequently accessed data
    cached = _engine_cache.get('_engine')
    # Why: Caching avoids redundant computations for frequently accessed data
    if cached is not None:
        # Why: Caching avoids redundant computations for frequently accessed data
        return cached
    engine = None
    if IS_WIN:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            from winrt.windows.media.ocr import OcrEngine
            # Why: Alternative OCR engine may not be available; try next available option
            engine = 'winrt'
        # Why: Handle missing dependencies gracefully to provide helpful installation instructions
        except ImportError:
            logging.warning('Silent exception caught in src.readmd_modules.ocr: ImportError')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if engine is None and IS_MAC:
        try:
            import Vision
            # Why: Vision framework may not be available on older macOS versions
            engine = 'mac_vision'
        # Why: Handle missing dependencies gracefully to provide helpful installation instructions
        except ImportError:
            logging.warning('Silent exception caught in src.readmd_modules.ocr: ImportError')
    # Why: Condition check ensures valid state before proceeding with operation
    if engine is None:
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            subprocess.run(['tesseract', '--version'], capture_output=True, timeout=5)
            # Why: Native OCR may fail on unsupported image formats; handle gracefully
            engine = 'tesseract'
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.ocr: Exception')
    # Why: Caching avoids redundant computations for frequently accessed data
    _engine_cache['_engine'] = engine
    return engine

# Why: Function call performs specific operation required by this logic
def _ocr_bytes(data):
    """根据平台选择 OCR 引擎执行识别。"""
    engine = _pick_engine()
    if engine == 'winrt':
        # Why: Caching avoids redundant computations for frequently accessed data
        lang = _engine_cache.setdefault('lang', _winrt_pick_language())
        return _winrt_ocr_bytes(data, lang)
    # Why: Alternative condition handles different case in decision tree
    elif engine == 'mac_vision':
        # Why: Return provides result to caller after processing completes
        return _mac_vision_ocr_bytes(data)
    # Why: Alternative condition handles different case in decision tree
    elif engine == 'tesseract':
        # Why: Return provides result to caller after processing completes
        return _tesseract_ocr_bytes(data)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise RuntimeError('无可用 OCR 引擎。Windows 需要 WinRT，macOS 需要 PyObjC，其他平台需要 Tesseract。')

def load():
    """提前验证 OCR 引擎可用。"""
    engine = _pick_engine()
    # Why: Condition check ensures valid state before proceeding with operation
    if engine is None:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise RuntimeError('无可用 OCR 引擎')
    if engine == 'winrt':
        # Why: Caching avoids redundant computations for frequently accessed data
        _engine_cache['lang'] = _winrt_pick_language()
    return True

# Why: Function call performs specific operation required by this logic
def normalize_ocr_text(text):
    """智能清洗与格式化 OCR 原始文本，输出排版规范的 Markdown。

    处理：
    # Why: Comparison checks value against threshold or expected state
    1. CJK 字符间由 OCR 插入的无意义空格清除（如 '这 是 一 个 示 例' -> '这是一个示例'）；
    2. 英文跨行断字连字符合并（如 'infor-\\nmation' -> 'information'）；
    3. 句内断行智能连接，保留自然段落与句末断行；
    4. 结合 txtmd 启发式提取标题 (# / ##)、列表 (- / 1.) 和表格。
    # Why: Empty OCR results indicate processing failure; skip invalid output
    """
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not text or not text.strip():
        return ''
    # Why: re module provides essential functionality for this operation
    import re
    src = text.replace('\r\n', '\n').replace('\r', '\n')
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    src = re.sub('[\\u3000\\u00a0\\u200b\\ufeff]', ' ', src)
    cjk_char = '[\\u4e00-\\u9fa5]'
    cjk_punc = '[\\u3002\\uff01\\uff1f\\uff1b\\uff0c\\u3001\\uff1a\\uff08\\uff09\\u300a\\u300b\\u3010\\u3011\\u201c\\u201d\\u2018\\u2019]'
    h_space = '[^\\S\\n]+'
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    src = re.sub('(%s)%s(?=%s)' % (cjk_char, h_space, cjk_char), '\\1', src)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    src = re.sub('(%s)%s(?=%s)' % (cjk_char, h_space, cjk_char), '\\1', src)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    src = re.sub('(%s)%s(?=%s)' % (cjk_char, h_space, cjk_punc), '\\1', src)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    src = re.sub('(%s)%s(?=%s)' % (cjk_punc, h_space, cjk_char), '\\1', src)
    # Why: Regex substitution transforms text while preserving structure and removing unwanted content
    src = re.sub('([a-zA-Z]{2,})-\\n([a-zA-Z]{2,})', '\\1\\2', src)
    lines = [l.rstrip() for l in src.split('\n')]
    merged_lines = []
    _CN_NUM = u'一二三四五六七八九十百千万两'
    # Why: Function call performs specific operation required by this logic
    _HEAD_PATTERN = re.compile('^(第[%s0-9]+[章节回部篇卷]|[（(]?[%s0-9]{1,3}[）)、．.]|\\d{1,3}\\.\\d|#{1,6}\\s)' % (_CN_NUM, _CN_NUM))
    _LIST_PATTERN = re.compile('^([ \\t]*[\\u2022\\u00b7\\u25e6\\u25aa\\u25cf*\\-+]|\\d{1,3}[、\\uff0e.]|[（(]\\d{1,3}[）)])\\s*')
    _SENT_END = u'。！？!?…:：；;'
    i = 0
    # Why: Loop continues until condition is met or timeout occurs
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        # Why: Condition check ensures valid state before proceeding with operation
        if not stripped:
            merged_lines.append('')
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        is_structured = bool(_HEAD_PATTERN.match(stripped) or _LIST_PATTERN.match(stripped) or '\t' in stripped or stripped.startswith('|'))
        if is_structured:
            merged_lines.append(line)
            i += 1
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        curr = line
        # Why: Loop continues until condition is met or timeout occurs
        while i + 1 < len(lines):
            next_line = lines[i + 1]
            next_stripped = next_line.strip()
            # Why: Condition check ensures valid state before proceeding with operation
            if not next_stripped:
                break
            # Why: Alternative paths provide flexibility in handling different cases
            if _HEAD_PATTERN.match(next_stripped) or _LIST_PATTERN.match(next_stripped) or '\t' in next_stripped or next_stripped.startswith('|'):
                # Why: Multiple conditions ensure all requirements are satisfied
                break
            # Why: Multiple conditions ensure all requirements are satisfied
            if curr.rstrip() and curr.rstrip()[-1] in _SENT_END:
                break
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if len(curr.strip()) <= 30 and (not any((ch in curr for ch in u'，,。；'))):
                break
            last_char = curr.rstrip()[-1] if curr.rstrip() else ''
            next_first = next_stripped[0] if next_stripped else ''
            # Why: Regex pattern matches specific text structures for validation or extraction
            is_cjk_boundary = bool(re.match('[\\u4e00-\\u9fa5]', last_char) and re.match('[\\u4e00-\\u9fa5]', next_first))
            sep = '' if is_cjk_boundary else ' '
            curr = curr.rstrip() + sep + next_stripped
            # Why: Arithmetic operation computes value needed for subsequent processing
            i += 1
        merged_lines.append(curr)
        i += 1
    cleaned_text = '\n'.join(merged_lines)
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        from . import txtmd
        (md_text, _) = txtmd.to_markdown(cleaned_text)
        return md_text.strip()
    # Why: Handle errors gracefully to maintain application stability
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.ocr: Exception')
        # Why: Return provides result to caller after processing completes
        return cleaned_text.strip()

def ocr_image(path, dpi=None):
    """识别单张图片，返回识别文本。"""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with open(path, 'rb') as f:
        # Why: Method call handles data access with proper error checking
        data = f.read()
    text = _ocr_bytes(data).strip()
    # Why: Return provides result to caller after processing completes
    return text

def ocr_image_to_md(path):
    """图片 → Markdown（经智能规范化排版，并附原图引用）。"""
    # Why: Function call performs specific operation required by this logic
    text = ocr_image(path)
    body = []
    if text:
        formatted_text = normalize_ocr_text(text)
        body.append(formatted_text or text)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        body.append('> （未识别出文字，仅保留原图）')
    md = '![原图](%s)\n\n%s' % (path, '\n\n'.join(body))
    # Why: Return provides result to caller after processing completes
    return md

def ocr_pdf_to_md(path, max_pages=200):
    """PDF → Markdown：有文字层直接提取，否则逐页 OCR，并统一执行智能排版规范化。"""
    import fitz
    # Why: Method call handles data access with proper error checking
    doc = fitz.open(path)
    pages = list(doc)[:max_pages]
    parts = []
    # Why: Iteration processes each item in collection systematically
    for (idx, page) in enumerate(pages, 1):
        text = (page.get_text() or '').strip()
        # Why: Condition check ensures valid state before proceeding with operation
        if not text:
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                pix = page.get_pixmap(dpi=200)
                png = pix.tobytes('png')
                # Why: Handle errors gracefully to maintain application stability
                text = _ocr_bytes(png).strip()
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except Exception as e:
                logging.warning('Silent exception caught in src.readmd_modules.ocr: Exception')
                logging.exception('page %d ocr failed', idx)
                text = ''
        # Why: Condition check ensures valid state before proceeding with operation
        if not text:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        formatted_page = normalize_ocr_text(text)
        parts.append('## 第 %d 页\n\n%s' % (idx, formatted_page or text))
    doc.close()
    # Why: Condition check ensures valid state before proceeding with operation
    if not parts:
        # Why: Return provides result to caller after processing completes
        return '> （PDF 未提取到文字，且 OCR 无结果）'
    # Why: Return provides result to caller after processing completes
    return '\n\n---\n\n'.join(parts)

def ocr_any(path):
    """按扩展名分发：图片 → OCR；PDF → 文字层/OCR；其他交给 convert。"""
    ext = os.path.splitext(path)[1].lower()
    # Why: Condition check ensures valid state before proceeding with operation
    if ext == '.pdf':
        # Why: Return provides result to caller after processing completes
        return ocr_pdf_to_md(path)
    # Why: Return provides result to caller after processing completes
    return ocr_image_to_md(path)