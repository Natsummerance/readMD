# -*- coding: utf-8 -*-
"""Unit tests for ReadMD TikZ scientific vector diagram rendering."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.diagrams import format_tikz_html, identify_diagram_blocks


class TestTikZRendering(unittest.TestCase):
    """测试 TikZ 科学矢量图识别与包装。"""

    def test_identify_tikz_blocks(self):
        """测试识别标准 ```tikz 与 ```latex {tikz=true} 代码块。"""
        doc = """
# 几何物理

```tikz
\draw (0,0) circle (1in);
```

```latex {tikz=true}
\draw[thick,->] (0,0) -- (4,5);
```
"""
        diags = identify_diagram_blocks(doc)
        self.assertEqual(len(diags), 2)
        self.assertEqual(diags[0]["type"], "tikz")
        self.assertEqual(diags[1]["type"], "tikz")

    def test_format_tikz_html(self):
        """测试包装为 TikZjax script 节点。"""
        raw = "\\draw (0,0) -- (1,1);"
        node = format_tikz_html(raw)
        self.assertIn('<script type="text/tikz">', node)
        self.assertIn('\\begin{tikzpicture}', node)
        self.assertIn('\\end{tikzpicture}', node)


if __name__ == '__main__':
    unittest.main()
