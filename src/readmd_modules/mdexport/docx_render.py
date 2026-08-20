# -*- coding: utf-8 -*-
"""Markdown 块 AST -> DOCX（python-docx，内置 Heading 样式 + 样式 token）。"""

import io
import re

from docx import Document
from docx.enum.section import WD_ORIENT
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement, parse_xml
from docx.oxml.ns import nsdecls, qn
from docx.parts.hdrftr import FooterPart, HeaderPart
from docx.shared import Mm, Pt, RGBColor
from docx.opc.constants import RELATIONSHIP_TYPE as RT

from . import parser as _parser

_BASE_FONT = 'Microsoft YaHei'


def _default_part_xml(original, tag, style_name):
    """Load python-docx's template, with a frozen-bundle-safe minimal fallback."""
    try:
        return original()
    except FileNotFoundError:
        return (
            "<?xml version='1.0' encoding='UTF-8' standalone='yes'?>"
            '<w:%s %s><w:p><w:pPr><w:pStyle w:val="%s"/>'
            '</w:pPr></w:p></w:%s>' % (tag, nsdecls('w'), style_name, tag)
        ).encode('utf-8')


_DOCX_DEFAULT_FOOTER = FooterPart._default_footer_xml
_DOCX_DEFAULT_HEADER = HeaderPart._default_header_xml


@classmethod
def _safe_default_footer_xml(cls):
    return _default_part_xml(_DOCX_DEFAULT_FOOTER, 'ftr', 'Footer')


@classmethod
def _safe_default_header_xml(cls):
    return _default_part_xml(_DOCX_DEFAULT_HEADER, 'hdr', 'Header')


FooterPart._default_footer_xml = _safe_default_footer_xml
HeaderPart._default_header_xml = _safe_default_header_xml


def _hex_rgb(v, default='262626'):
    m = re.match(r'^#([0-9a-fA-F]{6})$', v or '')
    if not m:
        m = re.match(r'^([0-9a-fA-F]{6})$', str(default))
    return RGBColor.from_string(m.group(1))


def _hex_val(v, default='262626'):
    m = re.match(r'^#([0-9a-fA-F]{6})$', v or '')
    return (m.group(1) if m else default).upper()


def _set_font(run, name=_BASE_FONT, size=None, color=None, bold=None, italic=None):
    run.font.name = name
    if size:
        run.font.size = Pt(size)
    if color:
        run.font.color.rgb = color
    if bold is not None:
        run.font.bold = bold
    if italic is not None:
        run.font.italic = italic
    try:
        rpr = run._element.get_or_add_rPr()
        rf = rpr.find(qn('w:rFonts'))
        if rf is None:
            rf = OxmlElement('w:rFonts')
            rpr.append(rf)
        rf.set(qn('w:ascii'), name)
        rf.set(qn('w:hAnsi'), name)
        rf.set(qn('w:eastAsia'), name)
    except Exception:
        pass


def _shade_paragraph(p, fill):
    pPr = p._p.get_or_add_pPr()
    shd = parse_xml('<w:shd %s w:val="clear" w:color="auto" w:fill="%s"/>' % (nsdecls('w'), fill))
    pPr.append(shd)


def _border_paragraph(p, edges, color, sz):
    """edges: list of 'left'/'top'/'bottom'/'right'。"""
    pPr = p._p.get_or_add_pPr()
    pBdr = pPr.find(qn('w:pBdr'))
    if pBdr is None:
        pBdr = OxmlElement('w:pBdr')
        pPr.append(pBdr)
    for edge in edges:
        el = OxmlElement('w:' + edge)
        el.set(qn('w:val'), 'single')
        el.set(qn('w:sz'), str(int(sz * 8)))
        el.set(qn('w:space'), '4')
        el.set(qn('w:color'), _hex_val(color))
        pBdr.append(el)


