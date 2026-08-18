# -*- coding: utf-8 -*-
"""Markdown 块 AST -> PDF（reportlab platypus，中文 TTF + 样式 token）。"""

import os
import re
from io import BytesIO

from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT, TA_CENTER, TA_RIGHT, TA_JUSTIFY
from reportlab.lib.pagesizes import A4, A5, B5, letter, legal
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.units import mm
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.cidfonts import UnicodeCIDFont
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (BaseDocTemplate, Frame, Image, PageBreak,
                                Paragraph, Spacer, Table, TableStyle,
                                XPreformatted, PageTemplate)
from reportlab.platypus.flowables import HRFlowable
from reportlab.platypus.tableofcontents import TableOfContents

from . import parser as _parser

_PAGE_MAP = {'A4': A4, 'A5': A5, 'B5': B5, 'Letter': letter, 'Legal': legal}
_TA = {'left': TA_LEFT, 'center': TA_CENTER, 'right': TA_RIGHT, 'justify': TA_JUSTIFY}

_registered_font_name = None


def _first_existing(*paths):
    for p in paths:
        if os.path.exists(p):
            return p
    return paths[0]


def register_fonts():
    """注册中文字体（微软雅黑 + 粗体，回退黑体）。"""
    global _registered_font_name
    if _registered_font_name and _font_ready(_registered_font_name):
        return _registered_font_name
    try:
        regular = _first_existing(
            r'C:\Windows\Fonts\msyh.ttc',
            '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc',
            '/System/Library/Fonts/PingFang.ttc',
        )
        bold = _first_existing(
            r'C:\Windows\Fonts\msyhbd.ttc',
            '/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc',
            '/System/Library/Fonts/PingFang.ttc',
        )
        if os.path.exists(regular):
            pdfmetrics.registerFont(TTFont('MicrosoftYaHei', regular, subfontIndex=0))
            if os.path.exists(bold):
                try:
                    pdfmetrics.registerFont(TTFont('MicrosoftYaHei-Bold', bold, subfontIndex=0))
                    pdfmetrics.registerFontFamily('MicrosoftYaHei',
                                                  normal='MicrosoftYaHei', bold='MicrosoftYaHei-Bold')
                except Exception:
                    pass
            _registered_font_name = 'MicrosoftYaHei'
            return 'MicrosoftYaHei'
    except Exception:
        pass
    fallback = _first_existing(r'C:\Windows\Fonts\simhei.ttf', '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc')
    if os.path.exists(fallback):
        try:
            pdfmetrics.registerFont(TTFont('SimHei', fallback))
            _registered_font_name = 'SimHei'
            return 'SimHei'
        except Exception:
            pass
    try:
        pdfmetrics.registerFont(UnicodeCIDFont('STSong-Light'))
        pdfmetrics.registerFontFamily('STSong-Light', normal='STSong-Light',
                                      bold='STSong-Light', italic='STSong-Light',
                                      boldItalic='STSong-Light')
        _registered_font_name = 'STSong-Light'
    except Exception:
        _registered_font_name = 'Helvetica'
    return _registered_font_name


def _font_ready(name):
    try:
        return name in pdfmetrics.getRegisteredFontNames()
    except Exception:
        return False


def register_mono_fonts():
    """注册 Consolas 等宽字体（含族映射），失败回退内置 Courier。"""
    try:
        reg = r'C:\Windows\Fonts\consola.ttf'
        if os.path.exists(reg):
            pdfmetrics.registerFont(TTFont('Consolas', reg))
            pdfmetrics.registerFontFamily('Consolas', normal='Consolas',
                                          bold='Consolas', italic='Consolas', boldItalic='Consolas')
            return 'Consolas'
    except Exception:
        pass
    return 'Courier'


def _hex(v, default='#000000'):
    m = re.match(r'^#([0-9a-fA-F]{6})$', v or '')
    return colors.HexColor(v if m else default)


def _size(v):
    return float(v or 11)


def _esc(s):
    return (s or '').replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;')


