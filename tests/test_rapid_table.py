# -*- coding: utf-8 -*-
import os
import sys
import pytest
from PIL import Image, ImageDraw
from src.readmd_modules import plugin_manager as pm
from src.readmd_modules.ocr import extract_table_to_md, _html_table_to_md


def test_rapid_table_plugin_spec():
    spec = pm.PLUGIN_SPECS.get('rapid_table')
    assert spec is not None
    assert spec['id'] == 'rapid_table'
    assert 'rapid_table>=0.1.3,<=0.3.0' in spec['pip_args']
    assert spec['import_name'] == 'rapid_table'
    assert spec['category'] == 'document'


def test_html_table_to_md():
    html = '<html><body><table><tr><td>Header 1</td><td>Header 2</td></tr><tr><td>Cell A</td><td>Cell B</td></tr></table></body></html>'
    md = _html_table_to_md(html)
    assert '| Header 1 | Header 2 |' in md
    assert '| --- | --- |' in md
    assert '| Cell A | Cell B |' in md


def test_rapid_table_extraction_with_mock_or_real(tmp_path):
    # Create sample table image
    img = Image.new('RGB', (300, 150), color='white')
    draw = ImageDraw.Draw(img)
    draw.rectangle([(10, 10), (290, 140)], outline='black', width=2)
    draw.line([(10, 50), (290, 50)], fill='black', width=2)
    draw.line([(150, 10), (150, 140)], fill='black', width=2)
    draw.text((20, 20), 'Col1', fill='black')
    draw.text((160, 20), 'Col2', fill='black')
    draw.text((20, 70), 'Val1', fill='black')
    draw.text((160, 70), 'Val2', fill='black')

    img_path = str(tmp_path / 'table.png')
    img.save(img_path)

    pm.mount_sandbox()
    appdata_sp = os.path.expandvars(r'%APPDATA%\ReadMD\plugins\site-packages')
    if os.path.isdir(appdata_sp) and appdata_sp not in sys.path:
        sys.path.insert(0, appdata_sp)
    try:
        from rapid_table import RapidTable
        table_engine = RapidTable()
        res = table_engine(img_path)
        table_html = res[0] if isinstance(res, (tuple, list)) else str(res or '')
        md = _html_table_to_md(table_html)
        assert '|' in md
    except ImportError:
        pytest.skip('rapid_table not installed in test runner environment')
