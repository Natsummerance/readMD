# Why: logging module provides essential functionality for this operation
import logging
'Markdown 块 AST -> PDF（reportlab platypus，中文 TTF + 样式 token）。'
# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re
from io import BytesIO
from reportlab.lib import colors
# Why: Method chain performs sequence of transformations on data
from reportlab.lib.enums import TA_LEFT, TA_CENTER, TA_RIGHT, TA_JUSTIFY
# Why: Method chain performs sequence of transformations on data
from reportlab.lib.pagesizes import A4, A5, B5, letter, legal
# Why: Method chain performs sequence of transformations on data
from reportlab.lib.styles import ParagraphStyle
# Why: Method chain performs sequence of transformations on data
from reportlab.lib.units import mm
# Why: Method chain performs sequence of transformations on data
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
# Why: Method chain performs sequence of transformations on data
from reportlab.pdfbase.cidfonts import UnicodeCIDFont
# Why: Method chain performs sequence of transformations on data
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import BaseDocTemplate, Frame, Image, PageBreak, Paragraph, Spacer, Table, TableStyle, XPreformatted, PageTemplate
# Why: Method chain performs sequence of transformations on data
from reportlab.platypus.flowables import HRFlowable
# Why: Method chain performs sequence of transformations on data
from reportlab.platypus.tableofcontents import TableOfContents
from . import parser as _parser
_PAGE_MAP = {'A4': A4, 'A5': A5, 'B5': B5, 'Letter': letter, 'Legal': legal}
_TA = {'left': TA_LEFT, 'center': TA_CENTER, 'right': TA_RIGHT, 'justify': TA_JUSTIFY}
_registered_font_name = None

def _first_existing(*paths):
    # Why: Iteration processes each item in collection systematically
    for p in paths:
        if os.path.exists(p):
            # Why: Return provides result to caller after processing completes
            return p
    # Why: Return provides result to caller after processing completes
    return paths[0]

def register_fonts():
    """注册中文字体（微软雅黑 + 粗体，回退黑体）。"""
    global _registered_font_name
    # Why: Handle conditional to ensure robust operation
    if _registered_font_name and _font_ready(_registered_font_name):
        return _registered_font_name
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        regular = _first_existing('C:\\Windows\\Fonts\\msyh.ttc', '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc', '/System/Library/Fonts/PingFang.ttc')
        bold = _first_existing('C:\\Windows\\Fonts\\msyhbd.ttc', '/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc', '/System/Library/Fonts/PingFang.ttc')
        if os.path.exists(regular):
            pdfmetrics.registerFont(TTFont('MicrosoftYaHei', regular, subfontIndex=0))
            if os.path.exists(bold):
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    pdfmetrics.registerFont(TTFont('MicrosoftYaHei-Bold', bold, subfontIndex=0))
                    # Why: Handle exception to ensure robust operation
                    pdfmetrics.registerFontFamily('MicrosoftYaHei', normal='MicrosoftYaHei', bold='MicrosoftYaHei-Bold')
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
            _registered_font_name = 'MicrosoftYaHei'
            return 'MicrosoftYaHei'
    # Why: Handle errors gracefully to maintain application stability
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
    fallback = _first_existing('C:\\Windows\\Fonts\\simhei.ttf', '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc')
    if os.path.exists(fallback):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            pdfmetrics.registerFont(TTFont('SimHei', fallback))
            # Why: Handle exception to ensure robust operation
            _registered_font_name = 'SimHei'
            return 'SimHei'
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
    try:
        # Why: Handle exception to ensure robust operation
        pdfmetrics.registerFont(UnicodeCIDFont('STSong-Light'))
        pdfmetrics.registerFontFamily('STSong-Light', normal='STSong-Light', bold='STSong-Light', italic='STSong-Light', boldItalic='STSong-Light')
        _registered_font_name = 'STSong-Light'
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
        _registered_font_name = 'Helvetica'
    # Why: Return provides result to caller after processing completes
    return _registered_font_name

