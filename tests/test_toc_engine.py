# -*- coding: utf-8 -*-
"""Unit tests for ReadMD [TOC] directory generator engine."""

import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.toc_engine import (
    extract_headings,
    generate_toc_markdown,
    process_toc_markers,
    slugify_heading,
)


class TestTOCEngine(unittest.TestCase):
    """测试 TOC 目录树生成引擎。"""

    def test_slugify_heading(self):
        """测试标题锚点生成。"""
        self.assertEqual(slugify_heading("1. 快速入门 Quick Start"), "1-快速入门-quick-start")
        self.assertEqual(slugify_heading("LaTeX 公式 $E=mc^2$"), "latex-公式-emc2")
        self.assertEqual(slugify_heading("`code_snippet()`"), "code_snippet")

    def test_extract_headings_ignoring_code_blocks(self):
        """测试提取标题并自动规避代码块内注释。"""
        doc = """
# 真实标题 1
正文内容。

```python
# 这是代码块内的注释，不应提取为标题
def foo():
    pass
```

## 真实标题 2
~~~markdown
### 代码块内的假标题
~~~
### 真实标题 3
"""
        headings = extract_headings(doc)
        self.assertEqual(len(headings), 3)
        self.assertEqual(headings[0][1], "真实标题 1")
        self.assertEqual(headings[1][1], "真实标题 2")
        self.assertEqual(headings[2][1], "真实标题 3")

    def test_generate_toc_markdown(self):
        """测试生成层级 Markdown 列表。"""
        headings = [
            (1, "简介", "简介"),
            (2, "安装步骤", "安装步骤"),
            (3, "macOS 安装", "macos-安装"),
            (2, "使用指南", "使用指南"),
        ]
        toc = generate_toc_markdown(headings, depth_from=1, depth_to=3)
        self.assertIn("- [简介](#简介)", toc)
        self.assertIn("  - [安装步骤](#安装步骤)", toc)
        self.assertIn("    - [macOS 安装](#macos-安装)", toc)
        self.assertIn("  - [使用指南](#使用指南)", toc)

    def test_process_toc_markers(self):
        """测试扫描并就地替换 [TOC] 标记。"""
        doc = """
# 文档总览

[TOC]

## 第一章 概述
内容 1。

## 第二章 深入
内容 2。
"""
        res = process_toc_markers(doc)
        self.assertNotIn("[TOC]", res)
        self.assertIn("[第一章 概述](#第一章-概述)", res)
        self.assertIn("[第二章 深入](#第二章-深入)", res)


if __name__ == '__main__':
    unittest.main()
