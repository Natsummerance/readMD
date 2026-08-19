"""Regression and unit test suite for ReadMD v2.2.9 features."""

import os
import sys
import unittest
import tempfile
import shutil

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

import readmd
from installer import setup_app
from src.readmd_modules import texmd
from src.readmd_modules import convert
from src.readmd_modules import mdexport

INDEX_HTML = os.path.join(ROOT_DIR, 'assets', 'index.html')
STYLE_CSS = os.path.join(ROOT_DIR, 'assets', 'style.css')
APP_JS = os.path.join(ROOT_DIR, 'assets', 'app.js')


class TestV229Features(unittest.TestCase):

    def test_version_bump_consistency(self):
        self.assertEqual(readmd.VERSION, '2.2.9')
        self.assertEqual(setup_app.APP_VERSION, '2.2.9')

    def test_texmd_bidirectional_conversion(self):
        # 1. Markdown to LaTeX
        md_text = """# Academic Research Paper

This is an introductory paragraph with **bold text**, *italic text*, and `inline code`.

## Mathematical Formulations

Inline formula $E = mc^2$ and block formulas:

$$
\\int_{0}^{\\infty} e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}
$$

### Experimental Results

| Model | Accuracy | F1-Score |
| :--- | :---: | ---: |
| Base | 88.5% | 0.87 |
| ReadMD | 99.2% | 0.99 |

- Feature 1: High speed
- Feature 2: High fidelity

```python
def solve():
    return 42
```
"""
        latex_standalone = texmd.md_to_latex(md_text, standalone=True, title='Test Paper', author='Author')
        self.assertIn('\\documentclass', latex_standalone)
        self.assertIn('amsmath', latex_standalone)
        self.assertIn('\\usepackage{booktabs}', latex_standalone)

        self.assertIn('\\section{Academic Research Paper}', latex_standalone)
        self.assertIn('\\subsection{Mathematical Formulations}', latex_standalone)
        self.assertIn('\\textbf{bold text}', latex_standalone)
        self.assertIn('\\textit{italic text}', latex_standalone)
        self.assertIn('\\texttt{inline code}', latex_standalone)
        self.assertIn('\\begin{tabular}', latex_standalone)
        self.assertIn('\\toprule', latex_standalone)
        self.assertIn('\\begin{lstlisting}', latex_standalone)

        # 2. LaTeX to Markdown
        back_md = texmd.latex_to_md(latex_standalone)
        self.assertIn('# Academic Research Paper', back_md)
        self.assertIn('## Mathematical Formulations', back_md)
        self.assertIn('**bold text**', back_md)
        self.assertIn('*italic text*', back_md)
        self.assertIn('`inline code`', back_md)
        self.assertIn('| Model | Accuracy | F1-Score |', back_md)
        self.assertIn('def solve():', back_md)

    def test_convert_module_tex_support(self):
        sample_tex = """\\documentclass{article}
\\begin{document}
\\section{Introduction to ReadMD}
ReadMD is a lightweight markdown viewer.
\\begin{equation}
\\nabla \\cdot \\mathbf{E} = \\frac{\\rho}{\\varepsilon_0}
\\end{equation}
\\end{document}"""
        with tempfile.TemporaryDirectory() as tmpdir:
            tex_file = os.path.join(tmpdir, 'sample.tex')
            with open(tex_file, 'w', encoding='utf-8') as f:
                f.write(sample_tex)

            md_content, engine, error = convert.convert_verbose(tex_file)
            self.assertIsNone(error)
            self.assertEqual(engine, 'texmd')
            self.assertIn('# Introduction to ReadMD', md_content)
            self.assertIn('ReadMD is a lightweight markdown viewer.', md_content)
            self.assertIn('\\nabla \\cdot \\mathbf{E}', md_content)

    def test_mdexport_tex_support(self):
        sample_md = '# Title\n\nExport to LaTeX test.\n\n$$a^2 + b^2 = c^2$$\n'
        with tempfile.TemporaryDirectory() as tmpdir:
            out_file = os.path.join(tmpdir, 'out.tex')
            res = mdexport.export('tex', sample_md, tmpdir, out_file, options={'title': 'My Title'})
            self.assertTrue(res.get('ok'), res.get('error'))

            self.assertTrue(os.path.isfile(out_file))
            with open(out_file, 'r', encoding='utf-8') as f:
                tex_data = f.read()
            self.assertIn('\\section{Title}', tex_data)
            self.assertIn('Export to LaTeX test.', tex_data)
            self.assertIn('a^2 + b^2 = c^2', tex_data)

    def test_frontend_dom_and_js_elements(self):
        with open(INDEX_HTML, 'r', encoding='utf-8') as f:
            html = f.read()
        js_files = [APP_JS]
        js_dir = os.path.join(ROOT_DIR, 'assets', 'js')
        if os.path.exists(js_dir):
            for root, _, files in os.walk(js_dir):
                for f in files:
                    if f.endswith('.js'):
                        js_files.append(os.path.join(root, f))
        js = ''
        for fp in js_files:
            with open(fp, 'r', encoding='utf-8') as f:
                js += '\n' + f.read()
        with open(STYLE_CSS, 'r', encoding='utf-8') as f:
            css = f.read()


        # Web dialog stepper & actions
        self.assertIn('id="url-pages-dec"', html)
        self.assertIn('id="url-pages"', html)
        self.assertIn('id="url-pages-inc"', html)
        self.assertIn('id="url-go"', html)
        self.assertIn('id="url-render"', html)
        self.assertIn('id="url-paste-btn"', html)
        self.assertIn('id="url-private"', html)

        # Unsaved dirty tab modal
        self.assertIn('id="close-confirm-modal"', html)
        self.assertIn('id="close-confirm-save"', html)
        self.assertIn('id="close-confirm-discard"', html)
        self.assertIn('id="close-confirm-cancel"', html)

        # LaTeX Export Button
        self.assertIn('data-fmt="tex"', html)

        # Editor Undo/Redo & Floating Selection Toolbar
        self.assertIn('id="edit-undo"', html)
        self.assertIn('id="edit-redo"', html)
        self.assertIn('id="cm-selection-toolbar"', html)
        self.assertIn('id="cm-sel-copy"', html)
        self.assertIn('id="cm-sel-cut"', html)
        self.assertIn('id="cm-sel-paste"', html)

        # JS functions & logic
        self.assertIn('function promptDirtyClose', js)
        self.assertIn('async function createFromClipboard', js)
        self.assertIn('TurndownService', js)
        self.assertIn('state.headings = []', js)
        self.assertIn('url-pages-dec', js)
        self.assertIn('url-pages-inc', js)
        self.assertIn('url-paste-btn', js)
        self.assertIn('state.isDraggingTab', js)
        self.assertIn('application/x-readmd-tab', js)
        self.assertIn('function findMatchingHeading', js)
        self.assertIn('function normalizeHeadingText', js)
        self.assertIn('function cmUndo', js)
        self.assertIn('function cmRedo', js)
        self.assertIn('function updateCmSelectionToolbar', js)

        # CSS styles
        self.assertIn('.url-stepper', css)
        self.assertIn('#close-confirm-box', css)
        self.assertIn('.close-confirm-actions', css)
        self.assertIn('.tab-drag-over-left', css)
        self.assertIn('.tab-drag-over-right', css)
        self.assertIn('.heading-target-highlight', css)
        self.assertIn('.cm-selection-toolbar', css)
        self.assertIn('.edit-actions-right', css)


if __name__ == '__main__':
    unittest.main()


