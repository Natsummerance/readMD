# -*- coding: utf-8 -*-
"""Unit tests for ReadMD diagram helper and PlantUML encoder."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.diagrams import (
    get_plantuml_svg_url,
    identify_diagram_blocks,
    plantuml_encode,
)


class TestDiagramsExpanded(unittest.TestCase):
    """测试多图表识别与 PlantUML 编码。"""

    def test_identify_diagram_blocks(self):
        """测试识别多种图表代码块。"""
        doc = """
# 系统架构

```mermaid
graph TD; A-->B;
```

```wavedrom
{ "signal": [{ "name": "clk", "wave": "p....." }] }
```

```dot
digraph G { A -> B }
```

```puml
@startuml
A -> B: Hello
@enduml
```
"""
        diags = identify_diagram_blocks(doc)
        self.assertEqual(len(diags), 4)
        types = [d["type"] for d in diags]
        self.assertIn("mermaid", types)
        self.assertIn("wavedrom", types)
        self.assertIn("dot", types)
        self.assertIn("puml", types)

    def test_plantuml_encode_and_url(self):
        """测试 PlantUML Base64 编码与在线 SVG URL 生成。"""
        code = "Alice -> Bob: Authentication Request"
        url = get_plantuml_svg_url(code)
        self.assertTrue(url.startswith("https://www.plantuml.com/plantuml/svg/"))
        self.assertNotIn("~1", url)
        self.assertGreater(len(url), 40)


if __name__ == '__main__':
    unittest.main()
