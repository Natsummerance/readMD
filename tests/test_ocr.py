# -*- coding: utf-8 -*-
"""ReadMD 跨平台 OCR 模块 (src.readmd_modules.ocr) 完整单元测试与 Mock 套件。"""

import os
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import ocr


class TestOcrModule(unittest.TestCase):
    """测试 WinRT / Vision / Tesseract OCR 引擎调度、图片与 PDF 识别流程。"""

    def setUp(self):
        ocr._engine_cache.clear()

    def test_pick_engine_winrt_fallback(self):
        """测试 Windows 环境下 WinRT 引擎探测。"""
        with patch.object(ocr, 'IS_WIN', True), patch.object(ocr, 'IS_MAC', False):
            with patch.dict('sys.modules', {'winrt.windows.media.ocr': MagicMock()}):
                eng = ocr._pick_engine()
                self.assertEqual(eng, 'winrt')

    def test_pick_engine_tesseract(self):
        """测试无原生 OCR 时回退至 Tesseract。"""
        with patch.object(ocr, 'IS_WIN', False), patch.object(ocr, 'IS_MAC', False):
            with patch('subprocess.run', return_value=MagicMock(returncode=0)):
                eng = ocr._pick_engine()
                self.assertEqual(eng, 'tesseract')

    def test_normalize_ocr_text(self):
        """测试 OCR 文本排版规范化（去除中文字符间空格、连字符修复）。"""
        raw = "自 动 化 测 试\nauto-\ncomplete\n第一章 标题"
        normalized = ocr.normalize_ocr_text(raw)
        self.assertIn("自动化测试", normalized)
        self.assertIn("autocomplete", normalized)

    def test_ocr_bytes_winrt(self):
        """测试 WinRT 字节识别流程。"""
        mock_data = b'\x89PNG\r\n\x1a\nfake_image_data'
        with patch('src.readmd_modules.ocr._pick_engine', return_value='winrt'):
            with patch('src.readmd_modules.ocr._winrt_pick_language', return_value='zh-Hans'):
                with patch('src.readmd_modules.ocr._winrt_ocr_bytes', return_value='识别出的中文文本'):
                    text = ocr._ocr_bytes(mock_data)
                    self.assertEqual(text, '识别出的中文文本')

    def test_ocr_bytes_tesseract_mock(self):
        """测试 Tesseract 命令行调用与输出解析。"""
        mock_data = b'fake_image_bytes'
        mock_res = MagicMock()
        mock_res.stdout = 'Tesseract Extracted Text\nSecond Line'.encode('utf-8')

        with patch('subprocess.run', return_value=mock_res):
            text = ocr._tesseract_ocr_bytes(mock_data)
            self.assertEqual(text, 'Tesseract Extracted Text\nSecond Line')

    def test_ocr_image_file(self):
        """测试从文件路径直接执行 OCR 识别。"""
        with tempfile.NamedTemporaryFile(suffix='.png', delete=False) as f:
            f.write(b'sample image file')
            img_path = f.name

        try:
            with patch('src.readmd_modules.ocr._ocr_bytes', return_value='图片中的文字'):
                res = ocr.ocr_image(img_path)
                self.assertEqual(res, '图片中的文字')

                md = ocr.ocr_image_to_md(img_path)
                self.assertIn('![原图]', md)
                self.assertIn('图片中的文字', md)
        finally:
            if os.path.exists(img_path):
                os.remove(img_path)

    def test_ocr_pdf_with_native_text(self):
        """测试包含文字层的 PDF 直接提取文本，无需调用耗时 OCR。"""
        mock_doc = MagicMock()
        mock_page = MagicMock()
        mock_page.get_text.return_value = '这是 PDF 原生文本内容'
        mock_doc.__iter__.return_value = [mock_page]
        mock_doc.__len__.return_value = 1

        mock_fitz = MagicMock()
        mock_fitz.open.return_value = mock_doc

        with patch.dict('sys.modules', {'fitz': mock_fitz}):
            text = ocr.ocr_pdf_to_md('dummy.pdf')
            self.assertIn('这是 PDF 原生文本内容', text)

    def test_ocr_pdf_scanned_page_fallback(self):
        """测试扫描件 PDF（无文字层）自动逐页渲染并调用 OCR。"""
        mock_doc = MagicMock()
        mock_page = MagicMock()
        mock_page.get_text.return_value = '   '  # 空白文字层
        mock_pix = MagicMock()
        mock_pix.tobytes.return_value = b'rendered_page_png_bytes'
        mock_page.get_pixmap.return_value = mock_pix
        mock_doc.__iter__.return_value = [mock_page]
        mock_doc.__len__.return_value = 1

        mock_fitz = MagicMock()
        mock_fitz.open.return_value = mock_doc

        with patch.dict('sys.modules', {'fitz': mock_fitz}):
            with patch('src.readmd_modules.ocr._ocr_bytes', return_value='扫描页面识别结果'):
                text = ocr.ocr_pdf_to_md('scanned.pdf')
                self.assertIn('扫描页面识别结果', text)


if __name__ == '__main__':
    unittest.main()