def _img_tag(tmpdir, data, width, height):
    """把 PNG bytes 落盘并生成 reportlab Paragraph <img> 标签。"""
    if data is None or not width:
        return ''
    path = os.path.join(tmpdir, 'm_%d_%d.png' % (abs(hash(data)), len(data)))
    if not os.path.exists(path):
        with open(path, 'wb') as f:
            f.write(data)
    return '<img src="%s" width="%.1f" height="%.1f" valign="middle"/>' % (path.replace('\\', '/'), width, height)


def _inline_markup(nodes, style, tmpdir, resolve):
    out = []
    mono = register_mono_fonts()
    code_size = max(6.0, float(style['code']['size']) - 0.5)
    link_color = _hex(style['link']['color'], '#2b6cb0')
    for nd in nodes:
        t = nd['t']
        if t == 'text':
            out.append(_esc(nd['v']))
        elif t == 'bold':
            out.append('<b>%s</b>' % _inline_markup(nd['v'] if isinstance(nd['v'], list) else _parser.parse_inline(nd['v']), style, tmpdir, resolve))
        elif t == 'italic':
            out.append('<i>%s</i>' % _inline_markup(nd['v'] if isinstance(nd['v'], list) else _parser.parse_inline(nd['v']), style, tmpdir, resolve))
        elif t == 'code':
            out.append('<font face="%s" size="%.1f" color="#c7254e">%s</font>' % (mono, code_size, _esc(nd['v'])))
        elif t == 'strike':
            out.append(_esc(nd['v'] if isinstance(nd['v'], str) else _parser.inline_text(nd['v'])))
        elif t == 'link':
            href = _esc(nd['href'])
            inner = _inline_markup(nd['text'], style, tmpdir, resolve)
            out.append('<a href="%s" color="#%s">%s</a>' % (href, link_color.hexval()[2:], inner))
        elif t == 'image':
            src = resolve(nd['src'])
            if src:
                try:
                    ir = ImageReader(src)
                    iw, ih = ir.getSize()
                    base = float(style['typography']['size']) * 1.3
                    scale = min(1.0, base / ih)
                    out.append('<img src="%s" width="%.1f" height="%.1f"/>' % (src.replace('\\', '/'), iw * scale, ih * scale))
                except Exception:
                    out.append(_esc(nd['alt'] or nd['src']))
        elif t == 'math':
            if nd.get('fallback'):
                out.append(_esc(nd['latex']))
            else:
                out.append(_img_tag(tmpdir, nd.get('png'), nd.get('w'), nd.get('h')))
    return ''.join(out)


class _ExportDoc(BaseDocTemplate):
    """带目录通知 / PDF 书签 / 页脚的模板。"""

    def __init__(self, filename, **kw):
        super(_ExportDoc, self).__init__(filename, **kw)
        self._footer_text = ''
        self._page_numbers = True

    def afterFlowable(self, flowable):
        if isinstance(flowable, Paragraph):
            sty = getattr(flowable, 'style', None)
            name = getattr(sty, 'name', '')
            text = flowable.getPlainText()
            if name.startswith('H') and len(name) > 1 and name[1:].isdigit():
                level = int(name[1:])
                self.notify('TOCEntry', (level, text, self.page))
                try:
                    self.canv.bookmarkPage('sec%d' % self.page)
                    self.canv.addOutlineEntry(text, 'sec%d' % self.page, level=level - 1)
                except Exception:
                    pass

    def _footer(self, canv, doc):
        canv.saveState()
        w, h = doc.pagesize
        try:
            if self._footer_text:
                canv.setFont('Helvetica', 8)
                canv.setFillColor(colors.HexColor('#999999'))
                canv.drawString(18 * mm, 10 * mm, self._footer_text[:120])
            if self._page_numbers:
                canv.setFont('Helvetica', 8)
                canv.setFillColor(colors.HexColor('#999999'))
                canv.drawCentredString(w / 2.0, 10 * mm, str(canv.getPageNumber()))
        except Exception:
            pass
        canv.restoreState()


