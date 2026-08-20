# -*- coding: utf-8 -*-
"""Unit tests for ReadMD custom CSS and HTML head injector."""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.style_injector import (
    get_custom_css,
    get_custom_head,
    inject_custom_styles_to_html,
)


class TestCustomStyling(unittest.TestCase):
    """测试自定义样式与脚本注入。"""

    def test_inject_custom_styles(self):
        """测试在 HTML 中注入 custom.css 与 head.html。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            readmd_dir = os.path.join(tmpdir, ".readmd")
            os.makedirs(readmd_dir, exist_ok=True)

            css_path = os.path.join(readmd_dir, "custom.css")
            head_path = os.path.join(readmd_dir, "head.html")

            with open(css_path, "w", encoding="utf-8") as f:
                f.write("body { font-family: 'Times New Roman'; }")

            with open(head_path, "w", encoding="utf-8") as f:
                f.write("<link rel='stylesheet' href='custom-font.css'>")

            html_in = "<html><head><title>Doc</title></head><body>Content</body></html>"
            html_out = inject_custom_styles_to_html(html_in, workspace_dir=tmpdir)

            self.assertIn("font-family: 'Times New Roman'", html_out)
            self.assertIn("custom-font.css", html_out)
            self.assertIn("<style id=\"readmd-custom-style\">", html_out)


if __name__ == '__main__':
    unittest.main()
