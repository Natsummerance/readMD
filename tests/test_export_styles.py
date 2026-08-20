# -*- coding: utf-8 -*-
"""ReadMD 导出样式引擎 (src.readmd_modules.mdexport.styles) 单元测试。"""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.mdexport import styles


class TestExportStyles(unittest.TestCase):
    """测试导出样式默认值、深度合并、预设校验与非法值修复。"""

    def test_default_style_integrity(self):
        """测试默认样式结构的完整性。"""
        s = styles.DEFAULT_STYLE
        self.assertIn('page', s)
        self.assertIn('typography', s)
        self.assertIn('headings', s)
        self.assertIn('table', s)
        self.assertIn('code', s)
        self.assertIn('quote', s)
        self.assertEqual(s['page']['size'], 'A4')
        self.assertEqual(s['htmlTheme'], 'light')

    def test_deep_merge(self):
        """测试深层字典合并逻辑。"""
        base = {'a': 1, 'nested': {'x': 10, 'y': 20}}
        overlay = {'b': 2, 'nested': {'y': 99, 'z': 30}}
        merged = styles.deep_merge(base, overlay)

        self.assertEqual(merged['a'], 1)
        self.assertEqual(merged['b'], 2)
        self.assertEqual(merged['nested']['x'], 10)
        self.assertEqual(merged['nested']['y'], 99)
        self.assertEqual(merged['nested']['z'], 30)

    def test_sanitize_style_clamps_and_defaults(self):
        """测试非法数值与格式被自动修正至安全区间。"""
        bad_style = {
            'page': {'size': 'INVALID_SIZE', 'marginTop': -100, 'marginRight': 9999},
            'typography': {'size': 999, 'lineHeight': 0.1, 'color': 'not-a-hex', 'align': 'bogus'},
            'headings': {
                'h1': {'size': 1, 'color': 'red'}
            },
            'htmlTheme': 'unknown-theme'
        }
        sanitized = styles.sanitize(bad_style)

        # 页面尺寸回退
        self.assertEqual(sanitized['page']['size'], 'A4')
        self.assertEqual(sanitized['page']['marginTop'], 0)  # clamped to 0
        self.assertEqual(sanitized['page']['marginRight'], 60)  # clamped to 60

        # 排版回退
        self.assertEqual(sanitized['typography']['size'], 20)  # clamped max 20
        self.assertEqual(sanitized['typography']['lineHeight'], 1.0)  # clamped min 1.0
        self.assertEqual(sanitized['typography']['color'], '#262626')  # fallback default
        self.assertEqual(sanitized['typography']['align'], 'left')  # fallback default

        # h1 字号钳位
        self.assertEqual(sanitized['headings']['h1']['size'], 8)  # clamped min 8
        self.assertEqual(sanitized['htmlTheme'], 'light')

    def test_presets_loading(self):
        """测试所有内置预设完整可用且合法。"""
        preset_names = ['minimal', 'academic', 'report', 'tech', 'warm', 'elegant', 'compact']
        for name in preset_names:
            p = styles.preset_style(name)
            self.assertIsNotNone(p)
            self.assertIn('typography', p)
            self.assertIn('headings', p)
            self.assertTrue(p['table']['borderWidth'] >= 0)


if __name__ == '__main__':
    unittest.main()