def render(blocks, out_path, style, tmpdir, resolve, warns):
    """生成 PDF。"""
    font = register_fonts()
    page = style['page']
    size = _PAGE_MAP.get(page['size'], A4)
    if page['orientation'] == 'landscape':
        size = (size[1], size[0])

    doc = _ExportDoc(
        out_path, pagesize=size,
        leftMargin=page['marginLeft'] * mm, rightMargin=page['marginRight'] * mm,
        topMargin=page['marginTop'] * mm, bottomMargin=page['marginBottom'] * mm,
        title=style['meta'].get('title') or '',
        author=style['meta'].get('author') or '',
        subject=style['meta'].get('subject') or '',
    )
    doc._footer_text = style['footer'].get('text', '')
    doc._page_numbers = bool(style['footer'].get('pageNumbers', True))

    frame = Frame(doc.leftMargin, doc.bottomMargin, doc.width, doc.height, id='main')
    doc.addPageTemplates([PageTemplate(id='all', frames=[frame], onPage=doc._footer)])

    avail = doc.width
    ty = style['typography']
    body_style = ParagraphStyle(
        'Body', fontName=font, fontSize=_size(ty['size']),
        leading=_size(ty['size']) * float(ty['lineHeight']),
        spaceAfter=float(ty['spacing']), textColor=_hex(ty['color']),
        alignment=_TA.get(ty['align'], TA_LEFT),
    )
    head_styles = {}
    for i in range(1, 7):
        h = style['headings']['h%d' % i]
        head_styles[i] = ParagraphStyle(
            'H%d' % i, parent=body_style, fontSize=_size(h['size']),
            leading=_size(h['size']) * 1.35, spaceBefore=float(h['before']),
            spaceAfter=float(h['after']), textColor=_hex(h['color']),
            alignment=_TA.get(h['align'], TA_LEFT),
            fontName=(font + '-Bold') if (h['bold'] and (font + '-Bold') in pdfmetrics.getRegisteredFontNames()) else font,
        )
    li_style = ParagraphStyle('Li', parent=body_style, leftIndent=18, firstLineIndent=0)
    code_style = ParagraphStyle(
        'Code', fontName=register_mono_fonts(),
        fontSize=float(style['code']['size']), leading=float(style['code']['size']) * 1.4,
        textColor=_hex(style['code']['color']), alignment=TA_LEFT,
    )
    quote_style = ParagraphStyle('Quote', parent=body_style,
                                 textColor=_hex(style['quote']['color']))

    story = []

    cover = style['cover']
    if cover.get('enabled'):
        align = _TA.get(cover.get('align', 'center'), TA_CENTER)
        story.append(Spacer(1, 60 * mm))
        title = cover.get('title') or ''
        story.append(Paragraph(_esc(title), ParagraphStyle(
            'CoverTitle', parent=body_style, fontSize=26, leading=34,
            alignment=align, fontName=(font + '-Bold') if (font + '-Bold') in pdfmetrics.getRegisteredFontNames() else font,
            textColor=_hex(style['headings']['h1']['color']))))
        if cover.get('subtitle'):
            story.append(Spacer(1, 8 * mm))
            story.append(Paragraph(_esc(cover['subtitle']), ParagraphStyle(
                'CoverSub', parent=body_style, fontSize=14, leading=20, alignment=align,
                textColor=colors.HexColor('#666666'))))
        if cover.get('date'):
            story.append(Spacer(1, 6 * mm))
            story.append(Paragraph(_esc(cover['date']), ParagraphStyle(
                'CoverDate', parent=body_style, fontSize=11, alignment=align,
                textColor=colors.HexColor('#888888'))))
        story.append(PageBreak())

    if style['toc'].get('enabled'):
        toc = TableOfContents()
        toc.levelStyles = [
            ParagraphStyle('T1', parent=body_style, fontSize=_size(ty['size']), leading=18, textColor=_hex('#111111')),
            ParagraphStyle('T2', parent=body_style, fontSize=_size(ty['size']) - 0.5, leading=16, leftIndent=14),
            ParagraphStyle('T3', parent=body_style, fontSize=_size(ty['size']) - 1, leading=15, leftIndent=28),
            ParagraphStyle('T4', parent=body_style, fontSize=_size(ty['size']) - 1, leading=14, leftIndent=42),
            ParagraphStyle('T5', parent=body_style, fontSize=_size(ty['size']) - 1, leading=13, leftIndent=56),
            ParagraphStyle('T6', parent=body_style, fontSize=_size(ty['size']) - 1, leading=13, leftIndent=70),
        ]
        story.append(Paragraph(_esc('目录'), ParagraphStyle('TocTitle', parent=body_style,
                         fontSize=18, spaceAfter=12, fontName=font, textColor=_hex('#111111'))))
        story.append(toc)
        story.append(PageBreak())

    def _add_inline_par(nodes, style_obj):
        story.append(Paragraph(_inline_markup(nodes, style, tmpdir, resolve), style_obj))

    for blk in blocks:
        t = blk['type']
        if t == 'heading':
            _add_inline_par(blk['text'], head_styles[min(blk['level'], 6)])
        elif t == 'paragraph':
            txt = blk.get('text', [])
            if len(txt) == 1 and txt[0].get('t') == 'image' and not txt[0].get('src', '').startswith(('http://', 'https://')):
                src = resolve(txt[0]['src'])
                if src:
                    try:
                        ir = ImageReader(src)
                        iw, ih = ir.getSize()
                        max_w = avail * 0.92
                        scale = min(1.0, max_w / iw)
                        story.append(Image(src, width=iw * scale, height=ih * scale))
                        story.append(Spacer(1, 6))
                    except Exception:
                        warns.append('图片无法嵌入：%s' % txt[0]['src'])
                else:
                    warns.append('图片不存在，已跳过：%s' % txt[0]['src'])
            else:
                _add_inline_par(txt, body_style)
        elif t == 'table':
            data = []
            header = [_inline_markup(c, style, tmpdir, resolve) for c in blk.get('header', [])]
            data.append(header)
            for row in blk.get('rows', []):
                data.append([_inline_markup(c, style, tmpdir, resolve) for c in row])
            if not data:
                continue
            ncols = max(len(r) for r in data)
            for r in data:
                while len(r) < ncols:
                    r.append('')
            tb = style['table']
            table_w = avail * float(tb['widthPct']) / 100.0
            col_w = table_w / ncols
            tbl = Table(data, colWidths=[col_w] * ncols, repeatRows=1)
            cmds = [
                ('BACKGROUND', (0, 0), (-1, 0), _hex(tb['headerBg'])),
                ('TEXTCOLOR', (0, 0), (-1, 0), _hex(tb['headerColor'])),
                ('FONTNAME', (0, 0), (-1, 0), font),
                ('FONTSIZE', (0, 0), (-1, 0), float(tb['cellSize'])),
                ('FONTNAME', (0, 1), (-1, -1), font),
                ('FONTSIZE', (0, 1), (-1, -1), float(tb['cellSize'])),
                ('GRID', (0, 0), (-1, -1), float(tb['borderWidth']), _hex(tb['borderColor'])),
                ('TOPPADDING', (0, 0), (-1, -1), float(tb['cellPadding'])),
                ('BOTTOMPADDING', (0, 0), (-1, -1), float(tb['cellPadding'])),
                ('LEFTPADDING', (0, 0), (-1, -1), 6),
                ('RIGHTPADDING', (0, 0), (-1, -1), 6),
                ('VALIGN', (0, 0), (-1, -1), 'MIDDLE'),
            ]
            for ci, al in enumerate(blk.get('aligns', [])):
                if al in ('center', 'right'):
                    cmds.append(('ALIGN', (ci, 0), (ci, -1), {'center': 'CENTER', 'right': 'RIGHT'}[al]))
            if tb.get('banded'):
                cmds.append(('ROWBACKGROUNDS', (0, 1), (-1, -1),
                             [colors.white, _hex(tb['bandColor'])]))
            tbl.setStyle(TableStyle(cmds))
            story.append(tbl)
            story.append(Spacer(1, 10))
        elif t == 'code':
            content = blk.get('content', '')
            if not content:
                continue
            lang = blk.get('lang', '')
            if lang:
                story.append(Paragraph(_esc(lang), ParagraphStyle('LangTag', parent=body_style,
                                     fontSize=8, textColor=colors.HexColor('#888888'), spaceBefore=6)))
            ct = Table([[XPreformatted(content, code_style)]], colWidths=[avail])
            cmds = [
                ('BACKGROUND', (0, 0), (-1, -1), _hex(style['code']['bg'])),
                ('BOX', (0, 0), (-1, -1), float(style['code']['borderWidth']), _hex(style['code']['borderColor'])),
                ('TOPPADDING', (0, 0), (-1, -1), 8),
                ('BOTTOMPADDING', (0, 0), (-1, -1), 8),
                ('LEFTPADDING', (0, 0), (-1, -1), 10),
                ('RIGHTPADDING', (0, 0), (-1, -1), 10),
            ]
            ct.setStyle(TableStyle(cmds))
            story.append(ct)
            story.append(Spacer(1, 10))
        elif t == 'list':
            for idx, it in enumerate(blk.get('items', [])):
                prefix = ''
                if it.get('task'):
                    prefix = '\u2612 ' if it.get('checked') else '\u2610 '
                elif it.get('ordered'):
                    prefix = '%d. ' % (idx + 1)
                else:
                    prefix = '\u2022 '
                markup = prefix + _inline_markup(it.get('text', []), style, tmpdir, resolve)
                story.append(Paragraph(markup, li_style))
            story.append(Spacer(1, 4))
        elif t == 'quote':
            inner = []
            for qb in blk.get('blocks', []):
                if qb['type'] == 'paragraph':
                    inner.append(_inline_markup(qb.get('text', []), style, tmpdir, resolve))
                elif qb['type'] == 'heading':
                    inner.append('<b>%s</b>' % _esc(_parser.inline_text(qb.get('text', []))))
            if inner:
                qt = Table([[Paragraph('<br/>'.join(inner), quote_style)]], colWidths=[avail])
                qt.setStyle(TableStyle([
                    ('BACKGROUND', (0, 0), (-1, -1), _hex(style['quote']['bg'])),
                    ('LINEBEFORE', (0, 0), (0, -1), 3, _hex(style['quote']['barColor'])),
                    ('LEFTPADDING', (0, 0), (-1, -1), 10),
                    ('RIGHTPADDING', (0, 0), (-1, -1), 8),
                    ('TOPPADDING', (0, 0), (-1, -1), 6),
                    ('BOTTOMPADDING', (0, 0), (-1, -1), 6),
                ]))
                story.append(qt)
                story.append(Spacer(1, 8))
        elif t == 'math':
            if blk.get('fallback'):
                story.append(Paragraph(_esc(blk.get('latex', '')), body_style))
            else:
                w, h = blk.get('w', 0), blk.get('h', 0)
                if w and h and blk.get('png'):
                    max_w = avail * 0.85
                    if w > max_w:
                        h *= max_w / w
                        w = max_w
                    story.append(Spacer(1, 6))
                    story.append(Image(BytesIO(blk['png']), width=w, height=h))
                    story.append(Spacer(1, 8))
        elif t == 'hr':
            story.append(HRFlowable(width='100%', thickness=1, color=_hex(style['hr']['color']),
                                    spaceBefore=6, spaceAfter=6))
        elif t == 'html':
            story.append(Paragraph(_esc(blk.get('raw', '')), body_style))

    if style['toc'].get('enabled'):
        doc.multiBuild(story)
    else:
        doc.build(story)
    return out_path
