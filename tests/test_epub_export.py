# -*- coding: utf-8 -*-
"""Unit tests for ReadMD native EPUB 3.0 ebook builder."""

import os
import sys
import tempfile
import unittest
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules.mdexport.epub_render import export_epub, split_into_chapters


class TestEpubExport(unittest.TestCase):
    """测试 EPUB 3.0 电子书打包引擎。"""

    def test_split_into_chapters(self):
        """测试按标题分章。"""
        doc = """
# 第一章：概述
这是第一章内容。

# 第二章：核心原理
这是第二章内容。
"""
        chaps = split_into_chapters(doc)
        self.assertEqual(len(chaps), 2)
        self.assertEqual(chaps[0][0], "第一章：概述")
        self.assertEqual(chaps[1][0], "第二章：核心原理")

    def test_export_epub_container_structure(self):
        """测试生成标准 EPUB ZIP 容器及内部结构。"""
        doc = """
# 第一章：开篇
欢迎阅读 ReadMD 电子书。

# 第二章：深入剖析
这是深入剖析的内容，包含 **粗体** 与 `代码`。
"""
        with tempfile.TemporaryDirectory() as tmpdir:
            out_epub = os.path.join(tmpdir, "book.epub")
            export_epub(doc, out_epub, title="测试书名", author="测试作者")

            self.assertTrue(os.path.isfile(out_epub))
            self.assertGreater(os.path.getsize(out_epub), 500)

            # 校验 ZIP 容器内部规范
            with zipfile.ZipFile(out_epub, 'r') as zf:
                names = zf.namelist()
                # 1. mimetype 必须位于首位
                self.assertEqual(names[0], 'mimetype')
                mimetype_content = zf.read('mimetype').decode('ascii')
                self.assertEqual(mimetype_content, 'application/epub+zip')

                # 2. 检查关键清单文件
                self.assertIn('META-INF/container.xml', names)
                self.assertIn('OEBPS/content.opf', names)
                self.assertIn('OEBPS/nav.xhtml', names)
                self.assertIn('OEBPS/toc.ncx', names)
                self.assertIn('OEBPS/style.css', names)
                self.assertIn('OEBPS/chapter_1.xhtml', names)
                self.assertIn('OEBPS/chapter_2.xhtml', names)

                # 3. 校验 content.opf 中的书名与作者
                opf_text = zf.read('OEBPS/content.opf').decode('utf-8')
                self.assertIn('<dc:title>测试书名</dc:title>', opf_text)
                self.assertIn('<dc:creator>测试作者</dc:creator>', opf_text)


if __name__ == '__main__':
    unittest.main()