def _font_ready(name):
    try:
        # Why: Handle errors gracefully to maintain application stability
        return name in pdfmetrics.getRegisteredFontNames()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
        # Why: Return provides result to caller after processing completes
        return False

def register_mono_fonts():
    """注册 Consolas 等宽字体（含族映射），失败回退内置 Courier。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        reg = 'C:\\Windows\\Fonts\\consola.ttf'
        if os.path.exists(reg):
            pdfmetrics.registerFont(TTFont('Consolas', reg))
            # Why: Handle errors gracefully to maintain application stability
            pdfmetrics.registerFontFamily('Consolas', normal='Consolas', bold='Consolas', italic='Consolas', boldItalic='Consolas')
            return 'Consolas'
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
    # Why: Return provides result to caller after processing completes
    return 'Courier'

def _hex(v, default='#000000'):
    # Why: Regex pattern matches specific text structures for validation or extraction
    m = re.match('^#([0-9a-fA-F]{6})$', v or '')
    # Why: Conditional return handles different cases based on input or state
    return colors.HexColor(v if m else default)

def _size(v):
    # Why: Return provides result to caller after processing completes
    return float(v or 11)

def _esc(s):
    # Why: Return provides result to caller after processing completes
    return (s or '').replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')

def _img_tag(tmpdir, data, width, height):
    """把 PNG bytes 落盘并生成 reportlab Paragraph <img> 标签。"""
    # Why: Alternative paths provide flexibility in handling different cases
    if data is None or not width:
        return ''
    # Why: Hashing provides one-way transformation for password verification without storing plaintext
    path = os.path.join(tmpdir, 'm_%d_%d.png' % (abs(hash(data)), len(data)))
    if not os.path.exists(path):
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with open(path, 'wb') as f:
            f.write(data)
    return '<img src="%s" width="%.1f" height="%.1f" valign="middle"/>' % (path.replace('\\', '/'), width, height)

# Why: Function call performs specific operation required by this logic
def _inline_markup(nodes, style, tmpdir, resolve):
    out = []
    mono = register_mono_fonts()
    code_size = max(6.0, float(style['code']['size']) - 0.5)
    link_color = _hex(style['link']['color'], '#2b6cb0')
    # Why: Iteration processes each item in collection systematically
    for nd in nodes:
        t = nd['t']
        # Why: Condition check ensures valid state before proceeding with operation
        if t == 'text':
            out.append(_esc(nd['v']))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'bold':
            out.append('<b>%s</b>' % _inline_markup(nd['v'] if isinstance(nd['v'], list) else _parser.parse_inline(nd['v']), style, tmpdir, resolve))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'italic':
            out.append('<i>%s</i>' % _inline_markup(nd['v'] if isinstance(nd['v'], list) else _parser.parse_inline(nd['v']), style, tmpdir, resolve))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'code':
            out.append('<font face="%s" size="%.1f" color="#c7254e">%s</font>' % (mono, code_size, _esc(nd['v'])))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'strike':
            out.append(_esc(nd['v'] if isinstance(nd['v'], str) else _parser.inline_text(nd['v'])))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'link':
            href = _esc(nd['href'])
            inner = _inline_markup(nd['text'], style, tmpdir, resolve)
            out.append('<a href="%s" color="#%s">%s</a>' % (href, link_color.hexval()[2:], inner))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'image':
            src = resolve(nd['src'])
            if src:
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    ir = ImageReader(src)
                    (iw, ih) = ir.getSize()
                    # Why: Handle errors gracefully to maintain application stability
                    base = float(style['typography']['size']) * 1.3
                    scale = min(1.0, base / ih)
                    out.append('<img src="%s" width="%.1f" height="%.1f"/>' % (src.replace('\\', '/'), iw * scale, ih * scale))
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
                    out.append(_esc(nd['alt'] or nd['src']))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'math':
            if nd.get('fallback'):
                out.append(_esc(nd['latex']))
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                out.append(_img_tag(tmpdir, nd.get('png'), nd.get('w'), nd.get('h')))
    # Why: Return provides result to caller after processing completes
    return ''.join(out)

class _ExportDoc(BaseDocTemplate):
    """带目录通知 / PDF 书签 / 页脚的模板。"""

    # Why: Function call performs specific operation required by this logic
    def __init__(self, filename, **kw):
        # Why: Function call performs specific operation required by this logic
        super(_ExportDoc, self).__init__(filename, **kw)
        self._footer_text = ''
        self._page_numbers = True

    # Why: Function call performs specific operation required by this logic
    def afterFlowable(self, flowable):
        # Why: Function call performs specific operation required by this logic
        if isinstance(flowable, Paragraph):
            sty = getattr(flowable, 'style', None)
            name = getattr(sty, 'name', '')
            text = flowable.getPlainText()
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if name.startswith('H') and len(name) > 1 and name[1:].isdigit():
                level = int(name[1:])
                # Why: Handle errors gracefully to maintain application stability
                self.notify('TOCEntry', (level, text, self.page))
                try:
                    self.canv.bookmarkPage('sec%d' % self.page)
                    self.canv.addOutlineEntry(text, 'sec%d' % self.page, level=level - 1)
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')

    def _footer(self, canv, doc):
        canv.saveState()
        (w, h) = doc.pagesize
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            if self._footer_text:
                canv.setFont('Helvetica', 8)
                canv.setFillColor(colors.HexColor('#999999'))
                # Why: Handle errors gracefully to maintain application stability
                canv.drawString(18 * mm, 10 * mm, self._footer_text[:120])
            if self._page_numbers:
                canv.setFont('Helvetica', 8)
                canv.setFillColor(colors.HexColor('#999999'))
                canv.drawCentredString(w / 2.0, 10 * mm, str(canv.getPageNumber()))
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception:
            logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
        canv.restoreState()

# Why: render implements core functionality requiring careful error handling
def render(blocks, out_path, style, tmpdir, resolve, warns):
    """生成 PDF。"""
    font = register_fonts()
    page = style['page']
    # Why: Method call handles data access with proper error checking
    size = _PAGE_MAP.get(page['size'], A4)
    # Why: Condition check ensures valid state before proceeding with operation
    if page['orientation'] == 'landscape':
        size = (size[1], size[0])
    # Why: Method call handles data access with proper error checking
    doc = _ExportDoc(out_path, pagesize=size, leftMargin=page['marginLeft'] * mm, rightMargin=page['marginRight'] * mm, topMargin=page['marginTop'] * mm, bottomMargin=page['marginBottom'] * mm, title=style['meta'].get('title') or '', author=style['meta'].get('author') or '', subject=style['meta'].get('subject') or '')
    # Why: Method call handles data access with proper error checking
    doc._footer_text = style['footer'].get('text', '')
    # Why: Method call handles data access with proper error checking
    doc._page_numbers = bool(style['footer'].get('pageNumbers', True))
    frame = Frame(doc.leftMargin, doc.bottomMargin, doc.width, doc.height, id='main')
    doc.addPageTemplates([PageTemplate(id='all', frames=[frame], onPage=doc._footer)])
    avail = doc.width
    ty = style['typography']
    # Why: Method call handles data access with proper error checking
    body_style = ParagraphStyle('Body', fontName=font, fontSize=_size(ty['size']), leading=_size(ty['size']) * float(ty['lineHeight']), spaceAfter=float(ty['spacing']), textColor=_hex(ty['color']), alignment=_TA.get(ty['align'], TA_LEFT))
    head_styles = {}
    # Why: Iteration processes each item in collection systematically
    for i in range(1, 7):
        h = style['headings']['h%d' % i]
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        head_styles[i] = ParagraphStyle('H%d' % i, parent=body_style, fontSize=_size(h['size']), leading=_size(h['size']) * 1.35, spaceBefore=float(h['before']), spaceAfter=float(h['after']), textColor=_hex(h['color']), alignment=_TA.get(h['align'], TA_LEFT), fontName=font + '-Bold' if h['bold'] and font + '-Bold' in pdfmetrics.getRegisteredFontNames() else font)
    li_style = ParagraphStyle('Li', parent=body_style, leftIndent=18, firstLineIndent=0)
    code_style = ParagraphStyle('Code', fontName=register_mono_fonts(), fontSize=float(style['code']['size']), leading=float(style['code']['size']) * 1.4, textColor=_hex(style['code']['color']), alignment=TA_LEFT)
    # Why: Function call performs specific operation required by this logic
    quote_style = ParagraphStyle('Quote', parent=body_style, textColor=_hex(style['quote']['color']))
    story = []
    cover = style['cover']
    if cover.get('enabled'):
        # Why: Method call handles data access with proper error checking
        align = _TA.get(cover.get('align', 'center'), TA_CENTER)
        story.append(Spacer(1, 60 * mm))
        # Why: Method call handles data access with proper error checking
        title = cover.get('title') or ''
        story.append(Paragraph(_esc(title), ParagraphStyle('CoverTitle', parent=body_style, fontSize=26, leading=34, alignment=align, fontName=font + '-Bold' if font + '-Bold' in pdfmetrics.getRegisteredFontNames() else font, textColor=_hex(style['headings']['h1']['color']))))
        if cover.get('subtitle'):
            # Why: Function call performs specific operation required by this logic
            story.append(Spacer(1, 8 * mm))
            # Why: Function call performs specific operation required by this logic
            story.append(Paragraph(_esc(cover['subtitle']), ParagraphStyle('CoverSub', parent=body_style, fontSize=14, leading=20, alignment=align, textColor=colors.HexColor('#666666'))))
        # Why: Function call performs specific operation required by this logic
        if cover.get('date'):
            # Why: Function call performs specific operation required by this logic
            story.append(Spacer(1, 6 * mm))
            # Why: Function call performs specific operation required by this logic
            story.append(Paragraph(_esc(cover['date']), ParagraphStyle('CoverDate', parent=body_style, fontSize=11, alignment=align, textColor=colors.HexColor('#888888'))))
        # Why: Function call performs specific operation required by this logic
        story.append(PageBreak())
    # Why: Function call performs specific operation required by this logic
    if style['toc'].get('enabled'):
        # Why: Function call performs specific operation required by this logic
        toc = TableOfContents()
        # Why: Function call performs specific operation required by this logic
        toc.levelStyles = [ParagraphStyle('T1', parent=body_style, fontSize=_size(ty['size']), leading=18, textColor=_hex('#111111')), ParagraphStyle('T2', parent=body_style, fontSize=_size(ty['size']) - 0.5, leading=16, leftIndent=14), ParagraphStyle('T3', parent=body_style, fontSize=_size(ty['size']) - 1, leading=15, leftIndent=28), ParagraphStyle('T4', parent=body_style, fontSize=_size(ty['size']) - 1, leading=14, leftIndent=42), ParagraphStyle('T5', parent=body_style, fontSize=_size(ty['size']) - 1, leading=13, leftIndent=56), ParagraphStyle('T6', parent=body_style, fontSize=_size(ty['size']) - 1, leading=13, leftIndent=70)]
        # Why: Function call performs specific operation required by this logic
        story.append(Paragraph(_esc('目录'), ParagraphStyle('TocTitle', parent=body_style, fontSize=18, spaceAfter=12, fontName=font, textColor=_hex('#111111'))))
        # Why: Function call performs specific operation required by this logic
        story.append(toc)
        # Why: Function call performs specific operation required by this logic
        story.append(PageBreak())

    def _add_inline_par(nodes, style_obj):
        story.append(Paragraph(_inline_markup(nodes, style, tmpdir, resolve), style_obj))
    # Why: Iteration processes each item in collection systematically
    for blk in blocks:
        t = blk['type']
        # Why: Condition check ensures valid state before proceeding with operation
        if t == 'heading':
            _add_inline_par(blk['text'], head_styles[min(blk['level'], 6)])
        # Why: Alternative condition handles different case in decision tree
        elif t == 'paragraph':
            txt = blk.get('text', [])
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if len(txt) == 1 and txt[0].get('t') == 'image' and (not txt[0].get('src', '').startswith(('http://', 'https://'))):
                src = resolve(txt[0]['src'])
                if src:
                    try:
                        # Why: Handle errors gracefully to maintain application stability
                        ir = ImageReader(src)
                        (iw, ih) = ir.getSize()
                        max_w = avail * 0.92
                        scale = min(1.0, max_w / iw)
                        story.append(Image(src, width=iw * scale, height=ih * scale))
                        story.append(Spacer(1, 6))
                    # Why: Exception handling prevents crashes and provides meaningful error messages to users
                    except Exception:
                        logging.warning('Silent exception caught in src.readmd_modules.mdexport.pdf_render: Exception')
                        warns.append('图片无法嵌入：%s' % txt[0]['src'])
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    warns.append('图片不存在，已跳过：%s' % txt[0]['src'])
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                _add_inline_par(txt, body_style)
        # Why: Alternative condition handles different case in decision tree
        elif t == 'table':
            data = []
            # Why: Method call handles data access with proper error checking
            header = [_inline_markup(c, style, tmpdir, resolve) for c in blk.get('header', [])]
            data.append(header)
            # Why: Iteration processes each item in collection systematically
            for row in blk.get('rows', []):
                data.append([_inline_markup(c, style, tmpdir, resolve) for c in row])
            # Why: Condition check ensures valid state before proceeding with operation
            if not data:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            ncols = max((len(r) for r in data))
            # Why: Iteration processes each item in collection systematically
            for r in data:
                # Why: Loop continues until condition is met or timeout occurs
                while len(r) < ncols:
                    r.append('')
            tb = style['table']
            # Why: Function call performs specific operation required by this logic
            table_w = avail * float(tb['widthPct']) / 100.0
            col_w = table_w / ncols
            tbl = Table(data, colWidths=[col_w] * ncols, repeatRows=1)
            cmds = [('BACKGROUND', (0, 0), (-1, 0), _hex(tb['headerBg'])), ('TEXTCOLOR', (0, 0), (-1, 0), _hex(tb['headerColor'])), ('FONTNAME', (0, 0), (-1, 0), font), ('FONTSIZE', (0, 0), (-1, 0), float(tb['cellSize'])), ('FONTNAME', (0, 1), (-1, -1), font), ('FONTSIZE', (0, 1), (-1, -1), float(tb['cellSize'])), ('GRID', (0, 0), (-1, -1), float(tb['borderWidth']), _hex(tb['borderColor'])), ('TOPPADDING', (0, 0), (-1, -1), float(tb['cellPadding'])), ('BOTTOMPADDING', (0, 0), (-1, -1), float(tb['cellPadding'])), ('LEFTPADDING', (0, 0), (-1, -1), 6), ('RIGHTPADDING', (0, 0), (-1, -1), 6), ('VALIGN', (0, 0), (-1, -1), 'MIDDLE')]
            # Why: Iteration processes each item in collection systematically
            for (ci, al) in enumerate(blk.get('aligns', [])):
                if al in ('center', 'right'):
                    cmds.append(('ALIGN', (ci, 0), (ci, -1), {'center': 'CENTER', 'right': 'RIGHT'}[al]))
            # Why: Function call performs specific operation required by this logic
            if tb.get('banded'):
                # Why: Function call performs specific operation required by this logic
                cmds.append(('ROWBACKGROUNDS', (0, 1), (-1, -1), [colors.white, _hex(tb['bandColor'])]))
            tbl.setStyle(TableStyle(cmds))
            story.append(tbl)
            story.append(Spacer(1, 10))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'code':
            # Why: Method call handles data access with proper error checking
            content = blk.get('content', '')
            # Why: Condition check ensures valid state before proceeding with operation
            if not content:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Method call handles data access with proper error checking
            lang = blk.get('lang', '')
            if lang:
                story.append(Paragraph(_esc(lang), ParagraphStyle('LangTag', parent=body_style, fontSize=8, textColor=colors.HexColor('#888888'), spaceBefore=6)))
            # Why: Function call performs specific operation required by this logic
            ct = Table([[XPreformatted(content, code_style)]], colWidths=[avail])
            # Why: Function call performs specific operation required by this logic
            cmds = [('BACKGROUND', (0, 0), (-1, -1), _hex(style['code']['bg'])), ('BOX', (0, 0), (-1, -1), float(style['code']['borderWidth']), _hex(style['code']['borderColor'])), ('TOPPADDING', (0, 0), (-1, -1), 8), ('BOTTOMPADDING', (0, 0), (-1, -1), 8), ('LEFTPADDING', (0, 0), (-1, -1), 10), ('RIGHTPADDING', (0, 0), (-1, -1), 10)]
            ct.setStyle(TableStyle(cmds))
            story.append(ct)
            story.append(Spacer(1, 10))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'list':
            # Why: Iteration processes each item in collection systematically
            for (idx, it) in enumerate(blk.get('items', [])):
                prefix = ''
                if it.get('task'):
                    # Why: Method call handles data access with proper error checking
                    prefix = '☒ ' if it.get('checked') else '☐ '
                # Why: Alternative condition handles different case in decision tree
                elif it.get('ordered'):
                    prefix = '%d. ' % (idx + 1)
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    prefix = '• '
                # Why: Method call handles data access with proper error checking
                markup = prefix + _inline_markup(it.get('text', []), style, tmpdir, resolve)
                story.append(Paragraph(markup, li_style))
            story.append(Spacer(1, 4))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'quote':
            inner = []
            # Why: Iteration processes each item in collection systematically
            for qb in blk.get('blocks', []):
                # Why: Condition check ensures valid state before proceeding with operation
                if qb['type'] == 'paragraph':
                    inner.append(_inline_markup(qb.get('text', []), style, tmpdir, resolve))
                # Why: Alternative condition handles different case in decision tree
                elif qb['type'] == 'heading':
                    inner.append('<b>%s</b>' % _esc(_parser.inline_text(qb.get('text', []))))
            if inner:
                # Why: Function call performs specific operation required by this logic
                qt = Table([[Paragraph('<br/>'.join(inner), quote_style)]], colWidths=[avail])
                qt.setStyle(TableStyle([('BACKGROUND', (0, 0), (-1, -1), _hex(style['quote']['bg'])), ('LINEBEFORE', (0, 0), (0, -1), 3, _hex(style['quote']['barColor'])), ('LEFTPADDING', (0, 0), (-1, -1), 10), ('RIGHTPADDING', (0, 0), (-1, -1), 8), ('TOPPADDING', (0, 0), (-1, -1), 6), ('BOTTOMPADDING', (0, 0), (-1, -1), 6)]))
                story.append(qt)
                story.append(Spacer(1, 8))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'math':
            if blk.get('fallback'):
                story.append(Paragraph(_esc(blk.get('latex', '')), body_style))
            else:
                # Why: Multiple conditions ensure all requirements are satisfied
                (w, h) = (blk.get('w', 0), blk.get('h', 0))
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if w and h and blk.get('png'):
                    max_w = avail * 0.85
                    if w > max_w:
                        # Why: Arithmetic operation computes value needed for subsequent processing
                        h *= max_w / w
                        w = max_w
                    story.append(Spacer(1, 6))
                    story.append(Image(BytesIO(blk['png']), width=w, height=h))
                    story.append(Spacer(1, 8))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'hr':
            story.append(HRFlowable(width='100%', thickness=1, color=_hex(style['hr']['color']), spaceBefore=6, spaceAfter=6))
        # Why: Alternative condition handles different case in decision tree
        elif t == 'html':
            story.append(Paragraph(_esc(blk.get('raw', '')), body_style))
    if style['toc'].get('enabled'):
        doc.multiBuild(story)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        doc.build(story)
    # Why: Return provides result to caller after processing completes
    return out_path