# -*- coding: utf-8 -*-
"""v2.3.8：PPTX/XLSX 内置 OOXML 转换器（2026-09-02 验收问题#1 修复）。

最小手工构造的 OOXML 包（不依赖 openpyxl / python-pptx），覆盖：
- XLSX：共享字符串 / 数字 / inlineStr / 布尔 / 多工作表 / 管道转义；
- PPTX：标题占位符 → 二级标题、正文段落、内嵌表格、无标题幻灯片兜底；
- convert_verbose 分发与旧版 .ppt/.xls 的稳定错误。
"""
import zipfile

from src.readmd_modules import convert

XLS_MAIN = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
P_MAIN = 'http://schemas.openxmlformats.org/presentationml/2006/main'
A_MAIN = 'http://schemas.openxmlformats.org/drawingml/2006/main'
R_NS = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
REL_NS = 'http://schemas.openxmlformats.org/package/2006/relationships'


def _write_zip(path, parts):
    with zipfile.ZipFile(path, 'w') as zf:
        for name, data in parts.items():
            zf.writestr(name, data)


def _make_xlsx(path):
    _write_zip(path, {
        'xl/workbook.xml': (
            '<?xml version="1.0"?>'
            '<workbook xmlns="%s" xmlns:r="%s"><sheets>'
            '<sheet name="数据" sheetId="1" r:id="rId1"/>'
            '<sheet name="Sheet2" sheetId="2" r:id="rId2"/>'
            '</sheets></workbook>' % (XLS_MAIN, R_NS)),
        'xl/_rels/workbook.xml.rels': (
            '<?xml version="1.0"?>'
            '<Relationships xmlns="%s">'
            '<Relationship Id="rId1" Target="worksheets/sheet1.xml"/>'
            '<Relationship Id="rId2" Target="worksheets/sheet2.xml"/>'
            '</Relationships>' % REL_NS),
        'xl/sharedStrings.xml': (
            '<?xml version="1.0"?>'
            '<sst xmlns="%s" count="5" uniqueCount="5">'
            '<si><t>名称</t></si><si><t>数值</t></si><si><t>P|管</t></si>'
            '<si><t>readMD</t></si><si><t>共享文本</t></si>'
            '</sst>' % XLS_MAIN),
        'xl/worksheets/sheet1.xml': (
            '<?xml version="1.0"?>'
            '<worksheet xmlns="%s"><sheetData>'
            '<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c>'
            '<c r="C1" t="s"><v>2</v></c></row>'
            '<row r="2"><c r="A2" t="s"><v>3</v></c><c r="B2"><v>3.14</v></c>'
            '<c r="C2" t="s"><v>4</v></c></row>'
            '<row r="3"><c r="A3" t="inlineStr"><is><t>行内 文本</t></is></c>'
            '<c r="B3" t="b"><v>1</v></c></row>'
            '</sheetData></worksheet>' % XLS_MAIN),
        'xl/worksheets/sheet2.xml': (
            '<?xml version="1.0"?>'
            '<worksheet xmlns="%s"><sheetData>'
            '<row r="1"><c r="A1"><v>7</v></c></row>'
            '</sheetData></worksheet>' % XLS_MAIN),
    })


def _slide(title_sp, body_sp, table):
    parts = []
    if title_sp:
        parts.append(
            '<p:sp><p:nvSpPr><p:cNvPr id="2" name="标题"/>'
            '<p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>'
            '<p:txBody><a:bodyPr/><a:p><a:r><a:t>%s</a:t></a:r></a:p></p:txBody></p:sp>' % title_sp)
    if body_sp:
        paras = ''.join('<a:p><a:r><a:t>%s</a:t></a:r></a:p>' % t for t in body_sp)
        parts.append(
            '<p:sp><p:nvSpPr><p:cNvPr id="3" name="内容"/><p:nvPr/></p:nvSpPr>'
            '<p:txBody><a:bodyPr/>%s</p:txBody></p:sp>' % paras)
    if table:
        rows = ''.join(
            '<a:tr>%s</a:tr>' % ''.join(
                '<a:tc><a:txBody><a:p><a:r><a:t>%s</a:t></a:r></a:p></a:txBody></a:tc>' % c
                for c in row)
            for row in table)
        parts.append(
            '<p:graphicFrame><a:graphic><a:graphicData>'
            '<a:tbl><a:tblGrid/>%s</a:tbl>'
            '</a:graphicData></a:graphic></p:graphicFrame>' % rows)
    return (
        '<p:sld xmlns:p="%s" xmlns:a="%s"><p:cSld><p:spTree>%s</p:spTree></p:cSld></p:sld>'
        % (P_MAIN, A_MAIN, ''.join(parts)))


