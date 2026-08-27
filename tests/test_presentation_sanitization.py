# -*- coding: utf-8 -*-
"""Presentation slides must not become an active-content bypass."""

import unittest
import re

from src.readmd_modules.mdexport import presentation_render


class PresentationSanitizationTest(unittest.TestCase):
    def test_sanitizer_removes_active_content_and_preserves_safe_markdown(self):
        source = (
            '<script>window.__presentationXss = true;</script>\n\n'
            '<iframe src="https://example.invalid"></iframe>\n'
            '<img src="x" onerror="window.__presentationXss = true">\n'
            '<a href="javascript:alert(1)">bad link</a>\n\n'
            '**Safe bold**\n\n'
        )

        clean = presentation_render._sanitize_slide_html(source)

        self.assertNotIn('<script', clean)
        self.assertNotIn('onerror=', clean)
        self.assertNotIn('javascript:', clean)
        self.assertNotIn('<iframe', clean)
        self.assertIn('**Safe bold**', clean)

    def test_sanitizer_preserves_scripts_inside_inline_code(self):
        clean = presentation_render._sanitize_slide_html(
            'Use `<script>sample</script>` carefully.'
        )

        # Inline code is protected from the HTML parser and remains a sample.
        self.assertIn('`<script>sample</script>`', clean)

    def test_generated_presentation_removes_active_slide_content(self):
        markdown = (
            '# Safe slide\n\n'
            '<script>window.__presentationXss = true;</script>\n\n'
            '<img src="x" onerror="window.__presentationXss = true">\n\n'
            '<a href="javascript:alert(1)">bad</a>'
        )
        html = presentation_render.render_presentation_html(markdown)

        self.assertIn('# Safe slide', html)
        self.assertNotIn('<script>window.__presentationXss', html)
        self.assertNotIn('onerror=', html)
        self.assertNotIn('javascript:', html)

    def test_standalone_title_notes_and_fonts_are_contained(self):
        markdown = (
            '---\n'
            'title: "</title><script>window.__pptTitleXss=1</script>"\n'
            '---\n\n'
            '# Safe slide\n\n'
            '<!-- note -->\n<img src=x onerror="window.__noteXss=1">Keep note text\n'
        )
        html = presentation_render.render_presentation_html(markdown, standalone=True)

        self.assertNotIn('<script>window.__pptTitleXss', html)
        self.assertIn('&lt;/title&gt;', html)
        self.assertNotIn('window.__noteXss', html)
        self.assertNotIn('<img src=x onerror', html)
        self.assertIn('Keep note text', html)
        self.assertNotIn('url(fonts/', html)
        self.assertIn('data:font/woff2;base64,', html)

    def test_in_app_presentation_loads_only_same_origin_active_resources(self):
        markdown = '# CSP\n\n$E=mc^2$\n<!-- slide -->\n## Second'
        html = presentation_render.render_presentation_html(markdown)

        # Meta policies cannot enforce frame-ancestors and only create console noise.
        self.assertNotIn('frame-ancestors', html)
        for source in re.findall(r'<script src="([^"]+)"', html):
            self.assertTrue(source.startswith('assets/vendor/reveal/dist/'))
        self.assertIn('assets/vendor/reveal/dist/readmd-boot.js', html)
        self.assertNotIn('<script>window.__presentationXss', html)


if __name__ == '__main__':
    unittest.main()