def add_inline(paragraph, nodes, style, tmpdir, resolve):
    base_size = float(style['typography']['size'])
    body_color = _hex_rgb(style['typography']['color'])
    link_color = _hex_rgb(style['link']['color'])
    code_color = RGBColor(0xC7, 0x25, 0x4E)
    mono = style['code']['font'] or 'Consolas'

    def _emit(nds, bold=False, italic=False, size=base_size, color=body_color):
        for nd in nds:
            t = nd['t']
            if t == 'text':
                r = paragraph.add_run(nd['v'])
                _set_font(r, size=size, color=color, bold=bold, italic=italic)
            elif t == 'bold':
                _emit(nd['v'] if isinstance(nd['v'], list) else _parser.parse_inline(nd['v']),
                      bold=True, italic=italic, size=size, color=color)
            elif t == 'italic':
                _emit(nd['v'] if isinstance(nd['v'], list) else _parser.parse_inline(nd['v']),
                      bold=bold, italic=True, size=size, color=color)
            elif t == 'strike':
                r = paragraph.add_run(nd['v'] if isinstance(nd['v'], str) else _parser.inline_text(nd['v']))
                _set_font(r, size=size, color=color, bold=bold, italic=italic)
                r.font.strike = True
            elif t == 'code':
                r = paragraph.add_run(nd['v'])
                _set_font(r, name=mono, size=max(6.0, size - 0.5), color=code_color)
            elif t == 'link':
                inner = _parser.inline_text(nd['text'])
                _add_hyperlink(paragraph, nd['href'], inner, link_color)
            elif t == 'image':
                src = resolve(nd['src'])
                if src:
                    try:
                        run = paragraph.add_run()
                        run.add_picture(src, width=Mm(40))
                    except Exception:
                        r = paragraph.add_run(nd['alt'] or nd['src'])
                        _set_font(r, size=size, color=color)
            elif t == 'math':
                latex = nd.get('latex', '')
                inserted = False
                if latex:
                    try:
                        from ..latex2omml import latex_to_omml
                        omml_xml = latex_to_omml(latex, is_block=False)
                        element = parse_xml(omml_xml)
                        paragraph._p.append(element)
                        inserted = True
                    except Exception:
                        inserted = False
                if not inserted:
                    if nd.get('fallback'):
                        r = paragraph.add_run(nd['latex'])
                        _set_font(r, size=size, color=color, italic=True)
                    elif nd.get('png') and nd.get('w'):
                        try:
                            run = paragraph.add_run()
                            run.add_picture(io.BytesIO(nd['png']), width=Pt(nd['w']))
                        except Exception:
                            r = paragraph.add_run(nd['latex'])
                            _set_font(r, size=size, color=color, italic=True)
                    else:
                        r = paragraph.add_run(nd.get('latex', ''))
                        _set_font(r, size=size, color=color, italic=True)
    _emit(nodes)


def _add_hyperlink(paragraph, url, text, color):
    part = paragraph.part
    try:
        r_id = part.relate_to(url, RT.HYPERLINK, is_external=True)
    except Exception:
        r_id = None
    hyperlink = OxmlElement('w:hyperlink')
    if r_id:
        hyperlink.set(qn('r:id'), r_id)
    new_run = OxmlElement('w:r')
    rPr = OxmlElement('w:rPr')
    c = OxmlElement('w:color')
    c.set(qn('w:val'), _hex_val('#' + str(color)))
    rPr.append(c)
    u = OxmlElement('w:u')
    u.set(qn('w:val'), 'single')
    rPr.append(u)
    new_run.append(rPr)
    t = OxmlElement('w:t')
    t.text = text
    new_run.append(t)
    hyperlink.append(new_run)
    paragraph._p.append(hyperlink)


def _shade_cell(cell, fill):
    tcPr = cell._tc.get_or_add_tcPr()
    shd = parse_xml('<w:shd %s w:val="clear" w:color="auto" w:fill="%s"/>' % (nsdecls('w'), fill))
    tcPr.append(shd)