def _make_pptx(path):
    _write_zip(path, {
        'ppt/presentation.xml': (
            '<?xml version="1.0"?>'
            '<p:presentation xmlns:p="%s" xmlns:r="%s"><p:sldIdLst>'
            '<p:sldId id="256" r:id="rId1"/>'
            '<p:sldId id="257" r:id="rId2"/>'
            '</p:sldIdLst></p:presentation>' % (P_MAIN, R_NS)),
        'ppt/_rels/presentation.xml.rels': (
            '<?xml version="1.0"?>'
            '<Relationships xmlns="%s">'
            '<Relationship Id="rId1" Target="slides/slide1.xml"/>'
            '<Relationship Id="rId2" Target="slides/slide2.xml"/>'
            '</Relationships>' % REL_NS),
        'ppt/slides/slide1.xml': _slide('项目标题', ['第一段内容', '第二段内容'],
                                        [['列A', '列B'], ['1', '2']]),
        'ppt/slides/slide2.xml': _slide(None, ['仅正文'], None),
    })


def test_ooxml_column_index():
    assert convert._ooxml_column_index('A1') == 0
    assert convert._ooxml_column_index('C5') == 2
    assert convert._ooxml_column_index('AA1') == 26
    assert convert._ooxml_column_index('') is None


def test_xlsx_to_md_tables(tmp_path):
    xlsx = str(tmp_path / 'book.xlsx')
    _make_xlsx(xlsx)
    text, engine, error = convert.convert_verbose(xlsx)
    assert error is None and engine == 'xlsx'
    assert text.startswith('# book.xlsx')
    assert '## 数据' in text
    assert '| 名称 | 数值 | P\\|管 |' in text
    assert '| readMD | 3.14 | 共享文本 |' in text
    assert '| 行内 文本 | TRUE |  |' in text
    assert '## Sheet2' in text and '| 7 |' in text


def test_pptx_to_md_slides(tmp_path):
    pptx = str(tmp_path / 'deck.pptx')
    _make_pptx(pptx)
    text, engine, error = convert.convert_verbose(pptx)
    assert error is None and engine == 'pptx'
    assert text.startswith('# deck.pptx')
    assert '## 项目标题' in text
    assert '第一段内容' in text and '第二段内容' in text
    assert '| 列A | 列B |' in text and '| 1 | 2 |' in text
    assert '## Slide 2' in text and '仅正文' in text


def test_native_readers_reject_junk(tmp_path):
    for name in ('broken.xlsx', 'broken.pptx'):
        junk = str(tmp_path / name)
        with open(junk, 'wb') as fh:
            fh.write(b'PK\x03\x04 not a real zip\x00\x01')
        text, engine, error = convert.convert_verbose(junk)
        assert text == '' and engine == '' and error


def test_legacy_ppt_xls_stable_error(tmp_path, monkeypatch):
    def _boom(_path):
        raise ImportError('MarkItDown 未安装')

    monkeypatch.setattr(convert, '_markitdown_convert', _boom)
    for name in ('old.ppt', 'old.xls'):
        legacy = str(tmp_path / name)
        with open(legacy, 'wb') as fh:
            fh.write(b'\xd0\xcf\x11\xe0\x00\x00junk\x00')
        text, engine, error = convert.convert_verbose(legacy)
        assert text == '' and engine == ''
        assert error.startswith('legacy-office')
