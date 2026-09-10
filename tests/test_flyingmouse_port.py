# -*- coding: utf-8 -*-
import os
import tempfile
import zipfile
import pytest
from src.readmd_modules import convert, convert_ext

def test_flyingmouse_table_cluster():
    """测试移植自飞鼠 pdf-table-extractor 的表格识别算法"""
    words = [
        {'x': 10, 'y': 20, 'width': 30, 'height': 10, 'text': '学科'},
        {'x': 70, 'y': 20, 'width': 30, 'height': 10, 'text': '分数'},
        {'x': 10, 'y': 45, 'width': 30, 'height': 10, 'text': '数学'},
        {'x': 70, 'y': 45, 'width': 30, 'height': 10, 'text': '145'},
        {'x': 10, 'y': 70, 'width': 30, 'height': 10, 'text': '物理'},
        {'x': 70, 'y': 70, 'width': 30, 'height': 10, 'text': '98'}
    ]
    md = convert_ext.cluster_words_into_table(words)
    assert '| 学科 | 分数 |' in md
    assert '| 数学 | 145 |' in md
    assert '| 物理 | 98 |' in md

def test_flyingmouse_epub_to_md():
    """测试移植自飞鼠 ebook.js 的 EPUB 解析转 Markdown"""
    with tempfile.NamedTemporaryFile(suffix='.epub', delete=False) as tf:
        epub_file = tf.name

    try:
        with zipfile.ZipFile(epub_file, 'w') as zf:
            zf.writestr('mimetype', 'application/epub+zip')
            zf.writestr('META-INF/container.xml', '''<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>''')
            zf.writestr('OEBPS/content.opf', '''<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <manifest>
    <item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="c1"/>
  </spine>
</package>''')
            zf.writestr('OEBPS/ch1.xhtml', '''<!DOCTYPE html>
<html>
<body>
  <h1>第一章：高考数学考点</h1>
  <p>这是<strong>导数单调性</strong>的核心推导：</p>
  <ul>
    <li>切线方程</li>
    <li>极值点偏移</li>
  </ul>
</body>
</html>''')

        res_md, engine, err = convert.convert_verbose(epub_file)
        assert err is None
        assert engine == 'epub'
        assert '# 第一章：高考数学考点' in res_md
        assert '**导数单调性**' in res_md
        assert '- 切线方程' in res_md
    finally:
        if os.path.exists(epub_file):
            os.remove(epub_file)
