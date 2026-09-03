import unittest
from unittest.mock import patch
import struct
import tempfile
import os
from src.readmd_modules.convert import (
    _classify_doc_heading,
    _parse_doc_stream_to_md,
    _extract_worddocument_stream,
    _doc_extract_text_pure_python,
    doc2md,
    convert_verbose,
    OLE2_MAGIC,
)


def build_ole2_document(entries):
    """构建标准的 OLE2 复合文档二进制字节。entries 为 [(name, stream_bytes), ...] 列表。"""
    sector_size = 512
    cur_sec = 0
    streams_layout = []
    for name, stream_bytes in entries:
        n_secs = (len(stream_bytes) + sector_size - 1) // sector_size
        streams_layout.append((name, stream_bytes, cur_sec, n_secs))
        cur_sec += n_secs

    total_stream_secs = max(1, cur_sec)
    total_secs = 3 + total_stream_secs
    data = bytearray(sector_size * total_secs)
    data[:8] = OLE2_MAGIC
    struct.pack_into('<H', data, 30, 9)   # sector shift = 9 (512 bytes)
    struct.pack_into('<I', data, 44, 1)   # 1 FAT sector
    struct.pack_into('<I', data, 48, 0)   # first dir sector = 0
    struct.pack_into('<I', data, 76, 1)   # first FAT sector = 1

    fat_offset = 1024
    struct.pack_into('<I', data, fat_offset + 0 * 4, 0xFFFFFFFE)
    struct.pack_into('<I', data, fat_offset + 1 * 4, 0xFFFFFFFD)

    for _name, _sbytes, sec_start, n_secs in streams_layout:
        for idx in range(n_secs):
            global_sec = 2 + sec_start + idx
            next_sec = (global_sec + 1) if idx < n_secs - 1 else 0xFFFFFFFE
            struct.pack_into('<I', data, fat_offset + global_sec * 4, next_sec)

    dir_offset = 512
    root_name = 'Root Entry\x00'.encode('utf-16le')
    data[dir_offset:dir_offset + len(root_name)] = root_name
    struct.pack_into('<H', data, dir_offset + 64, len(root_name))
    data[dir_offset + 66] = 5

    for entry_idx, (name, stream_bytes, sec_start, _n_secs) in enumerate(streams_layout, 1):
        e_offset = dir_offset + entry_idx * 128
        e_name = (name + '\x00').encode('utf-16le')
        data[e_offset:e_offset + len(e_name)] = e_name
        struct.pack_into('<H', data, e_offset + 64, len(e_name))
        data[e_offset + 66] = 2
        struct.pack_into('<I', data, e_offset + 116, 2 + sec_start)
        struct.pack_into('<I', data, e_offset + 120, len(stream_bytes))

        data_offset = 1536 + sec_start * sector_size
        data[data_offset:data_offset + len(stream_bytes)] = stream_bytes

    return bytes(data)


def make_dummy_ole2_doc(stream_text):
    return build_ole2_document([('WordDocument', stream_text.encode('utf-16le'))])


def make_dummy_fib_doc(stream_text):
    text_bytes = stream_text.encode('utf-16le')
    fc_min = 2048
    ccp_text = len(stream_text)
    stream_len = fc_min + len(text_bytes) + 2048
    w_stream = bytearray(stream_len)
    struct.pack_into('<H', w_stream, 0, 0xA5EC)
    struct.pack_into('<H', w_stream, 2, 193)
    struct.pack_into('<I', w_stream, 24, fc_min)
    struct.pack_into('<I', w_stream, 76, ccp_text)
    w_stream[fc_min:fc_min + len(text_bytes)] = text_bytes
    for k in range(fc_min + len(text_bytes), stream_len):
        w_stream[k] = (k % 250) + 1

    return build_ole2_document([('WordDocument', bytes(w_stream))])


def make_dummy_clx_doc(stream_text, f_compressed=True):
    """构建包含真实 CLX Piece Table (PlcPcd) 的 Word 97-2003 .doc 复合文档。"""
    fc_min = 2048
    if f_compressed:
        raw_text = stream_text.encode('latin1', errors='replace')
        fc_val = (fc_min * 2) | 0x40000000
    else:
        raw_text = stream_text.encode('utf-16le')
        fc_val = fc_min & ~0x40000000

    w_len = fc_min + len(raw_text) + 512
    w_stream = bytearray(w_len)
    struct.pack_into('<H', w_stream, 0, 0xA5EC)
    struct.pack_into('<H', w_stream, 2, 193)
    struct.pack_into('<H', w_stream, 10, 0x0200)  # fWhichTblStm = 1 (使用 1Table)
    struct.pack_into('<I', w_stream, 24, fc_min)
    struct.pack_into('<I', w_stream, 76, len(stream_text))

    w_stream[fc_min:fc_min + len(raw_text)] = raw_text

    # 构造 1Table 流及其 CLX
    pcdt_bytes = bytearray()
    pcdt_bytes.extend(struct.pack('<2I', 0, len(stream_text)))
    pcd = bytearray(8)
    struct.pack_into('<I', pcd, 2, fc_val)
    pcdt_bytes.extend(pcd)

    clx = bytearray()
    clx.append(2)  # clxt = 2 (pcdt)
    clx.extend(struct.pack('<I', len(pcdt_bytes)))
    clx.extend(pcdt_bytes)

    fc_clx = 64
    lcb_clx = len(clx)
    struct.pack_into('<I', w_stream, 418, fc_clx)
    struct.pack_into('<I', w_stream, 422, lcb_clx)

    t_stream = bytearray(fc_clx + lcb_clx + 128)
    t_stream[fc_clx:fc_clx + lcb_clx] = clx

    return build_ole2_document([('WordDocument', bytes(w_stream)), ('1Table', bytes(t_stream))])


