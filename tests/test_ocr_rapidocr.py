# -*- coding: utf-8 -*-
"""Tests for RapidOCR integration, reading order box sorting, and OCR fallback pipeline."""

import os
import sys
import unittest
from unittest.mock import patch, MagicMock

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import ocr
from src.readmd_modules import plugin_manager as pm


class TestRapidOCRIntegration(unittest.TestCase):
    """Test RapidOCR engine methods, reading-order sorting, and engine selection."""

    def test_sort_rapidocr_boxes_empty(self):
        self.assertEqual(ocr._sort_rapidocr_boxes([]), [])
        self.assertEqual(ocr._sort_rapidocr_boxes(None), [])

    def test_sort_rapidocr_boxes_reading_order(self):
        """Test that bounding boxes on a line are grouped and sorted left-to-right, top-to-bottom."""
        raw_result = [
            [[[120, 10], [210, 10], [210, 25], [120, 25]], "Title Right", 0.98],
            [[[10, 10], [100, 10], [100, 25], [10, 25]], "Title Left", 0.99],
            [[[130, 40], [220, 40], [220, 55], [130, 55]], "Body Right", 0.95],
            [[[10, 40], [110, 40], [110, 55], [10, 55]], "Body Left", 0.97],
        ]
        sorted_lines = ocr._sort_rapidocr_boxes(raw_result)
        self.assertEqual(len(sorted_lines), 2)
        self.assertEqual(sorted_lines[0], "Title Left Title Right")
        self.assertEqual(sorted_lines[1], "Body Left Body Right")

    def test_sort_rapidocr_boxes_multicol_xy_cut(self):
        """Test that multi-column layouts do NOT interleave horizontally and follow column order."""
        # 2 columns layout:
        # Left column: [20, 100] -> [20, 140]
        # Right column: [300, 100] -> [300, 140]
        multicol_boxes = [
            [[[300, 100], [400, 100], [400, 120], [300, 120]], "Col2 Line1", 0.95],
            [[[20, 100], [120, 100], [120, 120], [20, 120]], "Col1 Line1", 0.98],
            [[[300, 135], [400, 135], [400, 155], [300, 155]], "Col2 Line2", 0.94],
            [[[20, 135], [120, 135], [120, 155], [20, 155]], "Col1 Line2", 0.97],
        ]
        sorted_lines = ocr._sort_rapidocr_boxes(multicol_boxes)
        self.assertEqual(sorted_lines, ["Col1 Line1", "Col1 Line2", "Col2 Line1", "Col2 Line2"])

    def test_sort_rapidocr_boxes_multicol_boundary_no_merge(self):
        """Ensure boundary lines at identical y coordinates in different columns never merge."""
        boundary_boxes = [
            [[[300, 100], [400, 100], [400, 120], [300, 120]], "Col2 Line1", 0.95],
            [[[20, 100], [120, 100], [120, 120], [20, 120]], "Col1 Line1", 0.98],
        ]
        sorted_lines = ocr._sort_rapidocr_boxes(boundary_boxes)
        self.assertEqual(sorted_lines, ["Col1 Line1", "Col2 Line1"])

    def test_ocr_pdf_to_md_cascade_fallback(self):
        """Verify that scanned PDF pages cascade through native OCR to RapidOCR."""
        mock_page = MagicMock()
        mock_page.get_text.return_value = ""
        mock_pix = MagicMock()
        mock_pix.tobytes.return_value = b"\x89PNG\r\n\x1a\n"
        mock_page.get_pixmap.return_value = mock_pix

        mock_doc = MagicMock()
        mock_doc.__iter__.return_value = [mock_page]
        mock_doc.page_count = 1

        with patch('fitz.open', return_value=mock_doc):
            with patch.object(ocr, '_ocr_bytes', return_value=""):
                with patch.object(ocr, '_ocr_rapidocr', return_value="Scanned Text From RapidOCR"):
                    res = ocr.ocr_pdf_to_md("dummy.pdf")
                    self.assertIn("Scanned Text From RapidOCR", res)
                    self.assertIn("## 第 1 页", res)

    def test_ocr_pdf_to_md_empty_placeholder(self):
        """Verify that if all pages yield empty text after OCR cascade, placeholder is returned."""
        mock_page = MagicMock()
        mock_page.get_text.return_value = ""
        mock_pix = MagicMock()
        mock_pix.tobytes.return_value = b"\x89PNG\r\n\x1a\n"
        mock_page.get_pixmap.return_value = mock_pix

        mock_doc = MagicMock()
        mock_doc.__iter__.return_value = [mock_page]
        mock_doc.page_count = 1

        with patch('fitz.open', return_value=mock_doc):
            with patch.object(ocr, '_ocr_cascade', return_value=""):
                res = ocr.ocr_pdf_to_md("dummy_blank.pdf")
                self.assertEqual(res, ocr.OCR_PDF_EMPTY_PLACEHOLDER)

    def test_extract_table_to_md_from_html(self):
        """Test HTML table string converts cleanly to Markdown table."""
        html = "<table><tr><th>Metric</th><th>Value</th></tr><tr><td>Accuracy</td><td>99.2%</td></tr></table>"
        md = ocr._html_table_to_md(html)
        expected = "| Metric | Value |\n| --- | --- |\n| Accuracy | 99.2% |"
        self.assertEqual(md, expected)

    def test_extract_table_to_md_heuristic_fallback(self):
        """Test heuristic tab/space table construction when table plugin is not loaded."""
        with patch.object(pm, 'is_plugin_enabled', return_value=False):
            with patch.object(ocr, '_ocr_cascade', return_value="Header A\tHeader B\nData 1\tData 2"):
                md = ocr.extract_table_to_md(b"fake_image_bytes")
                self.assertIn("| Header A | Header B |", md)
                self.assertIn("| Data 1 | Data 2 |", md)


    def test_pick_engine_prefers_rapidocr_when_enabled(self):
        """When WinRT / Vision are absent and RapidOCR is installed/enabled, it picks rapidocr."""
        with patch.dict(ocr._engine_cache, {}, clear=True):
            with patch.object(ocr, 'IS_WIN', False):
                with patch.object(ocr, 'IS_MAC', False):
                    with patch.object(pm, 'is_plugin_enabled', return_value=True):
                        engine = ocr._pick_engine()
                        self.assertEqual(engine, 'rapidocr')

    def test_ocr_bytes_dispatches_rapidocr(self):
        """Verify _ocr_bytes dispatches to _rapidocr_bytes when engine is rapidocr."""
        with patch.dict(ocr._engine_cache, {'_engine': 'rapidocr'}):
            with patch.object(ocr, '_rapidocr_bytes', return_value="Recognized by RapidOCR"):
                res = ocr._ocr_bytes(b"dummy_image_data")
                self.assertEqual(res, "Recognized by RapidOCR")

    def test_ocr_image_fallback_to_rapidocr(self):
        """When primary OCR returns empty, ocr_image tries rapidocr first."""
        with patch.object(ocr, '_ocr_bytes', return_value=""):
            with patch.object(ocr, '_ocr_rapidocr', return_value="Fallback RapidOCR Text"):
                import tempfile
                with tempfile.NamedTemporaryFile(suffix='.png', delete=False) as tf:
                    tf.write(b"\x89PNG\r\n\x1a\n")
                    tmp_name = tf.name
                try:
                    res = ocr.ocr_image(tmp_name)
                    self.assertEqual(res, "Fallback RapidOCR Text")
                finally:
                    os.unlink(tmp_name)


if __name__ == '__main__':
    unittest.main()
