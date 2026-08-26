# -*- coding: utf-8 -*-
"""Unit tests for ReadMD Reveal.js presentation generator."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.mdexport.presentation_render import (
    parse_frontmatter,
    render_presentation_html,
    split_slides_structure,
)


class TestPresentationRender(unittest.TestCase):
    """测试 Reveal.js 演说模式渲染引擎。"""

    def test_parse_frontmatter(self):
        """测试 Front-matter 配置提取。"""
        doc = """---
title: "我的演示文稿"
theme: league
transition: zoom
---
# 首页
"""
        meta, body = parse_frontmatter(doc)
        self.assertEqual(meta.get("title"), "我的演示文稿")
        self.assertEqual(meta.get("theme"), "league")
        self.assertEqual(meta.get("transition"), "zoom")
        self.assertIn("# 首页", body)

    def test_split_slides_structure(self):
        """测试横向与垂直幻灯片切片。"""
        doc = """
# Slide 1: 标题页
欢迎来到 ReadMD 演示。
<!-- note -->
提醒观众保持静音。

<!-- slide -->

# Slide 2: 核心特性
1. 语法自愈
2. 模块化

<!-- subslide -->

## Slide 2.1: 深入细节
详细架构解析。
<!-- note -->
强调零外部依赖。
"""
        matrix = split_slides_structure(doc)
        self.assertEqual(len(matrix), 2)  # 2 个横向大页
        self.assertEqual(len(matrix[0]), 1)  # 第 1 页无子页
        self.assertEqual(len(matrix[1]), 2)  # 第 2 页有 1 个垂直子页

        # 检查演讲者备注
        self.assertIn("提醒观众保持静音", matrix[0][0]["note"])
        self.assertIn("强调零外部依赖", matrix[1][1]["note"])

    def test_render_presentation_html(self):
        """测试生成完整 HTML 演说稿。"""
        doc = "# Slide 1\n内容\n<!-- slide -->\n# Slide 2\n内容 2"
        html = render_presentation_html(doc, title="测试演说")
        self.assertIn("<title>测试演说</title>", html)
        self.assertIn("assets/vendor/reveal/dist/readmd-boot.js", html)
        self.assertIn("<section data-markdown>", html)

    def test_long_document_auto_split(self):
        """测试长文档在没有 explicit slide 标记时智能多页分片，不会被压缩丢失。"""
        long_doc = "# 第一章 介绍\n" + ("这是长篇段落说明内容。\n\n" * 15) + "## 第二章 架构\n" + ("架构设计深入分析与模块解析。\n\n" * 15) + "### 2.1 存储模块\n" + ("存储细节实现。\n\n" * 10)
        matrix = split_slides_structure(long_doc)
        # 应该自动切分成至少 4 页以上，而不是只有 1~2 页
        total_slides = sum(len(v) for v in matrix)
        self.assertGreaterEqual(total_slides, 3, "Long document should be split into multiple slides")

    def test_hr_delimiters(self):
        """测试使用标准 --- 和 -- 分割幻灯片。"""
        doc = """
# Slide 1
Welcome
---
# Slide 2
Next
--
## Subslide 2.1
Detail
"""
        matrix = split_slides_structure(doc)
        self.assertEqual(len(matrix), 2)
        self.assertEqual(len(matrix[1]), 2)

    def test_html_and_code_preservation(self):
        """测试包含 HTML 标签和代码块时，textarea data-template 不被过度转义。"""
        doc = """
# Code Demo
```python
if a < b and b > c:
    print("<tag>")
```
<span style="color: red;">Alert</span>
"""
        html = render_presentation_html(doc)
        self.assertIn('if a < b and b > c:', html)
        self.assertIn('<span style="color: red;">Alert</span>', html)
    def test_code_block_protection_during_auto_split(self):
        """测试包含大段代码块的超长 Markdown 在自动分片时代码块不被腰斩。"""
        long_code = "```python\n" + "\n".join([f"def func_{i}(): return {i}" for i in range(40)]) + "\n```"
        doc = "# 长代码演示\n这是引言段落。\n\n" + long_code + "\n\n这是总结段落。"
        matrix = split_slides_structure(doc)
        total_slides = sum(len(v) for v in matrix)
        self.assertGreaterEqual(total_slides, 1)
        # 确保代码块完整存在于某一页中，未被截断
        found_full_code = any(long_code in slide["content"] for v in matrix for slide in v)
        self.assertTrue(found_full_code, "Code block should remain intact without being chopped")


if __name__ == '__main__':
    unittest.main()

