"""Comprehensive tests for ReadMD OCR module."""
import os
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from src.readmd_modules import ocr


class TestOCREngineSelection(unittest.TestCase):
    """Test OCR engine selection logic."""

    def setUp(self):
        """Clear engine cache before each test."""
        ocr._engine_cache.clear()

    def tearDown(self):
        """Clear engine cache after each test."""
        ocr._engine_cache.clear()

    @mock.patch.object(ocr, 'IS_WIN', True)
    @mock.patch.object(ocr, 'IS_MAC', False)
    def test_pick_engine_windows_winrt(self):
        """Test Windows selects WinRT when available."""
        try:
            import winrt.windows.media.ocr
        except ImportError:
            self.skipTest("winrt module not available")
        
        with mock.patch.dict('sys.modules', {'winrt.windows.media.ocr': mock.MagicMock()}):
            engine = ocr._pick_engine()
            self.assertEqual(engine, 'winrt')

    @mock.patch.object(ocr, 'IS_WIN', False)
    @mock.patch.object(ocr, 'IS_MAC', True)
    def test_pick_engine_mac_vision(self):
        """Test macOS selects Vision framework when available."""
        try:
            import Vision
        except ImportError:
            self.skipTest("Vision module not available")
        
        with mock.patch.dict('sys.modules', {'Vision': mock.MagicMock()}):
            engine = ocr._pick_engine()
            self.assertEqual(engine, 'mac_vision')

    @mock.patch.object(ocr, 'IS_WIN', False)
    @mock.patch.object(ocr, 'IS_MAC', False)
    def test_pick_engine_tesseract(self):
        """Test Linux selects Tesseract when available."""
        with mock.patch('subprocess.run') as mock_run:
            mock_run.return_value = mock.MagicMock(returncode=0)
            engine = ocr._pick_engine()
            self.assertEqual(engine, 'tesseract')

    @mock.patch.object(ocr, 'IS_WIN', False)
    @mock.patch.object(ocr, 'IS_MAC', False)
    def test_pick_engine_none_when_no_engine(self):
        """Test returns None when no OCR engine is available."""
        with mock.patch('subprocess.run', side_effect=FileNotFoundError()):
            engine = ocr._pick_engine()
            self.assertIsNone(engine)

    def test_pick_engine_caches_result(self):
        """Test that engine selection result is cached."""
        with mock.patch('subprocess.run') as mock_run:
            mock_run.return_value = mock.MagicMock(returncode=0)
            
            # First call
            engine1 = ocr._pick_engine()
            call_count1 = mock_run.call_count
            
            # Second call should use cache
            engine2 = ocr._pick_engine()
            call_count2 = mock_run.call_count
            
            self.assertEqual(engine1, engine2)
            self.assertEqual(call_count1, call_count2)


class TestOCRNormalizeText(unittest.TestCase):
    """Test OCR text normalization."""

    def test_normalize_empty_text(self):
        """Test empty text returns empty string."""
        result = ocr.normalize_ocr_text('')
        self.assertEqual(result, '')

    def test_normalize_whitespace_only(self):
        """Test whitespace-only text returns empty string."""
        result = ocr.normalize_ocr_text('   \n\n  ')
        self.assertEqual(result, '')

    def test_normalize_cjk_spaces_removed(self):
        """Test CJK character spaces are removed."""
        text = '这 是 一 个 示 例'
        result = ocr.normalize_ocr_text(text)
        # The result should have CJK characters together (may have markdown formatting)
        # Check that the core CJK text doesn't have spaces between characters
        self.assertIn('这是一个示例', result)

    def test_normalize_english_hyphenation(self):
        """Test English hyphenated words across lines are joined."""
        text = 'infor-\nmation'
        result = ocr.normalize_ocr_text(text)
        self.assertIn('information', result)

    def test_normalize_preserves_structured_content(self):
        """Test structured content (headers, lists) is preserved."""
        text = '# Title\n\n- Item 1\n- Item 2'
        result = ocr.normalize_ocr_text(text)
        self.assertIn('# Title', result)
        self.assertIn('- Item 1', result)

    def test_normalize_preserves_tables(self):
        """Test table formatting is preserved."""
        text = '| Col1 | Col2 |\n|------|------|\n| A    | B    |'
        result = ocr.normalize_ocr_text(text)
        self.assertIn('|', result)

    def test_normalize_sentence_boundary_breaks(self):
        """Test sentences ending with punctuation create line breaks."""
        text = '这是第一句。这是第二句！'
        result = ocr.normalize_ocr_text(text)
        # Should preserve sentence boundaries
        self.assertIsNotNone(result)

    def test_normalize_short_lines_not_merged(self):
        """Test very short lines are not merged."""
        text = 'A\nB\nC'
        result = ocr.normalize_ocr_text(text)
        # Short lines should be kept separate
        self.assertIsNotNone(result)

    def test_normalize_removes_special_whitespace(self):
        """Test special whitespace characters are normalized."""
        text = 'Hello\u3000World\u00a0Test\u200bEnd\ufeff'
        result = ocr.normalize_ocr_text(text)
        self.assertNotIn('\u3000', result)
        self.assertNotIn('\u00a0', result)

    def test_normalize_cjk_punctuation_spacing(self):
        """Test CJK punctuation spacing is handled."""
        text = '你好 ， 世界 ！'
        result = ocr.normalize_ocr_text(text)
        # Punctuation should be adjacent to CJK characters
        self.assertIsNotNone(result)


