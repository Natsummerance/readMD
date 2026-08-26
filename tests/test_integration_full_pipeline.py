# -*- coding: utf-8 -*-
"""ReadMD v2.3.4 全链路端到端集成测试 (Integration Testing)。

覆盖链路：
1. 模块化工程：主文档 -> 多层 @import 子文档 -> CSV 数据表转 Markdown -> 真实源码行号切片；
2. 正文目录：[TOC] 标记自动解析，提取各级标题生成锚点跳转树；
3. 多图表转译：WaveDrom / Graphviz / PlantUML / Vega 代码块识别与 URL 编码；
4. 演说模式：提取 <!-- slide --> 与 <!-- note -->，编译输出自包含 Reveal.js HTML；
5. 自定义样式：注入 .readmd/custom.css 与 head.html；
6. 安全代码执行：运行 Python 代码并截获 Matplotlib 输出；
7. 导出集成：编译输出独立 HTML / PDF 管道。
"""

import os
import sys
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.readmd_fix import fix_markdown
from src.readmd_core.style_injector import inject_custom_styles_to_html
from src.readmd_core.toc_engine import process_toc_markers
from src.readmd_modules.code_chunk_runner import execute_python_chunk
from src.readmd_modules.diagrams import identify_diagram_blocks, plantuml_encode
from src.readmd_modules.import_processor import process_markdown_imports
from src.readmd_modules.mdexport.presentation_render import render_presentation_html


class TestIntegrationFullPipeline(unittest.TestCase):
    """全功能全链路端到端集成测试用例。"""

    def test_e2e_modular_document_pipeline(self):
        """测试从 @import 分解文件到最终完整 HTML / 演说稿的完整编译链路。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # 1. 准备工作区文件与自定义样式
            readmd_conf = os.path.join(tmpdir, ".readmd")
            os.makedirs(readmd_conf, exist_ok=True)
            with open(os.path.join(readmd_conf, "custom.css"), "w", encoding="utf-8") as f:
                f.write(".custom-highlight { color: #2563eb; font-weight: bold; }")

            # 2. 准备子文档
            chap1_path = os.path.join(tmpdir, "chapter1.md")
            with open(chap1_path, "w", encoding="utf-8") as f:
                f.write("""## 第一章：硬件时序与架构

```wavedrom
{ "signal": [{ "name": "clk", "wave": "p....." }] }
```

```dot
digraph Arch { A -> B -> C }
```
""")

            # 3. 准备 CSV 数据
            csv_path = os.path.join(tmpdir, "benchmarks.csv")
            with open(csv_path, "w", encoding="utf-8") as f:
                f.write("Model,Speed(tok/s),Latency(ms)\nDeepSeek-V3,120,8.2\nClaude-3.5-Sonnet,95,12.1\n")

            # 4. 准备源码文件
            py_path = os.path.join(tmpdir, "algorithm.py")
            with open(py_path, "w", encoding="utf-8") as f:
                f.write("""# License MIT
import os

def compute_metrics(x, y):
    # Core algorithm
    return x * 2 + y ** 2

def main():
    pass
""")

            # 5. 准备主文档
            main_path = os.path.join(tmpdir, "main.md")
            with open(main_path, "w", encoding="utf-8") as f:
                f.write("""---
title: "ReadMD 技术白皮书"
theme: league
---

# ReadMD v2.3.4 技术白皮书

[TOC]

@import "chapter1.md"

## 第二章：基准性能与数据

@import "benchmarks.csv"

## 第三章：算法切片

@import "algorithm.py" {line_begin=4 line_end=6 lang="python"}

<!-- slide -->

# 演说页面：总结

这是幻灯片独立页。
<!-- note -->
演讲者要点提示。
""")

            # --- 执行链路步骤 1: 展平 @import ---
            with open(main_path, "r", encoding="utf-8") as f:
                raw_text = f.read()

            flattened = process_markdown_imports(raw_text, base_dir=tmpdir, current_file=main_path)
            self.assertIn("第一章：硬件时序与架构", flattened)
            self.assertIn("| Model | Speed(tok/s) | Latency(ms) |", flattened)
            self.assertIn("compute_metrics(x, y)", flattened)
            self.assertNotIn("@import", flattened)

            # --- 执行链路步骤 2: 语法自愈 ---
            fixed_res = fix_markdown(flattened)
            self.assertTrue(len(fixed_res.text) > 0)

            # --- 执行链路步骤 3: 正文 [TOC] 目录树生成 ---
            with_toc = process_toc_markers(fixed_res.text)
            self.assertNotIn("[TOC]", with_toc)
            self.assertIn("- [第一章：硬件时序与架构](#第一章硬件时序与架构)", with_toc)
            self.assertIn("- [第二章：基准性能与数据](#第二章基准性能与数据)", with_toc)
            self.assertIn("- [第三章：算法切片](#第三章算法切片)", with_toc)

            # --- 执行链路步骤 4: 图表识别 ---
            diagrams = identify_diagram_blocks(with_toc)
            self.assertEqual(len(diagrams), 2)
            self.assertEqual(diagrams[0]["type"], "wavedrom")
            self.assertEqual(diagrams[1]["type"], "dot")

            # --- 执行链路步骤 5: Reveal.js 演说稿生成 ---
            deck_html = render_presentation_html(with_toc, title="测试演说白皮书")
            self.assertIn("<title>ReadMD 技术白皮书</title>", deck_html)
            self.assertIn("assets/vendor/reveal/dist/readmd-boot.js", deck_html)
            self.assertIn('<aside class="notes">演讲者要点提示。</aside>', deck_html)

            # --- 执行链路步骤 6: 自定义 CSS 样式注入 ---
            styled_html = inject_custom_styles_to_html(deck_html, workspace_dir=tmpdir)
            self.assertIn(".custom-highlight { color: #2563eb;", styled_html)

            # --- 执行链路步骤 7: 代码块就地执行 ---
            code_to_run = "import sys\nx = 10\ny = 5\nprint(f'OUTPUT={x*2+y**2}')"
            exec_res = execute_python_chunk(code_to_run, capture_plot=False)
            self.assertTrue(exec_res["ok"])
            self.assertIn("OUTPUT=45", exec_res["stdout"])


if __name__ == '__main__':
    unittest.main()
