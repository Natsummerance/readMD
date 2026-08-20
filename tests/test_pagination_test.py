# -*- coding: utf-8 -*-
"""
Tests for Ultra-Long Markdown Semantic Pagination and Reader Enhancements.
"""
import unittest
import os
import re
import json

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS_DIR = os.path.join(ROOT_DIR, "assets")

class TestPaginationAlgorithms(unittest.TestCase):
    """Test Python-equivalent implementation and JS definitions of splitMdIntoPages."""

    def _split_md_into_pages_py(self, md: str):
        """Python mirror of splitMdIntoPages for deterministic boundary and logic verification."""
        lines = md.split('\n')
        total_lines = len(lines)
        if total_lines <= 2000:
            title = ""
            for l in lines:
                m = re.match(r'^#{1,3}\s+(.+)$', l.strip())
                if m:
                    title = re.sub(r'[*_`#]', '', m.group(1)).strip()
                    break
            return [{
                'pageIndex': 0,
                'title': title or '第 1 部分',
                'startLine': 1,
                'endLine': total_lines,
                'content': md,
            }]

        pages = []
        current_lines = []
        current_start = 1
        in_fence = False
        fence_marker = ""
        in_math = False
        in_table = False
        page_chapter_title = ""

        TARGET_PAGE_LINES = 1800
        MIN_PAGE_LINES = 600
        HARD_MAX_PAGE_LINES = 2600

        for i, line in enumerate(lines):
            trimmed = line.strip()

            # 1. 代码块围栏跟踪
            if not in_fence and (trimmed.startswith('```') or trimmed.startswith('~~~')):
                in_fence = True
                fence_marker = trimmed[:3]
            elif in_fence and trimmed.startswith(fence_marker):
                in_fence = False
                fence_marker = ""

            # 2. 多行公式环境跟踪
            if not in_fence:
                if not in_math and (trimmed == '$$' or re.match(r'^\\begin\{(align\*?|aligned|equation\*?|cases|gather\*?|matrix|pmatrix|bmatrix)\}', trimmed)):
                    in_math = True
                elif in_math and (trimmed == '$$' or re.match(r'^\\end\{(align\*?|aligned|equation\*?|cases|gather\*?|matrix|pmatrix|bmatrix)\}', trimmed)):
                    in_math = False

            # 3. 表格行跟踪
            in_table = not in_fence and not in_math and trimmed.startswith('|') and trimmed.endswith('|')

            # 记录页面主标题
            if not page_chapter_title and not in_fence and not in_math and re.match(r'^#{1,3}\s+(.+)$', trimmed):
                m = re.match(r'^#{1,3}\s+(.+)$', trimmed)
                if m:
                    page_chapter_title = re.sub(r'[*_`#]', '', m.group(1)).strip()

            cur_len = len(current_lines)
            can_break = not in_fence and not in_math and not in_table

            should_break = False
            if can_break and cur_len >= MIN_PAGE_LINES:
                if re.match(r'^#{1,2}\s+', trimmed) and cur_len >= MIN_PAGE_LINES:
                    should_break = True
                elif cur_len >= TARGET_PAGE_LINES and trimmed == "":
                    should_break = True
                elif cur_len >= HARD_MAX_PAGE_LINES and (trimmed == "" or re.match(r'^#{1,4}\s+', trimmed)):
                    should_break = True

            if should_break and current_lines:
                pages.append({
                    'pageIndex': len(pages),
                    'title': page_chapter_title or f"第 {len(pages) + 1} 部分",
                    'startLine': current_start,
                    'endLine': current_start + len(current_lines) - 1,
                    'content': '\n'.join(current_lines),
                })
                current_lines = []
                current_start = i + 1
                page_chapter_title = ""

            current_lines.append(line)

        if current_lines:
            pages.append({
                'pageIndex': len(pages),
                'title': page_chapter_title or f"第 {len(pages) + 1} 部分",
                'startLine': current_start,
                'endLine': current_start + len(current_lines) - 1,
                'content': '\n'.join(current_lines),
            })

        return pages

    def test_short_document_returns_single_page(self):
        doc = "# Hello Short Doc\n\nThis is a normal size doc.\n" * 50
        pages = self._split_md_into_pages_py(doc)
        self.assertEqual(len(pages), 1)
        self.assertEqual(pages[0]['title'], 'Hello Short Doc')

    def test_ultra_long_document_splits_properly_with_boundary_safety(self):
        # Build 10,000+ line document
        chunks = []
        for ch in range(1, 26):
            chunks.append(f"# Chapter {ch}: Algorithms and Complex Systems\n\n")
            chunks.append("Introductory text for this chapter.\n\n")
            # Math block
            chunks.append("$$\n\\sum_{k=1}^n k = \\frac{n(n+1)}{2}\n$$\n\n")
            # Code fence
            chunks.append("```python\ndef test_fn():\n    for i in range(100):\n        print(i)\n```\n\n")
            # Table
            chunks.append("| Key | Val |\n| :--- | :--- |\n| A | 1 |\n| B | 2 |\n\n")
            # Large body
            for j in range(400):
                chunks.append(f"Paragraph line {j} discussing complex distributed computation.\n")
            chunks.append("\n")

        full_md = "".join(chunks)
        total_lines = len(full_md.split('\n'))
        self.assertGreater(total_lines, 8000)
        self.assertGreaterEqual(total_lines, 10000)

        pages = self._split_md_into_pages_py(full_md)
        self.assertGreaterEqual(len(pages), 4)

        # Verify boundary safety for every single page
        for p in pages:
            c = p['content']
            # Code fence ticks must be even
            fence_ticks = len(re.findall(r'^```', c, re.M))
            self.assertEqual(fence_ticks % 2, 0, f"Page {p['pageIndex']} has unclosed code fence!")

            # Display math $$ must be even
            math_dollars = len(re.findall(r'^\$\$', c, re.M))
            self.assertEqual(math_dollars % 2, 0, f"Page {p['pageIndex']} has unclosed display math $$!")

            # Page title should be meaningful
            self.assertTrue(len(p['title']) > 0)


