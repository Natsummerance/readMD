import unittest
import struct
import tempfile
import os
from src.readmd_modules.convert import (
    _classify_doc_heading,
    _parse_doc_stream_to_md,
    _extract_worddocument_stream,
    _doc_extract_text_pure_python,
    doc2md,
    OLE2_MAGIC,
)


def make_dummy_ole2_doc(stream_text):
    data = bytearray(512 * 4)
    data[:8] = OLE2_MAGIC
    struct.pack_into('<H', data, 30, 9)
    struct.pack_into('<I', data, 44, 1)
    struct.pack_into('<I', data, 48, 0)
    struct.pack_into('<I', data, 76, 1)
    fat_offset = 1024
    struct.pack_into('<I', data, fat_offset + 0 * 4, 0xFFFFFFFE)
    struct.pack_into('<I', data, fat_offset + 1 * 4, 0xFFFFFFFD)
    struct.pack_into('<I', data, fat_offset + 2 * 4, 0xFFFFFFFE)
    dir_offset = 512
    root_name = 'Root Entry\x00'.encode('utf-16le')
    data[dir_offset:dir_offset + len(root_name)] = root_name
    struct.pack_into('<H', data, dir_offset + 64, len(root_name))
    data[dir_offset + 66] = 5
    doc_entry = dir_offset + 128
    doc_name = 'WordDocument\x00'.encode('utf-16le')
    data[doc_entry:doc_entry + len(doc_name)] = doc_name
    struct.pack_into('<H', data, doc_entry + 64, len(doc_name))
    data[doc_entry + 66] = 2
    struct.pack_into('<I', data, doc_entry + 116, 2)
    stream_bytes = stream_text.encode('utf-16le')
    struct.pack_into('<I', data, doc_entry + 120, len(stream_bytes))
    data[1536:1536 + len(stream_bytes)] = stream_bytes
    return bytes(data)


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

    def test_synthetic_ole2_doc_conversion(self):
        content = '第一章 测试\r\n表头A\x07表头B\x07\r\n内容A\x07内容B\x07\r\n'
        doc_bytes = make_dummy_ole2_doc(content)
        with tempfile.NamedTemporaryFile(suffix='.doc', delete=False) as f:
            f.write(doc_bytes)
            tmp_path = f.name
        try:
            md, err = doc2md(tmp_path)
            self.assertIsNone(err)
            self.assertIn('# 第一章 测试', md)
            self.assertIn('| 表头A | 表头B |', md)
            self.assertIn('| 内容A | 内容B |', md)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)

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