def _table_borders(table, color, sz):
    tbl = table._tbl
    tblPr = tbl.tblPr
    sz_i = int(sz * 8)
    val = _hex_val(color)
    borders = parse_xml(
        '<w:tblBorders %s>'
        '  <w:top w:val="single" w:sz="%d" w:space="0" w:color="%s"/>'
        '  <w:left w:val="single" w:sz="%d" w:space="0" w:color="%s"/>'
        '  <w:bottom w:val="single" w:sz="%d" w:space="0" w:color="%s"/>'
        '  <w:right w:val="single" w:sz="%d" w:space="0" w:color="%s"/>'
        '  <w:insideH w:val="single" w:sz="%d" w:space="0" w:color="%s"/>'
        '  <w:insideV w:val="single" w:sz="%d" w:space="0" w:color="%s"/>'
        '</w:tblBorders>' % (nsdecls('w'), sz_i, val, sz_i, val, sz_i, val,
                             sz_i, val, sz_i, val, sz_i, val))
    tblPr.append(borders)


def render(blocks, out_path, style, tmpdir, resolve, warns):
    doc = Document()
    # 默认字体（中文回退）
    normal = doc.styles['Normal']
    normal.font.name = _BASE_FONT
    normal.font.size = Pt(float(style['typography']['size']))
    try:
        normal.element.rPr.rFonts.set(qn('w:eastAsia'), _BASE_FONT)
    except Exception:
        pass

    # 页面设置
    sec = doc.sections[0]
    page = style['page']
    sizes = {'A4': (210, 297), 'A5': (148, 210), 'B5': (176, 250), 'Letter': (215.9, 279.4), 'Legal': (215.9, 355.6)}
    w, h = sizes.get(page['size'], (210, 297))
    if page['orientation'] == 'landscape':
        w, h = h, w
        sec.orientation = WD_ORIENT.LANDSCAPE
    else:
        sec.orientation = WD_ORIENT.PORTRAIT
    sec.page_width = Mm(w)
    sec.page_height = Mm(h)
    sec.top_margin = Mm(page['marginTop'])
    sec.right_margin = Mm(page['marginRight'])
    sec.bottom_margin = Mm(page['marginBottom'])
    sec.left_margin = Mm(page['marginLeft'])

    # 页脚页码
    footer = sec.footer
    fp = footer.paragraphs[0]
    fp.alignment = WD_ALIGN_PARAGRAPH.CENTER
    if style['footer'].get('text'):
        r = fp.add_run(style['footer']['text'] + '  ')
        _set_font(r, size=8, color=RGBColor(0x99, 0x99, 0x99))
    if style['footer'].get('pageNumbers'):
        fld = OxmlElement('w:fldSimple')
        fld.set(qn('w:instr'), ' PAGE ')
        r = OxmlElement('w:r')
        t = OxmlElement('w:t')
        t.text = '1'
        r.append(t)
        fld.append(r)
        fp._p.append(fld)

    # 标题样式（导航窗格可用）
    for i in range(1, 7):
        try:
            hs = doc.styles['Heading %d' % i]
            hs.font.name = _BASE_FONT
            try:
                hs.element.rPr.rFonts.set(qn('w:eastAsia'), _BASE_FONT)
            except Exception:
                pass
        except Exception:
            pass

    def _add_heading(level, nodes):
        p = doc.add_heading(level=min(level, 6))
        h = style['headings']['h%d' % min(level, 6)]
        p.alignment = {'left': WD_ALIGN_PARAGRAPH.LEFT, 'center': WD_ALIGN_PARAGRAPH.CENTER,
                       'right': WD_ALIGN_PARAGRAPH.RIGHT, 'justify': WD_ALIGN_PARAGRAPH.JUSTIFY}.get(h['align'], WD_ALIGN_PARAGRAPH.LEFT)
        # 清空默认 run 后重写
        for r in list(p.runs):
            r._element.getparent().remove(r._element)
        add_inline(p, nodes, style, tmpdir, resolve)
        for r in p.runs:
            _set_font(r, size=float(h['size']), color=_hex_rgb(h['color']), bold=bool(h['bold']))

    def _add_paragraph(nodes):
        p = doc.add_paragraph()
        p.alignment = {'left': WD_ALIGN_PARAGRAPH.LEFT, 'center': WD_ALIGN_PARAGRAPH.CENTER,
                       'right': WD_ALIGN_PARAGRAPH.RIGHT, 'justify': WD_ALIGN_PARAGRAPH.JUSTIFY}.get(style['typography']['align'], WD_ALIGN_PARAGRAPH.LEFT)
        add_inline(p, nodes, style, tmpdir, resolve)
        return p

    # 封面
    cover = style['cover']
    if cover.get('enabled'):
        align = {'left': WD_ALIGN_PARAGRAPH.LEFT, 'center': WD_ALIGN_PARAGRAPH.CENTER,
                 'right': WD_ALIGN_PARAGRAPH.RIGHT}.get(cover.get('align'), WD_ALIGN_PARAGRAPH.CENTER)
        if cover.get('title'):
            p = doc.add_paragraph()
            p.alignment = align
            r = p.add_run(cover['title'])
            _set_font(r, size=26, color=_hex_rgb(style['headings']['h1']['color']), bold=True)
        if cover.get('subtitle'):
            p = doc.add_paragraph()
            p.alignment = align
            r = p.add_run(cover['subtitle'])
            _set_font(r, size=14, color=RGBColor(0x66, 0x66, 0x66))
        if cover.get('date'):
            p = doc.add_paragraph()
            p.alignment = align
            r = p.add_run(cover['date'])
            _set_font(r, size=11, color=RGBColor(0x88, 0x88, 0x88))
        doc.add_page_break()

    tb = style['table']
    for blk in blocks:
        t = blk['type']
        if t == 'heading':
            _add_heading(blk['level'], blk.get('text', []))
        elif t == 'paragraph':
            _add_paragraph(blk.get('text', []))
        elif t == 'table':
            node_rows = [blk.get('header', [])] + blk.get('rows', [])
            if not node_rows:
                continue
            ncols = max(len(r) for r in node_rows)
            for r in node_rows:
                while len(r) < ncols:
                    r.append([])
            table = doc.add_table(rows=len(node_rows), cols=ncols)
            table.style = 'Table Grid'
            table.alignment = WD_TABLE_ALIGNMENT.CENTER
            table.autofit = False
            content_w_mm = (sec.page_width - sec.left_margin - sec.right_margin) / 36000.0  # EMU->mm
            table_w_mm = content_w_mm * float(tb['widthPct']) / 100.0
            col_mm = table_w_mm / ncols
            for i, row in enumerate(node_rows):
                for j in range(ncols):
                    cell = table.cell(i, j)
                    cell.width = Mm(col_mm)
                    p0 = cell.paragraphs[0]
                    p0.alignment = {'left': WD_ALIGN_PARAGRAPH.LEFT, 'center': WD_ALIGN_PARAGRAPH.CENTER,
                                    'right': WD_ALIGN_PARAGRAPH.RIGHT, 'justify': WD_ALIGN_PARAGRAPH.JUSTIFY}.get(
                        tb['align'], WD_ALIGN_PARAGRAPH.LEFT)
                    for r0 in list(p0.runs):
                        r0._element.getparent().remove(r0._element)
                    add_inline(p0, row[j], style, tmpdir, resolve)
                    # 表头样式
                    if i == 0:
                        _shade_cell(cell, _hex_val(tb['headerBg']))
                        for r0 in p0.runs:
                            _set_font(r0, size=float(tb['cellSize']),
                                      color=_hex_rgb(tb['headerColor']), bold=bool(tb['headerBold']))
                    else:
                        for r0 in p0.runs:
                            _set_font(r0, size=float(tb['cellSize']))
                        if tb.get('banded') and i % 2 == 0:
                            _shade_cell(cell, _hex_val(tb['bandColor']))
            for j in range(ncols):
                table.columns[j].width = Mm(col_mm)
            _table_borders(table, tb['borderColor'], float(tb['borderWidth']))
            doc.add_paragraph()
        elif t == 'code':
            content = blk.get('content', '').rstrip('\n')
            if not content:
                continue
            if blk.get('lang'):
                lp = doc.add_paragraph()
                r = lp.add_run(blk['lang'])
                _set_font(r, size=8, color=RGBColor(0x88, 0x88, 0x88))
            p = doc.add_paragraph()
            p.paragraph_format.left_indent = Mm(4)
            _shade_paragraph(p, _hex_val(style['code']['bg']))
            _border_paragraph(p, ['top', 'bottom', 'left', 'right'],
                              style['code']['borderColor'], float(style['code']['borderWidth']))
            lines = content.split('\n')
            for k, line in enumerate(lines):
                r = p.add_run(line if line else ' ')
                _set_font(r, name=style['code']['font'] or 'Consolas', size=float(style['code']['size']),
                          color=_hex_rgb(style['code']['color']))
                if k < len(lines) - 1:
                    r.add_break()
        elif t == 'list':
            for idx, it in enumerate(blk.get('items', [])):
                if it.get('ordered'):
                    p = doc.add_paragraph(style='List Number')
                else:
                    p = doc.add_paragraph(style='List Bullet')
                prefix = ''
                if it.get('task'):
                    prefix = '\u2612 ' if it.get('checked') else '\u2610 '
                if prefix:
                    r = p.add_run(prefix)
                    _set_font(r, size=float(style['typography']['size']))
                add_inline(p, it.get('text', []), style, tmpdir, resolve)
        elif t == 'quote':
            inner = []
            for qb in blk.get('blocks', []):
                if qb['type'] == 'paragraph':
                    inner.append(_parser.inline_text(qb.get('text', [])))
                elif qb['type'] == 'heading':
                    inner.append(_parser.inline_text(qb.get('text', [])))
            if inner:
                p = doc.add_paragraph()
                p.paragraph_format.left_indent = Mm(6)
                _shade_paragraph(p, _hex_val(style['quote']['bg']))
                _border_paragraph(p, ['left'], style['quote']['barColor'], 3)
                r = p.add_run('\n'.join(inner))
                _set_font(r, size=float(style['typography']['size']), color=_hex_rgb(style['quote']['color']))
        elif t == 'math':
            latex = blk.get('latex', '')
            inserted = False
            if latex:
                try:
                    from ..latex2omml import latex_to_omml
                    omml_xml = latex_to_omml(latex, is_block=True)
                    element = parse_xml(omml_xml)
                    p = doc.add_paragraph()
                    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
                    p._p.append(element)
                    inserted = True
                except Exception:
                    inserted = False
            if not inserted:
                p = doc.add_paragraph()
                p.alignment = WD_ALIGN_PARAGRAPH.CENTER
                if blk.get('fallback'):
                    r = p.add_run(blk.get('latex', ''))
                    _set_font(r, size=float(style['typography']['size']), italic=True)
                elif blk.get('png') and blk.get('w'):
                    try:
                        p.add_run().add_picture(io.BytesIO(blk['png']), width=Pt(blk['w']))
                    except Exception:
                        r = p.add_run(blk.get('latex', ''))
                        _set_font(r, size=float(style['typography']['size']), italic=True)
                else:
                    r = p.add_run(blk.get('latex', ''))
                    _set_font(r, size=float(style['typography']['size']), italic=True)
        elif t == 'hr':
            p = doc.add_paragraph()
            _border_paragraph(p, ['bottom'], style['hr']['color'], 1)
        elif t == 'html':
            p = doc.add_paragraph()
            r = p.add_run(blk.get('raw', ''))
            _set_font(r, size=float(style['typography']['size']))

    doc.save(out_path)
    return out_path