class TestPaginationFrontendUI(unittest.TestCase):
    """Test index.html DOM, style.css rules, and JS integration."""

    def test_index_html_pagination_bar_and_pure_svg_icons(self):
        html_path = os.path.join(ASSETS_DIR, "index.html")
        with open(html_path, "r", encoding="utf-8") as f:
            html = f.read()

        # 1. Ensure #pagination-bar exists
        self.assertIn('id="pagination-bar"', html)
        self.assertIn('id="pg-prev-btn"', html)
        self.assertIn('id="pg-next-btn"', html)
        self.assertIn('id="pg-first-btn"', html)
        self.assertIn('id="pg-last-btn"', html)
        self.assertIn('id="pg-page-select"', html)
        self.assertIn('id="pg-mode-toggle"', html)
        self.assertIn('id="status-pagination"', html)

        # 2. Ensure pagination buttons use pure SVG and do NOT contain textual prompts inside buttons
        prev_btn_match = re.search(r'<button id="pg-prev-btn"[^>]*>(.*?)</button>', html, re.DOTALL)
        self.assertTrue(prev_btn_match)
        prev_btn_inner = prev_btn_match.group(1).strip()
        self.assertTrue('<svg' in prev_btn_inner)
        self.assertNotIn('上一页', prev_btn_inner)

        next_btn_match = re.search(r'<button id="pg-next-btn"[^>]*>(.*?)</button>', html, re.DOTALL)
        self.assertTrue(next_btn_match)
        next_btn_inner = next_btn_match.group(1).strip()
        self.assertTrue('<svg' in next_btn_inner)
        self.assertNotIn('下一页', next_btn_inner)

    def test_css_rules_exist(self):
        css_path = os.path.join(ASSETS_DIR, "style.css")
        with open(css_path, "r", encoding="utf-8") as f:
            css = f.read()

        self.assertIn('.pagination-bar', css)
        self.assertIn('.pg-select', css)
        self.assertIn('.status-pagination', css)

    def test_i18n_keys_exist(self):
        zh_path = os.path.join(ASSETS_DIR, "i18n", "zh-CN.json")
        en_path = os.path.join(ASSETS_DIR, "i18n", "en.json")
        with open(zh_path, "r", encoding="utf-8") as f:
            zh = json.load(f)
        with open(en_path, "r", encoding="utf-8") as f:
            en = json.load(f)

        required_keys = [
            "pagination.ariaLabel",
            "pagination.prevPage",
            "pagination.nextPage",
            "pagination.firstPage",
            "pagination.lastPage",
            "pagination.selectPage",
            "pagination.toggleTip",
            "pagination.pagedBadge",
            "pagination.continuousBadge",
        ]
        for k in required_keys:
            self.assertIn(k, zh, f"Missing {k} in zh-CN.json")
            self.assertIn(k, en, f"Missing {k} in en.json")

    def test_js_modules_include_pagination_functions(self):
        render_js_path = os.path.join(ASSETS_DIR, "js", "reader", "render.js")
        with open(render_js_path, "r", encoding="utf-8") as f:
            render_js = f.read()

        self.assertIn("splitMdIntoPages", render_js)
        self.assertIn("renderPage", render_js)
        self.assertIn("togglePaginationMode", render_js)
        self.assertIn("initPaginationEvents", render_js)
        self.assertIn("PAGINATION_THRESHOLD_LINES", render_js)

        formula_js_path = os.path.join(ASSETS_DIR, "js", "reader", "formula.js")
        with open(formula_js_path, "r", encoding="utf-8") as f:
            formula_js = f.read()
        self.assertIn("IntersectionObserver", formula_js)

        search_js_path = os.path.join(ASSETS_DIR, "js", "reader", "search.js")
        with open(search_js_path, "r", encoding="utf-8") as f:
            search_js = f.read()
        self.assertIn("globalSearchState", search_js)


if __name__ == "__main__":
    unittest.main()
