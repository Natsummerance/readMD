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
        self.assertIn("reveal.js", html)
        self.assertIn("<section data-markdown>", html)


if __name__ == '__main__':
    unittest.main()
