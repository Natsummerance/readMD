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


# ---------------------------------------------------------------- RapidOCR 极速高精引擎（ONNX 离线）

_rapidocr_engine = None


def _xy_cut_lines(items):
    """递归 XY-Cut 空间投影切分算法直接生成行字符串列表。

    投影优先顺序：
    1. 垂直空白投影（X-Cut）：切分多栏（Columns）。
       左栏与右栏分别递归生成行列表并拼接，彻底杜绝跨栏行误合并。
    2. 水平空白投影（Y-Cut）：在列块内部切分段落和行块（Rows）。
    3. 叶子块内部：按 y_top 行聚类，行内按 x_left 从左到右排序拼接为字符串。
    """
    if not items:
        return []
    if len(items) == 1:
        text = items[0].get('text', '').strip()
        return [text] if text else []

    avg_h = sum(it.get('height', 15.0) for it in items) / len(items)

    # 1. 优先尝试 X-Cut (垂直空白投影切割，学术/报纸多栏切分)
    sorted_by_x = sorted(items, key=lambda it: it['x_left'])
    x_cut_idx = -1
    max_x_gap = 0.0
    col_gap_threshold = max(25.0, avg_h * 1.8)

    curr_right = sorted_by_x[0]['x_left'] + sorted_by_x[0].get('width', 20.0)
    for i in range(len(sorted_by_x) - 1):
        it = sorted_by_x[i]
        curr_right = max(curr_right, it['x_left'] + it.get('width', 20.0))
        next_left = sorted_by_x[i + 1]['x_left']
        gap = next_left - curr_right
        if gap >= col_gap_threshold and gap > max_x_gap:
            max_x_gap = gap
            x_cut_idx = i + 1

    if x_cut_idx > 0:
        left_part = sorted_by_x[:x_cut_idx]
        right_part = sorted_by_x[x_cut_idx:]
        return _xy_cut_lines(left_part) + _xy_cut_lines(right_part)

    # 2. 尝试 Y-Cut (水平空白投影切割，上下大块/段落切分)
    sorted_by_y = sorted(items, key=lambda it: it['y_top'])
    y_cut_idx = -1
    max_y_gap = 0.0
    y_threshold = max(8.0, avg_h * 0.8)

    curr_bottom = sorted_by_y[0]['y_top'] + sorted_by_y[0].get('height', 15.0)
    for i in range(len(sorted_by_y) - 1):
        it = sorted_by_y[i]
        curr_bottom = max(curr_bottom, it['y_top'] + it.get('height', 15.0))
        next_top = sorted_by_y[i + 1]['y_top']
        gap = next_top - curr_bottom
        if gap >= y_threshold and gap > max_y_gap:
            max_y_gap = gap
            y_cut_idx = i + 1

    if y_cut_idx > 0:
        top_part = sorted_by_y[:y_cut_idx]
        bottom_part = sorted_by_y[y_cut_idx:]
        return _xy_cut_lines(top_part) + _xy_cut_lines(bottom_part)

    # 3. 基础叶子块（单栏/单段内）：行优先聚类，行内自左向右拼接
    line_thresh = max(6.0, avg_h * 0.5)
    rows = []
    for it in sorted(items, key=lambda x: x['y_top']):
        matched = None
        for r in rows:
            if abs(it['y_top'] - r['y_ref']) <= line_thresh:
                matched = r
                break
        if matched is not None:
            matched['items'].append(it)
            matched['y_ref'] = sum(x['y_top'] for x in matched['items']) / len(matched['items'])
        else:
            rows.append({'y_ref': it['y_top'], 'items': [it]})

    lines = []
    for r in sorted(rows, key=lambda row: row['y_ref']):
        sorted_row = sorted(r['items'], key=lambda it: it['x_left'])
        line_str = ' '.join(x['text'] for x in sorted_row if x.get('text'))
        if line_str:
            lines.append(line_str)
    return lines


