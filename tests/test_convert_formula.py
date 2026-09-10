# -*- coding: utf-8 -*-
"""Tests for native TeX math font formula detection in convert.py."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import convert


class TestConvertFormulaDetection(unittest.TestCase):
    """Test TeX font hints, Unicode math symbols, and _looks_like_formula behavior."""

    def test_is_math_font(self):
        self.assertTrue(convert._is_math_font("CMMI10"))
        self.assertTrue(convert._is_math_font("CMSY9"))
        self.assertTrue(convert._is_math_font("NimbusRomNo9L-ReguItal"))
        self.assertTrue(convert._is_math_font("MathJax_Math-Italic"))
        self.assertTrue(convert._is_math_font("TeX-Math-Symbol"))
        self.assertFalse(convert._is_math_font("Arial"))
        self.assertFalse(convert._is_math_font("TimesNewRoman"))
        self.assertFalse(convert._is_math_font(""))
        self.assertFalse(convert._is_math_font(None))

    def test_looks_like_formula_pure_math(self):
        self.assertTrue(convert._looks_like_formula("E = mc^2"))
        self.assertTrue(convert._looks_like_formula("f(x) = \\int_0^\\infty e^{-t} dt"))
        self.assertTrue(convert._looks_like_formula("\\sum_{i=1}^n x_i \\ge 0"))

    def test_looks_like_formula_tex_font_ratio(self):
        # A line with high TeX font ratio even if short
        self.assertTrue(convert._looks_like_formula("a + b = c", math_ratio=0.5))
        self.assertTrue(convert._looks_like_formula("x_k = A x_{k-1} + B u_{k-1}", math_ratio=0.8))

    def test_looks_like_formula_rejects_cjk_and_prose(self):
        self.assertFalse(convert._looks_like_formula("这是一个包含公式的中文句子：E=mc^2"))
        self.assertFalse(convert._looks_like_formula("Chapter 1: Introduction to Calculus"))
        self.assertFalse(convert._looks_like_formula("User manual version 2.0.1"))

    def test_looks_like_formula_2d_baseline_shift(self):
        """2D 拓扑空间几何关系：垂直基线跳变（上下标）且含算符/变量时，判定为公式。"""
        self.assertTrue(convert._looks_like_formula("x_i + y_i = z_i", math_ratio=0.0, has_baseline_shift=True))
        self.assertTrue(convert._looks_like_formula("a^2 + b^2 = c^2", math_ratio=0.0, has_baseline_shift=True))
        # 无算符/无变量的纯文本即使有轻微位移也不误报为公式
        self.assertFalse(convert._looks_like_formula("Just normal words", math_ratio=0.0, has_baseline_shift=True))

    def test_extracted_pdf_line_dataclass(self):
        """验证 ExtractedPdfLine 数据类消除 Data Clumps 且类型安全。"""
        line = convert.ExtractedPdfLine(
            y0=100.5,
            seq=1,
            text="E = mc^2",
            size=12.0,
            bold=False,
            mono=False,
            math_font=True,
            math_ratio=0.75,
            body_size=11.0,
            has_baseline_shift=True,
        )
        self.assertEqual(line.y0, 100.5)
        self.assertEqual(line.text, "E = mc^2")
        self.assertTrue(line.has_baseline_shift)
        self.assertTrue(line.math_font)


if __name__ == '__main__':
    unittest.main()