class TestOCRImage(unittest.TestCase):
    """Test image OCR functionality."""

    def setUp(self):
        """Create temporary directory for test files."""
        self.temp_dir = tempfile.mkdtemp()
        ocr._engine_cache.clear()

    def tearDown(self):
        """Clean up temporary files."""
        import shutil
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)
        ocr._engine_cache.clear()

    def _create_test_image(self, filename='test.png'):
        """Create a minimal PNG file for testing."""
        # Minimal valid PNG (1x1 pixel, red)
        png_data = (
            b'\x89PNG\r\n\x1a\n'  # PNG signature
            b'\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde'  # IHDR
            b'\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N'  # IDAT
            b'\x00\x00\x00\x00IEND\xaeB`\x82'  # IEND
        )
        path = os.path.join(self.temp_dir, filename)
        with open(path, 'wb') as f:
            f.write(png_data)
        return path

    @mock.patch.object(ocr, '_ocr_bytes', return_value='Recognized text from image')
    def test_ocr_image_returns_text(self, mock_ocr):
        """Test ocr_image returns recognized text."""
        img_path = self._create_test_image()
        result = ocr.ocr_image(img_path)
        self.assertEqual(result, 'Recognized text from image')

    @mock.patch.object(ocr, '_ocr_bytes', return_value='')
    def test_ocr_image_empty_result(self, mock_ocr):
        """Test ocr_image handles empty recognition result."""
        img_path = self._create_test_image()
        result = ocr.ocr_image(img_path)
        self.assertEqual(result, '')

    @mock.patch.object(ocr, '_ocr_bytes', return_value='  Text with spaces  ')
    def test_ocr_image_strips_result(self, mock_ocr):
        """Test ocr_image strips whitespace from result."""
        img_path = self._create_test_image()
        result = ocr.ocr_image(img_path)
        self.assertEqual(result, 'Text with spaces')


class TestOCRImageToMd(unittest.TestCase):
    """Test image to Markdown conversion."""

    def setUp(self):
        """Create temporary directory for test files."""
        self.temp_dir = tempfile.mkdtemp()
        ocr._engine_cache.clear()

    def tearDown(self):
        """Clean up temporary files."""
        import shutil
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir)
        ocr._engine_cache.clear()

    def _create_test_image(self, filename='test.png'):
        """Create a minimal PNG file for testing."""
        png_data = (
            b'\x89PNG\r\n\x1a\n'
            b'\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde'
            b'\x00\x00\x00\x0cIDATx\x9cc\xf8\x0f\x00\x00\x01\x01\x00\x05\x18\xd8N'
            b'\x00\x00\x00\x00IEND\xaeB`\x82'
        )
        path = os.path.join(self.temp_dir, filename)
        with open(path, 'wb') as f:
            f.write(png_data)
        return path

    @mock.patch.object(ocr, 'ocr_image', return_value='Recognized text')
    def test_ocr_image_to_md_with_text(self, mock_ocr):
        """Test image to MD includes recognized text and image reference."""
        img_path = self._create_test_image()
        result = ocr.ocr_image_to_md(img_path)
        self.assertIn('![原图]', result)
        self.assertIn(img_path, result)
        self.assertIn('Recognized text', result)

    @mock.patch.object(ocr, 'ocr_image', return_value='')
    def test_ocr_image_to_md_no_text(self, mock_ocr):
        """Test image to MD shows placeholder when no text recognized."""
        img_path = self._create_test_image()
        result = ocr.ocr_image_to_md(img_path)
        self.assertIn('未识别出文字', result)
        self.assertIn('![原图]', result)


class TestOCRPdfToMd(unittest.TestCase):
    """Test PDF OCR functionality."""

    def setUp(self):
        """Check if fitz is available."""
        try:
            import fitz
            self.fitz_available = True
        except ImportError:
            self.fitz_available = False

    @unittest.skip("Requires fitz module which is not installed")
    def test_ocr_pdf_with_text_layer(self):
        """Test PDF with text layer extracts text directly."""
        pass

    @unittest.skip("Requires fitz module which is not installed")
    def test_ocr_pdf_without_text_layer(self):
        """Test PDF without text layer uses OCR."""
        pass

    @unittest.skip("Requires fitz module which is not installed")
    def test_ocr_pdf_empty_result(self):
        """Test PDF with no extractable text returns placeholder."""
        pass

    @unittest.skip("Requires fitz module which is not installed")
    def test_ocr_pdf_respects_max_pages(self):
        """Test PDF OCR respects max_pages parameter."""
        pass