def _sort_rapidocr_boxes(result):
    """对 RapidOCR 识别框使用递归 XY-Cut 按人类自然阅读顺序排版。"""
    if not result:
        return []
    items = []
    for item in result:
        if not item or len(item) < 2 or not item[1]:
            continue
        box = item[0]
        try:
            xs = [float(p[0]) for p in box]
            ys = [float(p[1]) for p in box]
            x_left = min(xs)
            x_right = max(xs)
            y_top = min(ys)
            y_bottom = max(ys)
            width = max(x_right - x_left, 1.0)
            height = max(y_bottom - y_top, 5.0)
        except Exception:
            x_left, y_top, width, height = 0.0, 0.0, 50.0, 20.0
        items.append({
            'text': str(item[1]).strip(),
            'y_top': y_top,
            'x_left': x_left,
            'width': width,
            'height': height,
        })
    if not items:
        return []
    return _xy_cut_lines(items)


def _get_rapidocr_engine():
    global _rapidocr_engine
    if _rapidocr_engine is None:
        from . import plugin_manager as pm
        pm.mount_sandbox()
        from rapidocr_onnxruntime import RapidOCR
        _rapidocr_engine = RapidOCR()
    return _rapidocr_engine


def _ocr_rapidocr(image_path_or_bytes):
    """使用 RapidOCR 极速 ONNX 离线引擎识别文本与多列排版。"""
    try:
        from . import plugin_manager as pm
        if not pm.is_plugin_enabled('rapidocr'):
            return None
        engine = _get_rapidocr_engine()
        result, _ = engine(image_path_or_bytes)
        if not result:
            return ''
        lines = _sort_rapidocr_boxes(result)
        return '\n'.join(lines).strip()
    except Exception as exc:
        logging.warning('RapidOCR recognition skipped/failed: %s', exc)
        return None


def _rapidocr_bytes(data):
    """供统一入口 _ocr_bytes 调用的 RapidOCR 分支。"""
    res = _ocr_rapidocr(data)
    if res is None:
        raise RuntimeError('ocr-no-engine：RapidOCR 引擎未安装或初始化失败')
    return res


def _matrix_to_md_table(rows: list) -> str:
    """将二维单元格矩阵格式化为标准 Markdown 表格字符串。"""
    if not rows:
        return ''
    max_cols = max(len(r) for r in rows)
    if max_cols == 0:
        return ''
    normalized = [r + [''] * (max_cols - len(r)) for r in rows]
    header = '| ' + ' | '.join(normalized[0]) + ' |'
    separator = '| ' + ' | '.join(['---'] * max_cols) + ' |'
    body = ['| ' + ' | '.join(r) + ' |' for r in normalized[1:]]
    return '\n'.join([header, separator] + body)


def _html_table_to_md(html_str: str) -> str:
    """将 HTML 表格字符串转换为标准 Markdown 表格。"""
    if not html_str or '<table' not in html_str.lower():
        return ''
    import re
    rows = re.findall(r'<tr[^>]*>(.*?)</tr>', html_str, re.IGNORECASE | re.DOTALL)
    if not rows:
        return ''
    md_rows = []
    for row in rows:
        cells = re.findall(r'<t[dh][^>]*>(.*?)</t[dh]>', row, re.IGNORECASE | re.DOTALL)
        clean_cells = [re.sub(r'<[^>]+>', '', c).strip().replace('|', '\\|') for c in cells]
        if clean_cells:
            md_rows.append(clean_cells)
    return _matrix_to_md_table(md_rows)