class TestDocNativeParser(unittest.TestCase):
    def test_classify_doc_heading(self):
        self.assertEqual(_classify_doc_heading('第一章 系统设计'), 1)
        self.assertEqual(_classify_doc_heading('1.1 模块说明'), 2)
        self.assertEqual(_classify_doc_heading('1.1.1 协议规范'), 3)
        self.assertIsNone(_classify_doc_heading('普通正文句子。'))
        self.assertIsNone(_classify_doc_heading('- 列表条目'))

    def test_parse_doc_stream_to_md_with_tables(self):
        text = '第一章 系统设计方案\r\n1.1 模块说明\r\nColA\x07ColB\x07ColC\x07\r\nVal1\x07Val2\x07Val3\x07\r\n- Item 1\r\n'
        raw_bytes = text.encode('utf-16le')
        md = _parse_doc_stream_to_md(raw_bytes)
        self.assertIn('# 第一章 系统设计方案', md)
        self.assertIn('## 1.1 模块说明', md)
        self.assertIn('| ColA | ColB | ColC |', md)
        self.assertIn('| --- | --- | --- |', md)
        self.assertIn('| Val1 | Val2 | Val3 |', md)
        self.assertIn('- Item 1', md)

    def _assert_doc_conversion(self, doc_bytes, expected_snippets):
        with tempfile.NamedTemporaryFile(suffix='.doc', delete=False) as f:
            f.write(doc_bytes)
            tmp_path = f.name
        try:
            md, err = doc2md(tmp_path)
            self.assertIsNone(err)
            for snippet in expected_snippets:
                self.assertIn(snippet, md)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)

    def test_synthetic_ole2_doc_conversion(self):
        content = '第一章 测试\r\n表头A\x07表头B\x07\r\n内容A\x07内容B\x07\r\n'
        self._assert_doc_conversion(
            make_dummy_ole2_doc(content),
            ['# 第一章 测试', '| 表头A | 表头B |', '| 内容A | 内容B |']
        )

    def test_fib_ole2_doc_with_binary_padding(self):
        content = '第一章 真实格式测试\r\n姓名\x07学号\x07班级\x07\r\n张三\x07202601\x07甲班\x07\r\n'
        self._assert_doc_conversion(
            make_dummy_fib_doc(content),
            ['# 第一章 真实格式测试', '| 姓名 | 学号 | 班级 |', '| 张三 | 202601 | 甲班 |']
        )

    def test_doc_cjk_slice_without_table_correct_utf16(self):
        content = '第一章 纯中文正文无表格\r\n这是一段纯中文字符构成的正式公文报告段落，不含制表符。\r\n'
        self._assert_doc_conversion(
            make_dummy_fib_doc(content),
            ['# 第一章 纯中文正文无表格', '这是一段纯中文字符构成的正式公文报告段落']
        )

    def test_clx_piece_table_compressed(self):
        content = 'Chapter 1 Overview\r\nKey\x07Value\x07\r\nName\x07Alice\x07\r\n'
        self._assert_doc_conversion(
            make_dummy_clx_doc(content, f_compressed=True),
            ['# Chapter 1 Overview', '| Key | Value |', '| Name | Alice |']
        )

    def test_clx_piece_table_uncompressed(self):
        content = '第一章 复杂片段测试\r\n项目\x07得分\x07\r\n模块A\x0795\x07\r\n'
        self._assert_doc_conversion(
            make_dummy_clx_doc(content, f_compressed=False),
            ['# 第一章 复杂片段测试', '| 项目 | 得分 |', '| 模块A | 95 |']
        )

    def test_pdf_scanned_or_blank_fallback_to_ocr(self):
        with patch('src.readmd_modules.convert.pdf2md', side_effect=ValueError('pdf 未提取到文字内容')):
            with patch('src.readmd_modules.ocr.ocr_pdf_to_md', return_value='## 第 1 页\n\nOCR 识别结果') as mock_ocr:
                text, engine, err = convert_verbose('test_scanned.pdf')
                self.assertIsNone(err)
                self.assertEqual(engine, 'ocr')
                self.assertIn('OCR 识别结果', text)
                mock_ocr.assert_called_once_with('test_scanned.pdf')

    def test_markitdown_empty_string_does_not_override_ocr_placeholder(self):
        placeholder = '> （PDF 未提取到文字，且 OCR 无结果）'
        with patch('src.readmd_modules.convert.pdf2md', side_effect=ValueError('pdf 未提取到文字内容')):
            with patch('src.readmd_modules.ocr.ocr_pdf_to_md', return_value=placeholder):
                with patch('src.readmd_modules.convert._markitdown_convert', return_value=''):
                    text, engine, err = convert_verbose('test_blank.pdf')
                    self.assertIsNone(err)
                    self.assertEqual(engine, 'ocr')
                    self.assertIn('PDF 未提取到文字', text)

    def test_word_com_env_flag_zero_disabled(self):
        from src.readmd_modules.convert import _doc2docx_word_com
        old = os.environ.get('READMD_ENABLE_WORD_COM')
        try:
            os.environ['READMD_ENABLE_WORD_COM'] = '0'
            self.assertIsNone(_doc2docx_word_com('test.doc', '/tmp'))
            os.environ['READMD_ENABLE_WORD_COM'] = 'false'
            self.assertIsNone(_doc2docx_word_com('test.doc', '/tmp'))
        finally:
            if old is None:
                os.environ.pop('READMD_ENABLE_WORD_COM', None)
            else:
                os.environ['READMD_ENABLE_WORD_COM'] = old


if __name__ == '__main__':
    unittest.main()
