# -*- coding: utf-8 -*-
import glob, json, os, sys, tempfile, unittest, zipfile

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT_DIR)

import src.readmd_modules.mdexport as mdexport
import src.readmd_modules.mdexport.epub_render as epub_render
import src.readmd_modules.texmd as texmd


class TestV236Features(unittest.TestCase):
    def setUp(self):
        self.sample_md = """# Chapter 1

First chapter content with **bold** and code.

# Chapter 2

Second chapter content.
"""

    def test_epub_export_with_custom_options(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            out_epub = os.path.join(tmpdir, "test.epub")
            options = {
                "epub": {
                    "title": "ReadMD Guide",
                    "author": "DeepMind Engineer",
                    "publisher": "Antigravity Press",
                    "isbn": "978-7-111-12345-6",
                    "language": "zh-CN",
                    "splitLevel": "h1",
                    "fontSize": 12,
                    "lineHeight": 2.0,
                    "marginV": 6,
                    "marginH": 10
                }
            }
            res = mdexport.export("epub", self.sample_md, tmpdir, out_epub, options=options, source_name="test")
            self.assertTrue(res.get("ok"), f"EPUB export failed: {res.get('error')}")
            self.assertTrue(os.path.isfile(out_epub))

            with zipfile.ZipFile(out_epub, "r") as zf:
                namelist = zf.namelist()
                self.assertEqual(namelist[0], "mimetype")
                self.assertIn("META-INF/container.xml", namelist)
                self.assertIn("OEBPS/content.opf", namelist)
                self.assertIn("OEBPS/chapter_1.xhtml", namelist)
                self.assertIn("OEBPS/chapter_2.xhtml", namelist)
                opf = zf.read("OEBPS/content.opf").decode("utf-8")
                self.assertIn("ReadMD Guide", opf)
                self.assertIn("DeepMind Engineer", opf)
                css = zf.read("OEBPS/style.css").decode("utf-8")
                self.assertIn("font-size: 12pt;", css)

    def test_latex_export_with_custom_options(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            out_tex = os.path.join(tmpdir, "test.tex")
            options = {
                "tex": {
                    "title": "Antigravity Paper",
                    "author": "Team",
                    "docClass": "ctexart",
                    "fontSize": "12pt",
                    "paperSize": "a4paper",
                    "margin": "2cm",
                    "bibEngine": "biblatex",
                    "useCtex": True
                }
            }
            res = mdexport.export("tex", self.sample_md, tmpdir, out_tex, options=options, source_name="test")
            self.assertTrue(res.get("ok"), f"LaTeX export failed: {res.get('error')}")
            self.assertTrue(os.path.isfile(out_tex))

            with open(out_tex, "r", encoding="utf-8") as f:
                tex = f.read()
            self.assertIn(r"\documentclass[12pt,a4paper]{ctexart}", tex)
            self.assertIn(r"\usepackage[margin=2cm]{geometry}", tex)
            self.assertIn("Antigravity Paper", tex)

    def test_i18n_v236_keys_parity(self):
        i18n_files = [f for f in glob.glob(os.path.join(ROOT_DIR, "assets", "i18n", "*.json")) if os.path.basename(f) != "meta.json"]
        self.assertGreaterEqual(len(i18n_files), 46)
        required_keys = [
            "export.groupPublish", "export.groupWebCode", "export.fmtPdf", "export.fmtDocx",
            "export.fmtEpub", "export.fmtHtml", "export.fmtTex", "export.secEpubMeta",
            "export.epubTitle", "export.epubAuthor", "export.epubPublisher", "export.epubIsbn",
            "export.epubLanguage", "export.epubSplitLevel", "export.secEpubStyle",
            "export.secLatexDoc", "export.latexDocClass", "export.latexFontSize",
            "editor.docImportMode", "editor.frontmatterModalTitle"
        ]
        for p in i18n_files:
            with open(p, "r", encoding="utf-8") as f:
                data = json.load(f)
            fname = os.path.basename(p)
            for k in required_keys:
                self.assertIn(k, data, f"Missing key {k} in {fname}")


if __name__ == "__main__":
    unittest.main()