def extract_table_to_md(image_path_or_bytes) -> str:
    """从图片中提取表格并转换为原生 Markdown 表格格式。
    优先调用已挂载的 rapid_table 插件；若未启用则利用 OCR 文本对齐启发式构建表格。
    """
    # 1. 优先尝试 rapid_table 专用模型插件
    try:
        from . import plugin_manager as pm
        if pm.is_plugin_enabled('rapid_table'):
            pm.mount_sandbox()
            from rapid_table import RapidTable
            table_engine = RapidTable()
            table_html, _ = table_engine(image_path_or_bytes)
            md = _html_table_to_md(table_html)
            if md:
                return md
    except Exception as exc:
        logging.debug('rapid_table extraction failed or skipped: %s', exc)

    # 2. 兜底尝试从 OCR 结果中的行列特征构建 Markdown 表格
    try:
        if isinstance(image_path_or_bytes, str):
            with open(image_path_or_bytes, 'rb') as f:
                data = f.read()
        else:
            data = image_path_or_bytes
        raw_text = _ocr_cascade(data)
        if not raw_text:
            return ''
        lines = [l.strip() for l in raw_text.split('\n') if l.strip()]
        table_rows = []
        for l in lines:
            if '\t' in l:
                cols = [c.strip() for c in l.split('\t') if c.strip()]
            elif '  ' in l:
                import re
                cols = [c.strip() for c in re.split(r'\s{2,}', l) if c.strip()]
            else:
                cols = [l]
            if len(cols) >= 2:
                table_rows.append(cols)
        if len(table_rows) >= 2:
            return _matrix_to_md_table(table_rows)
    except Exception:
        pass

    return ''


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
        # 优先使用 RapidOCR（免系统外部依赖，毫秒级 ONNX）
        try:
            from . import plugin_manager as pm
            if pm.is_plugin_enabled('rapidocr'):
                engine = 'rapidocr'
        except Exception:
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
    elif engine == 'rapidocr':
        return _rapidocr_bytes(data)
    elif engine == 'tesseract':
        return _tesseract_ocr_bytes(data)
    else:
        # 兜底探测 RapidOCR
        rapid_text = _ocr_rapidocr(data)
        if rapid_text is not None:
            return rapid_text
        raise RuntimeError('ocr-no-engine：无可用 OCR 引擎。Windows 需要 WinRT，macOS 需要 PyObjC，其他平台需要 RapidOCR 插件或 Tesseract。')


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


_easyocr_reader = None


def _ocr_easyocr(image_path_or_bytes):
    """使用 EasyOCR 插件深度识别手写体、复杂倾斜或低对比度图片。"""
    try:
        from . import plugin_manager as pm
        if not pm.is_plugin_enabled('easyocr'):
            return None
        pm.mount_sandbox()
        import easyocr
        global _easyocr_reader
        if _easyocr_reader is None:
            _easyocr_reader = easyocr.Reader(['ch_sim', 'en'], gpu=True)
        results = _easyocr_reader.readtext(image_path_or_bytes)
        lines = [text for _, text, conf in results if conf > 0.3]
        return '\n'.join(lines).strip()
    except Exception as exc:
        logging.warning('EasyOCR recognition skipped/failed: %s', exc)
        return None


def _ocr_cascade(data, fallback_path=None):
    """OCR 识别渐进式级联回退：
    Tier 0: 原生引擎（WinRT / macOS Vision / Tesseract）
    Tier 1: RapidOCR 极速 ONNX 离线引擎
    Tier 2: EasyOCR 深度识别插件
    """
    # A user-selected optional engine takes precedence over native OCR.
    # Each helper checks current enablement, including after an engine switch.
    from . import plugin_manager as pm
    if pm.is_plugin_enabled('rapidocr'):
        selected = _ocr_rapidocr(data)
        if selected:
            return selected.strip()
    elif pm.is_plugin_enabled('easyocr'):
        selected = _ocr_easyocr(fallback_path if fallback_path else data)
        if selected:
            return selected.strip()
    text = ''
    try:
        text = _ocr_bytes(data).strip()
    except Exception as exc:
        logging.debug('Primary OCR attempt failed: %s', exc)
        text = ''

    if not text:
        try:
            rapid_text = _ocr_rapidocr(data)
            if rapid_text:
                text = rapid_text.strip()
        except Exception as exc:
            logging.debug('RapidOCR cascade failed: %s', exc)

    if not text:
        try:
            easy_target = fallback_path if fallback_path else data
            easy_text = _ocr_easyocr(easy_target)
            if easy_text:
                text = easy_text.strip()
        except Exception as exc:
            logging.debug('EasyOCR cascade failed: %s', exc)

    return text


def ocr_image(path, dpi=None):
    """识别单张图片，返回识别文本。通过 _ocr_cascade 自动按序尝试原生、RapidOCR 与 EasyOCR 深度识别兜底。"""
    with open(path, 'rb') as f:
        data = f.read()
    return _ocr_cascade(data, fallback_path=path)


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
    """PDF → Markdown：有文字层直接提取，否则逐页执行完整级联 OCR，并统一执行智能排版规范化。"""
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
                text = _ocr_cascade(png).strip()
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