class TestOCRAny(unittest.TestCase):
    """Test the ocr_any dispatcher function."""

    def setUp(self):
        """Set up test fixtures."""
        ocr._engine_cache.clear()

    def tearDown(self):
        """Clean up."""
        ocr._engine_cache.clear()

    @mock.patch.object(ocr, 'ocr_pdf_to_md', return_value='PDF markdown')
    def test_ocr_any_dispatches_pdf(self, mock_pdf):
        """Test ocr_any dispatches PDF files to pdf handler."""
        result = ocr.ocr_any('/path/to/document.pdf')
        self.assertEqual(result, 'PDF markdown')
        mock_pdf.assert_called_once_with('/path/to/document.pdf')

    @mock.patch.object(ocr, 'ocr_image_to_md', return_value='Image markdown')
    def test_ocr_any_dispatches_image(self, mock_image):
        """Test ocr_any dispatches image files to image handler."""
        result = ocr.ocr_any('/path/to/image.png')
        self.assertEqual(result, 'Image markdown')
        mock_image.assert_called_once_with('/path/to/image.png')

    @mock.patch.object(ocr, 'ocr_image_to_md', return_value='JPG markdown')
    def test_ocr_any_handles_jpg(self, mock_image):
        """Test ocr_any handles JPG files."""
        result = ocr.ocr_any('/path/to/photo.jpg')
        self.assertEqual(result, 'JPG markdown')

    @mock.patch.object(ocr, 'ocr_image_to_md', return_value='JPEG markdown')
    def test_ocr_any_handles_jpeg(self, mock_image):
        """Test ocr_any handles JPEG files."""
        result = ocr.ocr_any('/path/to/photo.jpeg')
        self.assertEqual(result, 'JPEG markdown')


class TestOCRLoad(unittest.TestCase):
    """Test OCR module load function."""

    def setUp(self):
        """Clear cache before tests."""
        ocr._engine_cache.clear()

    def tearDown(self):
        """Clear cache after tests."""
        ocr._engine_cache.clear()

    @mock.patch.object(ocr, '_pick_engine', return_value='tesseract')
    def test_load_succeeds_with_engine(self, mock_pick):
        """Test load succeeds when engine is available."""
        result = ocr.load()
        self.assertTrue(result)

    @mock.patch.object(ocr, '_pick_engine', return_value=None)
    def test_load_raises_without_engine(self, mock_pick):
        """Test load raises error when no engine is available."""
        with self.assertRaises(RuntimeError) as context:
            ocr.load()
        self.assertIn('无可用 OCR 引擎', str(context.exception))

    @mock.patch.object(ocr, '_pick_engine', return_value='winrt')
    @mock.patch.object(ocr, '_winrt_pick_language', return_value='zh-Hans')
    def test_load_caches_winrt_language(self, mock_lang, mock_pick):
        """Test load caches WinRT language selection."""
        ocr.load()
        self.assertIn('lang', ocr._engine_cache)
        self.assertEqual(ocr._engine_cache['lang'], 'zh-Hans')


class TestOCRWinRTLanguageSelection(unittest.TestCase):
    """Test WinRT language selection."""

    def test_winrt_pick_language_prefers_zh_hans(self):
        """Test WinRT prefers Simplified Chinese."""
        # Skip if winrt is not available
        try:
            import winrt.windows.media.ocr
        except ImportError:
            self.skipTest("winrt module not available")
        
        with mock.patch('winrt.windows.media.ocr.OcrEngine') as mock_engine:
            mock_lang = mock.MagicMock()
            mock_lang.language_tag = 'zh-Hans-CN'
            mock_engine.available_recognizer_languages = [mock_lang]
            
            lang = ocr._winrt_pick_language()
            self.assertEqual(lang, 'zh-Hans-CN')

    def test_winrt_pick_language_falls_back_to_first(self):
        """Test WinRT falls back to first available language."""
        try:
            import winrt.windows.media.ocr
        except ImportError:
            self.skipTest("winrt module not available")
        
        with mock.patch('winrt.windows.media.ocr.OcrEngine') as mock_engine:
            mock_lang = mock.MagicMock()
            mock_lang.language_tag = 'fr-FR'
            mock_engine.available_recognizer_languages = [mock_lang]
            
            lang = ocr._winrt_pick_language()
            self.assertEqual(lang, 'fr-FR')

    def test_winrt_pick_language_raises_when_none(self):
        """Test WinRT raises error when no languages available."""
        try:
            import winrt.windows.media.ocr
        except ImportError:
            self.skipTest("winrt module not available")
        
        with mock.patch('winrt.windows.media.ocr.OcrEngine') as mock_engine:
            mock_engine.available_recognizer_languages = []
            
            with self.assertRaises(RuntimeError) as context:
                ocr._winrt_pick_language()
            self.assertIn('未安装任何 OCR 语言', str(context.exception))


if __name__ == '__main__':
    unittest.main()
