# -*- coding: utf-8 -*-
"""ReadMD v2.3.4 全量特性自动化测试套件。"""

import os
import shutil
import tempfile
import unittest
import zipfile

from src.readmd_core.source_map import (
    annotate_markdown_source_lines,
    inject_source_line_attributes_to_html,
)
from src.readmd_core.style_injector import (
    get_custom_styles,
    inject_custom_styles_to_html,
    save_custom_styles,
)
from src.readmd_modules import code_chunk_runner, diagrams, import_processor
from src.readmd_modules.mdexport import epub_render, presentation_render


class TestV234FullFeatures(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.mkdtemp(prefix="readmd_vtest_")

    def tearDown(self):
        if os.path.exists(self.temp_dir):
            shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_source_map_annotation_and_injection(self):
        md = "# Heading 1\n\nParagraph text here."
        annotated = annotate_markdown_source_lines(md)
        self.assertIn('<!-- data-source-line="1" -->', annotated)
        html = '<!-- data-source-line="1" -->\n<h1>Heading 1</h1>'
        injected_html = inject_source_line_attributes_to_html(html)
        self.assertIn('<h1 data-source-line="1">', injected_html)

    def test_tikz_diagram_formatting(self):
        html = diagrams.format_tikz_html("\\draw (0,0) circle (1cm);")
        self.assertIn('type="text/tikz"', html)
        self.assertIn('\\begin{tikzpicture}', html)

    def test_python_code_chunk_execution(self):
        code = "print('Hello ReadMD v2.3.4')\nx = 10 + 20\nprint(f'sum={x}')"
        res = code_chunk_runner.execute_code_chunk(code, lang="python")
        self.assertTrue(res["ok"])
        self.assertIn("Hello ReadMD v2.3.4", res["stdout"])
        self.assertIn("sum=30", res["stdout"])

    def test_sql_in_memory_chunk_execution(self):
        sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT); INSERT INTO users (name) VALUES ('Alice'); SELECT * FROM users;"
        res = code_chunk_runner.execute_code_chunk(sql, lang="sql")
        self.assertTrue(res["ok"])
        self.assertIn("Alice", res["stdout"])

    def test_import_sub_markdown(self):
        sub = os.path.join(self.temp_dir, "sub.md")
        main = os.path.join(self.temp_dir, "main.md")
        with open(sub, 'w', encoding='utf-8') as f:
            f.write("## Sub Content")
        with open(main, 'w', encoding='utf-8') as f:
            f.write(f"# Main\n@import \"{os.path.basename(sub)}\"")
        with open(main, 'r', encoding='utf-8') as f:
            content = f.read()
        res = import_processor.process_markdown_imports(content, base_dir=self.temp_dir)
        self.assertIn("# Main", res)
        self.assertIn("## Sub Content", res)

    def test_epub3_export_packaging(self):
        md = "# Chapter 1\nText here.\n"
        epub_p = os.path.join(self.temp_dir, "book.epub")
        out = epub_render.export_epub(md, epub_p, title='Test Book', author='ReadMD')
        self.assertTrue(os.path.isfile(epub_p))
        with zipfile.ZipFile(epub_p, 'r') as f:
            self.assertIn("mimetype", f.namelist())

    def test_presentation_slides_generation(self):
        md = "# Slide 1\n\n<!-- slide -->\n# Slide 2"
        html = presentation_render.generate_presentation_html(md)
        self.assertIn("class=\"reveal\"", html)
        self.assertIn("Slide 1", html)

    def test_custom_style_saving_and_injection(self):
        css = ".markdown-body { font-size: 18px; }"
        head = "<script src='https://example.com/custom.js'></script>"
        ok = save_custom_styles(css, head, workspace_dir=self.temp_dir)
        self.assertTrue(ok)
        styles = get_custom_styles(workspace_dir=self.temp_dir)
        self.assertEqual(styles["css"], css)
        self.assertEqual(styles["head"], head)
        raw_html = "<html><head><title>Doc</title></head><body><h1>Hi</h1></body></html>"
        injected = inject_custom_styles_to_html(raw_html, workspace_dir=self.temp_dir)
        self.assertIn(css, injected)
        self.assertIn(head, injected)


if __name__ == '__main__':
    unittest.main()
